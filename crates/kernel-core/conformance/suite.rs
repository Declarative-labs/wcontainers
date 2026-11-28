//! The tests every implementation of wcontainers-kernel-core runs, written against its traits alone.
//!
//! The tests that run a task take, beside the core, two programs in its executor's instructions and
//! the accessors of its saved processor state that the trait cannot name.

use std::thread;
use std::time::Duration;

use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_kernel_core::KernelCore;
use wcontainers_kernel_core::execution::{Access, Execution, Interrupter, Stop};
use wcontainers_kernel_core::memory::{MemoryCapabilityTable, SetPermissionsError, SplitError};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};
use wcontainers_volatile_bytes::VolatileBytes;

/// The length of every shard the tests make.
const SHARD_LENGTH: u64 = u8::MAX as u64 + 1;
/// Where the tests put a program.
const CODE: u64 = u16::MAX as u64 + 1;
/// Where the tests put data.
const DATA: u64 = CODE + CODE;
/// Where the tests put a duplicate.
const COPY: u64 = DATA + CODE;
/// Every address the tests make a shard at, and the two ends of the address space.
const PROBES: &[u64] = &[u64::MIN, CODE, DATA, COPY, u64::MAX];
const READ_WRITE: MemoryPermissions = MemoryPermissions { read: true, write: true, execute: false };
const READ_EXECUTE: MemoryPermissions = MemoryPermissions { read: true, write: false, execute: true };
/// How long the interrupting thread waits before it interrupts a run that spins.
const SPIN_TIME: Duration = Duration::from_millis(u8::MAX as u64);
/// Half the length of every shard the tests make.
const HALF_SHARD_LENGTH: u64 = i8::MAX as u64 + 1;
/// The bytes the sharing tests write.
const WRITTEN: &[u8] = &[u8::MAX, u8::MIN, u8::MAX];
/// One byte the sharing tests write over the others.
const ZERO: &[u8] = &[u8::MIN];

/// The accessors of a core's saved processor state: set the program counter, read the counter, set the counter.
pub type Registers<State> = (fn(&mut State, u64), fn(&State) -> u64, fn(&mut State, u64));

fn create_task<C: KernelCore>(core: &mut C) -> C::Task {
    match core.create_task() {
        Ok(task) => task,
        Err(_) => panic!("the suite needs the core to make tasks"),
    }
}

fn create_shard<C: KernelCore>(task: &mut C::Task, start: u64, permissions: MemoryPermissions) {
    let created = C::memory_capabilities_mut(task).create(start, MemoryShardConfig { length: SHARD_LENGTH, permissions });
    assert!(created.is_ok(), "the suite needs a task to hold a shard of {SHARD_LENGTH} bytes at {start:#x}");
}

/// Checks that the task holds a shard at the start with the suite's length and the permissions.
fn assert_holds<C: KernelCore>(task: &C::Task, start: u64, permissions: MemoryPermissions) {
    let shard = C::memory_capabilities(task).get(&start).expect("a shard at the start");
    assert_eq!(shard.len(), SHARD_LENGTH);
    let held = shard.permissions();
    assert_eq!((held.read, held.write, held.execute), (permissions.read, permissions.write, permissions.execute));
}

fn write_bytes<C: KernelCore>(task: &C::Task, start: u64, bytes: &[u8]) {
    let shard = C::memory_capabilities(task).get(&start).expect("a shard at the start");
    let view = shard.borrow(u64::MIN, SHARD_LENGTH).ok().expect("the whole shard");
    assert_eq!(view.write_at(u64::MIN, bytes), bytes.len());
}

fn read_bytes<C: KernelCore>(task: &C::Task, start: u64, length: usize) -> Vec<u8> {
    let shard = C::memory_capabilities(task).get(&start).expect("a shard at the start");
    let view = shard.borrow(u64::MIN, SHARD_LENGTH).ok().expect("the whole shard");
    let mut read = vec![u8::MIN; length];
    assert_eq!(view.read_at(u64::MIN, &mut read), length);
    read
}

/// Makes a task whose program is at `CODE`, with its program counter there.
fn create_running_task<C: KernelCore>(core: &mut C, program: &[u8], registers: Registers<C::SavedProcessorState>) -> C::Task {
    let mut task = create_task(core);
    create_shard::<C>(&mut task, CODE, READ_EXECUTE);
    write_bytes::<C>(&task, CODE, program);
    (registers.0)(C::saved_processor_state_mut(&mut task), CODE);
    task
}

fn run<C: KernelCore>(task: &mut C::Task) -> Stop {
    let (execution, _interrupter) = C::prepare_execution(task);
    execution.run()
}

