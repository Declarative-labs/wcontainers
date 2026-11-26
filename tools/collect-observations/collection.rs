//! The collection on the Linux cores of this machine, whatever its architecture, and the writing of what it gives.

use std::convert::Infallible;
use std::fmt::Debug;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use databake::Bake;
use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_machine_labels::{MachineLabels, read_machine_labels};
use wcontainers_observation_collector::ObservationCollector;
use wcontainers_observation_collector_pinned_behavior_sources::PinnedBehaviorSourcesObservationCollector;
use wcontainers_precise_instruction_set_collected::bake_sample;
use wcontainers_processor_architecture::{ProcessorArchitecture, Simulator};
use wcontainers_processor_cores::ProcessorCores;
use wcontainers_processor_cores_linux::LinuxProcessorCores;

use crate::counted_states::CountedGenerator;
use crate::failure::describe_failure;

/// What a core of this machine says it is.
pub type CoreIdentity = <LinuxProcessorCores as ProcessorCores>::CoreIdentity;

/// The file beside the baked files that holds what the core did when the collection failed.
const FAILURE_EVIDENCE: &str = "collection-failure.txt";
const BAKED_EXTENSION: &str = "rs";

/// Collects a sample of each kind of core with a source made on each core, writes each to `<output>/<kind>.rs`, and
/// returns the status the command exits with.
pub fn collect_and_write<Source, MakeSourceError, Generator>(
    output: &Path,
    make_source: impl Fn(CoreIdentity) -> Result<Source, MakeSourceError> + Sync,
    generator: Generator,
    name_kind: fn(&CoreIdentity) -> String,
) -> ExitCode
where
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::Instruction: Bake + Debug + Send + Sync,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Bake + Debug + Simulator<Source::Architecture, Error = Infallible> + Send,
    <Source::Architecture as ProcessorArchitecture>::ProcessorState: Debug,
    <Source::Architecture as ProcessorArchitecture>::ProcessorStateExpectation: Debug,
    <Source::Architecture as ProcessorArchitecture>::PagePermission: Debug,
    Source::Error: Debug,
    MeasureInstructionError<Source::Architecture, Source::Error>: Send,
    MakeSourceError: Debug + Send,
    Generator: DistinguishingStateGenerator<Source::Architecture> + Sync,
    CoreIdentity: Bake,
{
    let labels = describe_labels(&read_machine_labels());
    match LinuxProcessorCores.list_cores() {
        Ok(cores) => print_kinds(&cores, name_kind),
        Err(error) => {
            eprintln!("collect-observations: the cores could not be listed: {error:?}");
            return ExitCode::FAILURE;
        }
    }
    let states = AtomicU64::new(u64::MIN);
    let collector = PinnedBehaviorSourcesObservationCollector::new(
        LinuxProcessorCores,
        make_source,
        CountedGenerator {
            generator,
            states: &states,
        },
    );
    let started = Instant::now();
    let collected = collector.collect_samples();
    println!(
        "measured {} states in {:.3} s",
        states.load(Ordering::Relaxed),
        started.elapsed().as_secs_f64()
    );
    if let Err(error) = fs::create_dir_all(output) {
        eprintln!(
            "collect-observations: {} could not be made: {error}",
            output.display()
        );
        return ExitCode::FAILURE;
    }
    let samples = match collected {
        Ok(samples) => samples,
        Err(error) => {
            let (summary, evidence) = describe_failure(&error, name_kind);
            let evidence_file = output.join(FAILURE_EVIDENCE);
            if let Err(write_error) = fs::write(&evidence_file, evidence) {
                eprintln!(
                    "collect-observations: {summary}; the evidence could not be written: {write_error}"
                );
                return ExitCode::FAILURE;
            }
            eprintln!(
                "collect-observations: {summary}; the evidence is in {}",
                evidence_file.display()
            );
            return ExitCode::FAILURE;
        }
    };
    for sample in &samples {
        let baked_file = output
            .join(name_kind(&sample.core))
            .with_extension(BAKED_EXTENSION);
        let written = bake_sample(sample, &labels)
            .map_err(|error| format!("{error:?}"))
            .and_then(|source| fs::write(&baked_file, source).map_err(|error| error.to_string()));
        match written {
            Ok(()) => println!(
                "wrote {}: {} instructions",
                baked_file.display(),
                sample.measurements.len()
            ),
            Err(error) => {
                eprintln!(
                    "collect-observations: {} could not be written: {error}",
                    baked_file.display()
                );
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

/// Prints how many cores the machine has and each kind with its count of cores.
fn print_kinds<Core>(cores: &[(Core, CoreIdentity)], name_kind: fn(&CoreIdentity) -> String) {
    let mut kinds: Vec<(&CoreIdentity, usize)> = Vec::new();
    for (_, identity) in cores {
        match kinds.iter_mut().find(|(kind, _)| *kind == identity) {
            Some((_, count)) => *count += 1,
            None => kinds.push((identity, 1)),
        }
    }
    println!("{} cores of {} kinds", cores.len(), kinds.len());
    for (identity, count) in kinds {
        println!("kind {}: {count} cores, {identity:?}", name_kind(identity));
    }
}

/// Returns the comment lines of a baked file: where it came from, then each label of the machine that is known.
pub fn describe_labels(labels: &MachineLabels) -> Vec<String> {
    let known = [
        ("operating system", &labels.operating_system),
        ("operating system build", &labels.operating_system_build),
        ("kernel", &labels.kernel),
        ("hypervisor", &labels.hypervisor),
        ("underlying hypervisor", &labels.underlying_hypervisor),
        ("firmware", &labels.firmware),
        ("machine model", &labels.machine_model),
    ];
    let mut lines = Vec::from([String::from(
        "Written by tools/collect-observations. The lines below describe the machine, not the core.",
    )]);
    lines.extend(
        known
            .into_iter()
            .filter_map(|(name, value)| value.as_ref().map(|value| format!("{name}: {value}"))),
    );
    lines.push(format!("date: {}", labels.date));
    lines
}
