//! Runs the suite on the core with the byte-code executor and the test table of sparse shards.

mod doubles;
mod programs;

use std::convert::Infallible;

use wcontainers_kernel_core_per_task_executor::PerTaskExecutorKernelCore;
use wcontainers_kernel_core_per_task_executor::byte_code::{ByteCodeExecutor, ByteCodeProcessor};
use wcontainers_kernel_core_per_task_executor::executor_boundary::TaskSource;

use crate::doubles::{SparseShard, SparseTable};
use crate::programs::{LINGERING, count, read_counter, set_counter, set_program_counter};

struct SparseParts;

impl TaskSource<SparseShard> for SparseParts {
    type Table = SparseTable;

    type Runner = ByteCodeExecutor;

    type CreateError = Infallible;

    fn create_parts(
        &mut self,
    ) -> Result<(SparseTable, ByteCodeExecutor, ByteCodeProcessor), Infallible> {
        let processor = ByteCodeProcessor {
            program_counter: u64::MIN,
            counter: u64::MIN,
        };
        Ok((SparseTable::default(), ByteCodeExecutor, processor))
    }
}

wcontainers_kernel_core_conformance::tests!(
    PerTaskExecutorKernelCore::<SparseShard, SparseParts>::new(SparseParts),
    count(),
    LINGERING,
    (set_program_counter, read_counter, set_counter)
);
