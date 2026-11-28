//! The executor boundary for any executor of the executor interface, which nothing proves.
//!
//! Each run copies every shard into the executor's guest memory, gives each page that lies wholly
//! inside a shard that shard's permissions, runs until the first trap, and copies the writable
//! shards back.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use wcontainers_abi::{PAGE_SIZE, Permissions, SystemCallRequest};
use wcontainers_executor::memory_space::{MemorySpace, PageTable};
use wcontainers_executor::{
    Execution as ExecutorExecution, ExecutionInterrupter, ExecutionStop, Executor,
    InterruptResponse, MemoryFaultResponse, PageTableOf, ProcessorStateOf, SystemCallResponse,
    TrapHandler,
};
use wcontainers_kernel_core::execution::{Access, Interrupter, Stop};
use wcontainers_kernel_core::memory::MemoryCapabilityTable;
use wcontainers_capability_table::CapabilityTable;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::disjoint_memory::DisjointMemory;
use crate::executor_boundary::ExecutorBoundary;

/// One task's executor, with the guest memory its shards are copied into for each run.
pub struct ExecutorRunner<E: Executor> {
    executor: E,
    guest: E::GuestMemory,
    pages: PageTableOf<E>,
    mapped: Vec<Range<u64>>,
}

impl<E: Executor> ExecutorRunner<E> {
    pub fn new(executor: E) -> Result<Self, E::CreateGuestMemoryError> {
        let (guest, pages) = executor.create_guest_memory()?;
        Ok(Self {
            executor,
            guest,
            pages,
            mapped: Vec::new(),
        })
    }
}

/// Whether a run was asked to stop, and the executor's interrupter while the run lasts.
pub struct InterruptionSlot<I> {
    interrupted: bool,
    executor: Option<I>,
}

pub struct ForwardingInterrupter<I> {
    slot: Arc<spin::Mutex<InterruptionSlot<I>>>,
}

impl<I: ExecutionInterrupter> Interrupter for ForwardingInterrupter<I> {
    fn interrupt(&self) {
        let mut slot = self.slot.lock();
        slot.interrupted = true;
        if let Some(executor) = &slot.executor {
            executor.interrupt();
        }
    }
}

/// The first trap of a run, which ends it.
struct FirstTrap {
    stop: Option<Stop>,
}

impl TrapHandler for FirstTrap {
    fn handle_system_call(&mut self, _: SystemCallRequest) -> SystemCallResponse {
        self.stop.get_or_insert(Stop::SystemCall);
        SystemCallResponse::Terminate
    }

    fn handle_memory_fault(
        &mut self,
        address: u64,
        access: wcontainers_abi::Access,
    ) -> MemoryFaultResponse {
        let access = match access {
            wcontainers_abi::Access::Read => Access::Read,
            wcontainers_abi::Access::Write => Access::Write,
            wcontainers_abi::Access::Execute => Access::Execute,
        };
        self.stop.get_or_insert(Stop::MemoryFault { address, access });
        MemoryFaultResponse::Terminate
    }

    fn handle_interrupt(&mut self) -> InterruptResponse {
        self.stop.get_or_insert(Stop::Interrupt);
        InterruptResponse::Terminate
    }
}

fn to_permissions(permissions: MemoryPermissions) -> Permissions {
    let mut allowed = Permissions::empty();
    allowed.set(Permissions::READ, permissions.read);
    allowed.set(Permissions::WRITE, permissions.write);
    allowed.set(Permissions::EXECUTE, permissions.execute);
    allowed
}

/// Returns the pages that lie wholly inside the addresses.
fn to_whole_pages(addresses: &Range<u64>) -> Range<u64> {
    let first = addresses.start.div_ceil(PAGE_SIZE).saturating_mul(PAGE_SIZE);
    let end = addresses.end / PAGE_SIZE * PAGE_SIZE;
    first..end.max(first)
}

fn copy_bytes(from: &impl VolatileBytes, to: &impl VolatileBytes) {
    let mut buffer = [u8::MIN; PAGE_SIZE as usize];
    let mut offset = u64::MIN;
    while offset < from.len() {
        let count = from.read_at(offset, &mut buffer);
        if count == usize::MIN {
            break;
        }
        to.write_at(offset, &buffer[..count]);
        offset = offset.saturating_add(count as u64);
    }
}

impl<E: Executor> ExecutorRunner<E> {
    fn map_shards<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>>(
        &mut self,
        memory: &DisjointMemory<Shard, Table>,
    ) {
        for start in memory.list_starts() {
            let Some(shard) = memory.get(start) else {
                continue;
            };
            let addresses = *start..start.saturating_add(shard.len());
            let (Ok(from), Ok(to)) = (shard.borrow(u64::MIN, shard.len()), self.guest.borrow(addresses.clone())) else {
                continue;
            };
            copy_bytes(&from, &to);
            let pages = to_whole_pages(&addresses);
            if !pages.is_empty() && self.pages.set_permissions(pages, to_permissions(shard.permissions())).is_err() {
                continue;
            }
            self.mapped.push(addresses);
        }
    }

    fn unmap_shards<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>>(
        &mut self,
        memory: &DisjointMemory<Shard, Table>,
    ) {
        for start in memory.list_starts() {
            let Some(shard) = memory.get(start) else {
                continue;
            };
            if !shard.permissions().write {
                continue;
            }
            let addresses = *start..start.saturating_add(shard.len());
            if let (Ok(from), Ok(to)) = (self.guest.borrow(addresses), shard.borrow(u64::MIN, shard.len())) {
                copy_bytes(&from, &to);
            }
        }
        for addresses in self.mapped.drain(..) {
            let pages = to_whole_pages(&addresses);
            if !pages.is_empty() {
                let _ = self.pages.set_permissions(pages, Permissions::empty());
            }
            let _ = self.guest.zero(addresses);
        }
    }
}

#[allow(
    reviewable::implementation_not_verified,
    reason = "TODO: prove it against the trait's contract, then remove this line"
)]
impl<Shard: MemoryShard, Table: MemoryCapabilityTable<Shard>, E: Executor>
    ExecutorBoundary<Shard, Table> for ExecutorRunner<E>
{
    type ProcessorState = ProcessorStateOf<E>;

    type Interruption = Arc<spin::Mutex<InterruptionSlot<E::Interrupter>>>;

    type Interrupter = ForwardingInterrupter<E::Interrupter>;

    fn prepare_interruption() -> (Self::Interruption, Self::Interrupter) {
        let slot = Arc::new(spin::Mutex::new(InterruptionSlot {
            interrupted: false,
            executor: None,
        }));
        (Arc::clone(&slot), ForwardingInterrupter { slot })
    }

    fn run(
        &mut self,
        memory: &DisjointMemory<Shard, Table>,
        processor: &mut ProcessorStateOf<E>,
        interruption: Self::Interruption,
    ) -> Stop {
        self.map_shards(memory);
        let mut trap = FirstTrap { stop: None };
        let stopped = {
            let (execution, interrupter) =
                self.executor.prepare_execution(processor.clone(), &self.guest);
            {
                let mut slot = interruption.lock();
                if slot.interrupted {
                    interrupter.interrupt();
                }
                slot.executor = Some(interrupter);
            }
            match execution.run(&mut trap) {
                ExecutionStop::Terminated(state) => state,
                ExecutionStop::Waiting { processor, .. } => processor,
            }
        };
        interruption.lock().executor = None;
        *processor = stopped;
        self.unmap_shards(memory);
        trap.stop.unwrap_or(Stop::Interrupt)
    }
}
