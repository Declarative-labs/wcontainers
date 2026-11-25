//! Records this host's chip, into one file for each instruction in the folder.
//!
//! `cargo run --release -p wcontainers-behavior-source-recording --example record -- <folder>` runs every state of the
//! committed Neoverse V2 recording again, in its order; `... -- <folder> <instruction>...` runs the named instructions
//! on every state the ARM64 state generator makes for them.

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn main() {
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_behavior_source_recording::Recording;
    use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
    use wcontainers_processor_architecture::ProcessorArchitecture;
    use wcontainers_processor_architecture_arm64::{Arm64, User};

    /// What a Neoverse V2 under KVM did, as committed.
    const COMMITTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recordings/neoverse-v2");

    let mut arguments = std::env::args().skip(1);
    let Some(folder) = arguments.next().map(PathBuf::from) else {
        panic!("the folder to record into must be given");
    };
    let named: Vec<String> = arguments.collect();
    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let started = Instant::now();
    let recording = if named.is_empty() {
        match Recording::read(Path::new(COMMITTED)) {
            Ok(committed) => committed.record_again(&mut chip),
            Err(error) => panic!("reading {COMMITTED}: {error}"),
        }
    } else {
        let instructions = Arm64::<User>::list_instructions()
            .filter(|instruction| named.contains(&format!("{instruction:?}")));
        Recording::record(&mut chip, &CriticalValueCut::default(), instructions)
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
