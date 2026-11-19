//! What a test does at the kernel's hooks and task id calls.

use wcontainers_abi::Cpu;
use wcontainers_kernel::{Access, Kernel, Process, TaskId, task_id_after};

/// A test's side of the kernel's hooks and task id calls; each left alone does what the kernel's
/// `System` does by default.
pub trait Hooks {
    fn task_id_allocate(&mut self, previous: TaskId) -> TaskId {
        task_id_after(previous)
    }
    fn task_id_free(&mut self, id: TaskId) {
        let _ = id;
    }
    fn on_process_starting(&mut self, process: Process<'_>) {
        let _ = process;
    }
    fn on_program_starting(&mut self, process: Process<'_>) {
        let _ = process;
    }
    fn on_thread_starting(&mut self, process: Process<'_>, thread: TaskId) {
        let _ = (process, thread);
    }
    fn on_process_exiting(&mut self, process: Process<'_>) {
        let _ = process;
    }
    fn on_process_outgrown(&mut self, process: Process<'_>, bytes: u64) {
        let _ = (process, bytes);
    }
    fn on_memory_accessing(
        &mut self,
        process: Process<'_>,
        thread: TaskId,
        address: u64,
        access: Access,
    ) {
        let _ = (process, thread, address, access);
    }
    /// The kernel has nothing to run: the test may act on it through ordinary calls. Whether it
    /// did anything that may let the kernel go on.
    fn between_steps(&mut self, kernel: &mut Kernel, cpu: &mut dyn Cpu) -> bool {
        let _ = (kernel, cpu);
        false
    }
}

/// The kernel's defaults for everything.
pub struct Defaults;

impl Hooks for Defaults {}
