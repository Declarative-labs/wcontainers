#![allow(
    reviewable::interface_item,
    reviewable::number_constant_without_link,
    reason = "Linux's siginfo_t for ARM64 and x86-64, which share one layout and one table of codes"
)]

use bitflags::bitflags;

use crate::signal::Signal;
use crate::{TaskId, UserId};

/// The [information associated with a signal](https://man7.org/linux/man-pages/man3/siginfo_t.3type.html), by the kind of report it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignalInformation {
    /// A signal a process sent with [`kill`](https://man7.org/linux/man-pages/man2/kill.2.html) or [`tgkill`](https://man7.org/linux/man-pages/man2/tkill.2.html), which carries no value.
    SentByProcess {
        signal: Signal,
        code: SendCode,
        sender: ProcessIdentity,
    },
    /// A signal a process queued with a value, as [`sigqueue`](https://man7.org/linux/man-pages/man3/sigqueue.3.html) does.
    QueuedByProcess {
        signal: Signal,
        code: QueueCode,
        sender: ProcessIdentity,
        value: SignalValue,
    },
    /// A `siginfo_t` a process filled in itself and sent with [`rt_sigqueueinfo`](https://man7.org/linux/man-pages/man2/rt_sigqueueinfo.2.html), every byte as the process chose it.
    SuppliedByProcess {
        signal: Signal,
        error_number: SuppliedErrorNumber,
        code: SuppliedCode,
        fields: SuppliedFields,
    },
    /// The expiry of a [POSIX timer](https://man7.org/linux/man-pages/man2/timer_create.2.html), `SI_TIMER`.
    TimerExpired {
        signal: Signal,
        timer: KernelTimerId,
        overrun: TimerOverrunCount,
        value: SignalValue,
    },
    /// A signal the kernel sent with nothing more to say, `SI_KERNEL`.
    SentByKernel { signal: Signal },
    /// The `SIGKILL` that destroys the other threads of a process calling [`execve`](https://man7.org/linux/man-pages/man2/execve.2.html), `SI_DETHREAD`.
    ThreadDestroyedByExecve,
    /// An event on a file descriptor, in the signal [`F_SETSIG`](https://man7.org/linux/man-pages/man2/F_SETSIG.2const.html) chose or `SIGIO`.
    InputOutputEvent {
        signal: Signal,
        code: PollCode,
        band: PollBand,
        descriptor: FileDescriptor,
    },
    /// A `SIGILL`.
    IllegalInstruction {
        code: IllegalInstructionCode,
        address: u64,
    },
    /// A `SIGFPE`.
    ArithmeticError {
        code: ArithmeticErrorCode,
        address: u64,
    },
    /// A `SIGSEGV`.
    SegmentationFault {
        code: SegmentationFaultCode,
        address: u64,
    },
    /// A `SIGBUS`.
    BusError { code: BusErrorCode, address: u64 },
    /// A `SIGTRAP` the processor or a perf event raised.
    Trap { code: TrapCode, address: u64 },
    /// The termination of a child, in the [termination signal](https://man7.org/linux/man-pages/man2/clone.2.html#:~:text=The%20child%20termination%20signal) it was created with.
    ChildTerminated {
        signal: Signal,
        code: ChildTerminationCode,
        child: ProcessIdentity,
        user_time: ClockTicks,
        system_time: ClockTicks,
    },
    /// A `SIGCHLD` about a child that stopped or continued.
    ChildStateChanged {
        code: ChildStateCode,
        child: ProcessIdentity,
        user_time: ClockTicks,
        system_time: ClockTicks,
    },
    /// A `SIGSYS` about a system call a filter or a dispatcher stopped.
    BadSystemCall {
        code: BadSystemCallCode,
        instruction_address: u64,
        system_call: SystemCallNumber,
        architecture: SystemCallArchitecture,
    },
    /// A `SIGTRAP` a [ptrace](https://man7.org/linux/man-pages/man2/ptrace.2.html) stop reports.
    TracerStop {
        stop: TracerStop,
        tracee: ProcessIdentity,
    },
}

