//! What a chip gave after each state a generator gives for one instruction, kept without the states before.
//!
//! A whole set's states before take far more room than what the chip changed, and the generator makes them again in
//! the same order, so a state before is held only as its fingerprint.

use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, ExclusiveMonitor, Instruction, InstructionFetch, PagePermission, Processor,
    User,
};

use crate::text::{self, LongRuns, MalformedRecording};

/// The chip's state after each generated state of one instruction, in the generator's order.
pub(crate) struct GeneratedCases {
    /// The fingerprint of each state before.
    fingerprints: Vec<u64>,
    /// For each state, the place of what the chip gave in `after_lines`.
    after_line_of_state: Vec<usize>,
    /// Each distinct `after` line, which names a run of `long_runs` by its number.
    after_lines: Vec<Box<str>>,
    long_runs: Vec<Vec<u8>>,
}

/// Why a file made from a generator does not hold the generator's states.
#[derive(Debug)]
pub(crate) enum UnreadableGeneratedCases {
    Malformed(MalformedRecording),
    /// The file holds this many states and the generator gives the other number.
    OtherStateCount {
        written: usize,
        generated: usize,
    },
    /// The `after` line of the state, counted from zero, does not apply to the generator's state before.
    UnappliedLine(usize),
}

impl fmt::Display for UnreadableGeneratedCases {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(malformed) => write!(formatter, "{malformed:?}"),
            Self::OtherStateCount { written, generated } => write!(
                formatter,
                "the file holds {written} states and the generator gives {generated}"
            ),
            Self::UnappliedLine(state) => write!(
                formatter,
                "the after line of state {state} does not apply to the generator's state"
            ),
        }
    }
}

impl GeneratedCases {
    /// Runs every state the generator gives for the instruction on the source, up to the first error.
    pub(crate) fn record<Source: BehaviorSource<Architecture = Arm64<User>>>(
        source: &mut Source,
        generator: &impl DistinguishingStateGenerator<Arm64<User>>,
        instruction: Instruction<Infallible>,
    ) -> Result<Self, Source::Error> {
        let mut long_runs = LongRuns::default();
        let mut lines = Interned::default();
        let mut fingerprints = Vec::new();
        let mut after_line_of_state = Vec::new();
        for before in generator.generate_states(instruction) {
            let mut processor = before.processor.clone();
            let mut made = Vec::new();
            source.execute_single_instruction_in_place(
                &mut processor,
                before.memory(),
                &mut made,
            )?;
            let line = text::to_after_line(&before.processor, &processor, &made, &mut long_runs);
            fingerprints.push(to_fingerprint(&before.processor, &before.memory_mutations));
            after_line_of_state.push(lines.intern(&line));
        }
        Ok(Self {
            fingerprints,
            after_line_of_state,
            after_lines: lines.lines,
            long_runs: long_runs.into_runs(),
        })
    }

    /// Reads the text `to_text` wrote and checks each line against the state the generator gives at its place.
    pub(crate) fn from_text(
        text: &str,
        generator: &impl DistinguishingStateGenerator<Arm64<User>>,
    ) -> Result<(Instruction<Infallible>, Self), UnreadableGeneratedCases> {
        let (instruction, long_runs, written_lines) =
            text::from_generated_text(text).map_err(UnreadableGeneratedCases::Malformed)?;
        let mut lines = Interned::default();
        let mut fingerprints = Vec::with_capacity(written_lines.len());
        let mut after_line_of_state = Vec::with_capacity(written_lines.len());
        let mut generated = 0;
        for (state, before) in generator.generate_states(instruction).enumerate() {
            generated += 1;
            let Some(line) = written_lines.get(state) else {
                continue;
            };
            text::apply_after_line(&before.processor, line, &long_runs)
                .ok_or(UnreadableGeneratedCases::UnappliedLine(state))?;
            fingerprints.push(to_fingerprint(&before.processor, &before.memory_mutations));
            after_line_of_state.push(lines.intern(line));
        }
        if generated != written_lines.len() {
            return Err(UnreadableGeneratedCases::OtherStateCount {
                written: written_lines.len(),
                generated,
            });
        }
        Ok((
            instruction,
            Self {
                fingerprints,
                after_line_of_state,
                after_lines: lines.lines,
                long_runs,
            },
        ))
    }

    /// Returns the text of the recording, one `after` line for each generated state.
    pub(crate) fn to_text(&self, instruction: Instruction<Infallible>) -> String {
        let lines = self
            .after_line_of_state
            .iter()
            .map(|&place| &*self.after_lines[place]);
        text::to_generated_text(instruction, &self.long_runs, lines)
    }

