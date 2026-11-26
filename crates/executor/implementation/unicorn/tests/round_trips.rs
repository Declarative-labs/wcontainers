//! A kernel's processes and tasks taken off it and put back, through the kernel interface's traits:
//! a container on the stand-in started with a program that has not run.

use std::num::NonZeroUsize;

use wcontainers_executor_unicorn::driver::{
    FIRST_PROCESS, InterfaceRunner, TestHost, program_not_run, program_on_runners, program_waiting,
};
use wcontainers_executor_unicorn::standin::{SPACE_32_BIT, StandIn};
use wcontainers_kernel::signal::Signal;
use wcontainers_kernel::signal_information::SignalInformation;
use wcontainers_kernel::{
    ExecutionConfig, ExecutorOf, Kernel, Process, ProcessEnd, ReplayDeterminism, Runner,
    StopReason, TaskId, ThreadOrdering,
};
use wcontainers_kernel_linux::KernelWithExecutor;

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
fn copy_leaves_the_process<K: Kernel>(kernel: K, process: TaskId) {
    let copy = kernel
        .copy_process_context(process)
        .expect("the process is on the kernel");
    assert!(kernel.copy_process_context(process).is_some());
    assert!(kernel.resume_process(copy, serial::<K>()).is_some());
    assert!(kernel.copy_process_context(process).is_some());
}

/// A suspended process leaves the kernel with its tasks, its tasks are handed back while its
/// context is away, and both come back.
fn suspended_process_comes_back<K: Kernel>(kernel: K, process: TaskId) {
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
fn suspended_tasks_come_back<K: Kernel>(kernel: K, process: TaskId) {
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

/// A process's handle takes only its own threads: its child's thread, asked for through it, is
/// neither handed out nor taken off the kernel.
#[test]
fn a_handle_takes_no_thread_of_another_process() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let mut kernel = program_waiting("forked", &[], &out);
    let child = (FIRST_PROCESS.0 + 1..1 << 12)
        .map(TaskId)
        .find(|&id| kernel.copy_process_context(id).is_some())
        .expect("the first process's child");
    let mut parent = kernel.process(FIRST_PROCESS).expect("the first process");
    assert!(parent.suspend_threads(&[child]).next().is_none());
    drop(parent);
    let taken: Vec<TaskId> = kernel.suspend_tasks(&[child]).map(|(id, _)| id).collect();
    assert_eq!(taken, [child], "the child's thread is still on the kernel");
}

/// Copies of a context made and let go of on several host threads at once leave the kernel whole:
/// what a copy shares with the kernel is touched under the kernel's lock.
#[test]
fn copies_are_made_and_let_go_of_on_several_threads() {
    const THREADS: usize = 4;
    const ROUNDS: usize = 200_000;
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let kernel = program_waiting("forked", &[], &out);
    std::thread::scope(|scope| {
        for _ in 0..THREADS {
            scope.spawn(|| {
                for _ in 0..ROUNDS {
                    assert!(kernel.copy_process_context(FIRST_PROCESS).is_some());
                }
            });
        }
    });
    let copy = kernel
        .copy_process_context(FIRST_PROCESS)
        .expect("the first process is still there");
    assert!(
        kernel
            .resume_process(copy, serial::<KernelWithExecutor<StandIn, u64, TestHost>>())
            .is_some()
    );
}

/// A copy let go of while a process's handle holds the kernel's lock is dropped by the lock's next
/// holder, and what it held goes with it: a copy of a child that alone has a shared memory segment
/// attached holds that attachment too, so once the child is killed the segment has none left only
/// if the copy was dropped.
#[test]
fn a_copy_let_go_of_under_a_handle_is_dropped_later() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let (mut kernel, mut runners, _host) =
        program_on_runners("heldattach", &[], &out, NonZeroUsize::MIN);
    let mut runner = runners.pop().expect("one runner");
    let mut executor = StandIn::with_space(SPACE_32_BIT);
    assert_eq!(
        run_until_stopped(&mut runner, &mut executor),
        StopReason::Idle { until: None },
        "the child waits for ever, attached, and the parent for its end"
    );
    let child = (FIRST_PROCESS.0 + 1..1 << 12)
        .map(TaskId)
        .find(|&id| kernel.copy_process_context(id).is_some())
        .expect("the first process's child");
    let mut process = kernel.process(child).expect("the child");
    drop(process.copy_process_context());
    process.send_signal(
        child,
        SignalInformation::SentByKernel {
            signal: Signal::SIGKILL,
        },
    );
    drop(process);
    assert!(
        kernel.copy_process_context(FIRST_PROCESS).is_some(),
        "the lock's next holder"
    );
    assert_eq!(
        run_until_stopped(&mut runner, &mut executor),
        StopReason::Finished(ProcessEnd::Exited { code: 0 }),
        "the segment has no attachment left: no copy holds the child's"
    );
}

/// Runs `runner` on `executor` until it stops for any reason but an interface call's, and returns
/// how.
fn run_until_stopped(runner: &mut InterfaceRunner<u64>, executor: &mut StandIn) -> StopReason {
    loop {
        match runner.run_on(executor) {
            StopReason::Interrupted => {}
            stopped => return stopped,
        }
    }
}

/// A process that has ended and is not yet reaped has no context to copy or take, while its
/// parent, waiting, has.
#[test]
fn an_ended_process_is_neither_copied_nor_taken() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let kernel = program_waiting("endedchild", &[], &out);
    let ids: Vec<TaskId> = (FIRST_PROCESS.0..1 << 12).map(TaskId).collect();
    let copied: Vec<TaskId> = ids
        .iter()
        .copied()
        .filter(|&id| kernel.copy_process_context(id).is_some())
        .collect();
    assert_eq!(copied, [FIRST_PROCESS], "only the parent is copied");
    let suspended: Vec<TaskId> = ids
        .iter()
        .copied()
        .filter(|&id| kernel.suspend_process(id).is_some())
        .collect();
    assert_eq!(suspended, [FIRST_PROCESS], "only the parent is taken");
}

/// The kernel the round trips start from: `hello`, not run yet.
fn hello_not_run() -> KernelWithExecutor<StandIn, u64, TestHost> {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    program_not_run("hello", &["-nostdlib"], &out)
}
