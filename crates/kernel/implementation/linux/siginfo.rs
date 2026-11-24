//! The interface's record of a signal and the `siginfo` bytes the kernel keeps, each made from the
//! other by Linux's layout of the record for its code (`siginfo_layout`).

use core::mem::offset_of;

use wcontainers_kernel::signal::Signal;
use wcontainers_kernel::signal_information::*;
use wcontainers_kernel::{TaskId, UserId};

use crate::linux::general::{
    __sifields__bindgen_ty_1 as Kill, __sifields__bindgen_ty_2 as Timer,
    __sifields__bindgen_ty_3 as Queued, __sifields__bindgen_ty_4 as Child,
    __sifields__bindgen_ty_5 as Fault,
    __sifields__bindgen_ty_5__bindgen_ty_1__bindgen_ty_1 as Bounds,
    __sifields__bindgen_ty_5__bindgen_ty_1__bindgen_ty_2 as Key,
    __sifields__bindgen_ty_5__bindgen_ty_1__bindgen_ty_3 as Perf, __sifields__bindgen_ty_6 as Poll,
    __sifields__bindgen_ty_7 as System, NSIGPOLL, POLLERR, POLLHUP, POLLIN, POLLMSG, POLLNVAL,
    POLLOUT, POLLPRI, POLLRDBAND, POLLRDHUP, POLLRDNORM, POLLWRBAND, POLLWRNORM, SI_DETHREAD,
    SI_KERNEL, SI_TIMER, TRAP_PERF_FLAG_ASYNC, siginfo,
    siginfo__bindgen_ty_1__bindgen_ty_1 as Header,
};

/// The bytes the kernel keeps for a signal's information.
pub(crate) type Bytes = [u8; SI_MAX_SIZE];

const _: () = assert!(size_of::<siginfo>() == SI_MAX_SIZE);
const _: () = assert!(offset_of!(Header, _sifields) == SIGINFO_HEADER_SIZE + size_of::<u32>());

const SIGNAL: usize = offset_of!(Header, si_signo);
const ERROR_NUMBER: usize = offset_of!(Header, si_errno);
const CODE: usize = offset_of!(Header, si_code);
const FIELDS: usize = offset_of!(Header, _sifields);
const PROCESS: usize = FIELDS + offset_of!(Kill, _pid);
const USER: usize = FIELDS + offset_of!(Kill, _uid);
const QUEUED_VALUE: usize = FIELDS + offset_of!(Queued, _sigval);
const TIMER: usize = FIELDS + offset_of!(Timer, _tid);
const OVERRUN: usize = FIELDS + offset_of!(Timer, _overrun);
const TIMER_VALUE: usize = FIELDS + offset_of!(Timer, _sigval);
const STATUS: usize = FIELDS + offset_of!(Child, _status);
const USER_TIME: usize = FIELDS + offset_of!(Child, _utime);
const SYSTEM_TIME: usize = FIELDS + offset_of!(Child, _stime);
const ADDRESS: usize = FIELDS + offset_of!(Fault, _addr);
const FAULT_DETAIL: usize = FIELDS + offset_of!(Fault, __bindgen_anon_1);
const LOWER: usize = FAULT_DETAIL + offset_of!(Bounds, _lower);
const UPPER: usize = FAULT_DETAIL + offset_of!(Bounds, _upper);
const KEY: usize = FAULT_DETAIL + offset_of!(Key, _pkey);
const PERF_DATA: usize = FAULT_DETAIL + offset_of!(Perf, _data);
const PERF_TYPE: usize = FAULT_DETAIL + offset_of!(Perf, _type);
const PERF_FLAGS: usize = FAULT_DETAIL + offset_of!(Perf, _flags);
const BAND: usize = FIELDS + offset_of!(Poll, _band);
const DESCRIPTOR: usize = FIELDS + offset_of!(Poll, _fd);
const CALL_ADDRESS: usize = FIELDS + offset_of!(System, _call_addr);
const SYSTEM_CALL: usize = FIELDS + offset_of!(System, _syscall);
const ARCHITECTURE: usize = FIELDS + offset_of!(System, _arch);

/// Names every signal of the interface by its number.
macro_rules! signals {
    ($($name:ident),* $(,)?) => {
        /// The interface's signal of a number, if Linux has one.
        pub(crate) fn signal(number: u32) -> Option<Signal> {
            [$(Signal::$name),*].into_iter().find(|signal| *signal as u32 == number)
        }
    };
}