/// A process ID and its real user ID, the `si_pid` and `si_uid` of a [`siginfo_t`](https://man7.org/linux/man-pages/man2/sigaction.2.html#:~:text=siginfo_t%20%7B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProcessIdentity {
    pub process: TaskId,
    pub real_user: UserId,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SendCode {
    SI_USER = 0,
    SI_TKILL = -6,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum QueueCode {
    SI_QUEUE = -1,
    SI_MESGQ = -3,
    SI_ASYNCIO = -4,
    SI_ASYNCNL = -60,
}

/// The `si_errno` value a process wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuppliedErrorNumber(pub i32);

/// The `si_code` a process wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuppliedCode(pub i32);

/// The bytes of a `siginfo_t` after `si_code`, as a process wrote them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SuppliedFields(pub [u8; SUPPLIED_FIELDS_SIZE]);

/// The size of a `siginfo_t`.
pub const SI_MAX_SIZE: usize = 128;

/// The size of `si_signo`, `si_errno` and `si_code`, the three `int`s a `siginfo_t` starts with.
pub const SIGINFO_HEADER_SIZE: usize = 12;

pub const SUPPLIED_FIELDS_SIZE: usize = SI_MAX_SIZE - SIGINFO_HEADER_SIZE;

/// The bytes of a [`union sigval`](https://man7.org/linux/man-pages/man3/sigval.3type.html), an `int` or a pointer as its sender chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SignalValue(pub u64);

/// The kernel's internal [ID of a POSIX timer](https://man7.org/linux/man-pages/man2/sigaction.2.html#:~:text=The%20si_timerid%20field), not the one `timer_create` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KernelTimerId(pub u32);

/// The [overrun count](https://man7.org/linux/man-pages/man2/timer_getoverrun.2.html) of a timer when its signal was delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimerOverrunCount(pub u32);

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum PollCode {
    /// A queued `SIGIO`, which Linux sent only up to 2.2.
    SI_SIGIO = -5,
    POLL_IN = 1,
    POLL_OUT = 2,
    POLL_MSG = 3,
    POLL_ERR = 4,
    POLL_PRI = 5,
    POLL_HUP = 6,
}

/// A set of [`poll`](https://man7.org/linux/man-pages/man2/poll.2.html) events with at least one event in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PollBand(PollEvents);

impl PollBand {
    pub const fn new(events: PollEvents) -> Option<Self> {
        if events.is_empty() {
            None
        } else {
            Some(Self(events))
        }
    }
    pub const fn get(self) -> PollEvents {
        self.0
    }
}

bitflags! {
    /// The events of a [`poll`](https://man7.org/linux/man-pages/man2/poll.2.html) `revents` mask.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PollEvents: u16 {
        const POLLIN = 1 << 0;
        const POLLPRI = 1 << 1;
        const POLLOUT = 1 << 2;
        const POLLERR = 1 << 3;
        const POLLHUP = 1 << 4;
        const POLLNVAL = 1 << 5;
        const POLLRDNORM = 1 << 6;
        const POLLRDBAND = 1 << 7;
        const POLLWRNORM = 1 << 8;
        const POLLWRBAND = 1 << 9;
        const POLLMSG = 1 << 10;
        const POLLRDHUP = 1 << 13;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileDescriptor(pub u32);

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum IllegalInstructionCode {
    ILL_ILLOPC = 1,
    ILL_ILLOPN = 2,
    ILL_ILLADR = 3,
    ILL_ILLTRP = 4,
    ILL_PRVOPC = 5,
    ILL_PRVREG = 6,
    ILL_COPROC = 7,
    ILL_BADSTK = 8,
    ILL_BADIADDR = 9,
    __ILL_BREAK = 10,
    __ILL_BNDMOD = 11,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ArithmeticErrorCode {
    FPE_INTDIV = 1,
    FPE_INTOVF = 2,
    FPE_FLTDIV = 3,
    FPE_FLTOVF = 4,
    FPE_FLTUND = 5,
    FPE_FLTRES = 6,
    FPE_FLTINV = 7,
    FPE_FLTSUB = 8,
    __FPE_DECOVF = 9,
    __FPE_DECDIV = 10,
    __FPE_DECERR = 11,
    __FPE_INVASC = 12,
    __FPE_INVDEC = 13,
    FPE_FLTUNK = 14,
    FPE_CONDTRAP = 15,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum SegmentationFaultCode {
    SEGV_MAPERR = 1,
    SEGV_ACCERR = 2,
    SEGV_BNDERR(AddressBounds) = 3,
    SEGV_PKUERR(ProtectionKey) = 4,
    SEGV_ACCADI = 5,
    SEGV_ADIDERR = 6,
    SEGV_ADIPERR = 7,
    SEGV_MTEAERR = 8,
    SEGV_MTESERR = 9,
    SEGV_CPERR = 10,
}

/// The [bounds](https://man7.org/linux/man-pages/man2/sigaction.2.html#:~:text=SEGV_BNDERR%20suberror) an access fell outside of, `si_lower` and `si_upper`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddressBounds {
    pub lower: u64,
    pub upper: u64,
}

/// A [memory protection key](https://man7.org/linux/man-pages/man7/pkeys.7.html).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProtectionKey(pub u32);

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum BusErrorCode {
    BUS_ADRALN = 1,
    BUS_ADRERR = 2,
    BUS_OBJERR = 3,
    BUS_MCEERR_AR(AddressLeastSignificantBit) = 4,
    BUS_MCEERR_AO(AddressLeastSignificantBit) = 5,
}

/// The [least significant bit](https://man7.org/linux/man-pages/man2/sigaction.2.html#:~:text=also%20fill%20in%20si_addr_lsb) of a reported address, and so the extent of the corruption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddressLeastSignificantBit(u8);

impl AddressLeastSignificantBit {
    pub const fn new(bit: u8) -> Option<Self> {
        if (bit as u32) < u64::BITS {
            Some(Self(bit))
        } else {
            None
        }
    }
    pub const fn get(self) -> u8 {
        self.0
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TrapCode {
    TRAP_BRKPT = 1,
    TRAP_TRACE = 2,
    TRAP_BRANCH = 3,
    TRAP_HWBKPT = 4,
    TRAP_UNK = 5,
    TRAP_PERF {
        data: PerfEventSignalData,
        event_type: PerfEventType,
        delivery: PerfEventDelivery,
    } = 6,
}

/// The [`sig_data`](https://man7.org/linux/man-pages/man2/perf_event_open.2.html#:~:text=sig_data%20%28since) of the perf event that raised the signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PerfEventSignalData(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PerfEventType(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PerfEventDelivery {
    Synchronous,
    /// With `TRAP_PERF_FLAG_ASYNC` set.
    Asynchronous,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ChildTerminationCode {
    CLD_EXITED(ExitStatus) = 1,
    CLD_KILLED(Signal) = 2,
    CLD_DUMPED(Signal) = 3,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ChildStateCode {
    CLD_TRAPPED(Signal) = 4,
    CLD_STOPPED(Signal) = 5,
    /// A continue by `SIGCONT`, which is also its `si_status`.
    CLD_CONTINUED = 6,
}

/// The status a process [exited](https://man7.org/linux/man-pages/man2/_exit.2.html) with, the low byte of the value it gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExitStatus(pub u8);

/// A time in [clock ticks](https://man7.org/linux/man-pages/man2/times.2.html#:~:text=clock%20ticks%20per%20second), of which a second has `sysconf(_SC_CLK_TCK)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockTicks(pub u64);

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum BadSystemCallCode {
    SYS_SECCOMP(SeccompReturnData) = 1,
    SYS_USER_DISPATCH = 2,
}

/// The [`SECCOMP_RET_DATA`](https://man7.org/linux/man-pages/man2/seccomp.2.html#:~:text=SECCOMP_RET_DATA) part of a filter's return value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SeccompReturnData(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SystemCallNumber(pub i32);

/// The [`AUDIT_ARCH_*`](https://man7.org/linux/man-pages/man2/seccomp.2.html#:~:text=AUDIT_ARCH_%2A%20value) value of the calling convention a system call was made with.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SystemCallArchitecture {
    AUDIT_ARCH_AARCH64 = 0xC000_00B7,
    AUDIT_ARCH_X86_64 = 0xC000_003E,
    /// The 32-bit x86 calling convention, which an x86-64 program reaches with `int 0x80`.
    AUDIT_ARCH_I386 = 0x4000_0003,
}

/// A stop of a traced thread, as [`PTRACE_GETSIGINFO`](https://man7.org/linux/man-pages/man2/ptrace.2.html#:~:text=PTRACE_GETSIGINFO%20%28since) reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TracerStop {
    /// A syscall-stop of a tracee without `PTRACE_O_TRACESYSGOOD`.
    SystemCall,
    /// A syscall-stop of a tracee with `PTRACE_O_TRACESYSGOOD`.
    SystemCallMarked,
    Event(PtraceEvent),
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PtraceEvent {
    PTRACE_EVENT_FORK = 1,
    PTRACE_EVENT_VFORK = 2,
    PTRACE_EVENT_CLONE = 3,
    PTRACE_EVENT_EXEC = 4,
    PTRACE_EVENT_VFORK_DONE = 5,
    PTRACE_EVENT_EXIT = 6,
    PTRACE_EVENT_SECCOMP = 7,
    PTRACE_EVENT_STOP = 128,
}
