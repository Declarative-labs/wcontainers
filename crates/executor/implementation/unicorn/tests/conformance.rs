//! The kernel interface's conformance suite on this kernel, with the stand-in as its executor: a
//! container started with a program that has not run.

use std::path::Path;
use std::sync::Arc;

use wcontainers_executor_unicorn::driver::{self, TestHost};
use wcontainers_executor_unicorn::standin::{SPACE_32_BIT, StandIn};
use wcontainers_executor_unicorn::{guest, guest_directory_entry, image, reference};
use wcontainers_kernel::TaskId;
use wcontainers_kernel_linux::KernelWithExecutor;

const PROGRAM: &str = "hello";

/// The container's first process, as the kernel numbers it inside the container.
const FIRST: TaskId = TaskId(1);

/// A kernel with a container started and its first process not run yet.
fn started() -> (KernelWithExecutor<StandIn, ()>, Arc<TestHost>) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = guest::build(PROGRAM, &["-nostdlib"], &out);
    let mut files = image::reference_image().to_vec();
    files.extend(image::host_directory(
        program.parent().expect("a directory"),
        guest_directory_entry(),
    ));
    let path = reference::in_guest_directory(PROGRAM);
    let arguments = [path.as_str()];
    let container = driver::Container {
        files: &files,
        path: &path,
        arguments: &arguments,
        environment: reference::ENVIRONMENT,
        working_directory: reference::ROOT_DIRECTORY,
        backend: None,
        cached: false,
        published: &[],
        root_image: None,
        address_space: SPACE_32_BIT,
    };
    driver::started_with_executor(driver::prepare(&container), &container, ())
}

wcontainers_kernel_conformance::tests!(
    kernel: (started().0, FIRST),
    process: started().0.process(FIRST).expect("the first process")
);
