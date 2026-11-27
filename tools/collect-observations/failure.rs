//! What the command says and writes when the collection fails: one line, and the evidence the owner's error holds.

use std::fmt::Debug;
use std::fmt::Write;

use wcontainers_behavior_source::MeasureInstructionError;
use wcontainers_observation_collector_pinned_behavior_sources::CollectSamplesError;
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture};

use crate::collection::CoreIdentity;

const STATE_BEFORE: &str = "state before";
/// The title of what the core left, whose memory is only what it changed.
const CORE_RESULT: &str = "the core did (memory: changes after the state before's)";

/// Returns a line that says what failed, and the whole evidence for the file beside the output.
pub fn describe_failure<
    ListCoresError,
    RunOnCoreError,
    MakeSourceError,
    Architecture,
    ExecutionError,
>(
    error: &CollectSamplesError<
        CoreIdentity,
        ListCoresError,
        RunOnCoreError,
        MakeSourceError,
        Architecture,
        ExecutionError,
    >,
    name_kind: fn(&CoreIdentity) -> String,
) -> (String, String)
where
    ListCoresError: Debug,
    RunOnCoreError: Debug,
    MakeSourceError: Debug,
    Architecture: ProcessorArchitecture,
    Architecture::Instruction: Debug,
    Architecture::PreciseInstruction: Debug,
    Architecture::ProcessorState: Debug,
    Architecture::ProcessorStateExpectation: Debug,
    Architecture::PagePermission: Debug,
    ExecutionError: Debug,
{
    match error {
        CollectSamplesError::CoresNotListed(error) => {
            let summary = format!("the cores could not be listed: {error:?}");
            (summary.clone(), summary)
        }
        CollectSamplesError::CoreNotHeld(error) => {
            let summary = format!("a run could not be held on its core: {error:?}");
            (summary.clone(), summary)
        }
        CollectSamplesError::SourceNotMade { core, error } => {
            let summary = format!(
                "no behavior source could be made on a core of kind {}: {error:?}",
                name_kind(core)
            );
            (summary.clone(), format!("{summary}\ncore {core:#x?}\n"))
        }
        CollectSamplesError::MeasurementFailed { core, error } => {
            let (what, evidence) = describe_measurement_failure(error);
            (
                format!("on kind {}, {what}", name_kind(core)),
                format!(
                    "on kind {}, {what}\ncore {core:#x?}\n{evidence}",
                    name_kind(core)
                ),
            )
        }
    }
}

/// Returns what the measurement found, and the instruction, states and behaviors the error holds.
fn describe_measurement_failure<Architecture, ExecutionError>(
    error: &MeasureInstructionError<Architecture, ExecutionError>,
) -> (String, String)
where
    Architecture: ProcessorArchitecture,
    Architecture::Instruction: Debug,
    Architecture::PreciseInstruction: Debug,
    Architecture::ProcessorState: Debug,
    Architecture::ProcessorStateExpectation: Debug,
    Architecture::PagePermission: Debug,
    ExecutionError: Debug,
{
    let mut evidence = String::new();
    let what = match error {
        MeasureInstructionError::NoKnownBehavior { instruction } => {
            format!("the instruction {instruction:?} has no known behavior")
        }
        MeasureInstructionError::ExecutionFailed {
            instruction,
            before,
            error,
        } => {
            write_section(
                &mut evidence,
                STATE_BEFORE,
                &before.processor,
                &before.memory_mutations,
            );
            format!("the core refused a state of {instruction:?}: {error:?}")
        }
        MeasureInstructionError::NoBehaviorAgrees {
            instruction,
            before,
            behaviors,
            observed,
        } => {
            let before_changes = before.memory_mutations.len();
            write_section(
                &mut evidence,
                STATE_BEFORE,
                &before.processor,
                &before.memory_mutations,
            );
            for (behavior, expectation) in behaviors {
                write_section(
                    &mut evidence,
                    &format!("{behavior:?} expects (memory: changes after the state before's)"),
                    &expectation.processor,
                    expectation
                        .memory_mutations
                        .get(before_changes..)
                        .unwrap_or_default(),
                );
            }
            write_section(
                &mut evidence,
                CORE_RESULT,
                &observed.processor,
                observed
                    .memory_mutations
                    .get(before_changes..)
                    .unwrap_or_default(),
            );
            format!("no known behavior of {instruction:?} agrees with what the core did")
        }
        MeasureInstructionError::IndistinguishableBehaviors {
            instruction,
            behaviors,
        } => {
            format!("no generated state tells apart the behaviors {behaviors:?} of {instruction:?}")
        }
    };
    (what, evidence)
}

/// Writes a titled processor and the memory changes that go with it.
fn write_section<Architecture: ProcessorArchitecture>(
    evidence: &mut String,
    title: &str,
    processor: &impl Debug,
    memory: &[MemoryDelta<'_, Architecture>],
) where
    Architecture::PagePermission: Debug,
{
    // Writing to a string cannot fail.
    let _ = writeln!(evidence, "\n{title}:\nprocessor {processor:#x?}\nmemory:");
    for change in memory {
        let _ = match change {
            MemoryDelta::ValueAssignment { start, bytes } => {
                writeln!(evidence, "  {start:#x} gets {bytes:02x?}")
            }
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => writeln!(evidence, "  {addresses:#x?} becomes {permission:?}"),
        };
    }
}
