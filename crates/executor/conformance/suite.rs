//! The tests every implementation of wcontainers-executor runs, written against its traits alone.

use std::convert::Infallible;
use std::fmt::{Debug, Write};

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_executor::Executor;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};

/// On every state the generator makes for every instruction, the executor ends where the precise instruction set
/// says.
///
/// Memory is compared by the bytes and permissions it ends with, not by the list of changes: an executor reports
/// a store of the bytes already there as no change, where a simulation may report it.
pub fn agrees_with_precise_instruction_set<Chip, Set, Generator>(
    mut executor: Chip,
    set: Set,
    generator: Generator,
) where
    Chip: Executor,
    Chip::Architecture: Clone,
    <Chip::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Chip::Architecture, Error = Infallible>,
    <Chip::Architecture as ProcessorArchitecture>::ProcessorState: Debug,
    <Chip::Architecture as ProcessorArchitecture>::Instruction: Debug,
    <Chip::Architecture as ProcessorArchitecture>::PagePermission: Debug,
    Set: PreciseInstructionSet<Chip::Architecture>,
    Generator: DistinguishingStateGenerator<Chip::Architecture>,
{
    let mut report = String::new();
    for instruction in Chip::Architecture::list_instructions() {
        let precise = set.to_precise(instruction);
        let mut states = 0;
        let mut disagreeing = 0;
        let mut first = None;
        for before in generator.generate_states(instruction) {
            states += 1;
            let Ok(simulated) = precise.simulate(before.clone());
            let executed = executor.execute_single_instruction(before.clone());
            if simulated != executed {
                disagreeing += 1;
                first.get_or_insert_with(|| describe(&before, &simulated, &executed));
            }
        }
        assert!(
            states > 0,
            "the generator gives no state for {instruction:?}"
        );
        if let Some(first) = first {
            let _ = writeln!(
                report,
                "{instruction:?}: {disagreeing} of {states} states disagree; the first:\n{first}"
            );
        }
    }
    assert!(report.is_empty(), "{report}");
}

fn describe<Architecture>(
    before: &ExecutionState<'_, Architecture>,
    simulated: &ExecutionState<'_, Architecture>,
    executed: &ExecutionState<'_, Architecture>,
) -> String
where
    Architecture: ProcessorArchitecture,
    Architecture::ProcessorState: Debug,
    Architecture::PagePermission: Debug,
{
    let start = before.memory_mutations.len();
    format!(
        "  before: {:?}\n    {}\n  simulated: {:?}\n    {}\n  executed: {:?}\n    {}",
        before.processor,
        describe_deltas(&before.memory_mutations),
        simulated.processor,
        describe_deltas(&simulated.memory_mutations[start..]),
        executed.processor,
        describe_deltas(&executed.memory_mutations[start..]),
    )
}

fn describe_deltas<Architecture>(deltas: &[MemoryDelta<'_, Architecture>]) -> String
where
    Architecture: ProcessorArchitecture,
    Architecture::PagePermission: Debug,
{
    let described: Vec<String> = deltas
        .iter()
        .map(|delta| match delta {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => format!("{addresses:#x?} becomes {permission:?}"),
            MemoryDelta::ValueAssignment { start, bytes } => {
                format!("{start:#x} gets {bytes:02x?}")
            }
        })
        .collect();
    if described.is_empty() {
        "memory unchanged".to_string()
    } else {
        described.join("; ")
    }
}

/// Runs each suite function above as its own test on an executor, the precise instruction set it should agree
/// with and the generator of the states, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($executor:expr, $set:expr, $generator:expr) => {
        #[test]
        fn agrees_with_precise_instruction_set() {
            $crate::agrees_with_precise_instruction_set($executor, $set, $generator);
        }
    };
}
