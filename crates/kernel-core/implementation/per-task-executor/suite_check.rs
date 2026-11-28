//! Checks the suite itself: each test runs a suite function on a core with one flaw and expects it to fail.

mod doubles;

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use wcontainers_capability_table::{CapabilityTable, CreateError, DuplicateError};
use wcontainers_kernel_core::execution::Stop;
use wcontainers_kernel_core_conformance::Program;
use wcontainers_kernel_core_per_task_executor::PerTaskExecutorKernelCore;
use wcontainers_kernel_core_per_task_executor::byte_code::{ByteCodeExecutor, ByteCodeProcessor, FlagInterrupter, Instruction};
use wcontainers_kernel_core_per_task_executor::disjoint_memory::DisjointMemory;
use wcontainers_kernel_core_per_task_executor::executor_boundary::{ExecutorBoundary, TaskSource};
use wcontainers_memory_capability_table::{MemoryCapabilityTable, MemoryError, SetPermissionsError, SplitError};
use wcontainers_memory_shard::{MemoryPermissions, MemoryShard, MemoryShardConfig};

use crate::doubles::{SparseShard, SparseTable};

/// How many times the counting program counts and calls, which fills most of one page.
const COUNTING_ROUNDS: usize = 0x7F;
/// How many instructions that do nothing the lingering program runs before its system call.
const LINGERING_STEPS: u64 = 0x400_0000;
const SYSTEM_CALL: &[u8] = &[Instruction::SystemCall as u8];
const LINGERING: Program = Program { length: LINGERING_STEPS + 1, offset: LINGERING_STEPS, bytes: SYSTEM_CALL };
/// An address near the top of the address space, which no suite shard holds.
const HIGH: u64 = 0xFFFF_0000;
/// An address between the suite's program and its data, which no suite shard holds.
const BETWEEN: u64 = 0x1_8000;
/// How long a flawed run lasts before it calls itself interrupted.
const SLICE: Duration = Duration::from_millis(5);

#[derive(Clone, Copy)]
enum Flaw {
    /// Refuses as an overlap every shard that would hold the address, as if a shard were held there.
    HoldsAt(u64),
    /// Answers that permissions changed and changes nothing.
    KeepsPermissions,
    /// Makes a duplicate from another table out of fresh bytes.
    CopiesDuplicates,
    /// Starts every run with the counter at zero.
    ResetsCounter,
    /// Names the address after the one that faulted.
    FaultsOneLater,
    /// Ends a run that lasts longer than `SLICE` and calls it interrupted.
    EndsSlices,
    /// Runs without looking at its interrupter.
    IgnoresInterrupter,
}

struct FlawedTable {
    table: SparseTable,
    flaw: Flaw,
}

impl CapabilityTable<u64, SparseShard, MemoryShardConfig, MemoryError> for FlawedTable {
    fn create(&mut self, identifier: u64, config: MemoryShardConfig) -> Result<(), CreateError<MemoryError>> {
        if let Flaw::HoldsAt(address) = self.flaw
            && identifier <= address
            && address - identifier < config.length
        {
            return Err(CreateError::Failed(MemoryError::Overlap));
        }
        self.table.create(identifier, config)
    }

    fn get(&self, identifier: &u64) -> Option<&SparseShard> {
        self.table.get(identifier)
    }

    fn drop(&mut self, identifier: &u64) {
        self.table.drop(identifier);
    }

    fn duplicate_within(&mut self, identifier: &u64, new_identifier: u64) -> Result<(), DuplicateError<MemoryError>> {
        self.table.duplicate_within(identifier, new_identifier)
    }

    fn duplicate_from(&mut self, other: &Self, identifier: &u64, new_identifier: u64) -> Result<(), DuplicateError<MemoryError>> {
        match (self.flaw, other.table.get(identifier)) {
            (Flaw::CopiesDuplicates, Some(shard)) => {
                let config = MemoryShardConfig { length: shard.len(), permissions: shard.permissions() };
                self.table.create(new_identifier, config).map_err(|_| DuplicateError::IdentifierTaken)
            },
            _ => self.table.duplicate_from(&other.table, identifier, new_identifier),
        }
    }
}

impl MemoryCapabilityTable<SparseShard> for FlawedTable {
    fn set_permissions(&mut self, address: &u64, permissions: MemoryPermissions) -> Result<(), SetPermissionsError> {
        match (self.flaw, self.table.get(address)) {
            (Flaw::KeepsPermissions, Some(_)) => Ok(()),
            _ => self.table.set_permissions(address, permissions),
        }
    }

    fn split(&mut self, address: &u64, offset: u64) -> Result<(), SplitError> {
        self.table.split(address, offset)
    }
}

struct FlawedRunner {
    runner: ByteCodeExecutor,
    flaw: Flaw,
}

impl ExecutorBoundary<SparseShard, FlawedTable> for FlawedRunner {
    type ProcessorState = ByteCodeProcessor;

    type Interruption = Arc<AtomicBool>;

    type Interrupter = FlagInterrupter;

    fn prepare_interruption() -> (Arc<AtomicBool>, FlagInterrupter) {
        <ByteCodeExecutor as ExecutorBoundary<SparseShard, FlawedTable>>::prepare_interruption()
    }

