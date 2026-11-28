use core::convert::Infallible;
use core::future::Ready;
use core::iter::Empty;
use core::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wcontainers_abi::{SystemCallNumber, SystemCallRequest};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_capability_table::CapabilityTable;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_executor::{
    Execution, ExecutionInterrupter, ExecutionStop, Executor, SystemCallResponse, TrapHandler,
};
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Execution as _, Interrupter as _, Stop};
use wcontainers_kernel_core::memory::TaskMemory;
use wcontainers_kernel_core::processor_state::TaskProcessorState;
use wcontainers_kernel_core_on_executor::OnExecutorKernelCore;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_memory_shard_shared_bytes::SharedBytesView;
use wcontainers_processor_architecture::{MemorySink, MemorySnapshot, ProcessorArchitecture};
use wcontainers_volatile_bytes::VolatileBytes;

/// An architecture whose whole processor state is one counter.
#[derive(Clone)]
struct Counter;

impl ProcessorArchitecture for Counter {
    type ProcessorState = u64;
    type ProcessorStateExpectation = u64;
    type PagePermission = ();
    type Instruction = ();
    type PreciseInstruction = ();
    type Instructions = Empty<()>;
    type KnownBehaviors = Empty<()>;

    const ADDRESSABLE: Range<u64> = 0..1 << 48;
    const DEFAULT_PERMISSION: () = ();

    fn list_instructions() -> Empty<()> {
        core::iter::empty()
    }

    fn list_known_behaviors(_instruction: ()) -> Empty<()> {
        core::iter::empty()
    }

    fn meets_expectations(observed: &u64, expectation: &u64) -> bool {
        observed == expectation
    }
}

/// An executor whose every run adds one to the counter and then makes a system call, unless interrupted first.
#[derive(Clone)]
struct FakeExecutor;

struct FakeMemory;

struct FakePageTable;

struct FakeExecution {
    processor: u64,
    interrupted: Arc<AtomicBool>,
}

struct FakeInterrupter(Arc<AtomicBool>);

impl BehaviorSource for FakeExecutor {
    type Architecture = Counter;
    type Error = Infallible;

    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut u64,
        _memory: MemorySnapshot<'_, '_, Counter>,
        _mutation_hook: &mut impl MemorySink<Counter>,
    ) -> Result<(), Infallible> {
        *processor += 1;
        Ok(())
    }
}

impl MemorySpace for FakeMemory {
    type Bytes<'space> = SharedBytesView<'space>;
    type PageTable = FakePageTable;

    fn borrow(&self, _addresses: Range<u64>) -> Result<SharedBytesView<'_>, InvalidAddressError> {
        Err(InvalidAddressError)
    }
}

impl PageTable for FakePageTable {
    fn set_permissions(
        &mut self,
        _addresses: Range<u64>,
        _allowed: wcontainers_abi::Permissions,
    ) -> Result<(), InvalidAddressError> {
        Ok(())
    }

    fn map_from(
        &mut self,
        _addresses: Range<u64>,
        _other: &Self,
        _other_start: u64,
    ) -> Result<(), MapMemoryError> {
        Ok(())
    }

    fn unmap_from(&mut self, _addresses: Range<u64>) -> Result<(), InvalidAddressError> {
        Ok(())
    }

    fn copy_from(
        &mut self,
        _addresses: Range<u64>,
        _other: &Self,
        _other_start: u64,
    ) -> Result<(), CopyMemoryError> {
        Ok(())
    }
}

impl Execution for FakeExecution {
    type ProcessorState = u64;
    type Wait = Ready<()>;

    fn run(self, handler: &mut impl TrapHandler) -> ExecutionStop<u64, Ready<()>> {
        if self.interrupted.load(Ordering::SeqCst) {
            let _ = handler.handle_interrupt();
            return ExecutionStop::Terminated(self.processor);
        }
        let processor = self.processor + 1;
        let request = SystemCallRequest {
            number: SystemCallNumber(processor),
            arguments: [0; 6],
        };
        match handler.handle_system_call(request) {
            SystemCallResponse::Return(_) | SystemCallResponse::Terminate => {
                ExecutionStop::Terminated(processor)
            }
        }
    }
}

