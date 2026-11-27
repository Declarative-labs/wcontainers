use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::LazyLock;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_behavior_source_recording::{Recording, WrittenStates};
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, PreciseInstruction, User};

/// What a Neoverse V2 under KVM did with the states that tell apart the known behaviors of every listed instruction,
/// written by the record tool's `deciding`.
const COMMITTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2-deciding"
);

/// The recording every test reads: made now from this host's chip where it has one, else the committed one.
static FOLDER: LazyLock<PathBuf> =
    LazyLock::new(|| record_chip().unwrap_or_else(|| PathBuf::from(COMMITTED)));

/// The recording read once, whose cases every test may walk.
static RECORDING: LazyLock<Recording> = LazyLock::new(recording);

type Measured = MeasuredInstructionSet<Arm64<User>>;

/// Records the deciding states of every listed instruction on this host's chip into a folder of the test's own.
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn record_chip() -> Option<PathBuf> {
    use std::path::Path;

    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
    use wcontainers_processor_architecture::ProcessorArchitecture;

    let mut chip = KvmSingleStepMachine::new().ok()?;
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("measured-chip-deciding");
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

/// Measures every instruction on the states the recording holds.
fn measure_recording() -> Vec<(Instruction<Infallible>, PreciseInstruction<Infallible>)> {
    match recording().measure_instructions(&WrittenStates::new(&RECORDING)) {
        Ok(measurements) => measurements,
        Err(_) => panic!("the recording shows one known behavior of every instruction"),
    }
}

/// Each recorded instruction with a state before and the state the chip gave after.
fn recorded_cases() -> impl Iterator<
    Item = (
        Instruction<Infallible>,
        ExecutionState<'static, Arm64<User>>,
        ExecutionState<'static, Arm64<User>>,
    ),
> {
    RECORDING
        .cases()
        .map(|(instruction, before, after)| (instruction, before.clone(), after.clone()))
}

wcontainers_precise_instruction_set_conformance::tests!(
    Measured,
    measure_recording(),
    recorded_cases()
);
