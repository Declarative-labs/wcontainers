//! The tests every implementation of wcontainers-kernel-core runs, written against its traits alone.
//!
//! The tests that run a task take, beside the core, two programs in its executor's instructions and
//! the accessors of its saved processor state that the trait cannot name.

use std::thread;
use std::time::Duration;

use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Access, Execution, Interrupter, Stop};
use wcontainers_memory_capability_table::{MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

/// The length of every shard the tests make, one page.
///
/// https://www.kernel.org/doc/html/latest/admin-guide/mm/concepts.html#pages
const SHARD_LENGTH: u64 = 0x1000;
/// Half the length of every shard the tests make.
const HALF_SHARD_LENGTH: u64 = 0x800;
/// Where the tests put a program, the lowest address Linux maps by default.
///
/// https://www.kernel.org/doc/html/latest/admin-guide/sysctl/vm.html#mmap-min-addr
const CODE: u64 = 0x1_0000;
/// Where the tests put data, past the end of a program's page.
const DATA: u64 = 0x2_0000;
/// Where the tests put a duplicate, past the end of the data's page.
const COPY: u64 = 0x3_0000;
/// The first byte after the program's page.
const CODE_END: u64 = 0x1_1000;
/// How many bytes lie between the end of the program's page and the data's.
const BETWEEN_CODE_AND_DATA: u64 = 0xF000;
/// The middle of the data's page.
const DATA_MIDDLE: u64 = 0x2_0800;
/// The last byte of the data's page.
const DATA_LAST: u64 = 0x2_0FFF;
/// The first byte after the data's page.
const DATA_END: u64 = 0x2_1000;
/// The start of the page below the data's.
const DATA_BELOW: u64 = 0x1_F000;
/// The start of a page whose last byte is the data's first.
const DATA_BELOW_BY_ONE: u64 = 0x1_F001;
const READ_WRITE: MemoryPermissions = MemoryPermissions { read: true, write: true, execute: false };
const READ_EXECUTE: MemoryPermissions = MemoryPermissions { read: true, write: false, execute: true };
/// How long the interrupting thread waits before it interrupts a run of the lingering program.
const SPIN_TIME: Duration = Duration::from_millis(20);
/// The bytes the sharing tests write.
const WRITTEN: &[u8] = &[u8::MAX, u8::MIN, u8::MAX];
/// One byte the sharing tests write over the others.
const ZERO: &[u8] = &[u8::MIN];

/// The accessors of a core's saved processor state: set the program counter, read the counter, set the counter.
pub type Registers<State> = (fn(&mut State, u64), fn(&State) -> u64, fn(&mut State, u64));

/// A program in a shard of `length` bytes that holds `bytes` at `offset` and zeros everywhere else.
#[derive(Clone, Copy)]
pub struct Program<'bytes> {
    pub length: u64,
    pub offset: u64,
    pub bytes: &'bytes [u8],
}

fn create_task<C: KernelCore>(core: &mut C) -> C::Task {
    match core.create_task() {
        Ok(task) => task,
        Err(_) => panic!("the suite needs the core to make tasks"),
    }
}

fn create_shard<C: KernelCore>(task: &mut C::Task, start: u64, length: u64, permissions: MemoryPermissions) {
    let created = C::memory_capabilities_mut(task).create(start, MemoryShardConfig { length, permissions });
    assert!(created.is_ok(), "the suite needs a task to hold a shard of {length} bytes at {start:#x}");
}

/// Checks that the task holds a shard at the start with the length and the permissions.
fn assert_holds<C: KernelCore>(task: &C::Task, start: u64, length: u64, permissions: MemoryPermissions) {
    let shard = C::memory_capabilities(task).get(&start).expect("a shard at the start");
    let held = shard.permissions();
    assert_eq!((shard.len(), held.read, held.write, held.execute), (length, permissions.read, permissions.write, permissions.execute));
}

