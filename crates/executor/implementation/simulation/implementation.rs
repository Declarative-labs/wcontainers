//! A stand-in for an ARM64 chip, for tests that need a chip whose behaviors are known.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::convert::Infallible;
use std::future::Ready;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use wcontainers_abi::{
    Access, HostError, SYSTEM_CALL_ARGUMENT_COUNT, SYSTEM_CALL_NUMBER_REGISTER, SystemCallNumber,
    SystemCallRequest,
};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, InterruptResponse,
    MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::encoding::FixedBits;
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, ConditionFlags, Exception, PreciseInstruction, Processor,
    ProcessorExpectation, RegisterValueExpectation, User,
};
use wcontainers_random_source::RandomSource;

mod memory;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;

use memory::{SpaceCopy, SpacePages, apply, create_space, refresh};

/// How many guest instructions an execution retires before the simulation asks its handler, unless configured otherwise.
pub const DEFAULT_SLICE_INSTRUCTIONS: u64 = 1_000_000;
/// What each instruction run takes from a slice.
const ONE_INSTRUCTION: u64 = 1;

/// What the simulation is configured with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimulationConfig {
    /// How many guest instructions an execution retires before it calls `TrapHandler::handle_interrupt`, as an
    /// interrupt from outside does, counted from its start and again after each interrupt.
    pub slice_instructions: u64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            slice_instructions: DEFAULT_SLICE_INSTRUCTIONS,
        }
    }
}

/// A chip that runs each instruction as the set simulates it, choosing what a behavior leaves undecided at random.
pub struct Simulation<Set, Random> {
    set: Set,
    random: Random,
    /// Every listed instruction's encoding, those that fix most bits first and otherwise in the list's order.
    encodings: Vec<Encoding>,
    config: SimulationConfig,
}

/// A listed instruction, the bits that tell its words, and whether the architecture knows a behavior of it.
struct Encoding {
    instruction: <Arm64<User> as ProcessorArchitecture>::Instruction,
    fixed: FixedBits,
    has_known_behavior: bool,
}

impl<Set: PreciseInstructionSet<Arm64<User>>, Random: RandomSource> Simulation<Set, Random> {
    /// Returns a chip that runs each instruction as the set simulates it, choosing undecided values from `random`.
    ///
    /// TODO: temporary placeholder until the executor interface says how an executor is built.
    pub fn new(set: Set, random: Random) -> Self {
        let mut encodings: Vec<Encoding> = Arm64::<User>::list_instructions()
            .map(|instruction| Encoding {
                instruction,
                fixed: instruction.to_fixed_bits(),
                has_known_behavior: Arm64::<User>::list_known_behaviors(instruction)
                    .next()
                    .is_some(),
            })
            .collect();
        encodings.sort_by_key(|encoding| Reverse(encoding.fixed.mask.count_ones()));
        Self {
            set,
            random,
            encodings,
            config: SimulationConfig::default(),
        }
    }

    /// The same chip with the configuration given.
    pub fn configured(mut self, config: SimulationConfig) -> Self {
        self.config = config;
        self
    }

    /// Returns the processor the expectation holds, each register it leaves undecided read from the random source.
    fn choose_processor(
        &self,
        expectation: ProcessorExpectation<()>,
    ) -> Result<Processor<()>, SimulationError> {
        let mut processor = expectation.processor;
        for (value, expected) in processor
            .general_purpose_registers
            .iter_mut()
            .zip(expectation.general_purpose_registers)
        {
            if expected == RegisterValueExpectation::NonDeterministic {
                *value = self.read_random_word()?;
            }
        }
        if expectation.condition_flags == RegisterValueExpectation::NonDeterministic {
            let [_, _, _, _, high @ ..] = self.read_random_word()?.to_le_bytes();
            processor.condition_flags =
                ConditionFlags::from_bits_truncate(u32::from_le_bytes(high));
        }
        Ok(processor)
    }

    /// Returns one word of the random source's bytes.
    fn read_random_word(&self) -> Result<u64, SimulationError> {
        let mut word = [u8::MIN; size_of::<u64>()];
        self.random
            .fill_random_bytes(&RefCell::new(word.as_mut_slice()))
            .map_err(SimulationError::RandomSourceFailed)?;
        Ok(u64::from_le_bytes(word))
    }
}

/// A guest's address space, every page of it owned or shared in host memory.
///
/// The page methods (`set_permissions`, `map_from`, `unmap_from` and `copy_from`) take whole pages: addresses that
/// do not start and end at a page boundary are refused. A copy copies the bytes at once rather than on a write.
pub struct SimulatedMemory {
    shared: Arc<SpacePages>,
}

/// The one page table of a [`SimulatedMemory`], made with it, which refuses every change once the space is gone.
pub struct SimulatedPageTable {
    shared: Weak<SpacePages>,
}

