//! The logical processors of a Windows machine, each held by giving the calling thread a hard affinity of that processor alone.
//!
//! Windows runs a thread only on the processors of its hard affinity. Before and after the work `run_on_core` confirms that
//! the affinity reads back as that processor; it cannot ask where the thread runs, as no safe call does.

#![cfg(all(
    target_os = "windows",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use gdt_cpus::{AffinityMask, CpuInfo, current_affinity, set_thread_affinity};
use wcontainers_processor_cores::ProcessorCores;
use windows_registry::LOCAL_MACHINE;

#[cfg(target_arch = "x86_64")]
mod cpuid;

#[cfg(target_arch = "aarch64")]
use wcontainers_processor_architecture_arm64::CoreIdentity;
#[cfg(target_arch = "x86_64")]
use wcontainers_processor_architecture_x86_64::CoreIdentity;

/// The logical processors of the machine, each named by its number across processor groups (group times 64 plus its bit).
pub struct WindowsProcessorCores;

/// A reason the cores could not be listed.
#[derive(Debug)]
pub enum ListCoresError {
    /// The logical processors could not be listed.
    ProcessorsUnlisted(gdt_cpus::Error),
    /// A registry value the identity is read from could not be read.
    RegistryValueUnreadable {
        key: String,
        value: String,
        error: String,
    },
    /// A registry value held fewer bytes than its layout has.
    RegistryValueMalformed {
        key: String,
        value: String,
        bytes: Vec<u8>,
    },
    /// The thread could not be held on the processor to read its identity there.
    CoreNotHeld { core: usize, error: RunOnCoreError },
}

/// A reason the work could not be run on the core.
#[derive(Debug)]
pub enum RunOnCoreError {
    /// The thread's earlier affinity could not be read.
    PlacementUnreadable(gdt_cpus::Error),
    /// Windows refused the affinity of the processor alone.
    PinRefused(gdt_cpus::Error),
    /// The affinity read back as other processors before the work; the work did not run.
    MovedBeforeWork { found: Vec<usize> },
    /// The affinity read back as other processors after the work; what the work returned is dropped.
    MovedDuringWork { found: Vec<usize> },
    /// The thread's earlier affinity could not be given back.
    PlacementNotRestored(gdt_cpus::Error),
}

/// The key of one logical processor in the registry, before its number.
const PROCESSOR_KEY: &str = r"HARDWARE\DESCRIPTION\System\CentralProcessor\";

fn read_registry_value<Value>(
    core: usize,
    value: &str,
    read: impl FnOnce(&windows_registry::Key) -> windows_registry::Result<Value>,
) -> Result<Value, ListCoresError> {
    let key = [PROCESSOR_KEY, &core.to_string()].concat();
    LOCAL_MACHINE
        .open(&key)
        .and_then(|opened| read(&opened))
        .map_err(|error| ListCoresError::RegistryValueUnreadable {
            key,
            value: value.to_owned(),
            error: error.to_string(),
        })
}

/// The value holding the processor's MIDR_EL1, named by the register's encoding, as QEMU's WHPX back end reads it.
#[cfg(target_arch = "aarch64")]
const MAIN_ID_REGISTER_VALUE: &str = "CP 4000";
/// The value that would hold REVIDR_EL1 by the same encoding; that Windows writes it is not documented.
#[cfg(target_arch = "aarch64")]
const REVISION_ID_REGISTER_VALUE: &str = "CP 4006";

#[cfg(target_arch = "aarch64")]
fn read_identity(
    _cores: &WindowsProcessorCores,
    core: usize,
) -> Result<CoreIdentity, ListCoresError> {
    Ok(CoreIdentity {
        main_id_register: read_registry_value(core, MAIN_ID_REGISTER_VALUE, |key| {
            key.get_u64(MAIN_ID_REGISTER_VALUE)
        })?,
        revision_id_register: read_registry_value(core, REVISION_ID_REGISTER_VALUE, |key| {
            key.get_u64(REVISION_ID_REGISTER_VALUE)
        })?,
    })
}

