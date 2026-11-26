//! Named counters for every slow, missing or approximated path, each also reported as a warning.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use atomic_refcell::AtomicRefCell;

use wcontainers_abi::{CounterReading, Warning};
use wcontainers_system::FallbackObserver;

/// The fixed reasons the kernel counts.
pub mod reason {
    /// A system call Linux has and the kernel does not yet. Detail: the call number.
    pub const NOT_IMPLEMENTED: &str = "system call not implemented";
    /// A system call number Linux does not have on ARM64. Detail: the call number.
    pub const UNKNOWN_SYSCALL: &str = "system call number unknown";
    /// A process died of a signal that dumps core where its registers were lost (a fault the CPU
    /// side could not return from), so no core file was written. Detail: the signal.
    pub const CORE_DUMP_NOT_WRITTEN: &str = "core dump not written";
    /// An `fcntl` command Linux has and the kernel does not build; it answers EINVAL, as Linux does
    /// for one it has not configured. Commands Linux does not know are a plain EINVAL. Detail: the
    /// command.
    pub const FCNTL_NOT_BUILT: &str = "fcntl command not built";
    /// Opening a device the kernel has no driver for. Detail: the device number.
    pub const DEVICE_NOT_BUILT: &str = "device not built";
    /// A `prctl` option or form Linux has on ARM64 and the kernel does not build; it answers EINVAL,
    /// as Linux does for one it has not configured. Options Linux does not know, or has only for
    /// other architectures, are a plain EINVAL. Detail: the option.
    pub const PRCTL_NOT_BUILT: &str = "prctl option not built";
    /// A flag or form of a call that the kernel does not build, answered as Linux answers without
    /// it. Detail: the call number times 2^32, plus the flag bits.
    pub const FLAG_NOT_BUILT: &str = "call flag not built";
    /// `madvise` advice Linux has that the kernel does not build; it answers EINVAL, as a Linux
    /// without that advice does. Detail: the advice.
    pub const ADVICE_NOT_BUILT: &str = "madvise advice not built";
    /// An `ioctl` request of a file whose Linux driver has requests this kernel does not build; it
    /// answers as that driver does for a request it does not know. Detail: the request.
    pub const IOCTL_NOT_BUILT: &str = "ioctl request not built";
    /// A terminal line discipline Linux has other than `n_tty`; it answers EINVAL, as a Linux that
    /// has not registered it does. Detail: the discipline's number.
    pub const LDISC_NOT_BUILT: &str = "line discipline not built";
    /// `mincore`, `getrusage`, `sysinfo` or `/proc/meminfo` reported every mapped page as in
    /// memory, `cachestat` or `statfs` counted a mapped file's pages without those its faults
    /// brought in, or a core dump took anonymous memory as touched and pages of zeros as never
    /// touched: which pages a program has touched is not known here. Detail: none.
    pub const RESIDENCY_NOT_TRACKED: &str = "page residency not tracked";
    /// A program started with an unlimited stack or `ADDR_COMPAT_LAYOUT`, for which Linux places
    /// new mappings upwards from a low base; here they go downwards from below the stack, as
    /// otherwise. Detail: none.
    pub const BOTTOM_UP_LAYOUT_NOT_BUILT: &str = "bottom-up mapping layout not built";
    /// A seccomp filter asked for a listener, through which another process would answer the calls it
    /// stops; not built. Detail: the flags.
    pub const SECCOMP_LISTENER_NOT_IMPLEMENTED: &str = "seccomp listener not implemented";
    /// A `/proc` entry Linux has and this kernel does not build was looked up; it answers ENOENT.
    /// Detail: the directory's index in Linux's list of entries, times 1024, plus the name's.
    pub const PROC_ENTRY_NOT_BUILT: &str = "proc entry not built";
    /// A name Linux has in `/sys` was looked up there; sysfs here holds none of its entries, so it
    /// answers ENOENT. Detail: the name's index in the reference's `/sys` (block, bus, class, dev,
    /// devices, firmware, fs, kernel, module).
    pub const SYS_ENTRY_NOT_BUILT: &str = "sys entry not built";
    /// A `/proc` file was made whose fields this kernel does not track all of (those show 0 or are
    /// left out). Detail: which file (0 `stat`, 1 `statm`, 2 `status`, 3 `mounts`, 4 `meminfo`,
    /// 5 `oom_score`, 6 a core file's notes, 7 a `PROCMAP_QUERY` build id, answered as none).
    pub const PROC_FIELDS_NOT_TRACKED: &str = "proc fields not tracked";
    /// A memory fault's syndrome in a handler's frame: whether the page was present and at which
    /// level a table was missing are Linux's page tables' own, so a level 3 fault of a page present
    /// when it allows something is given. Detail: the syndrome.
    pub const FAULT_SYNDROME_APPROXIMATED: &str = "fault syndrome approximated";
    /// The time between a thread being woken and it running, measured, not a slow path. Detail: none.
    pub const WAKE_TO_RUN: &str = "wake to run";
    /// One message to the file backend and the time until its answer, measured: every file call on
    /// the backend sends its messages, with no cache in phase one. Detail: the FUSE opcode.
    pub const BACKEND_MESSAGE: &str = "backend message";
    /// A notice the backend sent unasked and the kernel handled, measured, not a slow path. Detail:
    /// the notice's code.
    pub const BACKEND_NOTICE: &str = "backend notice";
    /// The host could not hand a message to the backend; the call that asked gets ENOTCONN, as from
    /// a FUSE connection that is gone. Detail: the FUSE opcode.
    pub const BACKEND_UNREACHABLE: &str = "backend unreachable";
    /// A call the kernel waits on its system for was larger than the room a waiting call has in
    /// its cell, so it waits on the heap, one allocation each time. Detail: what it asks (the FUSE
    /// opcode).
    pub const WAITING_CALL_ON_HEAP: &str = "waiting call on the heap";
    /// A mapped backend file's pages were written back whole, changed or not, since which pages a
    /// shared mapping wrote is not known (Linux writes back only dirty pages). Detail: none.
    pub const BACKEND_PAGES_WRITTEN_WHOLE: &str = "backend pages written whole";
    /// The kernel touched pages of a mapped backend file not read yet where no call or fault runs
    /// to wait for them (a signal frame written there): they are not asked for, and the touch is a
    /// fault, where Linux would read them. Detail: none.
    pub const BACKEND_PAGES_UNASKED: &str = "backend pages needed outside a call";
    /// A call copied part of its bytes, then reached pages of a mapped backend file not read yet:
    /// it ends with what it did, as at a fault, where Linux's copy would wait for the pages.
    /// Detail: the call's number.
    pub const COPY_SHORT_OF_PAGES: &str = "copy short of backend pages";
    /// The backend said a mapped file's bytes changed (INVAL_INODE): its mapped pages, and reads of
    /// it while mapped, keep the bytes they had, where Linux reads them again (plan section 15).
    /// Detail: the node number.
    pub const BACKEND_CHANGE_NOT_MAPPED: &str = "backend change not seen by mapped pages";
    /// A parked call asked the backend a question other than the one an earlier run of it had
    /// answered (what the call read changed while it waited, which the locks a waiting call holds
    /// do not cover): that answer and the ones after it were dropped and the question sent, so the
    /// backend may have done an earlier one's work too. Detail: the FUSE opcode.
    pub const BACKEND_ASKED_AGAIN: &str = "backend question asked again";
    /// A message the backend answers no one, under way when the container was saved, was lost when
    /// it was brought back on another system (its requests a call waits for are asked again).
    /// Detail: the FUSE opcode.
    pub const BACKEND_MESSAGE_LOST: &str = "backend message lost at restore";
    /// A socket of an address family that is not built. Detail: the family.
    pub const FAMILY_NOT_BUILT: &str = "socket family not built";
    /// A socket option that is not built. Detail: the level times 2^32 plus the option.
    pub const OPTION_NOT_BUILT: &str = "socket option not built";
    /// A socket option set, or a send flag given, whose effect on what the other end sees and
    /// when is not built: TCP_CORK, TCP_FASTOPEN_CONNECT and MSG_MORE on a connection to the
    /// outside (bytes go at once, where Linux holds them). The option is kept and read back.
    /// Detail: the level times 2^32 plus the option, or the flag for MSG_MORE.
    pub const OPTION_NOT_ACTED_ON: &str = "socket option not acted on";
    /// A socket of a built family whose kind or protocol is not built: raw packets, ping, SCTP,
    /// MPTCP, UDP-Lite (plan section 15). Answered as Linux without the power or the protocol.
    /// Detail: the family times 2^32, plus the kind times 2^16, plus the protocol.
    pub const PROTOCOL_NOT_BUILT: &str = "socket protocol not built";
    /// A netlink request Linux answers that is not built: answered EOPNOTSUPP, as Linux does
    /// for a request with no handler. Detail: the message type, plus 2^16 for a dump.
    pub const NETLINK_REQUEST_NOT_BUILT: &str = "netlink request not built";
    /// The neighbour table was dumped after packets left by eth0: the entries those make on
    /// Linux (the gateway's, which the host's own network stands for, and groups' and
    /// broadcasts') are not built, so only the loopback entries were given. Detail: none.
    pub const NEIGHBOURS_NOT_BUILT: &str = "neighbour entries not built";
    /// A name query to the container's name server asked for something other than an address of
    /// the Internet class, which the host's lookup cannot answer: "not implemented" was answered.
    /// Detail: the query type (0 for a message with other than one standard question).
    pub const NAME_QUERY_NOT_ANSWERED: &str = "name query not answered";
    /// A netlink answer about an interface carried its traffic counters, which are not kept here
    /// and read zero. Detail: none.
    pub const INTERFACE_COUNTERS_NOT_KEPT: &str = "interface counters not kept";
    /// A TCP send found no room: how much a connection holds is a fixed model here (the sender's
    /// SO_SNDBUF of unread bytes), where Linux's depends on timing (window and buffer growth).
    /// Detail: none.
    pub const TCP_ROOM_MODELLED: &str = "tcp room modelled";
    /// TCP_INFO was read of a connection: its round trip times, rates, windows past the
    /// handshake, segment counts and the like come from this TCP's model (every segment arrives
    /// and is acknowledged at once), where Linux's depend on timing. Detail: none.
    pub const TCP_INFO_MODELLED: &str = "tcp info modelled";
    /// A UDP datagram to a multicast group or a broadcast address left by eth0: the container's
    /// own sockets were given it, but the host carries no multicast or broadcast, so the network
    /// outside never sees it (plan section 15). Detail: none.
    pub const MULTICAST_NOT_SENT_OUT: &str = "multicast or broadcast not sent outside";
    /// A TCP connect found the listener's backlog full: it waits for room and completes when an
    /// accept makes some, where Linux drops the handshake and retries after 1, 3, 7 seconds and so
    /// on. Detail: none.
    pub const TCP_BACKLOG_WAIT: &str = "tcp connect waits for backlog room";
    /// A connection from outside came to a listener with TCP_DEFER_ACCEPT: the host has no way to
    /// hold it back until data comes, so it is accepted at once. Detail: none.
    pub const OUTSIDE_NOT_DEFERRED: &str = "outside connection not deferred";
    /// A listener the outside could reach (bound to the any address or eth0's) was refused by the
    /// host's listen, or an accept from it failed: the host cannot accept from outside (plan
    /// section 9) or its port is taken there. It goes on listening inside the container. Detail:
    /// the port.
    pub const OUTSIDE_LISTEN_FAILED: &str = "listening outside failed";
    /// A receiver asked for the TTL, hop limit, type of service or class of a datagram from
    /// outside or from the name server, which the host does not report: 64 and 0 were given.
    /// Detail: none.
    pub const OUTSIDE_HEADER_MADE_UP: &str = "outside datagram header made up";
    /// An outside destination refused a datagram from a socket that asked for queued errors: the
    /// socket hears of it, but the host does not say what the datagram held, so no error is
    /// queued. Detail: none.
    pub const OUTSIDE_ERROR_NOT_QUEUED: &str = "outside datagram error not queued";
    /// A notice from the backend that Linux refuses, with nothing done: a body of the wrong size,
    /// or a DELETE whose parent and name are not the child's. The backend is told the error when
    /// the next notice is asked for, as Linux answers its write. Detail: the notice's code times
    /// 2^32 plus the error number.
    pub const BACKEND_NOTICE_REFUSED: &str = "backend notice refused";
    /// The system's wait for the backend's notices failed, so no later notice is applied: a FUSE
    /// connection's notices end with it. Detail: the Linux error number, or 0 for another failure.
    pub const BACKEND_NOTICES_ENDED: &str = "backend notices ended";
    /// An answer or notice from the backend that the kernel could not read or does not handle.
    /// Detail: the request number, or the notice's code.
    pub const BACKEND_UNEXPECTED: &str = "backend message unexpected";
    /// vmsplice copied the caller's pages into a pipe; Linux puts the pages themselves there, so a
    /// later change to them shows in what is read. Detail: the call number.
    pub const VMSPLICE_COPIED: &str = "vmsplice pages copied";
    /// While the embedder watches memory, a call copied memory its description
    /// (`syscall/touch.rs`) did not name, so a watch there is met only at the copy. Detail: the
    /// call's number.
    pub const COPY_UNDESCRIBED: &str = "copy outside the call's description";
    /// A kernel copy met a page the embedder watches that the call's description did not name;
    /// the call runs again once the watch is met, doing again whatever it did before the copy.
    /// Detail: the call's number.
    pub const WATCH_MET_AT_A_COPY: &str = "watched memory met at a copy";
    /// A call did part of its work before a copy met a watched page, and keeps its short result.
    /// Detail: the call's number.
    pub const CALL_SHORT_OF_WATCHED_MEMORY: &str = "call short of watched memory";
    /// The task id source answered a number something still carries, and was asked again (as
    /// Linux's `alloc_pid` skips used numbers once they wrap). Detail: the number.
    pub const TASK_ID_CARRIED: &str = "task id still carried";
    /// The task id source answered a number outside 1 to `pid_max`; the fork or clone fails with
    /// EAGAIN. Detail: the number.
    pub const TASK_ID_OUT_OF_RANGE: &str = "task id out of range";
    /// A container's start set a sysctl this kernel holds no state for (a value fixed here, or a
    /// file `/proc/sys` does not have): the start fails, with EROFS or ENOENT. Detail: the file's
    /// index in `/proc`'s table, or the table's length where it has none.
    pub const SYSCTL_NOT_BUILT: &str = "sysctl not built";
    /// A container's start asked for mounts other than Docker's with some left out that this
    /// kernel makes: one not Docker's, `/dev` left out, or one inside a mount left out. The start
    /// fails with EINVAL. Detail: the mount's index in Docker's list, or the list's length for one
    /// not in it.
    pub const MOUNT_NOT_BUILT: &str = "mount not built";
    /// A request for memory found no room below the executor's ceiling. Detail: the call's number.
    pub const PROCESS_OUTGROWN: &str = "process outgrew the ceiling";
    /// A signal the embedder sent through a process's handle names no thread of that process, has
    /// no valid number, or does not fit the receiving user's queue; it is not sent. Detail: the
    /// signal's number.
    pub const SIGNAL_NOT_SENT: &str = "signal from the embedder not sent";
    /// A hook asked to suspend a task the kernel is in the middle of a step for (a call that has
    /// begun, a signal it acts on, a start, a process ending); it is not taken. Detail: the task.
    pub const TASK_IN_A_STEP_NOT_SUSPENDED: &str =
        "task in the middle of a kernel step not suspended";
    /// An event arrived that no thread was waiting for.
    pub const EVENT_UNEXPECTED: &str = "event with no waiter";
    /// The CPU side answered that this host cannot make a memory, a thread or a block of shared
    /// pages at all; the guest's call fails with ENOSYS. Detail: none.
    pub const HOST_CANNOT: &str = "host cannot do this";
    /// A mapping asked for at an address Linux's address space has and the host's does not (plan
    /// section 15, the address space is small): a fixed one is refused (ENOMEM), a hint is placed
    /// elsewhere. Detail: the address.
    pub const ADDRESS_ABOVE_SPACE: &str = "address above the address space";
    /// A container's address space asked for wider than the reference's 48 bit one (ARM64's 52
    /// bit addresses are not built), or not a whole number of pages: it gets the reference's, or
    /// the whole pages below what was asked. Detail: the size asked for.
    pub const ADDRESS_SPACE_NARROWED: &str = "address space narrowed";

