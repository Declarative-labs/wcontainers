//! The syndrome of an exception from EL0, decoded into the architecture's exception and encoded back.

use aarch64_cpu::registers::ESR_EL1;
use aarch64_cpu::registers::ESR_EL1::EC::Value as Class;
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Exception, FaultStatus, SmeTrapCode,
};

/// [WnR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=WnR%2C%20bit%20%5B6%5D): a data abort's access was a write.
const WRITE_NOT_READ: u64 = 1 << 6;

/// The bits of [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D), the same as IFSC's.
const FAULT_STATUS_BITS: u64 = 0b11_1111;
/// The [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D) of a translation fault at level 3.
const TRANSLATION_FAULT_LEVEL_3: u64 = 0b00_0111;
/// The [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D) of a permission fault at level 3.
const PERMISSION_FAULT_LEVEL_3: u64 = 0b00_1111;

/// The bits of [SMTC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D).
const SME_TRAP_CODE_BITS: u64 = 0b111;
/// The [SMTC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D) of SME turned off.
const SME_DISABLED: u64 = 0b000;
/// The [SMTC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=SMTC%2C%20bits%20%5B2%3A0%5D) of an instruction legal only in streaming mode.
const SME_NOT_IN_STREAMING_MODE: u64 = 0b010;

/// The exception that ESR_EL1 and FAR_EL1 name, or none for the software step that follows an instruction that
/// completed.
///
/// Panics where the syndrome holds a bit the exception leaves out, so nothing two behaviors could differ in is lost.
pub(crate) fn to_exception(syndrome: u64, fault_address: u64) -> Option<Exception> {
    let specific = ESR_EL1::ISS.read(syndrome);
    let class = ESR_EL1::EC.read_as_enum(syndrome);
    let exception = match class {
        Some(Class::SoftwareStepLowerEL) => return None,
        Some(Class::Unknown) => Exception::UndefinedInstruction,
        Some(Class::PCAlignmentFault) => Exception::ProgramCounterAlignmentFault {
            address: fault_address,
        },
        Some(Class::SPAlignmentFault) => Exception::StackPointerAlignmentFault,
        Some(Class::InstrAbortLowerEL) => Exception::InstructionAbort {
            status: to_fault_status(specific, syndrome),
            address: fault_address,
        },
        Some(Class::DataAbortLowerEL) => Exception::DataAbort {
            status: to_fault_status(specific, syndrome),
            access: if specific & WRITE_NOT_READ == 0 {
                AccessDirection::Read
            } else {
                AccessDirection::Write
            },
            address: fault_address,
        },
        Some(Class::Brk64) => Exception::Breakpoint {
            comment: specific as u16,
        },
        Some(Class::TrappedSME) => Exception::SmeTrap(to_sme_trap_code(specific, syndrome)),
        // Every other class is one the ten instructions never raise.
        _ => panic!(
            "the chip raised an exception the architecture does not name: syndrome {syndrome:#x}"
        ),
    };
    assert_eq!(
        to_syndrome(exception),
        syndrome,
        "the syndrome holds bits that {exception:?} leaves out"
    );
    Some(exception)
}

/// The ESR_EL1 an exception from EL0 gives.
pub(crate) fn to_syndrome(exception: Exception) -> u64 {
    let (class, specific) = match exception {
        Exception::UndefinedInstruction => (Class::Unknown, 0),
        Exception::ProgramCounterAlignmentFault { .. } => (Class::PCAlignmentFault, 0),
        Exception::StackPointerAlignmentFault => (Class::SPAlignmentFault, 0),
        Exception::InstructionAbort { status, .. } => {
            (Class::InstrAbortLowerEL, to_fault_status_bits(status))
        }
        Exception::DataAbort { status, access, .. } => {
            let write_not_read = match access {
                AccessDirection::Read => 0,
                AccessDirection::Write => WRITE_NOT_READ,
            };
            (
                Class::DataAbortLowerEL,
                to_fault_status_bits(status) | write_not_read,
            )
        }
        Exception::Breakpoint { comment } => (Class::Brk64, u64::from(comment)),
        Exception::SmeTrap(code) => (Class::TrappedSME, to_sme_trap_code_bits(code)),
    };
    ESR_EL1::EC.val(class as u64).value | ESR_EL1::IL::SET.value | ESR_EL1::ISS.val(specific).value
}

fn to_fault_status(specific: u64, syndrome: u64) -> FaultStatus {
    match specific & FAULT_STATUS_BITS {
        TRANSLATION_FAULT_LEVEL_3 => FaultStatus::TranslationFaultLevel3,
        PERMISSION_FAULT_LEVEL_3 => FaultStatus::PermissionFaultLevel3,
        _ => {
            panic!("the syndrome {syndrome:#x} holds a fault status the architecture does not name")
        }
    }
}

fn to_fault_status_bits(status: FaultStatus) -> u64 {
    match status {
        FaultStatus::TranslationFaultLevel3 => TRANSLATION_FAULT_LEVEL_3,
        FaultStatus::PermissionFaultLevel3 => PERMISSION_FAULT_LEVEL_3,
    }
}

fn to_sme_trap_code(specific: u64, syndrome: u64) -> SmeTrapCode {
    match specific & SME_TRAP_CODE_BITS {
        SME_DISABLED => SmeTrapCode::Disabled,
        SME_NOT_IN_STREAMING_MODE => SmeTrapCode::NotInStreamingMode,
        _ => panic!(
            "the syndrome {syndrome:#x} holds an SME trap code the architecture does not name"
        ),
    }
}

fn to_sme_trap_code_bits(code: SmeTrapCode) -> u64 {
    match code {
        SmeTrapCode::Disabled => SME_DISABLED,
        SmeTrapCode::NotInStreamingMode => SME_NOT_IN_STREAMING_MODE,
    }
}
