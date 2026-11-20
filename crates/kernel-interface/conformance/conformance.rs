//! The tests every implementation of wcontainers-kernel-interface runs, written against its traits alone.

use wcontainers_kernel_interface::Kernel;

/// Checks that suspending an empty batch takes no task.
pub fn suspend_no_tasks<K: Kernel>(mut kernel: K) {
    assert!(kernel.suspend_tasks(&[]).next().is_none());
}

/// Checks that resuming no tasks hands none back.
pub fn resume_no_tasks<K: Kernel>(mut kernel: K) {
    assert!(kernel.resume_tasks(core::iter::empty()).next().is_none());
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for
/// every test.
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
    };
}