impl ExecutionInterrupter for FakeInterrupter {
    fn interrupt(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl Executor for FakeExecutor {
    type TranslatorConfig = ();
    type GuestMemory = FakeMemory;
    type Interrupter = FakeInterrupter;
    type Execution<'execution> = FakeExecution;
    type CreateGuestMemoryError = Infallible;

    fn create_guest_memory(&self) -> Result<(FakeMemory, FakePageTable), Infallible> {
        Ok((FakeMemory, FakePageTable))
    }

    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: u64,
        _memory: &'execution FakeMemory,
    ) -> (FakeExecution, FakeInterrupter) {
        let interrupted = Arc::new(AtomicBool::new(false));
        (
            FakeExecution {
                processor,
                interrupted: Arc::clone(&interrupted),
            },
            FakeInterrupter(interrupted),
        )
    }
}

type Core = OnExecutorKernelCore<FakeExecutor>;

const PAGE: u64 = 4096;
const READ_WRITE: MemoryPermissions = MemoryPermissions {
    read: true,
    write: true,
    execute: false,
};

fn core() -> Core {
    OnExecutorKernelCore::new(FakeExecutor, 0)
}

#[test]
fn a_new_task_holds_no_memory() {
    let mut core = core();
    let task = core.create_task().ok().expect("a task");
    let memory = Core::memory_capabilities(&task);
    for address in [0, PAGE, 1 << 20, u64::MAX] {
        assert!(memory.get(&address).is_none());
    }
}

#[test]
fn memory_created_in_one_task_is_not_held_by_another() {
    let mut core = core();
    let mut first = core.create_task().ok().expect("a task");
    let second = core.create_task().ok().expect("a task");
    let config = MemoryShardConfig {
        length: PAGE,
        permissions: READ_WRITE,
    };
    assert!(Core::memory_capabilities_mut(&mut first)
        .create(PAGE, config)
        .is_ok());
    assert!(Core::memory_capabilities(&first).get(&PAGE).is_some());
    assert!(Core::memory_capabilities(&second).get(&PAGE).is_none());
}

#[test]
fn a_run_returns_its_stop_and_leaves_what_the_task_holds() {
    let mut core = core();
    let mut task = core.create_task().ok().expect("a task");
    let config = MemoryShardConfig {
        length: PAGE,
        permissions: READ_WRITE,
    };
    let memory = Core::memory_capabilities_mut(&mut task);
    assert!(memory.create(PAGE, config).is_ok());
    let view = memory.get(&PAGE).unwrap().borrow(0, 4).unwrap();
    assert_eq!(view.write(b"held"), 4);
    let (execution, _interrupter) = Core::prepare_execution(&mut task);
    assert!(matches!(execution.run(), Stop::SystemCall));
    let shard = Core::memory_capabilities(&task).get(&PAGE).expect("the shard");
    assert_eq!(shard.len(), PAGE);
    let mut bytes = [0; 4];
    assert_eq!(shard.borrow(0, 4).unwrap().read(&mut bytes), 4);
    assert_eq!(&bytes, b"held");
}

#[test]
fn an_interrupter_stops_the_one_run_it_was_prepared_with() {
    let mut core = core();
    let mut task = core.create_task().ok().expect("a task");
    let (execution, interrupter) = Core::prepare_execution(&mut task);
    interrupter.interrupt();
    assert!(matches!(execution.run(), Stop::Interrupt));
    let (execution, _later) = Core::prepare_execution(&mut task);
    interrupter.interrupt();
    assert!(matches!(execution.run(), Stop::SystemCall));
}

#[test]
fn the_processor_state_a_stop_saves_is_the_one_the_next_run_starts_from() {
    let mut core = core();
    let mut task = core.create_task().ok().expect("a task");
    assert_eq!(*Core::saved_processor_state(&task), 0);
    let (execution, _interrupter) = Core::prepare_execution(&mut task);
    assert!(matches!(execution.run(), Stop::SystemCall));
    assert_eq!(*Core::saved_processor_state(&task), 1);
    *Core::saved_processor_state_mut(&mut task) = 41;
    let (execution, _interrupter) = Core::prepare_execution(&mut task);
    assert!(matches!(execution.run(), Stop::SystemCall));
    assert_eq!(*Core::saved_processor_state(&task), 42);
}