/// Addresses of a [`SimulatedMemory`], which another thread's guest may be changing while they are held.
pub struct SimulatedBytes<'space> {
    space: &'space SimulatedMemory,
    addresses: Range<u64>,
}

/// The interrupter of one execution, which it checks before each instruction.
pub struct SimulationInterrupter {
    /// Whether an interrupt is raised and not yet answered; once the execution ends no one reads it.
    raised: Arc<AtomicBool>,
}

impl ExecutionInterrupter for SimulationInterrupter {
    fn interrupt(&self) {
        self.raised.store(true, Ordering::Release);
    }
}

/// The hook of a run whose memory changes are not kept.
struct DroppedChanges;

impl MemorySink<Arm64<User>> for DroppedChanges {
    fn on_memory_mutated(&mut self, _: MemoryDelta<'_, Arm64<User>>) {}
}

/// Why the simulation could not execute an instruction.
#[derive(Debug)]
pub enum SimulationError {
    /// The architecture knows no behavior of the instruction at the program counter, so no set holds one.
    NoKnownBehavior,
    /// The random source failed to choose a value the instruction left undecided, after its memory changes.
    RandomSourceFailed(HostError),
}

/// What the executor does when an instruction raised its exception.
enum Trap {
    /// A supervisor call, which leaves the program counter at the instruction after it.
    SystemCall,
    MemoryFault {
        address: u64,
        access: Access,
    },
    /// The exception always ends execution.
    Final,
}

impl<Set: PreciseInstructionSet<Arm64<User>>, Random: RandomSource> BehaviorSource
    for Simulation<Set, Random>
{
    type Architecture = Arm64<User>;
    type Error = SimulationError;

    /// Runs the instruction at the program counter, taking a word none of the listed instructions match as undefined.
    ///
    /// Of the instructions whose encoding matches the word, the first that fixes most bits runs.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), SimulationError> {
        let word = u32::from_le_bytes(read_bytes(memory.mutations, processor.program_counter));
        // A word several listed encodings match, as CFINV and MSR (immediate), is the one that fixes most bits.
        let listed = self
            .encodings
            .iter()
            .find(|encoding| word & encoding.fixed.mask == encoding.fixed.value);
        if let Some(encoding) = listed {
            if !encoding.has_known_behavior {
                return Err(SimulationError::NoKnownBehavior);
            }
            let instruction = encoding.instruction;
            let Ok(expectation) =
                self.set
                    .to_precise(instruction)
                    .simulate(processor, memory, mutation_hook);
            *processor = self.choose_processor(expectation)?;
            return Ok(());
        }
        // A breakpoint changes nothing but the exception, so it gives the fault of the fetch, if any.
        let Ok(fetched) = PreciseInstruction::Brk.simulate(processor, memory, &mut DroppedChanges);
        let fetch_fault = fetched
            .processor
            .exception
            .filter(|exception| !matches!(exception, Exception::Breakpoint { .. }));
        processor.exception = Some(fetch_fault.unwrap_or(Exception::UndefinedInstruction));
        Ok(())
    }
}

impl<Set: PreciseInstructionSet<Arm64<User>>, Random: RandomSource> Executor
    for Simulation<Set, Random>
{
    /// No translator; the slice is set as the simulation is built ([`Simulation::configured`]).
    type TranslatorConfig = ();
    type GuestMemory = SimulatedMemory;
    type Interrupter = SimulationInterrupter;
    type CreateGuestMemoryError = Infallible;
    type Execution<'execution>
        = SimulationExecution<'execution, Set, Random>
    where
        Self: 'execution;

    fn create_guest_memory(&self) -> Result<(SimulatedMemory, SimulatedPageTable), Infallible> {
        Ok(create_space())
    }

    /// Returns the execution, which runs one instruction at a time on a copy of the memory that sees what other threads wrote.
    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: Processor<()>,
        memory: &'execution SimulatedMemory,
    ) -> (
        SimulationExecution<'execution, Set, Random>,
        SimulationInterrupter,
    ) {
        let interrupter = SimulationInterrupter {
            raised: Arc::default(),
        };
        let slice = self.config.slice_instructions;
        let execution = SimulationExecution {
            slice,
            left_in_slice: slice,
            simulation: self,
            processor,
            memory,
            raised: Arc::clone(&interrupter.raised),
        };
        (execution, interrupter)
    }
}

/// One execution of the simulation, not yet run.
pub struct SimulationExecution<'execution, Set, Random> {
    simulation: &'execution mut Simulation<Set, Random>,
    processor: Processor<()>,
    memory: &'execution SimulatedMemory,
    raised: Arc<AtomicBool>,
    /// The instructions of a slice, and how many of the current slice the guest has yet to retire.
    slice: u64,
    left_in_slice: u64,
}

