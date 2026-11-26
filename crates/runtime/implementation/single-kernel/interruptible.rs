//! An executor whose executions a container's interrupter reaches, through each execution's own interrupter.
//!
//! The kernel keeps the interrupter of the turn it runs to itself, so the runtime keeps a share of each one too.

use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use wcontainers_abi::{
    AddressSpace, Cpu, CpuError, MemoryId, Permissions, RegisterBlock, SharedId, ThreadId,
};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_executor::{ExecutionInterrupter, Executor, PageTableOf, PerThreadWatch};
use wcontainers_processor_architecture::{MemorySink, MemorySnapshot, ProcessorArchitecture};

/// The executor `E`, each of whose executions an [`InterruptSlot`] can end from any host thread.
pub struct Interruptible<E: Executor> {
    executor: E,
    slot: Arc<InterruptSlot<E::Interrupter>>,
}

/// Where a runner's executor keeps the interrupter of the execution it runs, and whether its run is to end.
pub(crate) struct InterruptSlot<I> {
    state: Mutex<SlotState<I>>,
}

struct SlotState<I> {
    /// Whether every execution until the slot is lowered ends at once.
    raised: bool,
    /// The interrupter of the execution prepared last.
    running: Option<Arc<I>>,
}

/// An execution's interrupter, shared by the kernel and the runtime.
pub struct SharedInterrupter<I>(Arc<I>);

impl<I: ExecutionInterrupter> ExecutionInterrupter for SharedInterrupter<I> {
    fn interrupt(&self) {
        self.0.interrupt();
    }
}

impl<I: ExecutionInterrupter> InterruptSlot<I> {
    fn new() -> Self {
        Self {
            state: Mutex::new(SlotState {
                raised: false,
                running: None,
            }),
        }
    }

    fn state(&self) -> MutexGuard<'_, SlotState<I>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps the interrupter of an execution about to run, which ends at once if the slot is raised.
    fn keep(&self, interrupter: Arc<I>) {
        let mut state = self.state();
        if state.raised {
            interrupter.interrupt();
        }
        state.running = Some(interrupter);
    }

    /// Ends the execution running, and every later one until the slot is lowered.
    pub(crate) fn raise(&self) {
        let mut state = self.state();
        state.raised = true;
        if let Some(running) = &state.running {
            running.interrupt();
        }
    }

    /// Lets executions run again, once the run they were ended for is over.
    pub(crate) fn lower(&self) {
        let mut state = self.state();
        state.raised = false;
        state.running = None;
    }
}

impl<E: Executor> Interruptible<E> {
    /// Returns `executor` with a slot of its own.
    pub fn new(executor: E) -> Self {
        Self {
            executor,
            slot: Arc::new(InterruptSlot::new()),
        }
    }

    /// Returns the slot that ends this executor's executions.
    pub(crate) fn slot(&self) -> Arc<InterruptSlot<E::Interrupter>> {
        Arc::clone(&self.slot)
    }
}

impl<E: Executor> BehaviorSource for Interruptible<E> {
    type Architecture = E::Architecture;
    type Error = E::Error;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut <E::Architecture as ProcessorArchitecture>::ProcessorState,
        memory: MemorySnapshot<'_, '_, E::Architecture>,
        mutation_hook: &mut impl MemorySink<E::Architecture>,
    ) -> Result<(), E::Error> {
        self.executor
            .execute_single_instruction_in_place(processor, memory, mutation_hook)
    }
}

impl<E: Executor> Executor for Interruptible<E> {
    type TranslatorConfig = E::TranslatorConfig;
    type GuestMemory = E::GuestMemory;
    type Interrupter = SharedInterrupter<E::Interrupter>;
    type Execution<'execution>
        = E::Execution<'execution>
    where
        Self: 'execution;
    type CreateGuestMemoryError = E::CreateGuestMemoryError;

