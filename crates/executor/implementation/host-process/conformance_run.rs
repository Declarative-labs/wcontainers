//! The executor interface's suite run on form P against what a Neoverse V2 did, each test with an executor of its own
//! on one host per test process.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_behavior_source_recording::Recording;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_host_process::executor::HostProcess;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ExecutionState;
use wcontainers_processor_architecture_arm64::{
    Arm64, ExclusiveMonitor, Processor, User, ZERO_PROCESSOR,
};

/// What a Neoverse V2 under KVM did, written by the record tool: form P runs on that chip in the bar's machine.
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../behavior-source/implementation/recording/recordings/neoverse-v2"
);

/// Where the refused state's exclusive monitor holds an address.
const HELD_ADDRESS: u64 = 0x1000;

static HOST: OnceLock<Arc<HostProcessHost>> = OnceLock::new();

fn executor() -> HostProcess {
    HostProcess::new(Arc::clone(HOST.get_or_init(|| {
        Arc::new(HostProcessHost::new().expect("the host is refused"))
    })))
}

/// Returns every instruction's behavior as the recording measures it.
fn measure_recording() -> MeasuredInstructionSet<Arm64<User>> {
    let mut recording = match Recording::read(Path::new(RECORDINGS)) {
        Ok(recording) => recording,
        Err(error) => panic!("reading {RECORDINGS}: {error}"),
    };
    match recording.measure_instructions(&CriticalValueCut::default()) {
        Ok(measurements) => MeasuredInstructionSet::from_measurements(&measurements),
        Err(_) => panic!("the recording shows one known behavior of every instruction"),
    }
}

/// Returns the one state form P refuses: an exclusive monitor that holds an address, which no host call sets in a
/// stub's thread.
fn to_unexecutable() -> Vec<ExecutionState<'static, Arm64<User>>> {
    vec![ExecutionState {
        processor: Processor {
            exclusive_monitor: ExclusiveMonitor::Exclusive {
                address: HELD_ADDRESS,
            },
            ..ZERO_PROCESSOR
        },
        memory_mutations: Vec::new(),
    }]
}

wcontainers_executor_conformance::tests!(
    executor(),
    measure_recording(),
    CriticalValueCut::default(),
    to_unexecutable()
);