/// Why the simulation stopped running instructions.
enum Stop {
    Interrupted,
    /// An instruction raised its exception.
    Raised(Exception),
}

impl<Set: PreciseInstructionSet<Arm64<User>>, Random: RandomSource> Execution
    for SimulationExecution<'_, Set, Random>
{
    type ProcessorState = Processor<()>;
    /// Never made, since the simulation never waits.
    type Wait = Ready<()>;

    /// Runs until the next trap, then asks the handler, until the handler ends the execution.
    ///
    /// The copy of the memory is taken anew whenever anything but the instruction before changed a space.
    fn run(mut self, handler: &mut impl TrapHandler) -> ExecutionStop<Processor<()>, Ready<()>> {
        let mut copy = SpaceCopy::default();
        // The processor before the last instruction run, which a retry goes back to.
        let mut before = self.processor.clone();
        loop {
            match self.run_until_trap(&mut copy, &mut before) {
                Stop::Interrupted => match handler.handle_interrupt() {
                    InterruptResponse::Continue => {}
                    InterruptResponse::Terminate => break,
                },
                Stop::Raised(exception) => match to_trap(exception) {
                    Trap::SystemCall => {
                        match handler.handle_system_call(to_request(&self.processor)) {
                            SystemCallResponse::Return(value) => {
                                let [result, ..] = &mut self.processor.general_purpose_registers;
                                *result = value;
                                self.processor.exception = None;
                            }
                            SystemCallResponse::Terminate => break,
                        }
                    }
                    Trap::MemoryFault { address, access } => {
                        match handler.handle_memory_fault(address, access) {
                            MemoryFaultResponse::Retry => self.processor = before.clone(),
                            MemoryFaultResponse::Terminate => break,
                        }
                    }
                    Trap::Final => break,
                },
            }
        }
        ExecutionStop::Terminated(self.processor)
    }
}

impl<Set: PreciseInstructionSet<Arm64<User>>, Random: RandomSource>
    SimulationExecution<'_, Set, Random>
{
    /// Runs instructions until one raises its exception, an interrupt is raised or the slice ends, each answered between
    /// two instructions.
    ///
    /// Before each instruction the processor is kept in `before`.
    fn run_until_trap(&mut self, copy: &mut SpaceCopy, before: &mut Processor<()>) -> Stop {
        let mut changes: Vec<MemoryDelta<'static, Arm64<User>>> = Vec::new();
        loop {
            if self.raised.swap(false, Ordering::Acquire) || self.left_in_slice == u64::MIN {
                self.left_in_slice = self.slice;
                return Stop::Interrupted;
            }
            self.left_in_slice -= ONE_INSTRUCTION;
            before.clone_from(&self.processor);
            refresh(self.memory, copy);
            let executed = self.simulation.execute_single_instruction_in_place(
                &mut self.processor,
                MemorySnapshot {
                    mutations: copy.deltas(),
                },
                &mut changes,
            );
            // The trap handler has no answer for either error, and a chip cannot go on past the instruction.
            if let Err(error) = executed {
                panic!("the simulation cannot execute the instruction: {error:?}");
            }
            for delta in changes.drain(..) {
                apply(self.memory, delta, copy);
            }
            if let Some(exception) = self.processor.exception {
                return Stop::Raised(exception);
            }
        }
    }
}

/// Returns the system call the processor makes: its number in x8 and its arguments from x0.
fn to_request(processor: &Processor<()>) -> SystemCallRequest {
    let mut arguments = [u64::MIN; SYSTEM_CALL_ARGUMENT_COUNT];
    arguments.copy_from_slice(&processor.general_purpose_registers[..SYSTEM_CALL_ARGUMENT_COUNT]);
    SystemCallRequest {
        number: SystemCallNumber(processor.general_purpose_registers[SYSTEM_CALL_NUMBER_REGISTER]),
        arguments,
    }
}

fn to_trap(exception: Exception) -> Trap {
    match exception {
        Exception::SupervisorCall { .. } => Trap::SystemCall,
        Exception::InstructionAbort { address, .. } => Trap::MemoryFault {
            address,
            access: Access::Execute,
        },
        Exception::DataAbort {
            access: AccessDirection::Read,
            address,
            ..
        } => Trap::MemoryFault {
            address,
            access: Access::Read,
        },
        Exception::DataAbort {
            access: AccessDirection::Write,
            address,
            ..
        } => Trap::MemoryFault {
            address,
            access: Access::Write,
        },
        Exception::UndefinedInstruction
        | Exception::ProgramCounterAlignmentFault { .. }
        | Exception::StackPointerAlignmentFault
        | Exception::Breakpoint { .. }
        | Exception::SmeTrap(_)
        | Exception::WaitTrapped(_)
        | Exception::SystemAccessTrapped(_)
        | Exception::Undecoded { .. } => Trap::Final,
    }
}
