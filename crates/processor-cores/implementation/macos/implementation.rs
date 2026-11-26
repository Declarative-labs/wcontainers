//! The cores of a Mac by performance level: a core is a place among the logical cores of one level.
//!
//! macOS cannot pin a thread and no safe call says where one runs, so on Apple silicon `run_on_core` only gives the thread
//! the level's quality-of-service class, which Apple documents as making that kind of core more likely.

#![cfg(all(
    target_os = "macos",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use sysctl::{Ctl, CtlValue, Sysctl};
use wcontainers_processor_cores::ProcessorCores;

#[cfg(target_arch = "x86_64")]
mod cpuid;
#[cfg(target_arch = "aarch64")]
mod quality_of_service;

#[cfg(target_arch = "aarch64")]
use wcontainers_processor_architecture_arm64::CoreIdentity;
#[cfg(target_arch = "x86_64")]
use wcontainers_processor_architecture_x86_64::CoreIdentity;

/// The cores of the Mac, each a place among the logical cores of one performance level.
pub struct MacosProcessorCores;

/// One of the logical cores of a performance level.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CorePlace {
    /// The level, where 0 is the fastest.
    pub performance_level: usize,
    /// Which of the level's logical cores this is, counted from 0.
    pub position: usize,
}

/// A sysctl that could not be read as the number it holds.
#[derive(Debug)]
pub struct SysctlFailure {
    pub name: String,
    pub reason: String,
}

/// A reason the cores could not be listed.
#[derive(Debug)]
pub enum ListCoresError {
    /// A sysctl the cores are counted or identified by could not be read.
    SysctlUnreadable(SysctlFailure),
    /// No call of macOS reads MIDR_EL1 or REVIDR_EL1 of an Apple silicon core, which identify it.
    MainIdRegisterUnreadable,
    /// The thread could not be held at the place to read the identity there.
    CoreNotHeld {
        place: CorePlace,
        error: RunOnCoreError,
    },
}

/// A reason the work could not be run on the core.
#[derive(Debug)]
pub enum RunOnCoreError {
    /// The performance levels could not be counted.
    LevelsUncounted(SysctlFailure),
    /// The place names a level or a position the Mac does not have.
    PlaceOutOfRange { place: CorePlace },
    /// The system refused the level's quality-of-service class.
    #[cfg(target_arch = "aarch64")]
    ClassRefused(gdt_cpus::Error),
    /// The thread got something other than the class asked for, described as gdt-cpus describes it.
    #[cfg(target_arch = "aarch64")]
    ClassNotApplied { applied: String },
    /// The thread could not be given the Default class back.
    #[cfg(target_arch = "aarch64")]
    ClassNotRestored(gdt_cpus::Error),
}

/// [The number of performance levels](https://developer.apple.com/documentation/kernel/1387446-sysctlbyname/determining_system_capabilities); Intel Macs lack it and have one level.
const PERFORMANCE_LEVELS: &str = "hw.nperflevels";
/// The sysctl of a level's number of logical cores is this, the level, and the suffix after it.
const LEVEL_PREFIX: &str = "hw.perflevel";
const LEVEL_CORES_SUFFIX: &str = ".logicalcpu";
/// The number of logical cores of every level together, for a Mac without performance levels.
const LOGICAL_CORES: &str = "hw.logicalcpu";
/// The microcode revision of an Intel Mac.
#[cfg(target_arch = "x86_64")]
const MICROCODE_VERSION: &str = "machdep.cpu.microcode_version";

fn read_sysctl_number(name: &str) -> Result<u64, SysctlFailure> {
    let failure = |reason: String| SysctlFailure {
        name: name.to_owned(),
        reason,
    };
    let value = Ctl::new(name)
        .and_then(|control| control.value())
        .map_err(|error| failure(error.to_string()))?;
    match value {
        CtlValue::Int(number) | CtlValue::S32(number) => u64::try_from(number).ok(),
        CtlValue::Uint(number) | CtlValue::U32(number) => Some(number.into()),
        CtlValue::Long(number) | CtlValue::S64(number) => u64::try_from(number).ok(),
        CtlValue::Ulong(number) | CtlValue::U64(number) => Some(number),
        _ => None,
    }
    .ok_or_else(|| failure(format!("{value:?}")))
}

/// Returns the number of logical cores of each performance level, fastest first.
fn count_level_cores() -> Result<Vec<usize>, SysctlFailure> {
    if Ctl::new(PERFORMANCE_LEVELS).is_err() {
        return Ok(vec![read_sysctl_number(LOGICAL_CORES)? as usize]);
    }
    (0..read_sysctl_number(PERFORMANCE_LEVELS)?)
        .map(|level| {
            Ok(
                read_sysctl_number(
                    &[LEVEL_PREFIX, &level.to_string(), LEVEL_CORES_SUFFIX].concat(),
                )? as usize,
            )
        })
        .collect()
}

/// Returns the number of levels, once the place is one the Mac has.
fn check_place(place: CorePlace) -> Result<usize, RunOnCoreError> {
    let counts = count_level_cores().map_err(RunOnCoreError::LevelsUncounted)?;
    if counts
        .get(place.performance_level)
        .is_none_or(|count| place.position >= *count)
    {
        return Err(RunOnCoreError::PlaceOutOfRange { place });
    }
    Ok(counts.len())
}

impl ProcessorCores for MacosProcessorCores {
    type Core = CorePlace;
    type CoreIdentity = CoreIdentity;
    type ListCoresError = ListCoresError;
    type RunOnCoreError = RunOnCoreError;

    #[cfg(target_arch = "aarch64")]
    fn list_cores(&self) -> Result<Vec<(Self::Core, Self::CoreIdentity)>, Self::ListCoresError> {
        Err(ListCoresError::MainIdRegisterUnreadable)
    }

    #[cfg(target_arch = "x86_64")]
    fn list_cores(&self) -> Result<Vec<(Self::Core, Self::CoreIdentity)>, Self::ListCoresError> {
        let microcode_version =
            read_sysctl_number(MICROCODE_VERSION).map_err(ListCoresError::SysctlUnreadable)?;
        let mut cores = Vec::new();
        let counts = count_level_cores().map_err(ListCoresError::SysctlUnreadable)?;
        for (performance_level, count) in counts.into_iter().enumerate() {
            for position in 0..count {
                let place = CorePlace {
                    performance_level,
                    position,
                };
                let from_cpuid = self
                    .run_on_core(&place, cpuid::read_identity_by_cpuid)
                    .map_err(|error| ListCoresError::CoreNotHeld { place, error })?;
                cores.push((
                    place,
                    CoreIdentity {
                        microcode_version: u32::try_from(microcode_version).ok(),
                        ..from_cpuid
                    },
                ));
            }
        }
        Ok(cores)
    }

    /// Confirms only that macOS accepted the class before the work; afterwards the thread gets the Default class, its earlier one being unreadable.
    #[cfg(target_arch = "aarch64")]
    fn run_on_core<Output>(
        &self,
        core: &Self::Core,
        work: impl FnOnce() -> Output,
    ) -> Result<Output, Self::RunOnCoreError> {
        let levels = check_place(*core)?;
        quality_of_service::run_with_level_class(core.performance_level, levels, work)
    }

    /// Runs the work on whichever core macOS chooses, as an Intel Mac has one level and one kind of core.
    #[cfg(target_arch = "x86_64")]
    fn run_on_core<Output>(
        &self,
        core: &Self::Core,
        work: impl FnOnce() -> Output,
    ) -> Result<Output, Self::RunOnCoreError> {
        check_place(*core)?;
        Ok(work())
    }
}