signals!(
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL, SIGUSR1, SIGSEGV,
    SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGSTKFLT, SIGCHLD, SIGCONT, SIGSTOP, SIGTSTP, SIGTTIN,
    SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM, SIGPROF, SIGWINCH, SIGIO, SIGPWR, SIGSYS,
    SIGRTMIN, SIGRT_1, SIGRT_2, SIGRT_3, SIGRT_4, SIGRT_5, SIGRT_6, SIGRT_7, SIGRT_8, SIGRT_9,
    SIGRT_10, SIGRT_11, SIGRT_12, SIGRT_13, SIGRT_14, SIGRT_15, SIGRT_16, SIGRT_17, SIGRT_18,
    SIGRT_19, SIGRT_20, SIGRT_21, SIGRT_22, SIGRT_23, SIGRT_24, SIGRT_25, SIGRT_26, SIGRT_27,
    SIGRT_28, SIGRT_29, SIGRT_30, SIGRT_31, SIGRTMAX,
);

/// A code of the interface, which Linux gives as a number in `si_code`.
pub(crate) trait Code: Copy {
    /// The number Linux gives the code.
    fn number(self) -> i32;
}

/// The codes whose enums carry no fields, whose discriminants `as` gives.
macro_rules! plain_codes {
    ($($code:ty),* $(,)?) => {
        $(impl Code for $code {
            fn number(self) -> i32 {
                self as i32
            }
        })*
    };
}

plain_codes!(
    SendCode,
    QueueCode,
    PollCode,
    IllegalInstructionCode,
    ArithmeticErrorCode,
);

// The codes whose enums carry fields, which `as` cannot give, by the generated bindings' numbers
// for them. The `numbers_are_the_interfaces` test checks they are the interface's discriminants.

impl Code for SegmentationFaultCode {
    fn number(self) -> i32 {
        use crate::linux::general::*;
        (match self {
            Self::SEGV_MAPERR => SEGV_MAPERR,
            Self::SEGV_ACCERR => SEGV_ACCERR,
            Self::SEGV_BNDERR(_) => SEGV_BNDERR,
            Self::SEGV_PKUERR(_) => SEGV_PKUERR,
            Self::SEGV_ACCADI => SEGV_ACCADI,
            Self::SEGV_ADIDERR => SEGV_ADIDERR,
            Self::SEGV_ADIPERR => SEGV_ADIPERR,
            Self::SEGV_MTEAERR => SEGV_MTEAERR,
            Self::SEGV_MTESERR => SEGV_MTESERR,
            Self::SEGV_CPERR => SEGV_CPERR,
        }) as i32
    }
}

impl Code for BusErrorCode {
    fn number(self) -> i32 {
        use crate::linux::general::*;
        (match self {
            Self::BUS_ADRALN => BUS_ADRALN,
            Self::BUS_ADRERR => BUS_ADRERR,
            Self::BUS_OBJERR => BUS_OBJERR,
            Self::BUS_MCEERR_AR(_) => BUS_MCEERR_AR,
            Self::BUS_MCEERR_AO(_) => BUS_MCEERR_AO,
        }) as i32
    }
}

impl Code for TrapCode {
    fn number(self) -> i32 {
        use crate::linux::general::*;
        (match self {
            Self::TRAP_BRKPT => TRAP_BRKPT,
            Self::TRAP_TRACE => TRAP_TRACE,
            Self::TRAP_BRANCH => TRAP_BRANCH,
            Self::TRAP_HWBKPT => TRAP_HWBKPT,
            Self::TRAP_UNK => TRAP_UNK,
            Self::TRAP_PERF { .. } => TRAP_PERF,
        }) as i32
    }
}

impl Code for ChildTerminationCode {
    fn number(self) -> i32 {
        use crate::linux::general::*;
        (match self {
            Self::CLD_EXITED(_) => CLD_EXITED,
            Self::CLD_KILLED(_) => CLD_KILLED,
            Self::CLD_DUMPED(_) => CLD_DUMPED,
        }) as i32
    }
}

impl Code for ChildStateCode {
    fn number(self) -> i32 {
        use crate::linux::general::*;
        (match self {
            Self::CLD_TRAPPED(_) => CLD_TRAPPED,
            Self::CLD_STOPPED(_) => CLD_STOPPED,
            Self::CLD_CONTINUED => CLD_CONTINUED,
        }) as i32
    }
}

impl Code for BadSystemCallCode {
    fn number(self) -> i32 {
        use crate::linux::general::*;
        (match self {
            Self::SYS_SECCOMP(_) => SYS_SECCOMP,
            Self::SYS_USER_DISPATCH => SYS_USER_DISPATCH,
        }) as i32
    }
}

