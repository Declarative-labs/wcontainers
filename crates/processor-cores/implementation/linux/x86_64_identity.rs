//! The identity of each x86-64 logical processor, read by CPUID while pinned there, checked against /proc/cpuinfo and the hybrid PMU lists.

use std::path::Path;

use wcontainers_processor_architecture_x86_64::CoreIdentity;
use wcontainers_processor_cores::ProcessorCores;

use crate::cpuid::read_identity_by_cpuid;
use crate::system_files::{
    check_field, check_number, cpu_file, list_online_cpus, parse_cpu_list, read_cpu_information,
    read_file_if_present, read_number,
};
use crate::{LinuxProcessorCores, ListCoresError};

/// The file holding [the microcode revision](https://github.com/torvalds/linux/blob/master/Documentation/ABI/testing/sysfs-devices-system-cpu) the kernel loaded on the CPU; virtual machines usually lack it.
const MICROCODE_FILE: &str = "microcode/version";
/// The fields of a [section of /proc/cpuinfo](https://github.com/torvalds/linux/blob/master/arch/x86/kernel/cpu/proc.c) that the identity is checked against.
const VENDOR_FIELD: &str = "vendor_id";
const FAMILY_FIELD: &str = "cpu family";
const MODEL_FIELD: &str = "model";
const STEPPING_FIELD: &str = "stepping";
const MICROCODE_FIELD: &str = "microcode";
/// The CPUs of the [hybrid PMU](https://github.com/torvalds/linux/blob/master/tools/perf/Documentation/intel-hybrid.txt) of the Core cores.
const CORE_CPUS: &str = "/sys/devices/cpu_core/cpus";
/// The CPUs of the [hybrid PMU](https://github.com/torvalds/linux/blob/master/tools/perf/Documentation/intel-hybrid.txt) of the Atom cores.
const ATOM_CPUS: &str = "/sys/devices/cpu_atom/cpus";
/// The core type of leaf 0x1A for an Atom core ([Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) volume 2A, CPUID).
const ATOM_CORE_TYPE: u8 = 0x20;
/// The core type of leaf 0x1A for a Core core ([Intel SDM](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) volume 2A, CPUID).
const CORE_CORE_TYPE: u8 = 0x40;

/// Checks that a hybrid CPU is in the PMU list of its core type, where Linux has that list.
fn check_hybrid_lists(core: usize, core_type: u8) -> Result<(), ListCoresError> {
    let list = match core_type {
        ATOM_CORE_TYPE => ATOM_CPUS,
        CORE_CORE_TYPE => CORE_CPUS,
        _ => return Ok(()),
    };
    let path = Path::new(list);
    let Some(text) = read_file_if_present(path)? else {
        return Ok(());
    };
    if parse_cpu_list(path, &text)?.contains(&core) {
        return Ok(());
    }
    Err(ListCoresError::IdentityDisagrees {
        core,
        field: list.to_owned(),
        from_core: format!("core type {core_type:#x}"),
        from_cpu_information: text.trim().to_owned(),
    })
}

/// Returns each online CPU with what CPUID says there and the microcode revision Linux reports, once /proc/cpuinfo agrees.
pub(crate) fn list_identified_cores(
    cores: &LinuxProcessorCores,
) -> Result<Vec<(usize, CoreIdentity)>, ListCoresError> {
    let information = read_cpu_information()?;
    list_online_cpus()?
        .into_iter()
        .map(|core| {
            let from_cpuid = cores
                .run_on_core(&core, read_identity_by_cpuid)
                .map_err(|error| ListCoresError::CoreNotHeld { core, error })?;
            let microcode_file = cpu_file(core, MICROCODE_FILE);
            let microcode_version = match read_file_if_present(&microcode_file)? {
                Some(text) => Some(u32::try_from(read_number(&microcode_file)?).map_err(|_| {
                    ListCoresError::FileMalformed {
                        path: microcode_file.clone(),
                        contents: text,
                    }
                })?),
                None => None,
            };
            let identity = CoreIdentity {
                microcode_version,
                ..from_cpuid
            };
            let vendor = String::from_utf8_lossy(&identity.vendor_identification);
            check_field(&information, core, VENDOR_FIELD, &vendor, |text| {
                text == vendor
            })?;
            check_number(&information, core, FAMILY_FIELD, identity.family.into())?;
            check_number(&information, core, MODEL_FIELD, identity.model.into())?;
            check_number(&information, core, STEPPING_FIELD, identity.stepping.into())?;
            if let Some(version) = identity.microcode_version {
                check_number(&information, core, MICROCODE_FIELD, version.into())?;
            }
            if let Some(core_type) = identity.hybrid_core_type {
                check_hybrid_lists(core, core_type)?;
            }
            Ok((core, identity))
        })
        .collect()
}
