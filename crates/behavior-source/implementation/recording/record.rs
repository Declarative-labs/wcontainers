//! Records this host's chip: every listed instruction on every state the critical value generator makes.
//!
//! `cargo run --release -p wcontainers-behavior-source-recording --example record -- <folder>` writes one file for
//! each instruction into the folder.

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn main() {
    use std::path::PathBuf;
    use std::time::Instant;

    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_behavior_source_recording::Recording;
    use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
    use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;

    let Some(folder) = std::env::args_os().nth(1).map(PathBuf::from) else {
        panic!("the folder to record into must be given");
    };
    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let started = Instant::now();
    let recording = match Recording::record(&mut chip, &CriticalValueProduct::new(Arm64InputDomain))
    {
        Ok(recording) => recording,
        Err(refusal) => panic!("the chip did not run a generated state: {refusal:?}"),
    };
    if let Err(error) = recording.write(&folder) {
        panic!("writing into {}: {error}", folder.display());
    }
    println!(
        "recorded into {} in {:.3} s",
        folder.display(),
        started.elapsed().as_secs_f64()
    );
}

#[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
fn main() {
    panic!("recording needs ARM64 Linux with KVM");
}