/// The value holding the processor's microcode as the 64 bits of IA32_BIOS_SIGN_ID, little-endian.
#[cfg(target_arch = "x86_64")]
const MICROCODE_VALUE: &str = "Update Revision";
/// The byte at which the revision starts: it is the upper half of [IA32_BIOS_SIGN_ID](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) (MSR 8BH, volume 4).
#[cfg(target_arch = "x86_64")]
const REVISION_OFFSET: usize = 4;

#[cfg(target_arch = "x86_64")]
fn read_identity(
    cores: &WindowsProcessorCores,
    core: usize,
) -> Result<CoreIdentity, ListCoresError> {
    let from_cpuid = cores
        .run_on_core(&core, cpuid::read_identity_by_cpuid)
        .map_err(|error| ListCoresError::CoreNotHeld { core, error })?;
    let bytes = read_registry_value(core, MICROCODE_VALUE, |key| key.get_bytes(MICROCODE_VALUE))?;
    let Some(revision) = bytes
        .get(REVISION_OFFSET..REVISION_OFFSET + size_of::<u32>())
        .and_then(|revision| <[u8; size_of::<u32>()]>::try_from(revision).ok())
    else {
        return Err(ListCoresError::RegistryValueMalformed {
            key: [PROCESSOR_KEY, &core.to_string()].concat(),
            value: MICROCODE_VALUE.to_owned(),
            bytes,
        });
    };
    Ok(CoreIdentity {
        microcode_version: Some(u32::from_le_bytes(revision)),
        ..from_cpuid
    })
}

/// The thread's affinity from before it was pinned, given back when this is dropped (also when the work panics).
struct EarlierPlacement {
    affinity: AffinityMask,
    restored: bool,
}

impl EarlierPlacement {
    fn restore(mut self) -> Result<(), RunOnCoreError> {
        self.restored = true;
        set_thread_affinity(&self.affinity).map_err(RunOnCoreError::PlacementNotRestored)
    }
}

impl Drop for EarlierPlacement {
    fn drop(&mut self) {
        if !self.restored {
            // A panic is already unwinding, so a failure here has no caller to go to.
            let _ = set_thread_affinity(&self.affinity);
        }
    }
}

/// Checks that the affinity is the processor alone, returning the processors it holds when it is not.
fn check_only(core: usize, found: AffinityMask) -> Result<(), Vec<usize>> {
    if found == AffinityMask::single(core) {
        return Ok(());
    }
    Err(found.iter().collect())
}

impl ProcessorCores for WindowsProcessorCores {
    type Core = usize;
    type CoreIdentity = CoreIdentity;
    type ListCoresError = ListCoresError;
    type RunOnCoreError = RunOnCoreError;

    fn list_cores(&self) -> Result<Vec<(Self::Core, Self::CoreIdentity)>, Self::ListCoresError> {
        let mut cores = CpuInfo::detect()
            .map_err(ListCoresError::ProcessorsUnlisted)?
            .logical_processor_ids();
        cores.sort_unstable();
        cores
            .into_iter()
            .map(|core| Ok((core, read_identity(self, core)?)))
            .collect()
    }

    fn run_on_core<Output>(
        &self,
        core: &Self::Core,
        work: impl FnOnce() -> Output,
    ) -> Result<Output, Self::RunOnCoreError> {
        let core = *core;
        let earlier = EarlierPlacement {
            affinity: current_affinity().map_err(RunOnCoreError::PlacementUnreadable)?,
            restored: false,
        };
        set_thread_affinity(&AffinityMask::single(core)).map_err(RunOnCoreError::PinRefused)?;
        let before = current_affinity().map_err(RunOnCoreError::PlacementUnreadable);
        if let Err(error) = before.and_then(|found| {
            check_only(core, found).map_err(|found| RunOnCoreError::MovedBeforeWork { found })
        }) {
            earlier.restore()?;
            return Err(error);
        }
        let output = work();
        let after = current_affinity().map_err(RunOnCoreError::PlacementUnreadable);
        earlier.restore()?;
        check_only(core, after?).map_err(|found| RunOnCoreError::MovedDuringWork { found })?;
        Ok(output)
    }
}
