use std::convert::Infallible;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use wcontainers_abi::{Access as ExecutorAccess, SystemCallNumber, SystemCallRequest};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_capability_table::CapabilityTable;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, PageTable,
};
use wcontainers_executor::{
    Execution as ExecutorExecution, ExecutionInterrupter, ExecutionStop, Executor, TrapHandler,
};
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Access, Execution, Interrupter, Stop};
use wcontainers_kernel_core::memory::TaskMemory;
use wcontainers_kernel_core::processor_state::TaskProcessorState;
use wcontainers_kernel_core_executor::ExecutorKernelCore;
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_memory_shard_shared_allocation::{SharedAllocationMemoryShard, SharedBytes};
use wcontainers_processor_architecture::{MemorySink, MemorySnapshot, ProcessorArchitecture};
use wcontainers_volatile_bytes::VolatileBytes;

/// An architecture whose whole processor is one counter.
#[derive(Clone)]
struct Counter;

impl ProcessorArchitecture for Counter {
    type ProcessorState = u64;
    type ProcessorStateExpectation = u64;
    type PagePermission = bool;
    type Instruction = u8;
    type PreciseInstruction = u8;
    type Instructions = std::iter::Empty<u8>;
    type KnownBehaviors = std::iter::Empty<u8>;

    const ADDRESSABLE: Range<u64> = 0..1 << 48;
    const DEFAULT_PERMISSION: bool = false;

    fn list_instructions() -> Self::Instructions {
        std::iter::empty()
    }

    fn list_known_behaviors(_instruction: u8) -> Self::KnownBehaviors {
        std::iter::empty()
    }

    fn meets_expectations(observed: &u64, expectation: &u64) -> bool {
        observed == expectation
    }
}

/// What the next run of the fake executor does.
#[derive(Clone, Copy)]
enum Plan {
    SystemCall,
    Fault(u64),
    /// Spins until its interrupter stops it.
    Spin,
}

/// An executor whose run adds one to the counter and then does what the plan says.
#[derive(Clone)]
struct FakeExecutor {
    plan: Arc<Mutex<Plan>>,
    /// The counter each run started from.
    starts: Arc<Mutex<Vec<u64>>>,
}

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

struct FakeMemory {
    shard: SharedAllocationMemoryShard,
}

impl MemorySpace for FakeMemory {
    type Bytes<'space> = SharedBytes<'space>;
    type PageTable = FakePageTable;

    fn borrow(&self, addresses: Range<u64>) -> Result<SharedBytes<'_>, InvalidAddressError> {
        self.shard
            .borrow(addresses.start, addresses.end - addresses.start)
            .map_err(|_| InvalidAddressError)
    }
}

struct FakePageTable;

impl PageTable for FakePageTable {
    fn set_permissions(&mut self, _: Range<u64>, _: wcontainers_abi::Permissions) -> Result<(), InvalidAddressError> {
        Ok(())
    }

    fn map_from(&mut self, _: Range<u64>, _: &Self, _: u64) -> Result<(), MapMemoryError> {
        Ok(())
    }

    fn unmap_from(&mut self, _: Range<u64>) -> Result<(), InvalidAddressError> {
        Ok(())
    }

    fn copy_from(&mut self, _: Range<u64>, _: &Self, _: u64) -> Result<(), CopyMemoryError> {
        Ok(())
    }
}

struct FakeInterrupter {
    stopped: Arc<AtomicBool>,
}

impl ExecutionInterrupter for FakeInterrupter {
    fn interrupt(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }
}

struct FakeExecution<'execution> {
    processor: u64,
    plan: Plan,
    stopped: Arc<AtomicBool>,
    starts: &'execution Mutex<Vec<u64>>,
}

impl ExecutorExecution for FakeExecution<'_> {
    type ProcessorState = u64;
    type Wait = std::future::Ready<()>;

    fn run(self, handler: &mut impl TrapHandler) -> ExecutionStop<u64, Self::Wait> {
        self.starts.lock().unwrap().push(self.processor);
        let processor = self.processor + 1;
        match self.plan {
            Plan::SystemCall => {
                handler.handle_system_call(SystemCallRequest { number: SystemCallNumber(93), arguments: [0; 6] });
            },
            Plan::Fault(address) => {
                handler.handle_memory_fault(address, ExecutorAccess::Write);
            },
            Plan::Spin => {
                while !self.stopped.load(Ordering::SeqCst) {
                    thread::yield_now();
                }
                handler.handle_interrupt();
            },
        }
        ExecutionStop::Terminated(processor)
    }
}

impl Executor for FakeExecutor {
    type TranslatorConfig = ();
    type GuestMemory = FakeMemory;
    type Interrupter = FakeInterrupter;
    type Execution<'execution> = FakeExecution<'execution>;
    type CreateGuestMemoryError = Infallible;

    fn create_guest_memory(&self) -> Result<(FakeMemory, FakePageTable), Infallible> {
        let permissions = MemoryPermissions { read: true, write: true, execute: false };
        let shard = SharedAllocationMemoryShard::allocate(16, permissions).expect("allocate");
        Ok((FakeMemory { shard }, FakePageTable))
    }

