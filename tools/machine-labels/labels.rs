//! The labels of a machine: what surrounds its cores, which no core's identity holds.

use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

/// What the system says surrounds the cores; a label is none where it does not say.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MachineLabels {
    /// The operating system's name and version.
    pub operating_system: Option<String>,
    /// The operating system's build.
    pub operating_system_build: Option<String>,
    /// The kernel's release and version.
    pub kernel: Option<String>,
    /// The hypervisor the samples are measured through.
    pub hypervisor: Option<String>,
    /// One this system itself runs under; none where it shows no sign of one.
    pub underlying_hypervisor: Option<String>,
    pub firmware: Option<String>,
    /// The machine's or instance's maker and model.
    pub machine_model: Option<String>,
    /// The day, as YYYY-MM-DD in UTC.
    pub date: String,
}

/// Returns the labels of this machine, read from its own files, registry or sysctls, never over a network.
pub fn read_machine_labels() -> MachineLabels {
    #[cfg(target_os = "linux")]
    let labels = linux::read_labels();
    #[cfg(target_os = "macos")]
    let labels = macos::read_labels();
    #[cfg(target_os = "windows")]
    let labels = windows::read_labels();
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let labels = MachineLabels::default();
    MachineLabels {
        date: format_today(),
        ..labels
    }
}

const LABEL_SEPARATOR: &str = " ";

/// Returns the labels present, joined by spaces, or none when none is.
fn join_present(labels: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let present: Vec<String> = labels
        .into_iter()
        .flatten()
        .map(|label| label.trim().to_owned())
        .filter(|label| !label.is_empty())
        .collect();
    (!present.is_empty()).then(|| present.join(LABEL_SEPARATOR))
}

/// [Leaf 1](https://www.felixcloutier.com/x86/cpuid), whose ECX bit 31 a hypervisor sets for its guests.
#[cfg(target_arch = "x86_64")]
const VERSION_LEAF: u32 = 1;
/// The [subleaf](https://www.felixcloutier.com/x86/cpuid) in ECX that every leaf read here is asked with.
#[cfg(target_arch = "x86_64")]
const SUBLEAF: u32 = 0;
/// [Bit 31 of leaf 1's ECX](https://www.felixcloutier.com/x86/cpuid), which a hypervisor sets for its guests.
#[cfg(target_arch = "x86_64")]
const HYPERVISOR_PRESENT_BIT: u32 = 31;
/// The first leaf of the range a hypervisor answers, giving its vendor in EBX, ECX and EDX ([Hyper-V's TLFS](https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/tlfs/feature-discovery)).
#[cfg(target_arch = "x86_64")]
const HYPERVISOR_VENDOR_LEAF: u32 = 0x4000_0000;

/// Returns the vendor a hypervisor gives through CPUID, where the processor says one is present.
#[cfg(target_arch = "x86_64")]
fn read_hypervisor_vendor_by_cpuid() -> Option<String> {
    use core::arch::x86_64::__cpuid_count;
    if (__cpuid_count(VERSION_LEAF, SUBLEAF).ecx >> HYPERVISOR_PRESENT_BIT) & 1 == 0 {
        return None;
    }
    let vendor = __cpuid_count(HYPERVISOR_VENDOR_LEAF, SUBLEAF);
    let bytes: Vec<u8> = [vendor.ebx, vendor.ecx, vendor.edx]
        .iter()
        .flat_map(|register| register.to_le_bytes())
        .filter(|byte| *byte != 0)
        .collect();
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The seconds of a day of [Unix time](https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap04.html#tag_04_19), which has no leap seconds.
const SECONDS_PER_DAY: u64 = 86_400;
/// The year [Unix time](https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap04.html#tag_04_19) counts from.
const EPOCH_YEAR: u64 = 1970;
/// The days of each month of a year without a leap day, January first ([Gregorian calendar](https://en.wikipedia.org/wiki/Gregorian_calendar)).
const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
/// The [Gregorian](https://en.wikipedia.org/wiki/Gregorian_calendar) leap years: every 4th, but of every 100th only every 400th.
const LEAP_YEAR_DIVISORS: (u64, u64, u64) = (4, 100, 400);
/// The month a leap year's extra day falls in, counted from 0 ([Gregorian calendar](https://en.wikipedia.org/wiki/Gregorian_calendar)).
const FEBRUARY: usize = 1;
/// What separates the year, month and day.
const DATE_SEPARATOR: char = '-';
/// The digits of the year and of the month and day in an [ISO 8601](https://www.iso.org/iso-8601-date-and-time-format.html) date.
const DATE_FORMAT_WIDTHS: (usize, usize) = (4, 2);

fn is_leap_year(year: u64) -> bool {
    let (every, except, unless) = LEAP_YEAR_DIVISORS;
    year.is_multiple_of(every) && (!year.is_multiple_of(except) || year.is_multiple_of(unless))
}

/// Returns today's date in UTC as YYYY-MM-DD.
fn format_today() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    let mut days = seconds / SECONDS_PER_DAY;
    let mut year = EPOCH_YEAR;
    let year_days = |year| MONTH_DAYS.iter().sum::<u64>() + u64::from(is_leap_year(year));
    while days >= year_days(year) {
        days -= year_days(year);
        year += 1;
    }
    let mut month = MONTH_DAYS.len();
    for (index, days_of_month) in MONTH_DAYS.iter().enumerate() {
        let length = days_of_month + u64::from(index == FEBRUARY && is_leap_year(year));
        if days < length {
            month = index;
            break;
        }
        days -= length;
    }
    let (year_width, width) = DATE_FORMAT_WIDTHS;
    let (month, day) = (month + 1, days + 1);
    format!("{year:0year_width$}{DATE_SEPARATOR}{month:0width$}{DATE_SEPARATOR}{day:0width$}")
}