/// Checks that the task holds no shard at an address from `start` up to the end of the address space.
///
/// Making a shard there must be refused for no reason that says something is held.
fn assert_holds_nothing_from<C: KernelCore>(task: &mut C::Task, start: u64) {
    let length = u64::MAX - start;
    let config = MemoryShardConfig { length, permissions: READ_WRITE };
    let created = C::memory_capabilities_mut(task).create(start, config);
    let refused_as_held = matches!(created, Err(CreateError::IdentifierTaken | CreateError::Failed(MemoryError::Overlap)));
    assert!(!refused_as_held, "the task holds something from {start:#x} up");
    if created.is_ok() {
        C::memory_capabilities_mut(task).drop(&start);
    }
}

/// Checks that the task holds no shard at an address from `start` for `length` bytes.
fn assert_holds_nothing_in<C: KernelCore>(task: &mut C::Task, start: u64, length: u64) {
    let config = MemoryShardConfig { length, permissions: READ_WRITE };
    let created = C::memory_capabilities_mut(task).create(start, config);
    let refused_as_held = matches!(created, Err(CreateError::IdentifierTaken | CreateError::Failed(MemoryError::Overlap)));
    assert!(!refused_as_held, "the task holds something in {length} bytes at {start:#x}");
    if created.is_ok() {
        C::memory_capabilities_mut(task).drop(&start);
    }
}

fn write_bytes<C: KernelCore>(task: &C::Task, start: u64, offset: u64, bytes: &[u8]) {
    let shard = C::memory_capabilities(task).get(&start).expect("a shard at the start");
    let view = shard.borrow(offset, bytes.len() as u64).ok().expect("the bytes inside the shard");
    assert_eq!(view.write_at(u64::MIN, bytes), bytes.len());
}

fn read_bytes<C: KernelCore>(task: &C::Task, start: u64, length: usize) -> Vec<u8> {
    let shard = C::memory_capabilities(task).get(&start).expect("a shard at the start");
    let view = shard.borrow(u64::MIN, length as u64).ok().expect("the bytes inside the shard");
    let mut read = vec![u8::MAX; length];
    assert_eq!(view.read_at(u64::MIN, &mut read), length);
    read
}

/// Makes a task whose program is at `CODE`, with its program counter there.
fn create_running_task<C: KernelCore>(core: &mut C, program: Program, registers: Registers<C::SavedProcessorState>) -> C::Task {
    let mut task = create_task(core);
    create_shard::<C>(&mut task, CODE, program.length, READ_EXECUTE);
    write_bytes::<C>(&task, CODE, program.offset, program.bytes);
    (registers.0)(C::saved_processor_state_mut(&mut task), CODE);
    task
}

fn to_page(bytes: &[u8]) -> Program<'_> {
    Program { length: SHARD_LENGTH, offset: u64::MIN, bytes }
}

fn run<C: KernelCore>(task: &mut C::Task) -> Stop {
    let (execution, _interrupter) = C::prepare_execution(task);
    execution.run()
}

/// Checks that a new task holds nothing, so nothing taken is in its way.
pub fn holds_no_memory_when_created<C: KernelCore>(mut core: C) {
    let mut task = create_task(&mut core);
    for start in [u64::MIN, CODE, DATA, u64::MAX] {
        assert!(C::memory_capabilities(&task).get(&start).is_none());
    }
    assert_holds_nothing_from::<C>(&mut task, u64::MIN);
}

/// Checks that memory one task makes is held by it and by no task made before or after it.
pub fn keeps_memory_to_its_task<C: KernelCore>(mut core: C) {
    let mut first = create_task(&mut core);
    let mut earlier = create_task(&mut core);
    create_shard::<C>(&mut first, DATA, SHARD_LENGTH, READ_WRITE);
    let mut later = create_task(&mut core);
    assert_holds::<C>(&first, DATA, SHARD_LENGTH, READ_WRITE);
    assert_holds_nothing_from::<C>(&mut earlier, u64::MIN);
    assert_holds_nothing_from::<C>(&mut later, u64::MIN);
    create_shard::<C>(&mut later, DATA, SHARD_LENGTH, READ_EXECUTE);
    assert_holds::<C>(&first, DATA, SHARD_LENGTH, READ_WRITE);
    assert_holds::<C>(&later, DATA, SHARD_LENGTH, READ_EXECUTE);
}