/// The number Linux gives a code.
pub(crate) fn number(code: &impl Code) -> i32 {
    code.number()
}

/// The code of `candidates` whose number Linux gives as `code`.
fn code_of<T: Code>(candidates: impl IntoIterator<Item = T>, code: i32) -> Option<T> {
    candidates
        .into_iter()
        .find(|candidate| number(candidate) == code)
}

fn read<const N: usize>(bytes: &Bytes, at: usize) -> [u8; N] {
    bytes[at..at + N].try_into().expect("in the record")
}

fn put(bytes: &mut Bytes, at: usize, value: &[u8]) {
    bytes[at..at + value.len()].copy_from_slice(value);
}

fn i32_at(bytes: &Bytes, at: usize) -> i32 {
    i32::from_le_bytes(read(bytes, at))
}

fn u32_at(bytes: &Bytes, at: usize) -> u32 {
    u32::from_le_bytes(read(bytes, at))
}

fn u64_at(bytes: &Bytes, at: usize) -> u64 {
    u64::from_le_bytes(read(bytes, at))
}

/// Each interface poll event beside Linux's bit for it.
const POLL_BITS: &[(PollEvents, u32)] = &[
    (PollEvents::POLLIN, POLLIN),
    (PollEvents::POLLPRI, POLLPRI),
    (PollEvents::POLLOUT, POLLOUT),
    (PollEvents::POLLERR, POLLERR),
    (PollEvents::POLLHUP, POLLHUP),
    (PollEvents::POLLNVAL, POLLNVAL),
    (PollEvents::POLLRDNORM, POLLRDNORM),
    (PollEvents::POLLRDBAND, POLLRDBAND),
    (PollEvents::POLLWRNORM, POLLWRNORM),
    (PollEvents::POLLWRBAND, POLLWRBAND),
    (PollEvents::POLLMSG, POLLMSG),
    (PollEvents::POLLRDHUP, POLLRDHUP),
];

/// The events of a `poll` mask.
fn poll_events(mask: u64) -> PollEvents {
    POLL_BITS
        .iter()
        .copied()
        .filter(|(_, bit)| mask & u64::from(*bit) != 0)
        .map(|(event, _)| event)
        .collect()
}

/// The `poll` mask of a set of events.
fn poll_mask(events: PollEvents) -> u64 {
    POLL_BITS
        .iter()
        .copied()
        .filter(|(event, _)| events.contains(*event))
        .map(|(_, bit)| u64::from(bit))
        .sum()
}

fn identity(bytes: &Bytes) -> ProcessIdentity {
    ProcessIdentity {
        process: TaskId(u32_at(bytes, PROCESS)),
        real_user: UserId(u32_at(bytes, USER)),
    }
}

