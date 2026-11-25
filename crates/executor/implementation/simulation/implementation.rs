//! A stand-in for an ARM64 chip, for tests that need a chip whose behaviors are known.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_abi::{
    Access, SYSTEM_CALL_ARGUMENT_COUNT, SYSTEM_CALL_NUMBER_REGISTER, SystemCallNumber,
    SystemCallRequest,
};
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::syndrome::{UNDEFINED_INSTRUCTION, WRITE_NOT_READ};
use wcontainers_processor_architecture_arm64::{
    Arm64, Exception, INSTRUCTION_BYTES, Instruction, Processor,
};

mod memory;
#[cfg(test)]
mod tests;

use memory::{SimulatedMemory, SimulatedPageTable, SpaceCopy};

/// A chip that runs each instruction as the set's precise instruction simulates it.
pub struct Simulation<Set> {
    set: Set,
}

impl<Set: PreciseInstructionSet<Arm64>> Simulation<Set> {
    /// Returns a chip that runs each instruction as the set simulates it.
    ///
    /// TODO: temporary placeholder until the executor interface says how an executor is built.
    pub const fn new(set: Set) -> Self {
        Self { set }
    }
}

/// The interrupter of one execution, which it checks before each instruction.
#[derive(Default)]
pub struct SimulationInterrupter {
    /// Whether an interrupt is raised and not yet answered; once the execution ends no one reads it.
    raised: Arc<AtomicBool>,
}

impl ExecutionInterrupter for SimulationInterrupter {
    fn interrupt(&self) {
        self.raised.store(true, Ordering::Release);
    }
}

/// What the executor does when an instruction raised its exception.
enum Trap {
    SystemCall,
    MemoryFault {
        address: u64,
        access: Access,
    },
    /// The exception always ends execution.
    Final,
}

impl<Set: PreciseInstructionSet<Arm64>> Executor for Simulation<Set> {
    type Architecture = Arm64;
    type TranslatorConfig = ();
    type GuestMemory = SimulatedMemory;
    type Interrupter = SimulationInterrupter;
    type CreateGuestMemoryError = Infallible;

    fn create_guest_memory(&self) -> Result<(SimulatedMemory, SimulatedPageTable), Infallible> {
        Ok(SimulatedMemory::new())
    }

    /// Runs one instruction at a time on a copy of the memory, taken anew whenever anything but the instruction
    /// before changed a space, so that each instruction sees what other threads wrote.
    ///
    /// The execution blocks: it runs to its end in its first poll, answering an interrupt between two instructions.
    fn execute<'execution>(
        &'execution mut self,
        mut processor: Processor,
        memory: &'execution SimulatedMemory,
        handler: &'execution mut impl TrapHandler,
    ) -> (
        impl Future<Output = Processor> + 'execution,
        SimulationInterrupter,
    ) {
        let interrupter = SimulationInterrupter::default();
        let raised = Arc::clone(&interrupter.raised);
        let execution = async move {
            let mut copy = SpaceCopy::default();
            let mut changes = Vec::new();
            loop {
                if raised.swap(false, Ordering::Acquire) {
                    match handler.handle_interrupt() {
                        InterruptResponse::Continue => {}
                        InterruptResponse::Terminate => return processor,
                    }
                }
                let before = processor.clone();
                memory.refresh(&mut copy);
                self.execute_single_instruction_in_place(&mut processor, copy.deltas(), |delta| {
                    changes.push(delta.into_owned())
                });
                for delta in changes.drain(..) {
                    memory.apply(delta, &mut copy);
                }
                let Some(exception) = processor.exception else {
                    continue;
                };
                match to_trap(exception) {
                    Trap::SystemCall => match handler.handle_system_call(to_request(&processor)) {
                        SystemCallResponse::Return(value) => {
                            // x0 holds the result: https://man7.org/linux/man-pages/man2/syscall.2.html
                            if let Some(result) = processor.general_purpose_registers.first_mut() {
                                *result = value;
                            }
                            processor.program_counter =
                                processor.program_counter.wrapping_add(INSTRUCTION_BYTES);
                            processor.exception = None;
                        }
                        SystemCallResponse::Terminate => return processor,
                    },
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

    /// Runs the instruction at the program counter, taking a word none of the listed instructions match as undefined.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, Arm64>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) {
        let word = u32::from_le_bytes(read_bytes(memory, processor.program_counter));
        let listed = Arm64::list_instructions().find(|instruction| {
            let fixed = instruction.to_fixed_bits();
            word & fixed.mask == fixed.value
        });
        if let Some(instruction) = listed {
            let Ok(()) = self.set.to_precise(instruction).simulate_in_place(
                processor,
                memory,
                on_memory_mutated,
            );
            return;
        }
        // A breakpoint changes nothing but the exception, so on a copy it gives the fault of the fetch, if any.
        let mut fetched = processor.clone();
        let Ok(()) =
            self.set
                .to_precise(Instruction::Brk)
                .simulate_in_place(&mut fetched, memory, |_| {});
        let fetch_fault = fetched
            .exception
            .filter(|exception| !has_class(*exception, ESR_EL1::EC::Value::Brk64));
        processor.exception = Some(fetch_fault.unwrap_or(UNDEFINED_INSTRUCTION));
    }
}

fn has_class(exception: Exception, class: ESR_EL1::EC::Value) -> bool {
    ESR_EL1::EC.read(exception.syndrome) == class as u64
}

fn to_trap(exception: Exception) -> Trap {
    let writes = exception.syndrome & WRITE_NOT_READ == WRITE_NOT_READ;
    let access = match ESR_EL1::EC.read_as_enum(exception.syndrome) {
        Some(ESR_EL1::EC::Value::SVC64) => return Trap::SystemCall,
        Some(ESR_EL1::EC::Value::InstrAbortLowerEL) => Access::Execute,
        Some(ESR_EL1::EC::Value::DataAbortLowerEL) if writes => Access::Write,
        Some(ESR_EL1::EC::Value::DataAbortLowerEL) => Access::Read,
        _ => return Trap::Final,
    };
    match exception.fault_address {
        Some(address) => Trap::MemoryFault { address, access },
        None => Trap::Final,
    }
}

fn to_request(processor: &Processor) -> SystemCallRequest {
    let registers = &processor.general_purpose_registers;
    // The arguments are x0 onwards: https://man7.org/linux/man-pages/man2/syscall.2.html
    let mut arguments = [u64::MIN; SYSTEM_CALL_ARGUMENT_COUNT];
    arguments.copy_from_slice(&registers[..SYSTEM_CALL_ARGUMENT_COUNT]);
    SystemCallRequest {
        number: SystemCallNumber(registers[SYSTEM_CALL_NUMBER_REGISTER]),
        arguments,
    }
}