    /// Every reason that means something Linux has is not built here, each answered through
    /// `Counters::refuse`: the status file's list of what is missing comes
    /// from these names.
    pub const NOT_BUILT: &[&str] = &[
        NOT_IMPLEMENTED,
        FLAG_NOT_BUILT,
        FCNTL_NOT_BUILT,
        PRCTL_NOT_BUILT,
        IOCTL_NOT_BUILT,
        LDISC_NOT_BUILT,
        ADVICE_NOT_BUILT,
        DEVICE_NOT_BUILT,
        PROC_ENTRY_NOT_BUILT,
        SYS_ENTRY_NOT_BUILT,
        SECCOMP_LISTENER_NOT_IMPLEMENTED,
        FAMILY_NOT_BUILT,
        OPTION_NOT_BUILT,
        PROTOCOL_NOT_BUILT,
        NETLINK_REQUEST_NOT_BUILT,
        SYSCTL_NOT_BUILT,
        MOUNT_NOT_BUILT,
    ];
}

/// One counter detail made of two numbers, `high` in the upper half.
pub(crate) fn detail(high: u32, low: u32) -> u64 {
    (u64::from(high) << u32::BITS) | u64::from(low)
}

/// The detail of a socket that is not built: its family, then its type and protocol in the lower half.
pub(crate) fn socket_detail(family: u32, kind: u32, protocol: u32) -> u64 {
    detail(family, 0) | (u64::from(kind) << u16::BITS) | u64::from(protocol)
}

