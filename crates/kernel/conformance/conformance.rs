//! The tests every implementation of wcontainers-kernel runs, written against its traits alone.

use wcontainers_kernel::Kernel;
use wcontainers_kernel::TaskId;

/// The process the kernel placeholder asks for.
const PLACEHOLDER_PROCESS: TaskId = TaskId(u32::MIN);

/// Checks that suspending an empty batch takes no task.
pub fn suspend_no_tasks<K: Kernel>(mut kernel: K) {
    assert!(kernel.suspend_tasks(&[]).next().is_none());
}

/// Checks that resuming no tasks hands none back.
pub fn resume_no_tasks<K: Kernel>(mut kernel: K) {
    assert!(kernel.resume_tasks(core::iter::empty()).next().is_none());
}

/// Calls `Kernel::suspend_process` and `Kernel::copy_process_context` as a placeholder that fails until the real checks are written.
pub fn call_kernel_process_methods<K: Kernel>(mut kernel: K) {
    let _ = kernel.copy_process_context(PLACEHOLDER_PROCESS);
    let _ = kernel.suspend_process(PLACEHOLDER_PROCESS);
    todo!("this is a placeholder: the real checks of Kernel are not written yet");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn suspend_no_tasks() {
            $crate::suspend_no_tasks($value);
        }

        #[test]
        fn resume_no_tasks() {
            $crate::resume_no_tasks($value);
        }

        #[test]
        fn call_kernel_process_methods() {
            $crate::call_kernel_process_methods($value);
        }
    };
}
