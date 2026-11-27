//! The labels of a Mac, from its sysctls and, for the firmware, from system_profiler run on the Mac itself.

use std::process::Command;

use sysctl::{Ctl, CtlValue, Sysctl};

use crate::{MachineLabels, join_present};

/// The sysctls the labels are read from.
const PRODUCT_VERSION: &str = "kern.osproductversion";
const BUILD: &str = "kern.osversion";
const KERNEL_VERSION: &str = "kern.version";
const HYPERVISOR_SUPPORT: &str = "kern.hv_support";
const VIRTUAL_MACHINE_MONITOR_PRESENT: &str = "kern.hv_vmm_present";
const PRODUCT: &str = "hw.product";
const MODEL: &str = "hw.model";
/// The program, and the report of it, that name the firmware.
const SYSTEM_PROFILER: &str = "/usr/sbin/system_profiler";
const HARDWARE_REPORT: &str = "SPHardwareDataType";
/// The lines of that report that name the firmware.
const FIRMWARE_LINES: &[&str] = &["System Firmware Version", "OS Loader Version"];
/// What separates a line's name from its value in that report.
const REPORT_SEPARATOR: char = ':';
/// The names the labels start with.
const OPERATING_SYSTEM_LABEL: &str = "macOS";
const HYPERVISOR_LABEL: &str = "Hypervisor.framework of";
const VIRTUAL_MACHINE_MONITOR_LABEL: &str = "a virtual machine monitor, as said by";
/// The value a sysctl that says yes holds ([sysctl](https://developer.apple.com/documentation/kernel/1387446-sysctlbyname)).
const YES: i64 = 1;

fn read_sysctl(name: &str) -> Option<CtlValue> {
    Ctl::new(name).and_then(|control| control.value()).ok()
}

fn read_sysctl_text(name: &str) -> Option<String> {
    match read_sysctl(name)? {
        CtlValue::String(text) => Some(text),
        _ => None,
    }
}

fn read_sysctl_yes(name: &str) -> bool {
    match read_sysctl(name) {
        Some(CtlValue::Int(number) | CtlValue::S32(number)) => i64::from(number) == YES,
        Some(CtlValue::Uint(number) | CtlValue::U32(number)) => i64::from(number) == YES,
        _ => false,
    }
}

/// Returns the firmware lines system_profiler prints for the hardware.
fn read_firmware() -> Option<String> {
    let output = Command::new(SYSTEM_PROFILER)
        .arg(HARDWARE_REPORT)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    join_present(FIRMWARE_LINES.iter().map(|wanted| {
        text.lines()
            .filter_map(|line| line.trim().split_once(REPORT_SEPARATOR))
            .find(|(name, _)| name.trim() == *wanted)
            .map(|(name, value)| format!("{}{REPORT_SEPARATOR} {}", name.trim(), value.trim()))
    }))
}

/// Returns the hypervisor this macOS runs under: its CPUID vendor on an Intel Mac, and on Apple silicon only that one is present.
fn read_underlying_hypervisor() -> Option<String> {
    if !read_sysctl_yes(VIRTUAL_MACHINE_MONITOR_PRESENT) {
        return None;
    }
    #[cfg(target_arch = "x86_64")]
    if let Some(vendor) = crate::read_hypervisor_vendor_by_cpuid() {
        return Some(vendor);
    }
    join_present(
        [
            VIRTUAL_MACHINE_MONITOR_LABEL,
            VIRTUAL_MACHINE_MONITOR_PRESENT,
        ]
        .map(|part| Some(part.to_owned())),
    )
}

pub(crate) fn read_labels() -> MachineLabels {
    let build = read_sysctl_text(BUILD);
    MachineLabels {
        operating_system: join_present([
            Some(OPERATING_SYSTEM_LABEL.to_owned()),
            read_sysctl_text(PRODUCT_VERSION),
        ]),
        operating_system_build: build.clone(),
        kernel: read_sysctl_text(KERNEL_VERSION),
        hypervisor: read_sysctl_yes(HYPERVISOR_SUPPORT)
            .then(|| {
                join_present([
                    Some(HYPERVISOR_LABEL.to_owned()),
                    Some(OPERATING_SYSTEM_LABEL.to_owned()),
                    build,
                ])
            })
            .flatten(),
        underlying_hypervisor: read_underlying_hypervisor(),
        firmware: read_firmware(),
        machine_model: read_sysctl_text(PRODUCT).or_else(|| read_sysctl_text(MODEL)),
        date: String::new(),
    }
}
