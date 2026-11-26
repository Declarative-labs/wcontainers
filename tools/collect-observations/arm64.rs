//! The collection on an ARM64 machine: the KVM chip source on each core, and the state generator the measured set uses.

use std::path::Path;
use std::process::ExitCode;

use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_processor_architecture_arm64::CoreIdentity;

use crate::collection::collect_and_write;

pub fn collect(output: &Path) -> ExitCode {
    collect_and_write(
        output,
        |_| KvmSingleStepMachine::new(),
        CriticalValueCut::default(),
        name_kind,
    )
}

/// Returns the name of the kind's file: its [MIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/MIDR-EL1--Main-ID-Register) and [REVIDR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/REVIDR-EL1--Revision-ID-Register) in hexadecimal.
pub fn name_kind(core: &CoreIdentity) -> String {
    format!(
        "midr-{:08x}-revidr-{:08x}",
        core.main_id_register, core.revision_id_register
    )
}