/// Checks that a duplicate in another task and one in the same task reach the bytes of the original.
pub fn shares_bytes_of_a_duplicate<C: KernelCore>(mut core: C) {
    let mut first = create_task(&mut core);
    let mut second = create_task(&mut core);
    create_shard::<C>(&mut first, DATA, SHARD_LENGTH, READ_WRITE);
    let duplicated = C::memory_capabilities_mut(&mut second).duplicate_from(C::memory_capabilities(&first), &DATA, COPY);
    assert!(duplicated.is_ok(), "the suite needs a duplicate between tasks");
    assert_holds::<C>(&second, COPY, SHARD_LENGTH, READ_WRITE);
    write_bytes::<C>(&first, DATA, u64::MIN, WRITTEN);
    assert_eq!(read_bytes::<C>(&second, COPY, WRITTEN.len()), WRITTEN);
    let duplicated = C::memory_capabilities_mut(&mut first).duplicate_within(&DATA, CODE);
    assert!(duplicated.is_ok(), "the suite needs a duplicate within a task");
    write_bytes::<C>(&first, CODE, u64::MIN, ZERO);
    assert_eq!(read_bytes::<C>(&second, COPY, ZERO.len()), ZERO);
    let missing = C::memory_capabilities_mut(&mut second).duplicate_from(C::memory_capabilities(&first), &COPY, u64::MIN);
    assert!(matches!(missing, Err(DuplicateError::NothingHeld)));
    let taken = C::memory_capabilities_mut(&mut first).duplicate_within(&DATA, CODE);
    assert!(matches!(taken, Err(DuplicateError::IdentifierTaken)));
}

/// Checks that permissions belong to one entry: changing them leaves the duplicate's, and a split keeps them.
pub fn keeps_permissions_per_entry<C: KernelCore>(mut core: C) {
    let mut first = create_task(&mut core);
    let mut second = create_task(&mut core);
    create_shard::<C>(&mut first, DATA, SHARD_LENGTH, READ_WRITE);
    let duplicated = C::memory_capabilities_mut(&mut second).duplicate_from(C::memory_capabilities(&first), &DATA, DATA);
    assert!(duplicated.is_ok(), "the suite needs a duplicate between tasks");
    let set = C::memory_capabilities_mut(&mut second).set_permissions(&DATA, READ_EXECUTE);
    assert!(set.is_ok(), "the suite needs permissions to change");
    assert_holds::<C>(&first, DATA, SHARD_LENGTH, READ_WRITE);
    assert_holds::<C>(&second, DATA, SHARD_LENGTH, READ_EXECUTE);
    let missing = C::memory_capabilities_mut(&mut second).set_permissions(&CODE, READ_EXECUTE);
    assert!(matches!(missing, Err(SetPermissionsError::NothingHeld)));
    assert!(matches!(C::memory_capabilities_mut(&mut first).split(&DATA, SHARD_LENGTH), Err(SplitError::OutsideShard)));
    assert!(matches!(C::memory_capabilities_mut(&mut first).split(&DATA, u64::MIN), Err(SplitError::OutsideShard)));
    assert!(matches!(C::memory_capabilities_mut(&mut first).split(&CODE, HALF_SHARD_LENGTH), Err(SplitError::NothingHeld)));
    let split = C::memory_capabilities_mut(&mut first).split(&DATA, HALF_SHARD_LENGTH);
    assert!(split.is_ok(), "the suite needs a shard to split");
    assert_holds::<C>(&first, DATA, HALF_SHARD_LENGTH, READ_WRITE);
    assert_holds::<C>(&first, DATA_MIDDLE, HALF_SHARD_LENGTH, READ_WRITE);
}