#[derive(Default)]
pub(crate) struct Counters {
    counts: BTreeMap<(&'static str, u64), u64>,
    /// Measured times: how many and their sum in nanoseconds.
    times: BTreeMap<(&'static str, u64), (u64, u64)>,
    /// Occurrences seen where the host cannot be reached (a path walk), counted at the next flush.
    noted: AtomicRefCell<Vec<(&'static str, u64)>>,
}

impl Counters {
    /// Counts one occurrence and tells the host the fallback was used.
    pub(crate) fn count<H: FallbackObserver<Warning<'static>>>(
        &mut self,
        host: &H,
        reason: &'static str,
        detail: u64,
    ) {
        let count = self.counts.entry((reason, detail)).or_insert(0);
        *count += 1;
        host.on_fallback_used(&Warning {
            reason,
            count: *count,
            detail,
        });
    }

    /// The one path for something Linux has that is not built here: counts it under one of
    /// [`reason::NOT_BUILT`], warns, and gives back the error Linux answers when it lacks the
    /// feature.
    pub(crate) fn refuse<H: FallbackObserver<Warning<'static>>>(
        &mut self,
        host: &H,
        reason: &'static str,
        detail: u64,
        answer: crate::Errno,
    ) -> crate::Errno {
        debug_assert!(reason::NOT_BUILT.contains(&reason), "{reason}");
        self.count(host, reason, detail);
        answer
    }

    /// A flag or form of call `number` that is not built, as [`Counters::refuse`] counts it.
    pub(crate) fn refuse_flag<H: FallbackObserver<Warning<'static>>>(
        &mut self,
        host: &H,
        number: u32,
        flags: u64,
        answer: crate::Errno,
    ) -> crate::Errno {
        self.refuse(
            host,
            reason::FLAG_NOT_BUILT,
            detail(number, flags as u32),
            answer,
        )
    }

    /// Adds a measured time under a reason, without a warning: a measurement, not a slow path.
    pub(crate) fn measure(&mut self, reason: &'static str, detail: u64, nanoseconds: u64) {
        let entry = self.times.entry((reason, detail)).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += nanoseconds;
    }

    /// Notes an occurrence to count at the next [`Counters::flush`].
    pub(crate) fn note(&self, reason: &'static str, detail: u64) {
        self.noted.borrow_mut().push((reason, detail));
    }

    /// Counts and warns of every noted occurrence.
    pub(crate) fn flush<H: FallbackObserver<Warning<'static>>>(&mut self, host: &H) {
        for (reason, detail) in core::mem::take(self.noted.get_mut()) {
            self.count(host, reason, detail);
        }
    }

    pub(crate) fn readings(&self) -> impl Iterator<Item = CounterReading> + '_ {
        let counted = self
            .counts
            .iter()
            .map(|(&(reason, detail), &count)| CounterReading {
                reason,
                detail,
                count,
                nanoseconds: 0,
            });
        let measured =
            self.times.iter().map(
                |(&(reason, detail), &(count, nanoseconds))| CounterReading {
                    reason,
                    detail,
                    count,
                    nanoseconds,
                },
            );
        counted.chain(measured)
    }
}
