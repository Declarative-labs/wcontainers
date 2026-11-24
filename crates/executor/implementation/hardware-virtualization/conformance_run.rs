#![cfg(all(host_hypervisor, target_arch = "aarch64"))]

use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
use wcontainers_executor_hardware_virtualization::single_step::SingleStepMachine;
use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
#[cfg(target_os = "macos")]
use wcontainers_precise_instruction_set_apple_m5::AppleM5 as ThisChip;
#[cfg(target_os = "linux")]
use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2 as ThisChip;

wcontainers_executor_conformance::tests!(
    SingleStepMachine::new().expect("this host's hypervisor makes a machine"),
    ThisChip,
    CriticalValueProduct::new(Arm64InputDomain)
);
