//! The collection on an x86-64 machine: the KVM chip source on each core, and the state generator of the x86-64
//! input domain.

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
        name_kind,
    )
}

/// Returns the name of the kind's file: the vendor in lower case, then family, model, stepping, core type and
/// microcode in hexadecimal, as [CPUID](https://www.felixcloutier.com/x86/cpuid) gives them.
pub fn name_kind(core: &CoreIdentity) -> String {
    let vendor: String = core
        .vendor_identification
        .iter()
        .filter(|byte| byte.is_ascii_alphanumeric())
        .map(|byte| char::from(byte.to_ascii_lowercase()))
        .collect();
    let core_type = core
        .hybrid_core_type
        .map(|core_type| format!("-core-type-{core_type:02x}"))
        .unwrap_or_default();
    let microcode = core.microcode_version.map_or_else(
        || String::from("unreported"),
        |version| format!("{version:x}"),
    );
    format!(
        "{vendor}-family-{:x}-model-{:x}-stepping-{:x}{core_type}-microcode-{microcode}",
        core.family, core.model, core.stepping
    )
}
