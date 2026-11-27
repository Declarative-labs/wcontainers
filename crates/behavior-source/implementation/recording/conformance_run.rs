use std::path::PathBuf;
use std::sync::LazyLock;

use wcontainers_behavior_source_recording::{Recording, WrittenStates};
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, User};

/// What a Neoverse V2 under KVM did with the states that tell apart the known behaviors of every listed instruction,
/// written by `record -- <folder> deciding`.
const COMMITTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/recordings/neoverse-v2-deciding"
);

/// The recording every test reads: made now from this host's chip where it has one, else the committed one.
static FOLDER: LazyLock<PathBuf> =
    LazyLock::new(|| record_chip().unwrap_or_else(|| PathBuf::from(COMMITTED)));

/// Records the deciding states of every listed instruction on this host's chip into a folder of the test's own.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn record_chip() -> Option<PathBuf> {
    use std::path::Path;

    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
    use wcontainers_processor_architecture::ProcessorArchitecture;

    let mut chip = KvmSingleStepMachine::new().ok()?;
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("recording-chip-deciding");
    if let Err(error) = std::fs::remove_dir_all(&folder)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        panic!("emptying {}: {error}", folder.display());
    }
    if let Err(error) = std::fs::create_dir_all(&folder) {
        panic!("making {}: {error}", folder.display());
    }
    let recorded = Recording::record_deciding(
        &mut chip,
        &CriticalValueCut::default(),
        Arm64::<User>::list_instructions(),
    );
    let recording = match recorded {
        Ok(recording) => recording,
        Err(refusal) => panic!("the chip did not run a generated state: {refusal:?}"),
    };
    if let Err(error) = recording.write(&folder) {
        panic!("writing into {}: {error}", folder.display());
    }
    eprintln!(
        "recorded {} states of this host's chip into {}",
        recording.count_cases(),
        folder.display()
    );
    Some(folder)
}

#[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
fn record_chip() -> Option<PathBuf> {
    None
}

fn recording() -> Recording {
    match Recording::read(&FOLDER) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {}: {error}", FOLDER.display()),
    }
}

/// Returns a state the recording holds no case of: its first state before, with another stack pointer.
fn to_unrecorded() -> Vec<ExecutionState<'static, Arm64<User>>> {
    recording()
        .cases()
        .take(1)
        .map(|(_, before, _)| {
            let mut unrecorded = before.clone();
            unrecorded.processor.stack_pointer ^= 1;
            unrecorded
        })
        .collect()
}

wcontainers_behavior_source_conformance::tests!(
    recording(),
    WrittenStates::new(&recording()),
    to_unrecorded()
);
