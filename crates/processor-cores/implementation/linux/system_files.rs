//! Reading the files Linux keeps about its CPUs: the list of those online, a CPU's own directory and /proc/cpuinfo.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ListCoresError;

/// The [CPUs that are online](https://github.com/torvalds/linux/blob/master/Documentation/ABI/testing/sysfs-devices-system-cpu), as a CPU list.
const ONLINE_CPUS: &str = "/sys/devices/system/cpu/online";
/// The folder holding one `cpuN` folder for each CPU.
const CPU_FOLDERS: &str = "/sys/devices/system/cpu";
/// One section of fields for each online CPU.
const CPU_INFORMATION: &str = "/proc/cpuinfo";
/// The field of a /proc/cpuinfo section that gives the CPU's number.
const PROCESSOR_FIELD: &str = "processor";
/// What separates a field's name from its value in /proc/cpuinfo.
const CPU_INFORMATION_SEPARATOR: char = ':';
/// What separates the sections of /proc/cpuinfo.
const SECTION_SEPARATOR: &str = "\n\n";
/// What separates ranges in a [CPU list](https://docs.kernel.org/admin-guide/cputopology.html).
const RANGE_SEPARATOR: char = ',';
/// What separates the first and last CPU of a range in a CPU list.
const RANGE_DASH: char = '-';
/// What starts a number the kernel writes in hexadecimal.
const HEXADECIMAL_PREFIX: &str = "0x";
/// The base of a [hexadecimal](https://en.wikipedia.org/wiki/Hexadecimal) number.
const HEXADECIMAL_RADIX: u32 = 16;
/// The base of a [decimal](https://en.wikipedia.org/wiki/Decimal) number.
const DECIMAL_RADIX: u32 = 10;

/// The fields of each CPU's section of /proc/cpuinfo, by the CPU's number.
pub(crate) type CpuInformation = BTreeMap<usize, BTreeMap<String, String>>;

/// Returns the file's text.
pub(crate) fn read_file(path: &Path) -> Result<String, ListCoresError> {
    fs::read_to_string(path).map_err(|error| ListCoresError::FileUnreadable {
        path: path.to_path_buf(),
        error,
    })
}

/// Returns the file's text, or none when the file does not exist.
#[cfg(target_arch = "x86_64")]
pub(crate) fn read_file_if_present(path: &Path) -> Result<Option<String>, ListCoresError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ListCoresError::FileUnreadable {
            path: path.to_path_buf(),
            error,
        }),
    }
}

/// Returns the path of a file in the CPU's own folder.
pub(crate) fn cpu_file(core: usize, name: &str) -> PathBuf {
    Path::new(CPU_FOLDERS).join(format!("cpu{core}")).join(name)
}

fn malformed_file_error(path: &Path, contents: &str) -> ListCoresError {
    ListCoresError::FileMalformed {
        path: path.to_path_buf(),
        contents: contents.to_owned(),
    }
}

/// Returns the number, read as hexadecimal after a 0x and as decimal without one.
pub(crate) fn parse_number(text: &str) -> Option<u64> {
    let text = text.trim();
    match text.strip_prefix(HEXADECIMAL_PREFIX) {
        Some(digits) => u64::from_str_radix(digits, HEXADECIMAL_RADIX).ok(),
        None => u64::from_str_radix(text, DECIMAL_RADIX).ok(),
    }
}

/// Returns the number the file holds.
pub(crate) fn read_number(path: &Path) -> Result<u64, ListCoresError> {
    let text = read_file(path)?;
    parse_number(&text).ok_or_else(|| malformed_file_error(path, &text))
}

/// Returns each CPU of a CPU list such as `0-3,8,10-11`, in order.
pub(crate) fn parse_cpu_list(path: &Path, text: &str) -> Result<Vec<usize>, ListCoresError> {
    let mut cores = Vec::new();
    for range in text
        .trim()
        .split(RANGE_SEPARATOR)
        .filter(|range| !range.is_empty())
    {
        let (first, last) = range.split_once(RANGE_DASH).unwrap_or((range, range));
        let (Ok(first), Ok(last)) = (first.parse::<usize>(), last.parse::<usize>()) else {
            return Err(malformed_file_error(path, text));
        };
        cores.extend(first..=last);
    }
    Ok(cores)
}

/// Returns the CPUs Linux lists as online, in order.
pub(crate) fn list_online_cpus() -> Result<Vec<usize>, ListCoresError> {
    let path = Path::new(ONLINE_CPUS);
    parse_cpu_list(path, &read_file(path)?)
}

/// Returns the fields of every section of /proc/cpuinfo.
pub(crate) fn read_cpu_information() -> Result<CpuInformation, ListCoresError> {
    let path = Path::new(CPU_INFORMATION);
    let text = read_file(path)?;
    let mut sections = CpuInformation::new();
    for section in text.split(SECTION_SEPARATOR) {
        let fields: BTreeMap<String, String> = section
            .lines()
            .filter_map(|line| line.split_once(CPU_INFORMATION_SEPARATOR))
            .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
            .collect();
        if let Some(processor) = fields.get(PROCESSOR_FIELD) {
            let Ok(processor) = processor.parse::<usize>() else {
                return Err(malformed_file_error(path, section));
            };
            sections.insert(processor, fields);
        }
    }
    Ok(sections)
}

/// Checks that the CPU's section of /proc/cpuinfo gives the field the value the core reported.
pub(crate) fn check_field(
    information: &CpuInformation,
    core: usize,
    field: &str,
    from_core: &str,
    agrees: impl Fn(&str) -> bool,
) -> Result<(), ListCoresError> {
    let section = information
        .get(&core)
        .ok_or(ListCoresError::CpuInformationMissing { core })?;
    let from_cpu_information = section.get(field).map(String::as_str).unwrap_or_default();
    if agrees(from_cpu_information) {
        return Ok(());
    }
    Err(ListCoresError::IdentityDisagrees {
        core,
        field: field.to_owned(),
        from_core: from_core.to_owned(),
        from_cpu_information: from_cpu_information.to_owned(),
    })
}

/// Checks that the CPU's section of /proc/cpuinfo gives the numeric field the value the core reported.
pub(crate) fn check_number(
    information: &CpuInformation,
    core: usize,
    field: &str,
    from_core: u64,
) -> Result<(), ListCoresError> {
    check_field(
        information,
        core,
        field,
        &format!("{from_core:#x}"),
        |text| parse_number(text) == Some(from_core),
    )
}
