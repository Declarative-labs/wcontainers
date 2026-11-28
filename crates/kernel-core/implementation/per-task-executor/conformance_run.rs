//! Runs the suite on the core with the byte-code executor and the test table of sparse shards.

mod doubles;

use std::convert::Infallible;

use wcontainers_kernel_core_conformance::Program;
use wcontainers_kernel_core_per_task_executor::PerTaskExecutorKernelCore;
use wcontainers_kernel_core_per_task_executor::byte_code::{ByteCodeExecutor, ByteCodeProcessor, Instruction};
use wcontainers_kernel_core_per_task_executor::executor_boundary::TaskSource;

use crate::doubles::{SparseShard, SparseTable};

/// How many times the counting program counts and calls, which fills most of one page.
const COUNTING_ROUNDS: usize = 0x7F;
/// How many instructions that do nothing the lingering program runs before its system call.
const LINGERING_STEPS: u64 = 0x400_0000;
const SYSTEM_CALL: &[u8] = &[Instruction::SystemCall as u8];
const LINGERING: Program = Program { length: LINGERING_STEPS + 1, offset: LINGERING_STEPS, bytes: SYSTEM_CALL };

struct SparseParts;

impl TaskSource<SparseShard> for SparseParts {
    type Table = SparseTable;

    type Runner = ByteCodeExecutor;

    type CreateError = Infallible;

    fn create_parts(&mut self) -> Result<(SparseTable, ByteCodeExecutor, ByteCodeProcessor), Infallible> {
        let processor = ByteCodeProcessor {
            program_counter: u64::MIN,
            counter: u64::MIN,
        };
        Ok((SparseTable::default(), ByteCodeExecutor, processor))
    }
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

wcontainers_kernel_core_conformance::tests!(
    PerTaskExecutorKernelCore::<SparseShard, SparseParts>::new(SparseParts),
    [Instruction::Count as u8, Instruction::SystemCall as u8].repeat(COUNTING_ROUNDS),
    LINGERING,
    (set_program_counter, read_counter, set_counter)
);