    fn run(&mut self, memory: &DisjointMemory<SparseShard, FlawedTable>, processor: &mut ByteCodeProcessor, interruption: Arc<AtomicBool>) -> Stop {
        let interruption = match self.flaw {
            Flaw::IgnoresInterrupter => Arc::default(),
            Flaw::EndsSlices => {
                let slice = Arc::clone(&interruption);
                thread::spawn(move || {
                    thread::sleep(SLICE);
                    slice.store(true, Ordering::SeqCst);
                });
                interruption
            },
            _ => interruption,
        };
        if let Flaw::ResetsCounter = self.flaw {
            processor.counter = u64::MIN;
        }
        match self.runner.run(memory, processor, interruption) {
            Stop::MemoryFault { address, access } if matches!(self.flaw, Flaw::FaultsOneLater) => Stop::MemoryFault { address: address + 1, access },
            stop => stop,
        }
    }
}

struct FlawedParts {
    flaw: Flaw,
}

impl TaskSource<SparseShard> for FlawedParts {
    type Table = FlawedTable;

    type Runner = FlawedRunner;

    type CreateError = Infallible;

    fn create_parts(&mut self) -> Result<(FlawedTable, FlawedRunner, ByteCodeProcessor), Infallible> {
        let processor = ByteCodeProcessor { program_counter: u64::MIN, counter: u64::MIN };
        let table = FlawedTable { table: SparseTable::default(), flaw: self.flaw };
        Ok((table, FlawedRunner { runner: ByteCodeExecutor, flaw: self.flaw }, processor))
    }
}

fn create_core(flaw: Flaw) -> PerTaskExecutorKernelCore<SparseShard, FlawedParts> {
    PerTaskExecutorKernelCore::new(FlawedParts { flaw })
}

fn set_program_counter(processor: &mut ByteCodeProcessor, address: u64) {
    processor.program_counter = address;
}

fn read_counter(processor: &ByteCodeProcessor) -> u64 {
    processor.counter
}

fn set_counter(processor: &mut ByteCodeProcessor, counter: u64) {
    processor.counter = counter;
}

fn count() -> Vec<u8> {
    [Instruction::Count as u8, Instruction::SystemCall as u8].repeat(COUNTING_ROUNDS)
}

#[test]
#[should_panic(expected = "the task holds something from 0x0 up")]
fn finds_memory_held_when_created() {
    wcontainers_kernel_core_conformance::holds_no_memory_when_created(create_core(Flaw::HoldsAt(HIGH)));
}

#[test]
#[should_panic(expected = "the task holds something from 0x0 up")]
fn finds_memory_another_task_made() {
    wcontainers_kernel_core_conformance::keeps_memory_to_its_task(create_core(Flaw::HoldsAt(HIGH)));
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn finds_a_duplicate_that_copies() {
    wcontainers_kernel_core_conformance::shares_bytes_of_a_duplicate(create_core(Flaw::CopiesDuplicates));
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn finds_permissions_that_do_not_change() {
    wcontainers_kernel_core_conformance::keeps_permissions_per_entry(create_core(Flaw::KeepsPermissions));
}

#[test]
#[should_panic(expected = "the task holds something from 0x21000 up")]
fn finds_a_shard_held_past_the_others() {
    wcontainers_kernel_core_conformance::keeps_shards_apart(create_core(Flaw::HoldsAt(HIGH)));
}

#[test]
#[should_panic(expected = "the task holds something in 61440 bytes at 0x11000")]
fn finds_a_shard_held_between_after_a_run() {
    wcontainers_kernel_core_conformance::keeps_memory_through_a_run(create_core(Flaw::HoldsAt(BETWEEN)), &count(), (set_program_counter, read_counter, set_counter));
}

#[test]
#[should_panic(expected = "the task holds something from 0x21000 up")]
fn finds_a_shard_held_above_after_a_run() {
    wcontainers_kernel_core_conformance::keeps_memory_through_a_run(create_core(Flaw::HoldsAt(HIGH)), &count(), (set_program_counter, read_counter, set_counter));
}

#[test]
#[should_panic(expected = "assertion `left == right` failed")]
fn finds_a_run_that_forgets_the_counter() {
    wcontainers_kernel_core_conformance::resumes_from_the_saved_state(create_core(Flaw::ResetsCounter), &count(), (set_program_counter, read_counter, set_counter));
}

#[test]
#[should_panic(expected = "address: DATA, access: Access::Execute")]
fn finds_a_fault_at_the_wrong_address() {
    wcontainers_kernel_core_conformance::faults_where_nothing_allows(create_core(Flaw::FaultsOneLater), &count(), (set_program_counter, read_counter, set_counter));
}

#[test]
#[should_panic(expected = "Stop::SystemCall")]
fn finds_a_run_that_ends_its_slice_as_interrupted() {
    wcontainers_kernel_core_conformance::runs_until_its_program_stops(create_core(Flaw::EndsSlices), LINGERING, (set_program_counter, read_counter, set_counter));
}

#[test]
#[should_panic(expected = "Stop::Interrupt")]
fn finds_an_interrupter_that_does_nothing() {
    wcontainers_kernel_core_conformance::interrupts_only_its_own_run(create_core(Flaw::IgnoresInterrupter), LINGERING, (set_program_counter, read_counter, set_counter));
}
