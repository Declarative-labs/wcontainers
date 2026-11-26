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
        CoreIdentity::to_kind_name,
    )
}