/// The record the bytes make in the layout their code gives, if the interface has one for it.
fn structured(bytes: &Bytes) -> Option<SignalInformation> {
    let number = u32_at(bytes, SIGNAL);
    let signal = self::signal(number)?;
    let code = i32_at(bytes, CODE);
    let address = u64_at(bytes, ADDRESS);
    let child = || identity(bytes);
    let user_time = ClockTicks(u64_at(bytes, USER_TIME));
    let system_time = ClockTicks(u64_at(bytes, SYSTEM_TIME));
    let status = i32_at(bytes, STATUS);
    let status_signal = || self::signal(u32::try_from(status).ok()?);
    let lsb = || {
        AddressLeastSignificantBit::new(
            u8::try_from(i16::from_le_bytes(read(bytes, FAULT_DETAIL))).ok()?,
        )
    };
    let poll = |code| {
        Some(SignalInformation::InputOutputEvent {
            signal,
            code,
            band: PollBand::new(poll_events(u64_at(bytes, BAND)))?,
            descriptor: FileDescriptor(u32_at(bytes, DESCRIPTOR)),
        })
    };
    // A code a signal gives itself (`sig_specific_sicodes`); Linux's own codes are above SI_USER.
    if code > SendCode::SI_USER as i32 && code != SI_KERNEL as i32 {
        let specific = match signal {
            Signal::SIGILL => code_of(
                [
                    IllegalInstructionCode::ILL_ILLOPC,
                    IllegalInstructionCode::ILL_ILLOPN,
                    IllegalInstructionCode::ILL_ILLADR,
                    IllegalInstructionCode::ILL_ILLTRP,
                    IllegalInstructionCode::ILL_PRVOPC,
                    IllegalInstructionCode::ILL_PRVREG,
                    IllegalInstructionCode::ILL_COPROC,
                    IllegalInstructionCode::ILL_BADSTK,
                    IllegalInstructionCode::ILL_BADIADDR,
                    IllegalInstructionCode::__ILL_BREAK,
                    IllegalInstructionCode::__ILL_BNDMOD,
                ],
                code,
            )
            .map(|code| SignalInformation::IllegalInstruction { code, address }),
            Signal::SIGFPE => code_of(
                [
                    ArithmeticErrorCode::FPE_INTDIV,
                    ArithmeticErrorCode::FPE_INTOVF,
                    ArithmeticErrorCode::FPE_FLTDIV,
                    ArithmeticErrorCode::FPE_FLTOVF,
                    ArithmeticErrorCode::FPE_FLTUND,
                    ArithmeticErrorCode::FPE_FLTRES,
                    ArithmeticErrorCode::FPE_FLTINV,
                    ArithmeticErrorCode::FPE_FLTSUB,
                    ArithmeticErrorCode::__FPE_DECOVF,
                    ArithmeticErrorCode::__FPE_DECDIV,
                    ArithmeticErrorCode::__FPE_DECERR,
                    ArithmeticErrorCode::__FPE_INVASC,
                    ArithmeticErrorCode::__FPE_INVDEC,
                    ArithmeticErrorCode::FPE_FLTUNK,
                    ArithmeticErrorCode::FPE_CONDTRAP,
                ],
                code,
            )
            .map(|code| SignalInformation::ArithmeticError { code, address }),
            Signal::SIGSEGV => code_of(
                [
                    SegmentationFaultCode::SEGV_MAPERR,
                    SegmentationFaultCode::SEGV_ACCERR,
                    SegmentationFaultCode::SEGV_BNDERR(AddressBounds {
                        lower: u64_at(bytes, LOWER),
                        upper: u64_at(bytes, UPPER),
                    }),
                    SegmentationFaultCode::SEGV_PKUERR(ProtectionKey(u32_at(bytes, KEY))),
                    SegmentationFaultCode::SEGV_ACCADI,
                    SegmentationFaultCode::SEGV_ADIDERR,
                    SegmentationFaultCode::SEGV_ADIPERR,
                    SegmentationFaultCode::SEGV_MTEAERR,
                    SegmentationFaultCode::SEGV_MTESERR,
                    SegmentationFaultCode::SEGV_CPERR,
                ],
                code,
            )
            .map(|code| SignalInformation::SegmentationFault { code, address }),
            Signal::SIGBUS => {
                let mut codes = alloc::vec![
                    BusErrorCode::BUS_ADRALN,
                    BusErrorCode::BUS_ADRERR,
                    BusErrorCode::BUS_OBJERR,
                ];
                if let Some(lsb) = lsb() {
                    codes.extend([
                        BusErrorCode::BUS_MCEERR_AR(lsb),
                        BusErrorCode::BUS_MCEERR_AO(lsb),
                    ]);
                }
                code_of(codes, code).map(|code| SignalInformation::BusError { code, address })
            }
            Signal::SIGTRAP => code_of(
                [
                    TrapCode::TRAP_BRKPT,
                    TrapCode::TRAP_TRACE,
                    TrapCode::TRAP_BRANCH,
                    TrapCode::TRAP_HWBKPT,
                    TrapCode::TRAP_UNK,
                    TrapCode::TRAP_PERF {
                        data: PerfEventSignalData(u64_at(bytes, PERF_DATA)),
                        event_type: PerfEventType(u32_at(bytes, PERF_TYPE)),
                        delivery: if u32_at(bytes, PERF_FLAGS) & TRAP_PERF_FLAG_ASYNC != 0 {
                            PerfEventDelivery::Asynchronous
                        } else {
                            PerfEventDelivery::Synchronous
                        },
                    },
                ],
                code,
            )
            .map(|code| SignalInformation::Trap { code, address }),
            Signal::SIGCHLD => {
                let mut states = alloc::vec![ChildStateCode::CLD_CONTINUED];
                if let Some(stopped) = status_signal() {
                    states.extend([
                        ChildStateCode::CLD_TRAPPED(stopped),
                        ChildStateCode::CLD_STOPPED(stopped),
                    ]);
                }
                code_of(states, code)
                    .map(|code| SignalInformation::ChildStateChanged {
                        code,
                        child: child(),
                        user_time,
                        system_time,
                    })
                    .or_else(|| {
                        terminated(
                            signal,
                            code,
                            status,
                            status_signal(),
                            child(),
                            user_time,
                            system_time,
                        )
                    })
            }
            Signal::SIGSYS => {
                let codes = [
                    BadSystemCallCode::SYS_SECCOMP(SeccompReturnData(
                        u16::try_from(i32_at(bytes, ERROR_NUMBER)).ok()?,
                    )),
                    BadSystemCallCode::SYS_USER_DISPATCH,
                ];
                let architecture = [
                    SystemCallArchitecture::AUDIT_ARCH_AARCH64,
                    SystemCallArchitecture::AUDIT_ARCH_X86_64,
                    SystemCallArchitecture::AUDIT_ARCH_I386,
                ]
                .into_iter()
                .find(|architecture| *architecture as u32 == u32_at(bytes, ARCHITECTURE));
                code_of(codes, code)
                    .zip(architecture)
                    .map(|(code, architecture)| SignalInformation::BadSystemCall {
                        code,
                        instruction_address: u64_at(bytes, CALL_ADDRESS),
                        system_call: SystemCallNumber(i32_at(bytes, SYSTEM_CALL)),
                        architecture,
                    })
            }
            _ => None,
        };
        if specific.is_some() {
            return specific;
        }
        // Linux reads any other signal's small codes as an I/O event's, also a child's end in a
        // signal other than SIGCHLD (`siginfo_layout`).
        let polls = [
            PollCode::POLL_IN,
            PollCode::POLL_OUT,
            PollCode::POLL_MSG,
            PollCode::POLL_ERR,
            PollCode::POLL_PRI,
            PollCode::POLL_HUP,
        ];
        return code_of(polls, code)
            .filter(|_| code <= NSIGPOLL as i32)
            .and_then(poll);
    }
    if code == SI_KERNEL as i32 {
        return Some(SignalInformation::SentByKernel { signal });
    }
    if code == SI_DETHREAD && signal == Signal::SIGKILL {
        return Some(SignalInformation::ThreadDestroyedByExecve);
    }
    if code == SI_TIMER {
        return Some(SignalInformation::TimerExpired {
            signal,
            timer: KernelTimerId(u32_at(bytes, TIMER)),
            overrun: TimerOverrunCount(u32_at(bytes, OVERRUN)),
            value: SignalValue(u64_at(bytes, TIMER_VALUE)),
        });
    }
    if code == PollCode::SI_SIGIO as i32 {
        return poll(PollCode::SI_SIGIO);
    }
    if let Some(code) = code_of([SendCode::SI_USER, SendCode::SI_TKILL], code) {
        return Some(SignalInformation::SentByProcess {
            signal,
            code,
            sender: identity(bytes),
        });
    }
    let queued = [
        QueueCode::SI_QUEUE,
        QueueCode::SI_MESGQ,
        QueueCode::SI_ASYNCIO,
        QueueCode::SI_ASYNCNL,
    ];
    code_of(queued, code).map(|code| SignalInformation::QueuedByProcess {
        signal,
        code,
        sender: identity(bytes),
        value: SignalValue(u64_at(bytes, QUEUED_VALUE)),
    })
}