/// Checks that a new task holds nothing, so nothing taken is in its way.
pub fn holds_no_memory_when_created<C: KernelCore>(mut core: C) {
    let mut task = create_task(&mut core);
    for &start in PROBES {
        assert!(C::memory_capabilities(&task).get(&start).is_none());
        let created = C::memory_capabilities_mut(&mut task).create(start, MemoryShardConfig { length: u64::MIN, permissions: READ_WRITE });
        assert!(!matches!(created, Err(CreateError::IdentifierTaken)));
    }
}

/// Checks that memory one task makes is held by it and by no task made before or after it.
pub fn keeps_memory_to_its_task<C: KernelCore>(mut core: C) {
    let mut first = create_task(&mut core);
    let earlier = create_task(&mut core);
    create_shard::<C>(&mut first, DATA, READ_WRITE);
    let later = create_task(&mut core);
    assert_holds::<C>(&first, DATA, READ_WRITE);
    for other in [&earlier, &later] {
        for &start in PROBES {
            assert!(C::memory_capabilities(other).get(&start).is_none());
        }
    }
    let mut later = later;
    create_shard::<C>(&mut later, DATA, READ_EXECUTE);
    assert_holds::<C>(&first, DATA, READ_WRITE);
    assert_holds::<C>(&later, DATA, READ_EXECUTE);
}

/// Checks that a duplicate in another task and one in the same task reach the bytes of the original.
pub fn shares_bytes_of_a_duplicate<C: KernelCore>(mut core: C) {
    let mut first = create_task(&mut core);
    let mut second = create_task(&mut core);
    create_shard::<C>(&mut first, DATA, READ_WRITE);
    let duplicated = C::memory_capabilities_mut(&mut second).duplicate_from(C::memory_capabilities(&first), &DATA, COPY);
    assert!(duplicated.is_ok(), "the suite needs a duplicate between tasks");
    assert_holds::<C>(&second, COPY, READ_WRITE);
    write_bytes::<C>(&first, DATA, WRITTEN);
    assert_eq!(read_bytes::<C>(&second, COPY, WRITTEN.len()), WRITTEN);
    write_bytes::<C>(&second, COPY, WRITTEN);
    let duplicated = C::memory_capabilities_mut(&mut first).duplicate_within(&DATA, CODE);
    assert!(duplicated.is_ok(), "the suite needs a duplicate within a task");
    write_bytes::<C>(&first, CODE, ZERO);
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
    create_shard::<C>(&mut first, DATA, READ_WRITE);
    let duplicated = C::memory_capabilities_mut(&mut second).duplicate_from(C::memory_capabilities(&first), &DATA, DATA);
    assert!(duplicated.is_ok(), "the suite needs a duplicate between tasks");
    let set = C::memory_capabilities_mut(&mut second).set_permissions(&DATA, READ_EXECUTE);
    assert!(set.is_ok(), "the suite needs permissions to change");
    assert_holds::<C>(&first, DATA, READ_WRITE);
    assert_holds::<C>(&second, DATA, READ_EXECUTE);
    let missing = C::memory_capabilities_mut(&mut second).set_permissions(&CODE, READ_EXECUTE);
    assert!(matches!(missing, Err(SetPermissionsError::NothingHeld)));
    let half = HALF_SHARD_LENGTH;
    assert!(matches!(C::memory_capabilities_mut(&mut first).split(&DATA, SHARD_LENGTH), Err(SplitError::OutsideShard)));
    assert!(matches!(C::memory_capabilities_mut(&mut first).split(&CODE, half), Err(SplitError::NothingHeld)));
    let split = C::memory_capabilities_mut(&mut first).split(&DATA, half);
    assert!(split.is_ok(), "the suite needs a shard to split");
    for start in [DATA, DATA + half] {
        let shard = C::memory_capabilities(&first).get(&start).expect("both halves");
        assert_eq!(shard.len(), half);
        assert!(shard.permissions().write && !shard.permissions().execute);
    }
}

