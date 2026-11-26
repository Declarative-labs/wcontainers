//! The collection on an x86-64 machine: the KVM chip source on each core and the x86-64 input domain's generator.

use std::path::Path;
use std::process::ExitCode;

use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
use wcontainers_processor_architecture_x86_64::CoreIdentity;

use crate::collection::collect_and_write;

pub fn collect(output: &Path) -> ExitCode {
    collect_and_write(
        output,
        |_| SingleStepMachine::new(),
        CriticalValueCut::new(X86_64InputDomain),
        CoreIdentity::to_kind_name,
    )
}