/// Checks that a shard sharing even one address with one the task holds is refused, which keeps the task's shards apart.
pub fn keeps_shards_apart<C: KernelCore>(mut core: C) {
    let mut task = create_task(&mut core);
    create_shard::<C>(&mut task, DATA, SHARD_LENGTH, READ_WRITE);
    for start in [DATA_MIDDLE, DATA_LAST, DATA_BELOW_BY_ONE] {
        let created = C::memory_capabilities_mut(&mut task).create(start, MemoryShardConfig { length: SHARD_LENGTH, permissions: READ_WRITE });
        assert!(matches!(created, Err(CreateError::Failed(MemoryError::Overlap))));
        let duplicated = C::memory_capabilities_mut(&mut task).duplicate_within(&DATA, start);
        assert!(matches!(duplicated, Err(DuplicateError::Failed(MemoryError::Overlap))));
        assert!(C::memory_capabilities(&task).get(&start).is_none());
    }
    let taken = C::memory_capabilities_mut(&mut task).create(DATA, MemoryShardConfig { length: SHARD_LENGTH, permissions: READ_EXECUTE });
    assert!(matches!(taken, Err(CreateError::IdentifierTaken)));
    create_shard::<C>(&mut task, DATA_BELOW, SHARD_LENGTH, READ_WRITE);
    create_shard::<C>(&mut task, DATA_END, SHARD_LENGTH, READ_WRITE);
    C::memory_capabilities_mut(&mut task).drop(&DATA_END);
    let split = C::memory_capabilities_mut(&mut task).split(&DATA, HALF_SHARD_LENGTH);
    assert!(split.is_ok(), "the suite needs a shard to split");
    let created = C::memory_capabilities_mut(&mut task).create(DATA_LAST, MemoryShardConfig { length: SHARD_LENGTH, permissions: READ_WRITE });
    assert!(matches!(created, Err(CreateError::Failed(MemoryError::Overlap))));
    C::memory_capabilities_mut(&mut task).drop(&DATA);
    assert!(C::memory_capabilities(&task).get(&DATA).is_none());
    assert_holds_nothing_in::<C>(&mut task, DATA, HALF_SHARD_LENGTH);
    assert_holds::<C>(&task, DATA_MIDDLE, HALF_SHARD_LENGTH, READ_WRITE);
    assert_holds_nothing_from::<C>(&mut task, DATA_END);
}

/// Checks that a run leaves the task holding the same shards, with their permissions and bytes, and nothing else.
pub fn keeps_memory_through_a_run<C: KernelCore>(mut core: C, counting: &[u8], registers: Registers<C::SavedProcessorState>) {
    let mut task = create_running_task(&mut core, to_page(counting), registers);
    create_shard::<C>(&mut task, DATA, SHARD_LENGTH, READ_WRITE);
    write_bytes::<C>(&task, DATA, u64::MIN, WRITTEN);
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
    assert_holds::<C>(&task, CODE, SHARD_LENGTH, READ_EXECUTE);
    assert_holds::<C>(&task, DATA, SHARD_LENGTH, READ_WRITE);
    assert_eq!(read_bytes::<C>(&task, CODE, counting.len()), counting);
    assert_eq!(read_bytes::<C>(&task, DATA, WRITTEN.len()), WRITTEN);
    assert_holds_nothing_in::<C>(&mut task, u64::MIN, CODE);
    assert_holds_nothing_in::<C>(&mut task, CODE_END, BETWEEN_CODE_AND_DATA);
    assert_holds_nothing_from::<C>(&mut task, DATA_END);
}

/// Checks that each run starts from the processor state the last stop saved, as changed since.
pub fn resumes_from_the_saved_state<C: KernelCore>(mut core: C, counting: &[u8], registers: Registers<C::SavedProcessorState>) {
    let mut task = create_running_task(&mut core, to_page(counting), registers);
    let (_, read_counter, set_counter) = registers;
    let start = read_counter(C::saved_processor_state(&task));
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
    let first = read_counter(C::saved_processor_state(&task));
    assert_eq!(first, start + 1);
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
    assert_eq!(read_counter(C::saved_processor_state(&task)), first + 1);
    set_counter(C::saved_processor_state_mut(&mut task), SHARD_LENGTH);
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
    assert_eq!(read_counter(C::saved_processor_state(&task)), SHARD_LENGTH + 1);
}

