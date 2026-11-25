//! A stand-in for an ARM64 chip, for tests that need a chip whose behaviors are known.

use std::convert::Infallible;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use wcontainers_abi::Access;
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, TrapHandler,
};
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::{
    AccessDirection, Arm64, Exception, PreciseInstruction, Processor, User,
};

mod memory;
#[cfg(test)]
mod tests;

use memory::{SpaceCopy, SpacePages, apply, create_space, refresh};

/// A chip that runs each instruction as the set's precise instruction simulates it.
pub struct Simulation<Set> {
    set: Set,
}

impl<Set: PreciseInstructionSet<Arm64<User>>> Simulation<Set> {
    /// Returns a chip that runs each instruction as the set simulates it.
    ///
    /// TODO: temporary placeholder until the executor interface says how an executor is built.
    pub const fn new(set: Set) -> Self {
        Self { set }
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

/// What the executor does when an instruction raised its exception.
///
/// No listed instruction makes a system call, so the simulation never gives the handler one.
enum Trap {
    MemoryFault {
        address: u64,
        access: Access,
    },
    /// The exception always ends execution.
    Final,
}

impl<Set: PreciseInstructionSet<Arm64<User>>> BehaviorSource for Simulation<Set> {
    type Architecture = Arm64<User>;
    type Error = Infallible;

    /// Runs the instruction at the program counter, taking a word none of the listed instructions match as undefined.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), Infallible> {
        let word = u32::from_le_bytes(read_bytes(memory.mutations, processor.program_counter));
        let listed = Arm64::<User>::list_instructions().find(|instruction| {
            let fixed = instruction.to_fixed_bits();
            word & fixed.mask == fixed.value
        });
        if let Some(instruction) = listed {
            // ARM64 expects every value of the processor, so the expected processor is the one the chip ends with.
            let Ok(after) =
                self.set
                    .to_precise(instruction)
                    .simulate(processor, memory, mutation_hook);
            *processor = after;
            return Ok(());
        }
        // A breakpoint changes nothing but the exception, so it gives the fault of the fetch, if any.
        let Ok(fetched) = PreciseInstruction::Brk.simulate(processor, memory, &mut DroppedChanges);
        let fetch_fault = fetched
            .exception
            .filter(|exception| !matches!(exception, Exception::Breakpoint { .. }));
        processor.exception = Some(fetch_fault.unwrap_or(Exception::UndefinedInstruction));
        Ok(())
    }
}

impl<Set: PreciseInstructionSet<Arm64<User>>> Executor for Simulation<Set> {
    type TranslatorConfig = ();
    type GuestMemory = SimulatedMemory;
    type Interrupter = SimulationInterrupter;
    type CreateGuestMemoryError = Infallible;

    fn create_guest_memory(&self) -> Result<(SimulatedMemory, SimulatedPageTable), Infallible> {
        Ok(create_space())
    }

    /// Runs one instruction at a time on a copy of the memory, which sees what other threads wrote.
    ///
    /// The copy is taken anew whenever anything but the instruction before changed a space. The execution blocks: it runs to its end in its first poll, answering an interrupt between two instructions.
    fn execute<'execution>(
        &'execution mut self,
        mut processor: Processor<()>,
        memory: &'execution SimulatedMemory,
        handler: &'execution mut impl TrapHandler,
    ) -> (
        impl Future<Output = Processor<()>> + 'execution,
        SimulationInterrupter,
    ) {
        let interrupter = SimulationInterrupter {
            raised: Arc::default(),
        };
        let raised = Arc::clone(&interrupter.raised);
        let execution = async move {
            let mut copy = SpaceCopy::default();
            let mut changes: Vec<MemoryDelta<'static, Arm64<User>>> = Vec::new();
            loop {
                if raised.swap(false, Ordering::Acquire) {
                    match handler.handle_interrupt() {
                        InterruptResponse::Continue => {}
                        InterruptResponse::Terminate => return processor,
                    }
                }
                let before = processor.clone();
                refresh(memory, &mut copy);
                let Ok(()) = self.execute_single_instruction_in_place(
                    &mut processor,
                    MemorySnapshot {
                        mutations: copy.deltas(),
                    },
                    &mut changes,
                );
                for delta in changes.drain(..) {
                    apply(memory, delta, &mut copy);
                }
                let Some(exception) = processor.exception else {
                    continue;
                };
                match to_trap(exception) {
                    Trap::MemoryFault { address, access } => {
                        match handler.handle_memory_fault(address, access) {
                            MemoryFaultResponse::Retry => processor = before,
                            MemoryFaultResponse::Terminate => return processor,
                        }
                    }
                    Trap::Final => return processor,
                }
            }
        };
        (execution, interrupter)
    }
}

fn to_trap(exception: Exception) -> Trap {
    match exception {
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
        | Exception::Undecoded { .. } => Trap::Final,
    }
}
