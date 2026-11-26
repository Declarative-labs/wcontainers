//! The executor as a behavior source: one instruction of a state, run on a fresh space that holds the state's memory.

use std::collections::BTreeMap;

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

use crate::thread::{Ending, Mode};
use crate::{WasmtimeError, WasmtimeExecutor, WasmtimeMemory};

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

impl BehaviorSource for WasmtimeExecutor {
    type Architecture = Arm64<User>;
    type Error = WasmtimeError;

    /// Runs the instruction at the program counter on a fresh space holding the memory, and reports the bytes it changed
    /// on the pages the memory lets it write.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), WasmtimeError> {
        let (space, mut table) = WasmtimeMemory::create(self.shared.clone())?;
        let mut writable = BTreeMap::new();
        for delta in memory.mutations {
            match delta {
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => {
                    let allowed = to_permissions(*permission);
                    table
                        .assign(addresses.clone(), allowed)
                        .map_err(|_| WasmtimeError::AddressOutsideSpace)?;
                    let writes =
                        allowed.is_some_and(|allowed| allowed.contains(Permissions::WRITE));
                    for page in addresses.start / PAGE_SIZE..addresses.end.div_ceil(PAGE_SIZE) {
                        if writes {
                            writable.insert(page, ());
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
                }
            }
        }
        let read_page = |page: u64| {
            let mut bytes = vec![0; PAGE_SIZE as usize];
            if let Ok(view) = space.borrow(page * PAGE_SIZE..(page + 1) * PAGE_SIZE) {
                view.read(&mut bytes);
            }
            bytes
        };
        let before: Vec<(u64, Vec<u8>)> = writable
            .keys()
            .map(|&page| (page, read_page(page)))
            .collect();
        let start = processor.program_counter;
        let mode = Mode::Step {
            start,
            stops: 0,
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
        self.forget(&space.space);
        for (page, before) in before {
            report_changed_bytes(page * PAGE_SIZE, &before, &read_page(page), mutation_hook);
        }
        Ok(())
    }
}
