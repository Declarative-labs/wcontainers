//! The behavior source suite on this host's chip, which needs x86-64 Linux with KVM.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use wcontainers_behavior_source_kvm_single_step_x86_64::SingleStepMachine;
use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;

fn chip() -> SingleStepMachine {
    match SingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    }
}

wcontainers_behavior_source_conformance::tests!(chip(), CriticalValueCut::new(X86_64InputDomain));