/// A child's end, in the signal it was made to end with.
fn terminated(
    signal: Signal,
    code: i32,
    status: i32,
    status_signal: Option<Signal>,
    child: ProcessIdentity,
    user_time: ClockTicks,
    system_time: ClockTicks,
) -> Option<SignalInformation> {
    let mut codes = alloc::vec::Vec::new();
    if let Ok(exit) = u8::try_from(status) {
        codes.push(ChildTerminationCode::CLD_EXITED(ExitStatus(exit)));
    }
    if let Some(killer) = status_signal {
        codes.extend([
            ChildTerminationCode::CLD_KILLED(killer),
            ChildTerminationCode::CLD_DUMPED(killer),
        ]);
    }
    code_of(codes, code).map(|code| SignalInformation::ChildTerminated {
        signal,
        code,
        child,
        user_time,
        system_time,
    })
}

/// The interface's record of the bytes: the one their code's layout gives where it holds every
/// byte, else the bytes as a process supplied them.
pub(crate) fn decode(bytes: &Bytes) -> SignalInformation {
    if let Some(information) = structured(bytes).filter(|information| encode(information) == *bytes)
    {
        return information;
    }
    let mut fields = [0; SUPPLIED_FIELDS_SIZE];
    fields.copy_from_slice(&bytes[SIGINFO_HEADER_SIZE..]);
    SignalInformation::SuppliedByProcess {
        signal: signal(u32_at(bytes, SIGNAL)).expect("a queued signal is one of Linux's"),
        error_number: SuppliedErrorNumber(i32_at(bytes, ERROR_NUMBER)),
        code: SuppliedCode(i32_at(bytes, CODE)),
        fields: SuppliedFields(fields),
    }
}

