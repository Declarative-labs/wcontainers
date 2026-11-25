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
/// The [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D) of a translation fault at level 0.
const TRANSLATION_FAULT_LEVEL_0: u64 = 0b00_0100;
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
/// A syndrome that no other exception names, or that holds a bit the named one leaves out, is kept whole as
/// [`Exception::Undecoded`], so nothing two behaviors could differ in is lost.
pub(crate) fn to_exception(syndrome: u64, fault_address: u64) -> Option<Exception> {
    let class = ESR_EL1::EC.read_as_enum(syndrome);
    if matches!(class, Some(Class::SoftwareStepLowerEL)) {
        return None;
    }
    let undecoded = Exception::Undecoded {
        syndrome,
        fault_address,
    };
    Some(
        decode_exception(class, ESR_EL1::ISS.read(syndrome), fault_address)
            .filter(|exception| to_syndrome(*exception) == syndrome)
            .unwrap_or(undecoded),
    )
}

/// Returns the exception of a class other than the software step, from the syndrome's class-specific bits, or `None`
/// where the architecture's exception does not name them.
fn decode_exception(class: Option<Class>, specific: u64, fault_address: u64) -> Option<Exception> {
    let exception = match class? {
        Class::Unknown => Exception::UndefinedInstruction,
        Class::PCAlignmentFault => Exception::ProgramCounterAlignmentFault {
            address: fault_address,
        },
        Class::SPAlignmentFault => Exception::StackPointerAlignmentFault,
        Class::InstrAbortLowerEL => Exception::InstructionAbort {
            status: to_fault_status(specific)?,
            address: fault_address,
        },
        Class::DataAbortLowerEL => Exception::DataAbort {
            status: to_fault_status(specific)?,
            access: if specific & WRITE_NOT_READ == 0 {
                AccessDirection::Read
            } else {
                AccessDirection::Write
            },
            address: fault_address,
        },
        Class::Brk64 => Exception::Breakpoint {
            comment: u16::try_from(specific).ok()?,
        },
        Class::TrappedSME => Exception::SmeTrap(to_sme_trap_code(specific)?),
        // Every other class is one the ten instructions never raise.
        _ => return None,
    };
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
        Exception::Undecoded { syndrome, .. } => return syndrome,
    };
    ESR_EL1::EC.val(class as u64).value | ESR_EL1::IL::SET.value | ESR_EL1::ISS.val(specific).value
}

fn to_fault_status(specific: u64) -> Option<FaultStatus> {
    match specific & FAULT_STATUS_BITS {
        TRANSLATION_FAULT_LEVEL_0 => Some(FaultStatus::TranslationFaultLevel0),
        TRANSLATION_FAULT_LEVEL_3 => Some(FaultStatus::TranslationFaultLevel3),
        PERMISSION_FAULT_LEVEL_3 => Some(FaultStatus::PermissionFaultLevel3),
        _ => None,
    }
}

fn to_fault_status_bits(status: FaultStatus) -> u64 {
    match status {
        FaultStatus::TranslationFaultLevel0 => TRANSLATION_FAULT_LEVEL_0,
        FaultStatus::TranslationFaultLevel3 => TRANSLATION_FAULT_LEVEL_3,
        FaultStatus::PermissionFaultLevel3 => PERMISSION_FAULT_LEVEL_3,
    }
}

fn to_sme_trap_code(specific: u64) -> Option<SmeTrapCode> {
    match specific & SME_TRAP_CODE_BITS {
        SME_DISABLED => Some(SmeTrapCode::Disabled),
        SME_NOT_IN_STREAMING_MODE => Some(SmeTrapCode::NotInStreamingMode),
        _ => None,
    }
}

fn to_sme_trap_code_bits(code: SmeTrapCode) -> u64 {
    match code {
        SmeTrapCode::Disabled => SME_DISABLED,
        SmeTrapCode::NotInStreamingMode => SME_NOT_IN_STREAMING_MODE,
    }
}
