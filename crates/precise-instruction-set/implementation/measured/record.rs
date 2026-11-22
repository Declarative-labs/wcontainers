//! Records this host's chip: every listed instruction on every state the critical value generator makes.
//!
//! `cargo run -p wcontainers-precise-instruction-set-measured --example record -- <folder>` writes one file for
//! each instruction into the folder.

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
mod signing;

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
fn main() {
    use std::path::PathBuf;
    use std::time::Instant;

    use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
    use wcontainers_executor_hardware_virtualization::single_step::SingleStepMachine;
    use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
    use wcontainers_processor_architecture_recording::write_recordings;

    const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
        CriticalValueProduct::new(Arm64InputDomain);

    let Some(folder) = std::env::args_os().nth(1).map(PathBuf::from) else {
        panic!("the folder to record into must be given");
    };
    signing::be_signed();
    let mut chip = match SingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host's hypervisor makes no single step machine: {error:?}"),
    };
    let started = Instant::now();
    if let Err(error) = write_recordings(&mut chip, &GENERATOR, &folder) {
        panic!("writing into {}: {error}", folder.display());
    }
    println!(
        "recorded into {} in {:.3} s, {:.3} s of it in the guest",
        folder.display(),
        started.elapsed().as_secs_f64(),
        chip.time_in_guest().as_secs_f64()
    );
}

#[cfg(not(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux"))))]
fn main() {
    panic!("recording needs an ARM64 host with a hypervisor the single step machine drives");
}