/// The bytes of a record, built from zero as Linux's `clear_siginfo` builds one.
pub(crate) fn encode(information: &SignalInformation) -> Bytes {
    let mut bytes = [0; SI_MAX_SIZE];
    let mut header = |signal: Signal, code: i32| {
        put(&mut bytes, SIGNAL, &(signal as u32).to_le_bytes());
        put(&mut bytes, CODE, &code.to_le_bytes());
    };
    let child_times = |bytes: &mut Bytes,
                       child: &ProcessIdentity,
                       status: i32,
                       user: ClockTicks,
                       system: ClockTicks| {
        put(bytes, PROCESS, &child.process.0.to_le_bytes());
        put(bytes, USER, &child.real_user.0.to_le_bytes());
        put(bytes, STATUS, &status.to_le_bytes());
        put(bytes, USER_TIME, &user.0.to_le_bytes());
        put(bytes, SYSTEM_TIME, &system.0.to_le_bytes());
    };
    match *information {
        SignalInformation::SentByProcess {
            signal,
            code,
            sender,
        } => {
            header(signal, code as i32);
            put(&mut bytes, PROCESS, &sender.process.0.to_le_bytes());
            put(&mut bytes, USER, &sender.real_user.0.to_le_bytes());
        }
        SignalInformation::QueuedByProcess {
            signal,
            code,
            sender,
            value,
        } => {
            header(signal, code as i32);
            put(&mut bytes, PROCESS, &sender.process.0.to_le_bytes());
            put(&mut bytes, USER, &sender.real_user.0.to_le_bytes());
            put(&mut bytes, QUEUED_VALUE, &value.0.to_le_bytes());
        }
        SignalInformation::SuppliedByProcess {
            signal,
            error_number,
            code,
            fields,
        } => {
            header(signal, code.0);
            put(&mut bytes, ERROR_NUMBER, &error_number.0.to_le_bytes());
            put(&mut bytes, SIGINFO_HEADER_SIZE, &fields.0);
        }
        SignalInformation::TimerExpired {
            signal,
            timer,
            overrun,
            value,
        } => {
            header(signal, SI_TIMER);
            put(&mut bytes, TIMER, &timer.0.to_le_bytes());
            put(&mut bytes, OVERRUN, &overrun.0.to_le_bytes());
            put(&mut bytes, TIMER_VALUE, &value.0.to_le_bytes());
        }
        SignalInformation::SentByKernel { signal } => header(signal, SI_KERNEL as i32),
        SignalInformation::ThreadDestroyedByExecve => header(Signal::SIGKILL, SI_DETHREAD),
        SignalInformation::InputOutputEvent {
            signal,
            code,
            band,
            descriptor,
        } => {
            header(signal, code as i32);
            put(&mut bytes, BAND, &poll_mask(band.get()).to_le_bytes());
            put(&mut bytes, DESCRIPTOR, &descriptor.0.to_le_bytes());
        }
        SignalInformation::IllegalInstruction { code, address } => {
            header(Signal::SIGILL, code as i32);
            put(&mut bytes, ADDRESS, &address.to_le_bytes());
        }
        SignalInformation::ArithmeticError { code, address } => {
            header(Signal::SIGFPE, code as i32);
            put(&mut bytes, ADDRESS, &address.to_le_bytes());
        }
        SignalInformation::SegmentationFault { code, address } => {
            header(Signal::SIGSEGV, number(&code));
            put(&mut bytes, ADDRESS, &address.to_le_bytes());
            match code {
                SegmentationFaultCode::SEGV_BNDERR(bounds) => {
                    put(&mut bytes, LOWER, &bounds.lower.to_le_bytes());
                    put(&mut bytes, UPPER, &bounds.upper.to_le_bytes());
                }
                SegmentationFaultCode::SEGV_PKUERR(key) => {
                    put(&mut bytes, KEY, &key.0.to_le_bytes())
                }
                _ => {}
            }
        }
        SignalInformation::BusError { code, address } => {
            header(Signal::SIGBUS, number(&code));
            put(&mut bytes, ADDRESS, &address.to_le_bytes());
            if let BusErrorCode::BUS_MCEERR_AR(lsb) | BusErrorCode::BUS_MCEERR_AO(lsb) = code {
                put(
                    &mut bytes,
                    FAULT_DETAIL,
                    &i16::from(lsb.get()).to_le_bytes(),
                );
            }
        }
        SignalInformation::Trap { code, address } => {
            header(Signal::SIGTRAP, number(&code));
            put(&mut bytes, ADDRESS, &address.to_le_bytes());
            if let TrapCode::TRAP_PERF {
                data,
                event_type,
                delivery,
            } = code
            {
                put(&mut bytes, PERF_DATA, &data.0.to_le_bytes());
                put(&mut bytes, PERF_TYPE, &event_type.0.to_le_bytes());
                let flags = match delivery {
                    PerfEventDelivery::Synchronous => 0,
                    PerfEventDelivery::Asynchronous => TRAP_PERF_FLAG_ASYNC,
                };
                put(&mut bytes, PERF_FLAGS, &flags.to_le_bytes());
            }
        }
        SignalInformation::ChildTerminated {
            signal,
            code,
            child,
            user_time,
            system_time,
        } => {
            header(signal, number(&code));
            let status = match code {
                ChildTerminationCode::CLD_EXITED(status) => i32::from(status.0),
                ChildTerminationCode::CLD_KILLED(signal)
                | ChildTerminationCode::CLD_DUMPED(signal) => signal as i32,
            };
            child_times(&mut bytes, &child, status, user_time, system_time);
        }
        SignalInformation::ChildStateChanged {
            code,
            child,
            user_time,
            system_time,
        } => {
            header(Signal::SIGCHLD, number(&code));
            let status = match code {
                ChildStateCode::CLD_TRAPPED(signal) | ChildStateCode::CLD_STOPPED(signal) => {
                    signal as i32
                }
                ChildStateCode::CLD_CONTINUED => Signal::SIGCONT as i32,
            };
            child_times(&mut bytes, &child, status, user_time, system_time);
        }
        SignalInformation::BadSystemCall {
            code,
            instruction_address,
            system_call,
            architecture,
        } => {
            header(Signal::SIGSYS, number(&code));
            if let BadSystemCallCode::SYS_SECCOMP(data) = code {
                put(&mut bytes, ERROR_NUMBER, &i32::from(data.0).to_le_bytes());
            }
            put(&mut bytes, CALL_ADDRESS, &instruction_address.to_le_bytes());
            put(&mut bytes, SYSTEM_CALL, &system_call.0.to_le_bytes());
            put(
                &mut bytes,
                ARCHITECTURE,
                &(architecture as u32).to_le_bytes(),
            );
        }
        SignalInformation::TracerStop { stop, tracee } => {
            let code = match stop {
                TracerStop::SystemCall => Signal::SIGTRAP as i32,
                TracerStop::SystemCallMarked => Signal::SIGTRAP as i32 | PTRACE_SYSCALL_MARK,
                TracerStop::Event(event) => {
                    Signal::SIGTRAP as i32 | (event as i32) << PTRACE_EVENT_SHIFT
                }
            };
            header(Signal::SIGTRAP, code);
            put(&mut bytes, PROCESS, &tracee.process.0.to_le_bytes());
            put(&mut bytes, USER, &tracee.real_user.0.to_le_bytes());
        }
    }
    bytes
}

