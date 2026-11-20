//! Cases run at once on one pool of threads. The translator's and wasmtime's own parallel work runs on the same
//! pool, so a run keeps to its number of jobs.

use rayon::prelude::*;

/// Where Linux gives the load averages.
const LOAD_AVERAGES: &str = "/proc/loadavg";
/// What gives them on macOS, which has no such file.
const LOAD_AVERAGES_COMMAND: [&str; 3] = ["sysctl", "-n", "vm.loadavg"];

/// The machine's load averages as the system prints them, for the record beside a timing.
pub fn load() -> String {
    if let Ok(averages) = std::fs::read_to_string(LOAD_AVERAGES) {
        return averages.trim().to_string();
    }
    let [command, arguments @ ..] = LOAD_AVERAGES_COMMAND;
    let output = std::process::Command::new(command)
        .args(arguments)
        .output()
        .expect("the load averages");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

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
