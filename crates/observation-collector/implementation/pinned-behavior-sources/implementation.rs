//! A collector that runs on every core a behavior source made on that core, the cores of one kind sharing one queue.
//!
//! Each run measures the next instruction of its kind's queue on every generated state until the queue is empty. The
//! first failure stops every run before its next instruction.

use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_observation_collector::{ObservationCollector, Sample};
use wcontainers_processor_architecture::{ProcessorArchitecture, Simulator};
use wcontainers_processor_cores::{ProcessorCores, RunOnAllCoresError};

/// The cores, a way to make a behavior source on the core a run is on, and the generator of the states to measure.
pub struct PinnedBehaviorSourcesObservationCollector<Cores, MakeSource, Generator> {
    cores: Cores,
    /// Called once in each run, on the core the run holds, with that core's identity.
    make_source: MakeSource,
    generator: Generator,
}

impl<Cores, MakeSource, Generator>
    PinnedBehaviorSourcesObservationCollector<Cores, MakeSource, Generator>
{
    pub const fn new(cores: Cores, make_source: MakeSource, generator: Generator) -> Self {
        Self {
            cores,
            make_source,
            generator,
        }
    }
}

/// A reason the samples could not be collected.
pub enum CollectSamplesError<
    CoreIdentity,
    ListCoresError,
    RunOnCoreError,
    MakeSourceError,
    Architecture: ProcessorArchitecture,
    ExecutionError,
> {
    CoresNotListed(ListCoresError),
    CoreNotHeld(RunOnCoreError),
    SourceNotMade {
        core: CoreIdentity,
        error: MakeSourceError,
    },
    /// A core of this identity showed no single known behavior for an instruction, or refused one of its states.
    MeasurementFailed {
        core: CoreIdentity,
        error: MeasureInstructionError<Architecture, ExecutionError>,
    },
}

/// The error of a collector built of these cores, sources and the architecture of the sources.
type CollectorError<Cores, MakeSourceError, Source> = CollectSamplesError<
    <Cores as ProcessorCores>::CoreIdentity,
    <Cores as ProcessorCores>::ListCoresError,
    <Cores as ProcessorCores>::RunOnCoreError,
    MakeSourceError,
    <Source as BehaviorSource>::Architecture,
    <Source as BehaviorSource>::Error,
>;

/// A reason one run stopped, which the collection gives with the run's core.
enum RunFailure<MakeSourceError, Architecture: ProcessorArchitecture, ExecutionError> {
    SourceNotMade(MakeSourceError),
    MeasurementFailed(MeasureInstructionError<Architecture, ExecutionError>),
}

/// The queue of each kind of core: its identity and the position in the listed instructions its runs take next.
type Queues<CoreIdentity> = Mutex<Vec<(CoreIdentity, Arc<AtomicUsize>)>>;

impl<Cores, MakeSource, Source, MakeSourceError, Generator> ObservationCollector
    for PinnedBehaviorSourcesObservationCollector<Cores, MakeSource, Generator>
