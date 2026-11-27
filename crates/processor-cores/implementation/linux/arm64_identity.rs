//! The identity of each ARM64 CPU, from the registers Linux read on that CPU when it came online, checked against /proc/cpuinfo.

use wcontainers_processor_architecture_arm64::CoreIdentity;

use crate::system_files::{
    check_number, cpu_file, list_online_cpus, read_cpu_information, read_number,
};
use crate::{LinuxProcessorCores, ListCoresError};

/// The file holding [the CPU's MIDR_EL1](https://github.com/torvalds/linux/blob/master/Documentation/ABI/testing/sysfs-devices-system-cpu), read on that CPU.
const MAIN_ID_REGISTER_FILE: &str = "regs/identification/midr_el1";
/// The file holding the CPU's REVIDR_EL1, read on that CPU.
const REVISION_ID_REGISTER_FILE: &str = "regs/identification/revidr_el1";

/// A field of MIDR_EL1 that /proc/cpuinfo also gives, under its own name.
struct MainIdField {
    cpu_information_name: &'static str,
    lowest_bit: u32,
    width: u32,
}

/// The implementer code, bits 31 to 24 of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const IMPLEMENTER_LOWEST_BIT: u32 = 24;
/// The width of the implementer field of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const IMPLEMENTER_WIDTH: u32 = 8;
/// The variant, bits 23 to 20 of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const VARIANT_LOWEST_BIT: u32 = 20;
/// The width of the variant field of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const VARIANT_WIDTH: u32 = 4;
/// The part number, bits 15 to 4 of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const PART_NUMBER_LOWEST_BIT: u32 = 4;
/// The width of the part number field of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const PART_NUMBER_WIDTH: u32 = 12;
/// The revision, bits 3 to 0 of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const REVISION_LOWEST_BIT: u32 = 0;
/// The width of the revision field of [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register).
const REVISION_WIDTH: u32 = 4;

/// The fields of MIDR_EL1 that [Linux's /proc/cpuinfo](https://github.com/torvalds/linux/blob/master/arch/arm64/kernel/cpuinfo.c) prints for each CPU.
const CPU_INFORMATION_FIELDS: [MainIdField; 4] = [
    MainIdField {
        cpu_information_name: "CPU implementer",
        lowest_bit: IMPLEMENTER_LOWEST_BIT,
        width: IMPLEMENTER_WIDTH,
    },
    MainIdField {
        cpu_information_name: "CPU variant",
        lowest_bit: VARIANT_LOWEST_BIT,
        width: VARIANT_WIDTH,
    },
    MainIdField {
        cpu_information_name: "CPU part",
        lowest_bit: PART_NUMBER_LOWEST_BIT,
        width: PART_NUMBER_WIDTH,
    },
    MainIdField {
        cpu_information_name: "CPU revision",
        lowest_bit: REVISION_LOWEST_BIT,
        width: REVISION_WIDTH,
    },
];

/// Returns each online CPU with its MIDR_EL1 and REVIDR_EL1, once every MIDR_EL1 field agrees with /proc/cpuinfo.
pub(crate) fn list_identified_cores(
    _cores: &LinuxProcessorCores,
) -> Result<Vec<(usize, CoreIdentity)>, ListCoresError> {
    let information = read_cpu_information()?;
    list_online_cpus()?
        .into_iter()
        .map(|core| {
            let identity = CoreIdentity {
                main_id_register: read_number(&cpu_file(core, MAIN_ID_REGISTER_FILE))?,
                revision_id_register: read_number(&cpu_file(core, REVISION_ID_REGISTER_FILE))?,
            };
            for field in &CPU_INFORMATION_FIELDS {
                let value =
                    (identity.main_id_register >> field.lowest_bit) & ((1 << field.width) - 1);
                check_number(&information, core, field.cpu_information_name, value)?;
            }
            Ok((core, identity))
        })
        .collect()
}
