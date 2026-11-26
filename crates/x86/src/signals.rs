//! The signal each way an x86-64 thread stops raises, as Linux reports it (plan section 6, X10).
//!
//! The kernel's x86-64 personality calls [`signal_information`] where a thread stops, and [`resume_flag_in_frame`]
//! where it builds the signal frame's rflags. The answers are the reference's (plan section 2).

use wcontainers_guest_cpu::{MemoryFault, TrapKind};
use wcontainers_kernel::signal::Signal;
use wcontainers_kernel::signal_information::{
    ArithmeticErrorCode, BusErrorCode, IllegalInstructionCode, SegmentationFaultCode,
    SignalInformation, TrapCode,
};

use crate::{
    MXCSR_DENORMAL, MXCSR_DIVIDE_BY_ZERO, MXCSR_INVALID, MXCSR_OVERFLOW, MXCSR_PRECISION,
    MXCSR_UNDERFLOW,
};

/// A breakpoint trap's detail for `int1`; `int3` and `int 3` give 0 (the x86 meanings' `Trap::Int1`).
pub const INT1_DETAIL: i64 = 1;

/// Why a thread stopped, as the CPU side gives it to the kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// A trap of an instruction: its kind, its detail, and the pc the trap left.
    ///
    /// The pc is after the instruction for a breakpoint or `int 4`, on it otherwise.
    Trap {
        kind: TrapKind,
        detail: i64,
        pc: u64,
    },
    /// A memory fault the CPU side classified, with the address that faulted.
    ///
    /// `mapped` when the kernel has a mapping there that refuses the access, which Linux reports apart.
    Fault {
        fault: MemoryFault,
        address: u64,
        mapped: bool,
    },
    /// One instruction ran with the trap flag set; pc is after it.
    SingleStep { pc: u64 },
}

/// Returns the signal and its information the stop raises.
pub fn signal_information(stop: Stop) -> SignalInformation {
    match stop {
        Stop::Trap { kind, detail, pc } => match kind {
            TrapKind::Undefined | TrapKind::Unsupported => SignalInformation::IllegalInstruction {
                code: IllegalInstructionCode::ILL_ILLOPN,
                address: pc,
            },
            TrapKind::Breakpoint if detail == INT1_DETAIL => SignalInformation::Trap {
                code: TrapCode::TRAP_BRKPT,
                address: pc,
            },
            TrapKind::Breakpoint => SignalInformation::SentByKernel {
                signal: Signal::SIGTRAP,
            },
            // AC is never held (plan 5.1), so no access raises #AC; were one to, Linux sends this.
            TrapKind::Alignment => SignalInformation::BusError {
                code: BusErrorCode::BUS_ADRALN,
                address: detail as u64,
            },
            TrapKind::DivideError => SignalInformation::ArithmeticError {
                code: ArithmeticErrorCode::FPE_INTDIV,
                address: pc,
            },
            TrapKind::GeneralProtection | TrapKind::Overflow => SignalInformation::SentByKernel {
                signal: Signal::SIGSEGV,
            },
            TrapKind::FloatException => SignalInformation::ArithmeticError {
                code: float_exception_code(detail as u32),
                address: pc,
            },
        },
        Stop::Fault {
            fault: MemoryFault::Page,
            address,
            mapped,
        } => SignalInformation::SegmentationFault {
            code: if mapped {
                SegmentationFaultCode::SEGV_ACCERR
            } else {
                SegmentationFaultCode::SEGV_MAPERR
            },
            address,
        },
        Stop::Fault {
            fault: MemoryFault::GeneralProtection,
            ..
        } => SignalInformation::SentByKernel {
            signal: Signal::SIGSEGV,
        },
        Stop::Fault {
            fault: MemoryFault::Stack,
            ..
        } => SignalInformation::SentByKernel {
            signal: Signal::SIGBUS,
        },
        Stop::SingleStep { pc } => SignalInformation::Trap {
            code: TrapCode::TRAP_TRACE,
            address: pc,
        },
    }
}

/// Returns the `si_code` of a float exception from its raised and unmasked flags.
///
/// The flags are MXCSR's or the x87 status word's, at the same bits; the first raised of invalid, divide by zero,
/// overflow, underflow (with denormal) and precision decides (Intel SDM Vol. 1, 11.5.2). With none, `FPE_FLTUNK`.
pub fn float_exception_code(flags: u32) -> ArithmeticErrorCode {
    let codes = [
        (MXCSR_INVALID, ArithmeticErrorCode::FPE_FLTINV),
        (MXCSR_DIVIDE_BY_ZERO, ArithmeticErrorCode::FPE_FLTDIV),
        (MXCSR_OVERFLOW, ArithmeticErrorCode::FPE_FLTOVF),
        (
            MXCSR_UNDERFLOW | MXCSR_DENORMAL,
            ArithmeticErrorCode::FPE_FLTUND,
        ),
        (MXCSR_PRECISION, ArithmeticErrorCode::FPE_FLTRES),
    ];
    codes
        .iter()
        .find(|&&(bits, _)| flags & bits != 0)
        .map_or(ArithmeticErrorCode::FPE_FLTUNK, |&(_, code)| code)
}

