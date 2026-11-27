//! The executor as a behavior source: one instruction of a state, run on a scratch space that holds the state's memory.

use std::collections::BTreeSet;

use wcontainers_abi::{Access, PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::memory_space::MemorySpace;
use wcontainers_executor::{
    InterruptResponse, MemoryFaultResponse, SystemCallResponse, TrapHandler,
};
use wcontainers_processor_architecture::{MemoryDelta, MemorySink, MemorySnapshot};
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::{
    Arm64, DataAccess, InstructionFetch, PagePermission, Processor, User,
};
use wcontainers_volatile_bytes::VolatileBytes;
use wcontainers_wasmtime::Wasmtime;

use crate::space::SLOTS_COVER;
use crate::thread::{Ending, Mode};
use crate::{WasmtimeError, WasmtimeExecutor, WasmtimeMemory, WasmtimePageTable};

/// The handler of a step, which no trap reaches: a step ends at every trap itself.
struct Unasked;

impl TrapHandler for Unasked {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(&mut self, _: u64, _: Access) -> MemoryFaultResponse {
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        InterruptResponse::Terminate
    }
}

/// Returns the permissions a page of a state has, none for an unmapped one.
fn to_permissions(permission: PagePermission) -> Option<Permissions> {
    match permission {
        PagePermission::Unmapped => None,
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data = match data_access {
                DataAccess::Forbidden => Permissions::empty(),
                DataAccess::ReadOnly => Permissions::READ,
                DataAccess::ReadWrite => Permissions::READ | Permissions::WRITE,
            };
            let fetch = match instruction_fetch {
                InstructionFetch::Forbidden => Permissions::empty(),
                InstructionFetch::Allowed => Permissions::EXECUTE,
            };
            Some(data | fetch)
        }
    }
}

/// A space one executor runs single instructions on, cleared after each, and made anew after [`SCRATCH_STEPS`] of them.
pub(crate) struct Scratch {
    memory: WasmtimeMemory,
    table: WasmtimePageTable,
    steps: u32,
}

/// The single instructions run on one scratch space before it is made anew, which bounds what its store accumulates.
///
/// The store gains an instance and lookup table slots for each instruction.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
const SCRATCH_STEPS: u32 = 1 << 10;

/// Returns whether a scratch space that ran `steps` instructions and gave `windows_given` windows runs the next one.
///
/// A space holds only so many windows, so one a state gave windows past window 0 is made anew.
fn reusable(steps: u32, windows_given: u64) -> bool {
    steps < SCRATCH_STEPS && windows_given == 1
}

impl BehaviorSource for WasmtimeExecutor {
    type Architecture = Arm64<User>;
    type Error = WasmtimeError;

    /// Runs the instruction at the program counter on a scratch space holding the memory.
    ///
    /// It reports the bytes the instruction changed on the pages the memory lets it write.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), WasmtimeError> {
        let mut scratch = match self.scratch.take() {
            Some(scratch) if reusable(scratch.steps, scratch.memory.space.windows_given()) => {
                scratch
            }
            old => {
                if let Some(old) = old {
                    self.forget(&old.memory.space);
                }
                let (memory, table) = WasmtimeMemory::create(self.shared.clone())?;
                Scratch {
                    memory,
                    table,
                    steps: u32::MIN,
                }
            }
        };
        let stepped = self.step(&mut scratch, processor, memory, mutation_hook);
        match stepped {
            Ok(()) => {
                scratch.steps += 1;
                self.scratch = Some(scratch);
            }
            // A state refused part way leaves the scratch space holding part of it.
            Err(_) => self.forget(&scratch.memory.space),
        }
        stepped
    }
}

impl WasmtimeExecutor {
    /// Runs one instruction on the scratch space, then clears what the state and the instruction left in it.
    fn step(
        &mut self,
        scratch: &mut Scratch,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), WasmtimeError> {
        // Code runs only from addresses the array of slots covers.
        if processor.program_counter >= SLOTS_COVER {
            return Err(WasmtimeError::AddressOutsideSpace);
        }
        let space = &scratch.memory;
        let mut assigned = Vec::new();
        let mut written = BTreeSet::new();
        let mut writable = BTreeSet::new();
        for delta in memory.mutations {
            match delta {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    let allowed = to_permissions(*permission);
                    scratch
                        .table
                        .assign(addresses.clone(), allowed)
                        .map_err(|_| WasmtimeError::AddressOutsideSpace)?;
                    assigned.push(addresses.clone());
                    let writes =
                        allowed.is_some_and(|allowed| allowed.contains(Permissions::WRITE));
                    for page in addresses.start / PAGE_SIZE..addresses.end.div_ceil(PAGE_SIZE) {
                        if writes {
                            writable.insert(page);
                        } else {
                            writable.remove(&page);
                        }
                    }
                }
                MemoryDelta::ValueAssignment { start, bytes } => {
                    let end = start
                        .checked_add(bytes.len() as u64)
                        .ok_or(WasmtimeError::AddressOutsideSpace)?;
                    let view = space
                        .borrow(*start..end)
                        .map_err(|_| WasmtimeError::AddressOutsideSpace)?;
                    view.write(bytes);
                    written.extend(start / PAGE_SIZE..end.div_ceil(PAGE_SIZE));
                }
            }
        }
        let read_page = |page: u64| {
            let mut bytes = vec![u8::MIN; PAGE_SIZE as usize];
            if let Ok(view) = space.borrow(page * PAGE_SIZE..(page + 1) * PAGE_SIZE) {
                view.read(&mut bytes);
            }
            bytes
        };
        let before: Vec<(u64, Vec<u8>)> = writable
            .iter()
            .map(|&page| (page, read_page(page)))
            .collect();
        let start = processor.program_counter;
        let mode = Mode::Step {
            start,
            stops: u32::MIN,
            loaded: false,
        };
        let shared = self.shared.clone();
        let (thread, _) = self.prepare(&space.space, processor, mode);
        let block = thread.store.data().block;
        space.space.raise_stop_flag(block);
        let ending = shared
            .wasmtime
            .call_with_trap_handler(&mut Unasked, || thread.run_to_end());
        let exception = match ending {
            Ending::Raised(exception) => Some(exception),
            Ending::Interrupted | Ending::Stepped => None,
        };
        *processor = thread.processor(processor, exception);
        thread.finish();
        for (page, before) in before {
            report_changed_bytes(page * PAGE_SIZE, &before, &read_page(page), mutation_hook);
        }
        for addresses in assigned {
            scratch
                .table
                .assign(addresses, None)
                .map_err(|_| WasmtimeError::AddressOutsideSpace)?;
        }
        for page in written.into_iter().chain(writable) {
            space
                .zero(page * PAGE_SIZE..(page + 1) * PAGE_SIZE)
                .map_err(|_| WasmtimeError::AddressOutsideSpace)?;
        }
        space.space.drop_every_load();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SCRATCH_STEPS, reusable};

    #[test]
    fn a_scratch_space_is_made_anew_after_its_steps_or_once_it_gave_a_window_past_window_0() {
        assert!(reusable(u32::MIN, 1));
        assert!(reusable(SCRATCH_STEPS - 1, 1));
        assert!(!reusable(SCRATCH_STEPS, 1));
        assert!(!reusable(u32::MIN, 2));
        assert!(!reusable(SCRATCH_STEPS - 1, 8));
    }
}