/// Checks that a run reports the address and the access that nothing the task holds allows.
pub fn faults_where_nothing_allows<C: KernelCore>(mut core: C, counting: &[u8], registers: Registers<C::SavedProcessorState>) {
    let mut task = create_running_task(&mut core, to_page(counting), registers);
    create_shard::<C>(&mut task, DATA, SHARD_LENGTH, READ_WRITE);
    (registers.0)(C::saved_processor_state_mut(&mut task), DATA);
    assert!(matches!(run::<C>(&mut task), Stop::MemoryFault { address: DATA, access: Access::Execute }));
    (registers.0)(C::saved_processor_state_mut(&mut task), COPY);
    assert!(matches!(run::<C>(&mut task), Stop::MemoryFault { address: COPY, access: Access::Execute }));
    (registers.0)(C::saved_processor_state_mut(&mut task), CODE);
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
}

/// Checks that a run nobody interrupts goes on until its program stops it, however long that takes.
pub fn runs_until_its_program_stops<C: KernelCore>(mut core: C, lingering: Program, registers: Registers<C::SavedProcessorState>) {
    let mut task = create_running_task(&mut core, lingering, registers);
    let (execution, _interrupter) = C::prepare_execution(&mut task);
    assert!(matches!(execution.run(), Stop::SystemCall));
}

/// Checks that an interrupter stops the run it was prepared with, before or during it, and no later run.
pub fn interrupts_only_its_own_run<C: KernelCore>(mut core: C, lingering: Program, registers: Registers<C::SavedProcessorState>) {
    let mut task = create_running_task(&mut core, lingering, registers);
    let (execution, interrupter) = C::prepare_execution(&mut task);
    interrupter.interrupt();
    assert!(matches!(execution.run(), Stop::Interrupt));
    (registers.0)(C::saved_processor_state_mut(&mut task), CODE);
    let (execution, interrupter) = C::prepare_execution(&mut task);
    let stop = thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(SPIN_TIME);
            interrupter.interrupt();
        });
        execution.run()
    });
    assert!(matches!(stop, Stop::Interrupt));
    interrupter.interrupt();
    let (unrun, unused) = C::prepare_execution(&mut task);
    drop(unrun);
    unused.interrupt();
    (registers.0)(C::saved_processor_state_mut(&mut task), CODE);
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
}

/// Runs each suite function above as its own test on a core made anew for every test.
///
/// The arguments are the core, a program that adds one to the counter and makes a system call over and over within one page,
/// a `Program` that makes a system call only after running for far longer than the interrupting thread waits, and the core's
/// `Registers`.
#[macro_export]
macro_rules! tests {
    ($core:expr, $counting:expr, $lingering:expr, $registers:expr) => {
        #[test]
        fn holds_no_memory_when_created() {
            $crate::holds_no_memory_when_created($core);
        }

        #[test]
        fn keeps_memory_to_its_task() {
            $crate::keeps_memory_to_its_task($core);
        }

        #[test]
        fn shares_bytes_of_a_duplicate() {
            $crate::shares_bytes_of_a_duplicate($core);
        }

        #[test]
        fn keeps_permissions_per_entry() {
            $crate::keeps_permissions_per_entry($core);
        }

        #[test]
        fn keeps_shards_apart() {
            $crate::keeps_shards_apart($core);
        }

        #[test]
        fn keeps_memory_through_a_run() {
            $crate::keeps_memory_through_a_run($core, &$counting, $registers);
        }

        #[test]
        fn resumes_from_the_saved_state() {
            $crate::resumes_from_the_saved_state($core, &$counting, $registers);
        }

        #[test]
        fn faults_where_nothing_allows() {
            $crate::faults_where_nothing_allows($core, &$counting, $registers);
        }

        #[test]
        fn runs_until_its_program_stops() {
            $crate::runs_until_its_program_stops($core, $lingering, $registers);
        }

        #[test]
        fn interrupts_only_its_own_run() {
            $crate::interrupts_only_its_own_run($core, $lingering, $registers);
        }
    };
}
