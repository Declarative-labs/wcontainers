//! The behavior source suite on this host's chip, which needs ARM64 Linux with KVM.

#![cfg(all(target_arch = "aarch64", target_os = "linux"))]

use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;

fn chip() -> KvmSingleStepMachine {
    match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    }
}

wcontainers_behavior_source_conformance::tests!(
    chip(),
    CriticalValueProduct::new(Arm64InputDomain)
);