/// Tells whether the signal frame of the stop holds RF (rflags bit 16), as a fault's frame does.
///
/// A trap's (single step, `int3`, `int1`, `int 4`) does not (plan section 2, run 10).
pub fn resume_flag_in_frame(stop: Stop) -> bool {
    match stop {
        Stop::Trap { kind, .. } => !matches!(kind, TrapKind::Breakpoint | TrapKind::Overflow),
        Stop::Fault { .. } => true,
        Stop::SingleStep { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PC: u64 = 0x40_1000;
    const ADDRESS: u64 = 0x7000_0000;

    fn trap(kind: TrapKind, detail: i64) -> Stop {
        Stop::Trap {
            kind,
            detail,
            pc: PC,
        }
    }

    /// Checks every stop's signal against what the reference reports for it.
    #[test]
    fn every_stop_raises_the_reference_signal() {
        use SignalInformation as S;
        let kernel = |signal| S::SentByKernel { signal };
        for (stop, expected) in [
            (
                trap(TrapKind::Undefined, 0),
                S::IllegalInstruction {
                    code: IllegalInstructionCode::ILL_ILLOPN,
                    address: PC,
                },
            ),
            (
                trap(TrapKind::Unsupported, 0),
                S::IllegalInstruction {
                    code: IllegalInstructionCode::ILL_ILLOPN,
                    address: PC,
                },
            ),
            (trap(TrapKind::Breakpoint, 0), kernel(Signal::SIGTRAP)),
            (
                trap(TrapKind::Breakpoint, INT1_DETAIL),
                S::Trap {
                    code: TrapCode::TRAP_BRKPT,
                    address: PC,
                },
            ),
            (
                trap(TrapKind::DivideError, 0),
                S::ArithmeticError {
                    code: ArithmeticErrorCode::FPE_INTDIV,
                    address: PC,
                },
            ),
            (
                trap(TrapKind::GeneralProtection, 0),
                kernel(Signal::SIGSEGV),
            ),
            (trap(TrapKind::Overflow, 0), kernel(Signal::SIGSEGV)),
            (
                trap(
                    TrapKind::FloatException,
                    i64::from(MXCSR_DIVIDE_BY_ZERO | MXCSR_PRECISION),
                ),
                S::ArithmeticError {
                    code: ArithmeticErrorCode::FPE_FLTDIV,
                    address: PC,
                },
            ),
            (
                Stop::Fault {
                    fault: MemoryFault::Page,
                    address: ADDRESS,
                    mapped: false,
                },
                S::SegmentationFault {
                    code: SegmentationFaultCode::SEGV_MAPERR,
                    address: ADDRESS,
                },
            ),
            (
                Stop::Fault {
                    fault: MemoryFault::Page,
                    address: ADDRESS,
                    mapped: true,
                },
                S::SegmentationFault {
                    code: SegmentationFaultCode::SEGV_ACCERR,
                    address: ADDRESS,
                },
            ),
            (
                Stop::Fault {
                    fault: MemoryFault::GeneralProtection,
                    address: ADDRESS,
                    mapped: false,
                },
                kernel(Signal::SIGSEGV),
            ),
            (
                Stop::Fault {
                    fault: MemoryFault::Stack,
                    address: ADDRESS,
                    mapped: false,
                },
                kernel(Signal::SIGBUS),
            ),
            (
                Stop::SingleStep { pc: PC },
                S::Trap {
                    code: TrapCode::TRAP_TRACE,
                    address: PC,
                },
            ),
        ] {
            assert_eq!(signal_information(stop), expected, "{stop:?}");
        }
    }

    /// Checks the order of the float exceptions' codes and RF in the frame of each kind of stop.
    #[test]
    fn float_codes_and_the_resume_flag() {
        assert_eq!(
            float_exception_code(MXCSR_PRECISION | MXCSR_INVALID),
            ArithmeticErrorCode::FPE_FLTINV
        );
        assert_eq!(
            float_exception_code(MXCSR_OVERFLOW | MXCSR_PRECISION),
            ArithmeticErrorCode::FPE_FLTOVF
        );
        assert_eq!(
            float_exception_code(MXCSR_DENORMAL),
            ArithmeticErrorCode::FPE_FLTUND
        );
        assert_eq!(
            float_exception_code(MXCSR_PRECISION),
            ArithmeticErrorCode::FPE_FLTRES
        );
        assert_eq!(float_exception_code(0), ArithmeticErrorCode::FPE_FLTUNK);
        assert!(resume_flag_in_frame(trap(TrapKind::GeneralProtection, 0)));
        assert!(resume_flag_in_frame(trap(TrapKind::DivideError, 0)));
        assert!(!resume_flag_in_frame(trap(TrapKind::Breakpoint, 0)));
        assert!(!resume_flag_in_frame(trap(TrapKind::Overflow, 0)));
        assert!(!resume_flag_in_frame(Stop::SingleStep { pc: PC }));
        assert!(resume_flag_in_frame(Stop::Fault {
            fault: MemoryFault::Stack,
            address: 0,
            mapped: false
        }));
    }
}