    fn prepare_execution<'execution>(
        &'execution mut self,
        processor: u64,
        _memory: &'execution FakeMemory,
    ) -> (FakeExecution<'execution>, FakeInterrupter) {
        let stopped = Arc::new(AtomicBool::new(false));
        let plan = *self.plan.lock().unwrap();
        (
            FakeExecution { processor, plan, stopped: Arc::clone(&stopped), starts: &self.starts },
            FakeInterrupter { stopped },
        )
    }
}

type Core = ExecutorKernelCore<FakeExecutor>;

const START: u64 = 40;
const ADDRESS: u64 = 0x4000;

fn core_with(plan: Plan) -> (Core, Arc<Mutex<Plan>>, Arc<Mutex<Vec<u64>>>) {
    let plan = Arc::new(Mutex::new(plan));
    let starts = Arc::new(Mutex::new(Vec::new()));
    let executor = FakeExecutor { plan: Arc::clone(&plan), starts: Arc::clone(&starts) };
    (ExecutorKernelCore::new(executor, START), plan, starts)
}

fn config() -> MemoryShardConfig {
    MemoryShardConfig {
        length: 4096,
        permissions: MemoryPermissions { read: true, write: true, execute: false },
    }
}

#[test]
fn a_new_task_holds_no_memory() {
    let (mut core, _, _) = core_with(Plan::SystemCall);
    let task = core.create_task().expect("create a task");
    let memory = Core::memory_capabilities(&task);
    for address in [0, ADDRESS, u64::MAX] {
        assert!(memory.get(&address).is_none());
    }
}

#[test]
fn memory_created_in_one_task_is_not_held_by_another() {
    let (mut core, _, _) = core_with(Plan::SystemCall);
    let mut first = core.create_task().expect("create a task");
    let second = core.create_task().expect("create a task");
    Core::memory_capabilities_mut(&mut first).create(ADDRESS, config()).expect("create memory");
    assert!(Core::memory_capabilities(&first).get(&ADDRESS).is_some());
    assert!(Core::memory_capabilities(&second).get(&ADDRESS).is_none());
}

#[test]
fn a_run_returns_its_stop_and_leaves_what_the_task_holds() {
    let (mut core, plan, _) = core_with(Plan::SystemCall);
    let mut task = core.create_task().expect("create a task");
    Core::memory_capabilities_mut(&mut task).create(ADDRESS, config()).expect("create memory");
    let shard = Core::memory_capabilities(&task).get(&ADDRESS).expect("held");
    shard.borrow(0, 4).expect("borrow").write(b"held");

    let (execution, _interrupter) = Core::prepare_execution(&mut task);
    assert!(matches!(execution.run(), Stop::SystemCall));

    *plan.lock().unwrap() = Plan::Fault(0x77);
    let (execution, _interrupter) = Core::prepare_execution(&mut task);
    assert!(matches!(
        execution.run(),
        Stop::MemoryFault { address: 0x77, access: Access::Write }
    ));

    let shard = Core::memory_capabilities(&task).get(&ADDRESS).expect("still held");
    assert_eq!(shard.len(), 4096);
    let mut bytes = [0; 4];
    shard.borrow(0, 4).expect("borrow").read(&mut bytes);
    assert_eq!(&bytes, b"held");
}

#[test]
fn an_interrupter_stops_the_one_run_it_was_prepared_with() {
    let (mut core, plan, starts) = core_with(Plan::Spin);
    let mut task = core.create_task().expect("create a task");

    let (execution, interrupter) = Core::prepare_execution(&mut task);
    let stop = thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(50));
            interrupter.interrupt();
        });
        execution.run()
    });
    assert!(matches!(stop, Stop::Interrupt));

    *plan.lock().unwrap() = Plan::SystemCall;
    let (execution, next_interrupter) = Core::prepare_execution(&mut task);
    interrupter.interrupt();
    assert!(matches!(execution.run(), Stop::SystemCall));

    next_interrupter.interrupt();
    assert_eq!(starts.lock().unwrap().len(), 2);
}

#[test]
fn an_interrupt_before_the_run_stops_it_before_it_starts() {
    let (mut core, _, starts) = core_with(Plan::SystemCall);
    let mut task = core.create_task().expect("create a task");
    let (execution, interrupter) = Core::prepare_execution(&mut task);
    interrupter.interrupt();
    assert!(matches!(execution.run(), Stop::Interrupt));
    assert!(starts.lock().unwrap().is_empty());
    assert_eq!(*Core::saved_processor_state(&task), START);
}

#[test]
fn the_processor_state_saved_by_a_stop_is_the_one_the_next_run_starts_from() {
    let (mut core, _, starts) = core_with(Plan::SystemCall);
    let mut task = core.create_task().expect("create a task");
    assert_eq!(*Core::saved_processor_state(&task), START);
    let (execution, _interrupter) = Core::prepare_execution(&mut task);
    execution.run();
    assert_eq!(*Core::saved_processor_state(&task), START + 1);
    *Core::saved_processor_state_mut(&mut task) = 100;
    let (execution, _interrupter) = Core::prepare_execution(&mut task);
    execution.run();
    assert_eq!(*starts.lock().unwrap(), vec![START, 100]);
    assert_eq!(*Core::saved_processor_state(&task), 101);
}
