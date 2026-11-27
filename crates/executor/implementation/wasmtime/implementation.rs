//! The WebAssembly executor: the translator's output for ARM64, run by wasmtime behind the executor interface.
//!
//! A space's bytes are in a memory file that every thread's store takes as its guest memory. Code is translated where a
//! thread first reaches it and dropped when it is written over; the handler is asked from inside the hooks it calls.

mod engine;
mod single_instruction;
mod space;
mod state_memory;
mod thread;

use std::collections::HashMap;
use std::future::Ready;
use std::num::NonZeroU64;
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};

use thiserror::Error;
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, PageTableOf, TrapHandler,
};
use wcontainers_processor_architecture_arm64::Processor;
use wcontainers_translator::LookAhead;
use wcontainers_wasmtime::Wasmtime;

use engine::Shared;
use space::Space;
pub use space::{WasmtimeBytes, WasmtimeMemory, WasmtimePageTable};
use thread::{Ending, InterruptState, Mode, ThreadStore};

/// The runner of guest threads on wasmtime: one store for each space it ran a thread on.
pub struct WasmtimeExecutor {
    shared: Arc<Shared>,
    config: WasmtimeTranslatorConfig,
    /// By the address of its space, the store this executor runs that space's threads in.
    threads: HashMap<usize, ThreadStore>,
    /// The space single instructions run on.
    scratch: Option<single_instruction::Scratch>,
}

/// How the executor translates code, and how long a run goes on before it asks the handler.
#[derive(Clone, Copy, Debug)]
pub struct WasmtimeTranslatorConfig {
    /// The look-ahead code reached at run time is translated at.
    pub look_ahead: LookAhead,
    /// The slice of a run, in events of the translator's progress count; none for a run no slice bounds.
    ///
    /// The events are the guest's executed jumps that can go backward, calls and returns; after each slice the run asks
    /// the handler as at an interrupt from outside.
    pub slice: Option<NonZeroU64>,
}

/// Tuning number: the progress events of a slice, a few milliseconds of a loop of a few instructions.
///
/// <https://github.com/Declarative-labs/wcontainers/blob/executor-wasmtime/docs/status-executor-wasmtime.md#tuning-numbers>
pub const DEFAULT_SLICE: NonZeroU64 = NonZeroU64::new(1 << 22).expect("a slice of events");

impl Default for WasmtimeTranslatorConfig {
    fn default() -> Self {
        Self {
            look_ahead: LookAhead::AT_RUN_TIME,
            slice: Some(DEFAULT_SLICE),
        }
    }
}

/// Why the executor could not make a space or execute a state.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum WasmtimeError {
    #[error("the host refused the memory of a space: {0}")]
    HostRefused(String),
    /// A state's memory lies outside a space or in more windows than it holds, or its code past where code runs.
    #[error("the state reaches past the addresses of a space")]
    AddressOutsideSpace,
}

/// The interrupter of one execution: it raises the thread's stop flag while the execution runs.
pub struct WasmtimeInterrupter {
    state: Arc<InterruptState>,
}

impl ExecutionInterrupter for WasmtimeInterrupter {
    fn interrupt(&self) {
        self.state.raised.store(true, Ordering::SeqCst);
        let target = self
            .state
            .target
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some((space, block)) = target.as_ref() {
            space.raise_stop_flag(*block);
        }
    }
}

impl WasmtimeExecutor {
    /// Returns an executor that translates as the configuration says.
    ///
    /// TODO: temporary placeholder until the executor interface says how an executor is built.
    pub fn new(config: WasmtimeTranslatorConfig) -> Self {
        Self {
            shared: Shared::get(),
            config,
            threads: HashMap::new(),
            scratch: None,
        }
    }

    /// Returns this executor's store for the space, made on its first run there; the stores of spaces gone are dropped.
    fn thread_for(&mut self, space: &Arc<Space>) -> &mut ThreadStore {
        self.threads
            .retain(|_, thread| thread.space.strong_count() > 0);
        let key = Arc::as_ptr(space) as usize;
        let current = self
            .threads
            .get(&key)
            .is_some_and(|thread| std::ptr::eq(thread.space.as_ptr(), Arc::as_ptr(space)));
        if !current {
            self.threads.insert(key, ThreadStore::new(space));
        }
        self.threads
            .get_mut(&key)
            .expect("the store just made or kept")
    }

    /// Drops this executor's store for the space.
    fn forget(&mut self, space: &Arc<Space>) {
        self.threads.remove(&(Arc::as_ptr(space) as usize));
    }

    /// Readies a run from the processor on the space in the mode, and returns the thread and its interrupter's state.
    fn prepare(
        &mut self,
        space: &Arc<Space>,
        processor: &Processor<()>,
        mode: Mode,
    ) -> (&mut ThreadStore, Arc<InterruptState>) {
        let interrupt = Arc::new(InterruptState::default());
        // A step runs one instruction, which no slice ends.
        let slice = match mode {
            Mode::Execute(_) => self.config.slice,
            Mode::Step { .. } => None,
        };
        let thread = self.thread_for(space);
        thread.prepare(space, processor, (mode, slice), interrupt.clone());
        let block = thread.store.data().block;
        *interrupt
            .target
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((space.clone(), block));
        (thread, interrupt)
    }
}

impl Executor for WasmtimeExecutor {
    type TranslatorConfig = WasmtimeTranslatorConfig;
    type GuestMemory = WasmtimeMemory;
    type Interrupter = WasmtimeInterrupter;
    type CreateGuestMemoryError = WasmtimeError;
    type Execution<'execution>
        = WasmtimeExecution<'execution>
    where
        Self: 'execution;

    fn create_guest_memory(&self) -> Result<(WasmtimeMemory, PageTableOf<Self>), WasmtimeError> {
        WasmtimeMemory::create(self.shared.clone())
    }

    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: Processor<()>,
        memory: &'execution WasmtimeMemory,
    ) -> (WasmtimeExecution<'execution>, WasmtimeInterrupter) {
        let mode = Mode::Execute(self.config.look_ahead);
        let (thread, state) = self.prepare(&memory.space, &processor, mode);
        let execution = WasmtimeExecution {
            thread,
            start: processor,
        };
        (execution, WasmtimeInterrupter { state })
    }
}

/// One execution on a [`WasmtimeExecutor`], not yet run.
pub struct WasmtimeExecution<'execution> {
    thread: &'execution mut ThreadStore,
    start: Processor<()>,
}

impl Execution for WasmtimeExecution<'_> {
    type ProcessorState = Processor<()>;
    /// Never made: a thread of this executor never waits.
    type Wait = Ready<()>;

    /// Runs until the next trap, then asks the handler from inside the trap's hook, until the handler ends it.
    fn run(self, handler: &mut impl TrapHandler) -> ExecutionStop<Processor<()>, Ready<()>> {
        let thread = self.thread;
        let wasmtime = thread
            .store
            .data()
            .space
            .as_ref()
            .map(|space| space.shared.wasmtime.clone());
        let wasmtime = wasmtime.expect("a prepared execution's space");
        let ending = wasmtime.call_with_trap_handler(handler, || thread.run_to_end());
        let exception = match ending {
            Ending::Raised(exception) => Some(exception),
            Ending::Interrupted | Ending::Stepped => None,
        };
        let processor = thread.processor(&self.start, exception);
        thread.finish();
        ExecutionStop::Terminated(processor)
    }
}
