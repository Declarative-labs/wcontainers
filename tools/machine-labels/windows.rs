//! The labels of a Windows machine, from its registry and, on x86-64, CPUID.

use windows_registry::LOCAL_MACHINE;

use crate::{MachineLabels, join_present};

/// The key naming the installed Windows and its build, and its values read here.
const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const PRODUCT_NAME: &str = "ProductName";
const DISPLAY_VERSION: &str = "DisplayVersion";
const BUILD_NUMBER: &str = "CurrentBuildNumber";
const UPDATE_BUILD_REVISION: &str = "UBR";
/// The key holding what the firmware's SMBIOS tables say of it and of the machine, and its values read here.
const BIOS: &str = r"HARDWARE\DESCRIPTION\System\BIOS";
const FIRMWARE_VALUES: &[&str] = &["BIOSVendor", "BIOSVersion", "BIOSReleaseDate"];
const MACHINE_VALUES: &[&str] = &["SystemManufacturer", "SystemProductName"];
/// What separates the build number from the update build revision.
const BUILD_SEPARATOR: &str = ".";
/// The name the hypervisor label starts with.
const HYPERVISOR_LABEL: &str = "Windows Hypervisor Platform of Windows build";
/// The vendor Hyper-V gives in the hypervisor leaves.
#[cfg(target_arch = "x86_64")]
const HYPER_V_VENDOR: &str = "Microsoft Hv";
/// The leaf of Hyper-V's partition privileges, whose EBX bit 0 (CreatePartitions) is set only in the root partition ([TLFS](https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/tlfs/feature-discovery)).
#[cfg(target_arch = "x86_64")]
const PARTITION_PRIVILEGES_LEAF: u32 = 0x4000_0003;
/// The CreatePartitions bit of that leaf's EBX ([TLFS](https://learn.microsoft.com/en-us/virtualization/hyper-v-on-windows/tlfs/feature-discovery)).
#[cfg(target_arch = "x86_64")]
const CREATE_PARTITIONS: u32 = 1;

fn read_text(key: &str, value: &str) -> Option<String> {
    LOCAL_MACHINE
        .open(key)
        .and_then(|opened| opened.get_string(value))
        .ok()
}

/// Returns the hypervisor this Windows runs under, which is none when it is Hyper-V's own root partition.
#[cfg(target_arch = "x86_64")]
fn read_underlying_hypervisor() -> Option<String> {
    let vendor = crate::read_hypervisor_vendor_by_cpuid()?;
    let privileges =
        core::arch::x86_64::__cpuid_count(PARTITION_PRIVILEGES_LEAF, crate::SUBLEAF).ebx;
    let root = vendor == HYPER_V_VENDOR && privileges & CREATE_PARTITIONS != 0;
    (!root).then_some(vendor)
}

/// Returns none: on ARM64 Windows no safe call tells the root partition from a guest.
#[cfg(not(target_arch = "x86_64"))]
fn read_underlying_hypervisor() -> Option<String> {
    None
}

pub(crate) fn read_labels() -> MachineLabels {
    let update = LOCAL_MACHINE
        .open(CURRENT_VERSION)
        .and_then(|opened| opened.get_u32(UPDATE_BUILD_REVISION))
        .ok();
    let build = read_text(CURRENT_VERSION, BUILD_NUMBER).map(|build| match update {
        Some(update) => [build, update.to_string()].join(BUILD_SEPARATOR),
        None => build,
    });
    MachineLabels {
        operating_system: join_present([
            read_text(CURRENT_VERSION, PRODUCT_NAME),
            read_text(CURRENT_VERSION, DISPLAY_VERSION),
        ]),
        operating_system_build: build.clone(),
        kernel: build.clone(),
        hypervisor: join_present([Some(HYPERVISOR_LABEL.to_owned()), build]),
        underlying_hypervisor: read_underlying_hypervisor(),
        firmware: join_present(FIRMWARE_VALUES.iter().map(|value| read_text(BIOS, value))),
        machine_model: join_present(MACHINE_VALUES.iter().map(|value| read_text(BIOS, value))),
        date: String::new(),
    }
}