where
    Cores: ProcessorCores,
    MakeSource: Fn(Cores::CoreIdentity) -> Result<Source, MakeSourceError> + Sync,
    Source: BehaviorSource,
    <Source::Architecture as ProcessorArchitecture>::Instruction: Send + Sync,
    <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
        Simulator<Source::Architecture, Error = Infallible> + Send,
    MeasureInstructionError<Source::Architecture, Source::Error>: Send,
    MakeSourceError: Send,
    Generator: DistinguishingStateGenerator<Source::Architecture> + Sync,
{
    type Architecture = Source::Architecture;
    type CoreIdentity = Cores::CoreIdentity;
    type Error = CollectorError<Cores, MakeSourceError, Source>;

    /// Returns the samples in the order the cores list their kinds first.
    fn collect_samples(
        &self,
    ) -> Result<Vec<Sample<Source::Architecture, Cores::CoreIdentity>>, Self::Error> {
        let instructions: Vec<_> = Source::Architecture::list_instructions().collect();
        let queues = Queues::new(Vec::new());
        let failed = AtomicBool::new(false);
        let runs = self
            .cores
            .run_on_all_cores(|core| {
                let queue = join_queue(&queues, &core);
                let measured = self.measure_from_queue(&core, &instructions, &queue, &failed);
                if measured.is_err() {
                    failed.store(true, Ordering::Relaxed);
                }
                (core, measured)
            })
            .map_err(|error| match error {
                RunOnAllCoresError::CoresNotListed(error) => {
                    CollectSamplesError::CoresNotListed(error)
                }
                RunOnAllCoresError::CoreNotHeld(error) => CollectSamplesError::CoreNotHeld(error),
            })?;
        let mut kinds: Vec<(Cores::CoreIdentity, Vec<(usize, _)>)> = Vec::new();
        for (core, measured) in runs {
            let measured = measured.map_err(|failure| match failure {
                RunFailure::SourceNotMade(error) => CollectSamplesError::SourceNotMade {
                    core: core.clone(),
                    error,
                },
                RunFailure::MeasurementFailed(error) => CollectSamplesError::MeasurementFailed {
                    core: core.clone(),
                    error,
                },
            })?;
            match kinds.iter_mut().find(|(kind, _)| *kind == core) {
                Some((_, kind_measured)) => kind_measured.extend(measured),
                None => kinds.push((core, measured)),
            }
        }
        // Every run of a kind ends only when its queue is empty, so its positions are each listed position once.
        Ok(kinds
            .into_iter()
            .map(|(core, mut measured)| {
                measured.sort_unstable_by_key(|(position, _)| *position);
                Sample {
                    core,
                    measurements: measured
                        .into_iter()
                        .map(|(position, precise)| (instructions[position], precise))
                        .collect(),
                }
            })
            .collect())
    }
}

impl<Cores, MakeSource, Generator>
    PinnedBehaviorSourcesObservationCollector<Cores, MakeSource, Generator>
{
    /// Measures queued instructions with their listed positions until none is left or another run has failed.
    fn measure_from_queue<Source, MakeSourceError>(
        &self,
        core: &Cores::CoreIdentity,
        instructions: &[<Source::Architecture as ProcessorArchitecture>::Instruction],
        queue: &AtomicUsize,
        failed: &AtomicBool,
    ) -> Result<
        Vec<(
            usize,
            <Source::Architecture as ProcessorArchitecture>::PreciseInstruction,
        )>,
        RunFailure<MakeSourceError, Source::Architecture, Source::Error>,
    >
    where
        Cores: ProcessorCores,
        MakeSource: Fn(Cores::CoreIdentity) -> Result<Source, MakeSourceError>,
        Source: BehaviorSource,
        <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Source::Architecture, Error = Infallible>,
        Generator: DistinguishingStateGenerator<Source::Architecture>,
    {
        let mut source = (self.make_source)(core.clone()).map_err(RunFailure::SourceNotMade)?;
        let mut measured = Vec::new();
        while !failed.load(Ordering::Relaxed) {
            let position = queue.fetch_add(1, Ordering::Relaxed);
            let Some(&instruction) = instructions.get(position) else {
                break;
            };
            let precise = source
                .measure_instruction(instruction, &self.generator)
                .map_err(RunFailure::MeasurementFailed)?;
            measured.push((position, precise));
        }
        Ok(measured)
    }
}

/// Returns the queue of the core's kind, made with no instruction taken for the first run of the kind.
fn join_queue<CoreIdentity: Clone + Eq>(
    queues: &Queues<CoreIdentity>,
    core: &CoreIdentity,
) -> Arc<AtomicUsize> {
    let mut queues = queues.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, queue)) = queues.iter().find(|(kind, _)| kind == core) {
        return Arc::clone(queue);
    }
    let queue = Arc::new(AtomicUsize::new(usize::MIN));
    queues.push((core.clone(), Arc::clone(&queue)));
    queue
}
