//! Measures this host's chip against the known sets as a program would at start-up, and says how long it took.

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn main() {
    use std::time::Instant;

    use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
    use wcontainers_executor_hardware_virtualization::single_step::SingleStepMachine;
    use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
    use wcontainers_precise_instruction_set::PreciseInstructionSet;
    use wcontainers_precise_instruction_set_apple_m5::AppleM5;
    use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
    use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
    use wcontainers_processor_architecture::ProcessorArchitecture;
    use wcontainers_processor_architecture_arm64::Arm64;

    const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
        CriticalValueProduct::new(Arm64InputDomain);

    let Ok(mut chip) = SingleStepMachine::new() else {
        panic!("this host's hypervisor makes no single step machine");
    };
    let started = Instant::now();
    let known: [&dyn PreciseInstructionSet<Arm64>; 2] = [&AppleM5, &NeoverseV2];
    let Ok(measured) = MeasuredInstructionSet::measure(&mut chip, &known, &GENERATOR) else {
        panic!("this chip's behaviors are not one known set's");
    };
    println!(
        "measured in {:.3} ms, {:.3} ms of it in the guest",
        started.elapsed().as_secs_f64() * 1e3,
        chip.time_in_guest().as_secs_f64() * 1e3
    );
    for instruction in Arm64::list_instructions() {
        assert!(
            measured.to_precise(instruction) == NeoverseV2.to_precise(instruction),
            "the Linux host is a Neoverse V2, but {instruction:?} measures otherwise"
        );
    }
}

#[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
fn main() {
    println!("measuring a chip runs on an ARM64 Linux host with KVM");
}
