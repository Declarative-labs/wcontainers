//! The tests every implementation of wcontainers-kernel runs, written against its traits alone.
//!
//! A kernel's checks take the kernel with one of its processes, which is not running. A process's
//! checks take a handle to a container's first process that has not run yet, which maps writable
//! memory and leaves its first page unmapped.

use wcontainers_kernel::{
    ExecutionConfig, ExecutorOf, Kernel, MemoryError, Process, ReplayDeterminism, TaskId,
    ThreadOrdering,
};

/// An id no kernel hands out: past the largest a task can have.
const ABSENT: TaskId = TaskId(u32::MAX);

/// The bytes a process's checks write.
const WRITTEN: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// The way of running a process every kernel supports.
fn serial<K: Kernel>() -> ExecutionConfig<ExecutorOf<K>> {
    ExecutionConfig {
        thread_ordering: ThreadOrdering::Serial,
        replay_determinism: ReplayDeterminism::Exact,
        translator: Default::default(),
    }
}

/// Checks that suspending an empty batch takes no task.
pub fn suspend_no_tasks<K: Kernel>((mut kernel, _): (K, TaskId)) {
    assert!(kernel.suspend_tasks(&[]).next().is_none());
}

/// Checks that resuming no tasks hands none back.
pub fn resume_no_tasks<K: Kernel>((mut kernel, _): (K, TaskId)) {
    assert!(kernel.resume_tasks(core::iter::empty()).next().is_none());
}

/// Checks that a process the kernel does not hold is neither copied nor suspended, and its task
/// not taken.
pub fn absent_process_is_not_found<K: Kernel>((mut kernel, _): (K, TaskId)) {
    assert!(kernel.copy_process_context(ABSENT).is_none());
    assert!(kernel.suspend_process(ABSENT).is_none());
    assert!(kernel.suspend_tasks(&[ABSENT]).next().is_none());
}

/// Checks that copying a process's context leaves the process on the kernel, and that a copy is
/// handed back when resumed beside the process itself.
pub fn copy_leaves_the_process<K: Kernel>((mut kernel, process): (K, TaskId)) {
    let copy = kernel
        .copy_process_context(process)
        .expect("the process is on the kernel");
    assert!(kernel.copy_process_context(process).is_some());
    assert!(kernel.resume_process(copy, serial::<K>()).is_some());
    assert!(kernel.copy_process_context(process).is_some());
}

/// Checks that a suspended process leaves the kernel with its tasks, that its tasks are handed
/// back while its context is away, and that both come back.
pub fn suspended_process_comes_back<K: Kernel>((mut kernel, process): (K, TaskId)) {
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

/// Checks that a process's tasks, taken off the kernel, are taken again once they are back.
pub fn suspended_tasks_come_back<K: Kernel>((mut kernel, process): (K, TaskId)) {
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

/// Checks what a container's first process tells of itself before it has run.
pub fn first_process_tells_of_itself<P: Process>(process: P) {
    assert_eq!(process.parent(), None, "the container's first process");
    assert!(process.mapped_memory() > 0, "its program is mapped");
    assert!(process.address_ceiling_distance() > 0);
    assert!(!process.is_inspected());
    assert_eq!(process.memory_id(), process.memory_id());
}

/// Checks that bytes written into the process's writable memory are read back.
pub fn reads_back_what_it_writes<P: Process>(mut process: P) {
    let writable = process.take_dirty_memory();
    let first = writable
        .first()
        .expect("the first answer is every writable page")
        .start;
    let range = first..first + WRITTEN.len() as u64;
    process
        .write_memory(range.clone(), &WRITTEN)
        .expect("a writable page is written");
    let read: Vec<u8> = process
        .read_memory(range)
        .expect("a written range is read")
        .flatten()
        .copied()
        .collect();
    assert_eq!(read, WRITTEN);
}

/// Checks that a range ending before it starts, and a page not mapped, are refused by name.
pub fn bad_reads_are_refused<P: Process>(mut process: P) {
    let length = WRITTEN.len() as u64;
    assert!(matches!(
        process.read_memory(length..0),
        Err(MemoryError::InvalidRange)
    ));
    assert!(matches!(
        process.read_memory(0..length),
        Err(MemoryError::NotMapped)
    ));
}

/// Checks that a process suspended through its handle leaves with its tasks.
pub fn process_suspends_itself<P: Process>(process: P) {
    let (_, tasks) = process.suspend_process();
    assert!(tasks.count() > 0, "a process that has not ended has a task");
}

/// Runs each suite function above as its own test: `kernel` gives a kernel with one of its
/// processes, `process` a handle to a container's first process; each is made anew for every test.
#[macro_export]
macro_rules! tests {
    (kernel: $kernel:expr, process: $process:expr) => {
        #[test]
        fn suspend_no_tasks() {
            $crate::suspend_no_tasks($kernel);
        }

        #[test]
        fn resume_no_tasks() {
            $crate::resume_no_tasks($kernel);
        }

        #[test]
        fn absent_process_is_not_found() {
            $crate::absent_process_is_not_found($kernel);
        }

        #[test]
        fn copy_leaves_the_process() {
            $crate::copy_leaves_the_process($kernel);
        }

        #[test]
        fn suspended_process_comes_back() {
            $crate::suspended_process_comes_back($kernel);
        }

        #[test]
        fn suspended_tasks_come_back() {
            $crate::suspended_tasks_come_back($kernel);
        }

        #[test]
        fn first_process_tells_of_itself() {
            $crate::first_process_tells_of_itself($process);
        }

        #[test]
        fn reads_back_what_it_writes() {
            $crate::reads_back_what_it_writes($process);
        }

        #[test]
        fn bad_reads_are_refused() {
            $crate::bad_reads_are_refused($process);
        }

        #[test]
        fn process_suspends_itself() {
            $crate::process_suspends_itself($process);
        }
    };
}
