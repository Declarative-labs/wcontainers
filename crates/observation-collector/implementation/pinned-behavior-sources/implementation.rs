//! A collector that runs on every core a behavior source made on that core, the cores of one kind sharing one queue.
//!
//! Each run measures the next instruction of its kind's queue on every generated state until the queue is empty. When
//! runs fail, the collection gives the failure of the kind listed first, at the instruction listed first: a run stops
//! before its next instruction once a run of an earlier kind, or of its kind at an earlier instruction, has failed, and
//! every run measuring an earlier instruction goes on, so two collections on one machine give the same failure.

use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_observation_collector::{CoreSample, ObservationCollector};
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

/// Where a run failed: the place of its kind among the kinds as the cores list them, then the listed position of the
/// instruction, none for the making of the source, which comes before every instruction.
type FailurePlace = (usize, Option<usize>);

/// The queue of each kind of core, in the order the cores list the kinds: its identity and the position in the listed
/// instructions its runs take next.
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
    ) -> Result<Vec<CoreSample<Source::Architecture, Cores::CoreIdentity>>, Self::Error> {
        let instructions: Vec<_> = Source::Architecture::list_instructions().collect();
        let listed = self
            .cores
            .list_cores()
            .map_err(CollectSamplesError::CoresNotListed)?;
        let queues = Queues::new(Vec::new());
        for (_, core) in &listed {
            join_queue(&queues, core);
        }
        let lowest_failure = Mutex::new(None);
        let runs = self
            .cores
            .run_on_all_cores(|core| {
                let (kind, queue) = join_queue(&queues, &core);
                let measured =
                    self.measure_from_queue(&core, kind, &instructions, &queue, &lowest_failure);
                (core, measured)
            })
            .map_err(|error| match error {
                RunOnAllCoresError::CoresNotListed(error) => {
                    CollectSamplesError::CoresNotListed(error)
                }
                RunOnAllCoresError::CoreNotHeld(error) => CollectSamplesError::CoreNotHeld(error),
            })?;
        let mut kinds: Vec<(Cores::CoreIdentity, Vec<(usize, _)>)> = Vec::new();
        let mut failure: Option<(FailurePlace, Cores::CoreIdentity, RunFailure<_, _, _>)> = None;
        for (core, measured) in runs {
            match measured {
                Ok(measured) => match kinds.iter_mut().find(|(kind, _)| *kind == core) {
                    Some((_, kind_measured)) => kind_measured.extend(measured),
                    None => kinds.push((core, measured)),
                },
                // Of failures at one place, only making a source can give more than one: the first core's is kept.
                Err((place, run_failure)) => {
                    if failure.as_ref().is_none_or(|(lowest, ..)| place < *lowest) {
                        failure = Some((place, core, run_failure));
                    }
                }
            }
        }
        if let Some((_, core, run_failure)) = failure {
            return Err(match run_failure {
                RunFailure::SourceNotMade(error) => {
                    CollectSamplesError::SourceNotMade { core, error }
                }
                RunFailure::MeasurementFailed(error) => {
                    CollectSamplesError::MeasurementFailed { core, error }
                }
            });
        }
        // Every run of a kind ends only when its queue is empty, so its positions are each listed position once.
        Ok(kinds
            .into_iter()
            .map(|(core, mut measured)| {
                measured.sort_unstable_by_key(|(position, _)| *position);
                CoreSample {
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
    /// Measures queued instructions with their listed positions until none is left or a run has failed before them.
    ///
    /// A failure comes with its place, which it also leaves in `lowest_failure` if no run failed before it.
    fn measure_from_queue<Source, MakeSourceError>(
        &self,
        core: &Cores::CoreIdentity,
        kind: usize,
        instructions: &[<Source::Architecture as ProcessorArchitecture>::Instruction],
        queue: &AtomicUsize,
        lowest_failure: &Mutex<Option<FailurePlace>>,
    ) -> Result<
        Vec<(
            usize,
            <Source::Architecture as ProcessorArchitecture>::PreciseInstruction,
        )>,
        (
            FailurePlace,
            RunFailure<MakeSourceError, Source::Architecture, Source::Error>,
        ),
    >
    where
        Cores: ProcessorCores,
        MakeSource: Fn(Cores::CoreIdentity) -> Result<Source, MakeSourceError>,
        Source: BehaviorSource,
        <Source::Architecture as ProcessorArchitecture>::PreciseInstruction:
            Simulator<Source::Architecture, Error = Infallible>,
        Generator: DistinguishingStateGenerator<Source::Architecture>,
    {
        let fail = |place: FailurePlace| {
            let mut lowest = lowest_failure
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if lowest.is_none_or(|lowest| place < lowest) {
                *lowest = Some(place);
            }
            place
        };
        let mut measured = Vec::new();
        let making = (kind, None);
        if is_preceded(lowest_failure, making) {
            return Ok(measured);
        }
        let mut source = (self.make_source)(core.clone())
            .map_err(|error| (fail(making), RunFailure::SourceNotMade(error)))?;
        loop {
            let position = queue.fetch_add(1, Ordering::Relaxed);
            let Some(&instruction) = instructions.get(position) else {
                break;
            };
            let place = (kind, Some(position));
            if is_preceded(lowest_failure, place) {
                break;
            }
            let precise = source
                .measure_instruction(instruction, &self.generator)
                .map_err(|error| (fail(place), RunFailure::MeasurementFailed(error)))?;
            measured.push((position, precise));
        }
        Ok(measured)
    }
}

/// Whether a run has failed at a place before this one, so that what happens here cannot be the collection's failure.
fn is_preceded(lowest_failure: &Mutex<Option<FailurePlace>>, place: FailurePlace) -> bool {
    lowest_failure
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|lowest| lowest < place)
}

/// Returns the place of the core's kind and its queue, made with no instruction taken for the first core of the kind.
fn join_queue<CoreIdentity: Clone + Eq>(
    queues: &Queues<CoreIdentity>,
    core: &CoreIdentity,
) -> (usize, Arc<AtomicUsize>) {
    let mut queues = queues.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(place) = queues.iter().position(|(kind, _)| kind == core) {
        return (place, Arc::clone(&queues[place].1));
    }
    let queue = Arc::new(AtomicUsize::new(usize::MIN));
    queues.push((core.clone(), Arc::clone(&queue)));
    (queues.len() - 1, queue)
}
