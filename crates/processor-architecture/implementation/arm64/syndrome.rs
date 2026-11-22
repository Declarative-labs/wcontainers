//! The parts of ESR_EL1 the ten instructions' exceptions use.

use aarch64_cpu::registers::ESR_EL1;

use crate::Exception;

/// The [WnR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=WnR%2C%20bit%20%5B6%5D) bit of a data abort's syndrome.
pub const WRITE_NOT_READ: u64 = 1 << 6;

pub const UNDEFINED_INSTRUCTION: Exception = Exception {
    syndrome: encode_syndrome(ESR_EL1::EC::Value::Unknown, 0),
    fault_address: None,
};

/// An abort's [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D) or IFSC; every page of an EL1 as Linux sets it is at level 3.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u64)]
pub enum FaultStatus {
    TranslationFaultLevel3 = 0b00_0111,
    PermissionFaultLevel3 = 0b00_1111,
}

/// The syndrome of an exception a 32-bit instruction raised.
pub const fn encode_syndrome(class: ESR_EL1::EC::Value, specific: u64) -> u64 {
    ESR_EL1::EC.val(class as u64).value | ESR_EL1::IL::SET.value | ESR_EL1::ISS.val(specific).value
}
