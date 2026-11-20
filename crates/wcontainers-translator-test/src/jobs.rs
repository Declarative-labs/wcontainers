//! Cases run at once on one pool of threads. The translator's and wasmtime's own parallel work runs on the same
//! pool, so a run keeps to its number of jobs.

use rayon::prelude::*;

/// The stack of each thread that runs cases.
const CASE_THREAD_STACK_BYTES: usize = 64 << 20;

/// Call `case` with each of `0..cases`, `jobs` at once. A case may start while another waits on the same
/// thread, so a case holds nothing of its thread's.
pub fn at_once(jobs: u64, cases: u64, case: impl Fn(u64) + Sync + Send) {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs.max(1) as usize)
        .stack_size(CASE_THREAD_STACK_BYTES)
        .build()
        .expect("a pool of threads");
    pool.install(|| (0..cases).into_par_iter().for_each(case));
}
