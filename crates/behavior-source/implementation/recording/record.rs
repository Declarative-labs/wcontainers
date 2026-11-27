//! Records this host's chip, into one file for each instruction in the folder.
//!
//! `cargo run --release -p wcontainers-behavior-source-recording --example record -- <folder>` runs every state of the
//! committed Neoverse V2 recording again, in its order; `... -- <folder> <instruction>...` runs the named instructions
//! on every state the ARM64 state generator makes for them, and `... -- <folder> all` every listed instruction;
//! `... -- <folder> deciding` keeps, of every listed instruction, only the states that tell its known behaviors apart.

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
    /// The argument that records every listed instruction.
    const EVERY_INSTRUCTION: &str = "all";
    /// The argument that records the deciding states of every listed instruction.
    const DECIDING: &str = "deciding";

    let mut arguments = std::env::args().skip(1);
    let Some(folder) = arguments.next().map(PathBuf::from) else {
        panic!("the folder to record into must be given");
    };
    let named: Vec<String> = arguments.collect();
    let generator = CriticalValueCut::default();
    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("this host makes no single step machine: {error:?}"),
    };
    let started = Instant::now();
    let recorded = if named == [DECIDING] {
        Recording::record_deciding(&mut chip, &generator, Arm64::<User>::list_instructions())
    } else if named.is_empty() {
        match Recording::read(Path::new(COMMITTED)) {
            Ok(committed) => committed.record_again(&mut chip, &generator),
            Err(error) => panic!("reading {COMMITTED}: {error}"),
        }
    } else {
        let every = named == [EVERY_INSTRUCTION];
        let instructions = Arm64::<User>::list_instructions()
            .filter(|instruction| every || named.contains(&format!("{instruction:?}")));
        Recording::record(&mut chip, &generator, instructions)
    };
    let recording = match recorded {
        Ok(recording) => recording,
        Err(refusal) => panic!("the chip did not run a recorded or generated state: {refusal:?}"),
    };
    if let Err(error) = recording.write(&folder) {
        panic!("writing into {}: {error}", folder.display());
    }
    println!(
        "recorded {} instructions, {} states into {} in {:.3} s",
        recording.instructions().count(),
        recording.count_cases(),
        folder.display(),
        started.elapsed().as_secs_f64()
    );
}

#[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
fn main() {
    panic!("recording needs ARM64 Linux with KVM");
}
