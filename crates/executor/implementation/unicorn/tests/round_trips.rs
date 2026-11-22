//! A kernel's processes and tasks taken off it and put back, through the kernel interface's traits:
//! a container on the stand-in started with a program that has not run.

use wcontainers_executor_unicorn::driver::{FIRST_PROCESS, program_not_run};
use wcontainers_kernel::{
    ExecutionConfig, ExecutorOf, Kernel, ReplayDeterminism, TaskId, ThreadOrdering,
};

/// The way of running a process every kernel supports.
fn serial<K: Kernel>() -> ExecutionConfig<ExecutorOf<K>> {
    ExecutionConfig {
        thread_ordering: ThreadOrdering::Serial,
        replay_determinism: ReplayDeterminism::Exact,
        translator: Default::default(),
    }
}

/// Copying a process's context leaves the process on the kernel, and a copy is handed back when
/// resumed beside the process itself.
fn copy_leaves_the_process<K: Kernel>(mut kernel: K, process: TaskId) {
    let copy = kernel
        .copy_process_context(process)
        .expect("the process is on the kernel");
    assert!(kernel.copy_process_context(process).is_some());
    assert!(kernel.resume_process(copy, serial::<K>()).is_some());
    assert!(kernel.copy_process_context(process).is_some());
}

/// A suspended process leaves the kernel with its tasks, its tasks are handed back while its
/// context is away, and both come back.
fn suspended_process_comes_back<K: Kernel>(mut kernel: K, process: TaskId) {
    let (context, tasks) = kernel
        .suspend_process(process)
        .expect("the process is on the kernel");
    let tasks: Vec<_> = tasks.map(|(_, task)| task).collect();
    assert!(!tasks.is_empty(), "a process that has not ended has a task");
    assert!(kernel.copy_process_context(process).is_none());
    assert!(kernel.suspend_process(process).is_none());
    let handed_back: Vec<_> = kernel.resume_tasks(tasks).collect();
    assert!(
        !handed_back.is_empty(),
        "tasks without their context are handed back"
    );
    assert!(kernel.resume_process(context, serial::<K>()).is_none());
    assert!(kernel.resume_tasks(handed_back).next().is_none());
    assert!(kernel.copy_process_context(process).is_some());
}

/// A process's tasks, taken off the kernel, are taken again once they are back.
fn suspended_tasks_come_back<K: Kernel>(mut kernel: K, process: TaskId) {
    let (context, tasks) = {
        let (context, tasks) = kernel
            .suspend_process(process)
            .expect("the process is on the kernel");
        (context, tasks.collect::<Vec<_>>())
    };
    let ids: Vec<TaskId> = tasks.iter().map(|(id, _)| *id).collect();
    assert!(kernel.resume_process(context, serial::<K>()).is_none());
    let tasks = tasks.into_iter().map(|(_, task)| task);
    assert!(kernel.resume_tasks(tasks).next().is_none());
    let taken: Vec<_> = kernel.suspend_tasks(&ids).collect();
    assert_eq!(
        taken.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        ids,
        "each task asked for, as found"
    );
    let tasks = taken.into_iter().map(|(_, task)| task);
    assert!(kernel.resume_tasks(tasks).next().is_none());
    assert_eq!(kernel.suspend_tasks(&ids).count(), ids.len());
}

#[test]
fn a_copy_leaves_the_process() {
    copy_leaves_the_process(hello_not_run(), FIRST_PROCESS);
}

#[test]
fn a_suspended_process_comes_back() {
    suspended_process_comes_back(hello_not_run(), FIRST_PROCESS);
}

#[test]
fn suspended_tasks_come_back_and_are_taken_again() {
    suspended_tasks_come_back(hello_not_run(), FIRST_PROCESS);
}

/// The kernel the round trips start from: `hello`, not run yet.
fn hello_not_run()
-> wcontainers_kernel_linux::KernelWithExecutor<wcontainers_executor_unicorn::standin::StandIn, u64>
{
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    program_not_run("hello", &["-nostdlib"], &out)
}