/// Checks that a shard overlapping one the task holds is refused, which keeps the task's shards apart.
pub fn keeps_shards_apart<C: KernelCore>(mut core: C) {
    let mut task = create_task(&mut core);
    create_shard::<C>(&mut task, DATA, READ_WRITE);
    let inside = DATA + HALF_SHARD_LENGTH;
    let created = C::memory_capabilities_mut(&mut task).create(inside, MemoryShardConfig { length: SHARD_LENGTH, permissions: READ_WRITE });
    assert!(matches!(created, Err(CreateError::Failed(_))));
    let below = DATA - HALF_SHARD_LENGTH;
    let duplicated = C::memory_capabilities_mut(&mut task).duplicate_within(&DATA, below);
    assert!(matches!(duplicated, Err(DuplicateError::Failed(_))));
    assert!(C::memory_capabilities(&task).get(&inside).is_none());
    assert!(C::memory_capabilities(&task).get(&below).is_none());
    let taken = C::memory_capabilities_mut(&mut task).create(DATA, MemoryShardConfig { length: SHARD_LENGTH, permissions: READ_EXECUTE });
    assert!(matches!(taken, Err(CreateError::IdentifierTaken)));
    C::memory_capabilities_mut(&mut task).drop(&DATA);
    assert!(C::memory_capabilities(&task).get(&DATA).is_none());
    create_shard::<C>(&mut task, inside, READ_WRITE);
}

/// Checks that a run leaves the task holding the same shards, with their permissions and bytes.
pub fn keeps_memory_through_a_run<C: KernelCore>(mut core: C, counting: &[u8], registers: Registers<C::SavedProcessorState>) {
    let mut task = create_running_task(&mut core, counting, registers);
    create_shard::<C>(&mut task, DATA, READ_WRITE);
    write_bytes::<C>(&task, DATA, WRITTEN);
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
    assert_holds::<C>(&task, CODE, READ_EXECUTE);
    assert_holds::<C>(&task, DATA, READ_WRITE);
    assert_eq!(read_bytes::<C>(&task, CODE, counting.len()), counting);
    assert_eq!(read_bytes::<C>(&task, DATA, WRITTEN.len()), WRITTEN);
    for start in [u64::MIN, COPY, u64::MAX] {
        assert!(C::memory_capabilities(&task).get(&start).is_none());
    }
}

/// Checks that each run starts from the processor state the last stop saved, as changed since.
pub fn resumes_from_the_saved_state<C: KernelCore>(mut core: C, counting: &[u8], registers: Registers<C::SavedProcessorState>) {
    let mut task = create_running_task(&mut core, counting, registers);
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
    let mut task = create_running_task(&mut core, counting, registers);
    create_shard::<C>(&mut task, DATA, READ_WRITE);
    (registers.0)(C::saved_processor_state_mut(&mut task), DATA);
    assert!(matches!(run::<C>(&mut task), Stop::MemoryFault { address: DATA, access: Access::Execute }));
    (registers.0)(C::saved_processor_state_mut(&mut task), COPY);
    assert!(matches!(run::<C>(&mut task), Stop::MemoryFault { address: COPY, access: Access::Execute }));
    (registers.0)(C::saved_processor_state_mut(&mut task), CODE);
    assert!(matches!(run::<C>(&mut task), Stop::SystemCall));
}

/// Checks that an interrupter stops the run it was prepared with, before or during it, and no later run.
pub fn interrupts_only_its_own_run<C: KernelCore>(mut core: C, counting: &[u8], spinning: &[u8], registers: Registers<C::SavedProcessorState>) {
    let mut spinner = create_running_task(&mut core, spinning, registers);
    let (execution, interrupter) = C::prepare_execution(&mut spinner);
    interrupter.interrupt();
    assert!(matches!(execution.run(), Stop::Interrupt));
    let (execution, interrupter) = C::prepare_execution(&mut spinner);
    let stop = thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(SPIN_TIME);
            interrupter.interrupt();
        });
        execution.run()
    });
    assert!(matches!(stop, Stop::Interrupt));
    let mut counter = create_running_task(&mut core, counting, registers);
    let (execution, earlier) = C::prepare_execution(&mut counter);
    assert!(matches!(execution.run(), Stop::SystemCall));
    earlier.interrupt();
    interrupter.interrupt();
    assert!(matches!(run::<C>(&mut counter), Stop::SystemCall));
    let (unrun, unused) = C::prepare_execution(&mut counter);
    drop(unrun);
    unused.interrupt();
    let before = (registers.1)(C::saved_processor_state(&counter));
    assert!(matches!(run::<C>(&mut counter), Stop::SystemCall));
    assert_eq!((registers.1)(C::saved_processor_state(&counter)), before + 1);
}

/// Runs each suite function above as its own test on a core made anew for every test.
///
/// The arguments are the core, a program that adds one to the counter and makes a system call over and over, a program that never stops by
/// itself, and the core's `Registers`.
#[macro_export]
macro_rules! tests {
    ($core:expr, $counting:expr, $spinning:expr, $registers:expr) => {
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
        fn interrupts_only_its_own_run() {
            $crate::interrupts_only_its_own_run($core, &$counting, &$spinning, $registers);
        }
    };
}