/// What `PTRACE_O_TRACESYSGOOD` adds to a syscall-stop's code (`ptrace_report_syscall`).
const PTRACE_SYSCALL_MARK: i32 = 0x80;

/// Where a ptrace event sits in a stop's code (`ptrace_event`: `SIGTRAP | event << 8`).
const PTRACE_EVENT_SHIFT: u32 = u8::BITS;

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps the first `i32` a value hashes, which for an enum with derived `Hash` is its
    /// discriminant (the derive hashes `discriminant_value`, an `i32` for a `repr(i32)` enum).
    #[derive(Default)]
    struct Discriminant(Option<i32>);

    impl core::hash::Hasher for Discriminant {
        fn write(&mut self, _: &[u8]) {}
        fn write_i32(&mut self, value: i32) {
            self.0.get_or_insert(value);
        }
        fn finish(&self) -> u64 {
            0
        }
    }

    fn discriminant(code: &impl core::hash::Hash) -> i32 {
        let mut hasher = Discriminant::default();
        code.hash(&mut hasher);
        hasher.0.expect("an enum's derived hash starts with its discriminant")
    }

    /// The numbers the kernel gives the codes whose enums carry fields, from the generated
    /// bindings, are the interface's discriminants for them.
    #[test]
    fn numbers_are_the_interfaces() {
        fn check<T: Code + core::hash::Hash + core::fmt::Debug>(codes: &[T]) -> usize {
            for code in codes {
                assert_eq!(code.number(), discriminant(code), "{code:?}");
            }
            codes.len()
        }
        let lsb = AddressLeastSignificantBit::new(0).expect("bit 0");
        let checked = check(&[
            SegmentationFaultCode::SEGV_MAPERR,
            SegmentationFaultCode::SEGV_ACCERR,
            SegmentationFaultCode::SEGV_BNDERR(AddressBounds { lower: 0, upper: 0 }),
            SegmentationFaultCode::SEGV_PKUERR(ProtectionKey(0)),
            SegmentationFaultCode::SEGV_ACCADI,
            SegmentationFaultCode::SEGV_ADIDERR,
            SegmentationFaultCode::SEGV_ADIPERR,
            SegmentationFaultCode::SEGV_MTEAERR,
            SegmentationFaultCode::SEGV_MTESERR,
            SegmentationFaultCode::SEGV_CPERR,
        ]) + check(&[
            BusErrorCode::BUS_ADRALN,
            BusErrorCode::BUS_ADRERR,
            BusErrorCode::BUS_OBJERR,
            BusErrorCode::BUS_MCEERR_AR(lsb),
            BusErrorCode::BUS_MCEERR_AO(lsb),
        ]) + check(&[
            TrapCode::TRAP_BRKPT,
            TrapCode::TRAP_TRACE,
            TrapCode::TRAP_BRANCH,
            TrapCode::TRAP_HWBKPT,
            TrapCode::TRAP_UNK,
            TrapCode::TRAP_PERF {
                data: PerfEventSignalData(0),
                event_type: PerfEventType(0),
                delivery: PerfEventDelivery::Synchronous,
            },
        ]) + check(&[
            ChildTerminationCode::CLD_EXITED(ExitStatus(0)),
            ChildTerminationCode::CLD_KILLED(Signal::SIGKILL),
            ChildTerminationCode::CLD_DUMPED(Signal::SIGKILL),
        ]) + check(&[
            ChildStateCode::CLD_TRAPPED(Signal::SIGTRAP),
            ChildStateCode::CLD_STOPPED(Signal::SIGSTOP),
            ChildStateCode::CLD_CONTINUED,
        ]) + check(&[
            BadSystemCallCode::SYS_SECCOMP(SeccompReturnData(0)),
            BadSystemCallCode::SYS_USER_DISPATCH,
        ]);
        assert_eq!(checked, 29);
    }

    /// Every code from the queue codes to the kernel's own, on every signal, with zero fields and
    /// with every field byte set: each comes back from the interface's record as it was.
    #[test]
    fn every_record_comes_back_as_its_bytes() {
        let mut structured = 0;
        for number in Signal::SIGHUP as u32..=Signal::SIGRTMAX as u32 {
            for code in QueueCode::SI_ASYNCNL as i32..=SI_KERNEL as i32 {
                for fill in [0, u8::MAX, 1] {
                    let mut bytes = [fill; SI_MAX_SIZE];
                    put(&mut bytes, SIGNAL, &number.to_le_bytes());
                    put(&mut bytes, ERROR_NUMBER, &0i32.to_le_bytes());
                    put(&mut bytes, CODE, &code.to_le_bytes());
                    let information = decode(&bytes);
                    if !matches!(information, SignalInformation::SuppliedByProcess { .. }) {
                        structured += 1;
                    }
                    assert_eq!(encode(&information), bytes, "{information:?}");
                }
            }
        }
        assert!(structured > 0);
    }

    #[test]
    fn the_kernels_own_records_take_their_layouts() {
        let kill =
            crate::signal::sender_info(Signal::SIGTERM as u32, SendCode::SI_USER as i32, 7, 99);
        assert_eq!(
            decode(&kill),
            SignalInformation::SentByProcess {
                signal: Signal::SIGTERM,
                code: SendCode::SI_USER,
                sender: ProcessIdentity {
                    process: TaskId(7),
                    real_user: UserId(99)
                },
            }
        );
        let fault = crate::signal::fault_info(
            Signal::SIGSEGV as u32,
            number(&SegmentationFaultCode::SEGV_MAPERR),
            wcontainers_abi::PAGE_SIZE,
        );
        assert_eq!(
            decode(&fault),
            SignalInformation::SegmentationFault {
                code: SegmentationFaultCode::SEGV_MAPERR,
                address: wcontainers_abi::PAGE_SIZE
            }
        );
        assert_eq!(
            decode(&crate::signal::kernel_info(Signal::SIGKILL as u32)),
            SignalInformation::SentByKernel {
                signal: Signal::SIGKILL
            }
        );
    }
}