    fn create_guest_memory(
        &self,
    ) -> Result<(E::GuestMemory, PageTableOf<E>), E::CreateGuestMemoryError> {
        self.executor.create_guest_memory()
    }

    /// Prepares `E`'s execution and keeps a share of its interrupter in the slot.
    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: wcontainers_executor::ProcessorStateOf<E>,
        memory: &'execution E::GuestMemory,
    ) -> (E::Execution<'execution>, SharedInterrupter<E::Interrupter>) {
        let (execution, interrupter) = self.executor.prepare_execution(processor, memory);
        let interrupter = Arc::new(interrupter);
        self.slot.keep(Arc::clone(&interrupter));
        (execution, SharedInterrupter(interrupter))
    }
}

impl<E: PerThreadWatch> PerThreadWatch for Interruptible<E> {
    fn set_thread_permissions(
        &mut self,
        thread: ThreadId,
        range: Range<u64>,
        allowed: Permissions,
    ) -> Result<(), CpuError> {
        self.executor.set_thread_permissions(thread, range, allowed)
    }
}

/// The kernel still reads the size of its spaces through the abi's `Cpu`; every call goes to `E`.
impl<E: Executor + Cpu> Cpu for Interruptible<E> {
    fn address_space(&self) -> AddressSpace {
        self.executor.address_space()
    }

    fn memory_new(&mut self) -> Result<MemoryId, CpuError> {
        self.executor.memory_new()
    }

    fn memory_duplicate(&mut self, source: MemoryId) -> Result<MemoryId, CpuError> {
        self.executor.memory_duplicate(source)
    }

    fn memory_drop(&mut self, memory: MemoryId) {
        self.executor.memory_drop(memory);
    }

    fn read(&mut self, memory: MemoryId, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        self.executor.read(memory, address, into)
    }

    fn write(&mut self, memory: MemoryId, address: u64, from: &[u8]) -> Result<(), CpuError> {
        self.executor.write(memory, address, from)
    }

    fn zero(&mut self, memory: MemoryId, address: u64, length: u64) -> Result<(), CpuError> {
        self.executor.zero(memory, address, length)
    }

    fn set_permissions(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
        permissions: Permissions,
    ) -> Result<(), CpuError> {
        self.executor
            .set_permissions(memory, address, length, permissions)
    }

    fn code_changed(&mut self, memory: MemoryId, address: u64, length: u64, starts: &[u64]) {
        self.executor.code_changed(memory, address, length, starts);
    }

    fn shared_new(&mut self, length: u64) -> Result<SharedId, CpuError> {
        self.executor.shared_new(length)
    }

    fn shared_map(
        &mut self,
        shared: SharedId,
        offset: u64,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.executor
            .shared_map(shared, offset, memory, address, length)
    }

    fn shared_unmap(
        &mut self,
        memory: MemoryId,
        address: u64,
        length: u64,
    ) -> Result<(), CpuError> {
        self.executor.shared_unmap(memory, address, length)
    }

    fn shared_read(
        &mut self,
        shared: SharedId,
        offset: u64,
        into: &mut [u8],
    ) -> Result<(), CpuError> {
        self.executor.shared_read(shared, offset, into)
    }

    fn shared_write(&mut self, shared: SharedId, offset: u64, from: &[u8]) -> Result<(), CpuError> {
        self.executor.shared_write(shared, offset, from)
    }

    fn shared_drop(&mut self, shared: SharedId) {
        self.executor.shared_drop(shared);
    }

    fn thread_new(&mut self, memory: MemoryId) -> Result<ThreadId, CpuError> {
        self.executor.thread_new(memory)
    }

    fn thread_drop(&mut self, thread: ThreadId) {
        self.executor.thread_drop(thread);
    }

    fn registers(&mut self, thread: ThreadId, into: &mut RegisterBlock) {
        self.executor.registers(thread, into);
    }

    fn set_registers(&mut self, thread: ThreadId, from: &RegisterBlock) {
        self.executor.set_registers(thread, from);
    }
}
