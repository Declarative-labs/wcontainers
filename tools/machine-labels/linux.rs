//! The labels of a Linux machine, from /etc/os-release, /proc, /sys and /dev.

use std::fs;
use std::path::Path;

use crate::{MachineLabels, join_present};

/// The [operating system identification](https://www.freedesktop.org/software/systemd/man/latest/os-release.html) file.
const OS_RELEASE: &str = "/etc/os-release";
/// The kernel's release, as `uname -r` gives it.
const KERNEL_RELEASE: &str = "/proc/sys/kernel/osrelease";
/// The kernel's build number and date, as `uname -v` gives it.
const KERNEL_VERSION: &str = "/proc/sys/kernel/version";
/// The device a program opens to use KVM.
const KVM_DEVICE: &str = "/dev/kvm";
/// The type of the hypervisor Linux runs under, where one makes itself known there (Xen does).
const HYPERVISOR_TYPE: &str = "/sys/hypervisor/type";
/// The hypervisor node of the device tree, where the hypervisor gives one (as ARM64 guests of Xen and others get).
const DEVICE_TREE_HYPERVISOR: &str = "/sys/firmware/devicetree/base/hypervisor/compatible";
/// The folder of the [DMI fields](https://www.kernel.org/doc/Documentation/ABI/testing/sysfs-class-dmi) the firmware gives.
const DMI_FOLDER: &str = "/sys/class/dmi/id";

/// The fields of os-release that name the system and its build.
const PRETTY_NAME: &str = "PRETTY_NAME";
const VERSION_ID: &str = "VERSION_ID";
const BUILD_ID: &str = "BUILD_ID";
/// What separates a field of os-release from its value.
const OS_RELEASE_SEPARATOR: char = '=';
/// The quotes os-release may put around a value.
const OS_RELEASE_QUOTES: &[char] = &['"', '\''];
/// What ends the text of a device tree property.
const PROPERTY_END: char = '\0';
/// The DMI fields naming the firmware, then the machine.
const FIRMWARE_FIELDS: &[&str] = &["bios_vendor", "bios_version", "bios_date"];
const MACHINE_FIELDS: &[&str] = &["sys_vendor", "product_name"];
/// The name the KVM label starts with.
const KVM_LABEL: &str = "KVM of Linux";

fn read_label(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let label =
        text.trim_matches(|character: char| character.is_whitespace() || character == PROPERTY_END);
    (!label.is_empty()).then(|| label.to_owned())
}

fn read_dmi(field: &str) -> Option<String> {
    read_label(&Path::new(DMI_FOLDER).join(field))
}

/// Returns the value of a field of os-release, without its quotes.
fn read_os_release_field(text: &str, field: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once(OS_RELEASE_SEPARATOR))
        .find(|(name, _)| name.trim() == field)
        .map(|(_, value)| value.trim().trim_matches(OS_RELEASE_QUOTES).to_owned())
}

/// Returns the hypervisor under this system: the one x86-64 processors report through CPUID, or one in /sys on any processor.
fn read_underlying_hypervisor() -> Option<String> {
    #[cfg(target_arch = "x86_64")]
    if let Some(vendor) = crate::read_hypervisor_vendor_by_cpuid() {
        return Some(vendor);
    }
    read_label(Path::new(HYPERVISOR_TYPE)).or_else(|| read_label(Path::new(DEVICE_TREE_HYPERVISOR)))
}

pub(crate) fn read_labels() -> MachineLabels {
    let os_release = fs::read_to_string(OS_RELEASE).unwrap_or_default();
    let release = read_label(Path::new(KERNEL_RELEASE));
    MachineLabels {
        operating_system: read_os_release_field(&os_release, PRETTY_NAME),
        operating_system_build: join_present([
            read_os_release_field(&os_release, VERSION_ID),
            read_os_release_field(&os_release, BUILD_ID),
        ]),
        kernel: join_present([release.clone(), read_label(Path::new(KERNEL_VERSION))]),
        hypervisor: Path::new(KVM_DEVICE)
            .exists()
            .then(|| join_present([Some(KVM_LABEL.to_owned()), release]))
            .flatten(),
        underlying_hypervisor: read_underlying_hypervisor(),
        firmware: join_present(FIRMWARE_FIELDS.iter().map(|field| read_dmi(field))),
        machine_model: join_present(MACHINE_FIELDS.iter().map(|field| read_dmi(field))),
        date: String::new(),
    }
}