    pub(crate) fn len(&self) -> usize {
        self.fingerprints.len()
    }

    /// Whether the case was recorded from the state with this fingerprint.
    pub(crate) fn is_recorded_from(&self, case: usize, fingerprint: u64) -> bool {
        self.fingerprints.get(case) == Some(&fingerprint)
    }

    /// Returns the place of the case recorded from the state with this fingerprint.
    pub(crate) fn find(&self, fingerprint: u64) -> Option<usize> {
        self.fingerprints
            .iter()
            .position(|recorded| *recorded == fingerprint)
    }

    /// Returns the processor the chip gave from the case's state before, which is the processor given, and the memory
    /// changes it made.
    pub(crate) fn play(
        &self,
        case: usize,
        before: &Processor<()>,
    ) -> Option<(Processor<()>, Vec<MemoryDelta<'static, Arm64<User>>>)> {
        let place = *self.after_line_of_state.get(case)?;
        text::apply_after_line(before, &self.after_lines[place], &self.long_runs)
    }

    /// Returns each generated state before with the state the chip gave after, in order.
    pub(crate) fn generate_cases<'cases>(
        &'cases self,
        generator: &'cases impl DistinguishingStateGenerator<Arm64<User>>,
        instruction: Instruction<Infallible>,
    ) -> impl Iterator<
        Item = (
            ExecutionState<'static, Arm64<User>>,
            ExecutionState<'static, Arm64<User>>,
        ),
    > + 'cases {
        generator
            .generate_states(instruction)
            .enumerate()
            .filter_map(|(case, before)| {
                let (processor, made) = self.play(case, &before.processor)?;
                let mut memory_mutations = before.memory_mutations.clone();
                memory_mutations.extend(made);
                Some((
                    before,
                    ExecutionState {
                        processor,
                        memory_mutations,
                    },
                ))
            })
    }
}

/// Each distinct line once, with its place.
#[derive(Default)]
struct Interned {
    lines: Vec<Box<str>>,
    places: HashMap<Box<str>, usize>,
}

impl Interned {
    fn intern(&mut self, line: &str) -> usize {
        if let Some(place) = self.places.get(line) {
            return *place;
        }
        let place = self.lines.len();
        self.lines.push(line.into());
        self.places.insert(line.into(), place);
        place
    }
}

/// Returns a hash of every value of the state, which tells two states apart unless 64 bits collide.
pub(crate) fn to_fingerprint(
    processor: &Processor<()>,
    memory: &[MemoryDelta<'_, Arm64<User>>],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    let Processor {
        general_purpose_registers,
        stack_pointer,
        program_counter,
        condition_flags,
        vector_registers,
        predicate_registers,
        first_fault_register,
        floating_point_control,
        floating_point_status,
        software_thread_id,
        exclusive_monitor,
        exception,
        level_specific: (),
    } = processor;
    general_purpose_registers.hash(&mut hasher);
    stack_pointer.hash(&mut hasher);
    program_counter.hash(&mut hasher);
    condition_flags.bits().hash(&mut hasher);
    vector_registers.hash(&mut hasher);
    predicate_registers.hash(&mut hasher);
    first_fault_register.hash(&mut hasher);
    floating_point_control.hash(&mut hasher);
    floating_point_status.hash(&mut hasher);
    software_thread_id.hash(&mut hasher);
    match exclusive_monitor {
        ExclusiveMonitor::Open => None,
        ExclusiveMonitor::Exclusive { address } => Some(address),
    }
    .hash(&mut hasher);
    exception
        .map(|exception| format!("{exception:?}"))
        .hash(&mut hasher);
    for mutation in memory {
        match mutation {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => (
                0_u8,
                addresses.start,
                addresses.end,
                to_permission_number(*permission),
            )
                .hash(&mut hasher),
            MemoryDelta::ValueAssignment { start, bytes } => {
                (1_u8, start, &bytes[..]).hash(&mut hasher)
            }
        }
    }
    hasher.finish()
}

/// Gives each permission a number of its own.
fn to_permission_number(permission: PagePermission) -> u8 {
    match permission {
        PagePermission::Unmapped => 0,
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data_access = match data_access {
                DataAccess::Forbidden => 1,
                DataAccess::ReadOnly => 2,
                DataAccess::ReadWrite => 3,
            };
            match instruction_fetch {
                InstructionFetch::Forbidden => data_access,
                InstructionFetch::Allowed => data_access + 4,
            }
        }
    }
}
