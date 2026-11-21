//! The tests every implementation of wcontainers-system runs, written against its traits alone.

use domain::base::Name;
use wcontainers_kernel::DomainResolutionId;
use wcontainers_kernel::TaskId;
use wcontainers_system::{DomainNameResolver, FuseServer, TaskIdAllocator};

/// The id today's kernel gives as the one before its first.
const BEFORE_FIRST_TASK_ID: TaskId = TaskId(u32::MIN);

/// Checks that an id handed out and not freed is not handed out next.
pub fn allocate_task_ids_apart<S: TaskIdAllocator>(source: S) {
    let first = source.allocate_task_id(BEFORE_FIRST_TASK_ID);
    let second = source.allocate_task_id(first);
    assert_ne!(first, second);
}

/// Checks that freeing one id does not let the next allocation hand out another still held.
pub fn free_task_id_alone<S: TaskIdAllocator>(source: S) {
    let first = source.allocate_task_id(BEFORE_FIRST_TASK_ID);
    let second = source.allocate_task_id(first);
    source.free_task_id(first);
    assert_ne!(source.allocate_task_id(second), second);
}

/// Calls each method of `FuseServer` as a placeholder that fails until the real checks are written.
pub fn call_fuse_server_methods<F: FuseServer>(server: F) {
    let _ = server.send_request(&[]);
    todo!("this is a placeholder: the real checks of FuseServer are not written yet");
}

/// Calls each method of `DomainNameResolver` as a placeholder that fails until the real checks are written.
pub fn call_domain_name_resolver_methods<R: DomainNameResolver>(resolver: R) {
    let _ = resolver.resolve_domain(DomainResolutionId(u64::MIN), Name::root_slice());
    todo!("this is a placeholder: the real checks of DomainNameResolver are not written yet");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn allocate_task_ids_apart() {
            $crate::allocate_task_ids_apart($value);
        }

        #[test]
        fn free_task_id_alone() {
            $crate::free_task_id_alone($value);
        }

        #[test]
        fn call_fuse_server_methods() {
            $crate::call_fuse_server_methods($value);
        }

        #[test]
        fn call_domain_name_resolver_methods() {
            $crate::call_domain_name_resolver_methods($value);
        }
    };
}
