//! The tests every implementation of wcontainers-kernel runs, written against its traits alone.
//!
//! A kernel's checks take a kernel with two processes, a container's first process and its child,
//! each of one thread whose id is its process's, both waiting for ever, with ids below [`PROBED`].
//! A process's checks take a handle to such a container's first process, started to run serially
//! and exactly, which leaves its first page unmapped. Its start has written pages for it, as
//! Linux's `execve` writes the new program's arguments onto its stack before its first
//! instruction.

use wcontainers_kernel::signal::Signal;
use wcontainers_kernel::signal_information::SignalInformation;
use wcontainers_kernel::{
    Kernel, MemoryError, Process, ReplayDeterminism, TaskId, ThreadOrdering, WatchesFor,
};

/// An id no kernel hands out: past the largest a task can have.
const ABSENT: TaskId = TaskId(u32::MAX);

/// One past the largest id the checks look for a process at: the ids a kernel hands out first.
pub const PROBED: u32 = 1 << 12;

/// The bytes a process's checks write, a word's worth of distinct values.
const WRITTEN: [u8; size_of::<u64>()] = [1, 2, 3, 4, 5, 6, 7, 8];

/// Checks that suspending an empty batch takes no task.
pub fn suspend_no_tasks<K: Kernel>(kernel: K) {
    assert!(kernel.suspend_tasks(&[]).next().is_none());
}

/// Checks that resuming no tasks hands none back.
pub fn resume_no_tasks<K: Kernel>(kernel: K) {
    assert!(kernel.resume_tasks(core::iter::empty()).next().is_none());
}

/// Checks that a process the kernel does not hold is not copied or suspended, nor its task taken.
pub fn absent_process_is_not_found<K: Kernel>(kernel: K) {
    assert!(kernel.copy_process_context(ABSENT).is_none());
    assert!(kernel.suspend_process(ABSENT).is_none());
    assert!(kernel.suspend_tasks(&[ABSENT]).next().is_none());
}

/// Checks that one call takes the threads of two processes, each once, and that they are then
/// off the kernel.
pub fn tasks_of_several_processes_are_taken_at_once<K: Kernel>(kernel: K) {
    let processes = processes_of(&kernel);
    assert_eq!(
        processes.len(),
        2,
        "the container's first process and its child"
    );
    let taken: Vec<TaskId> = kernel
        .suspend_tasks(&processes)
        .map(|(task, _)| task)
        .collect();
    assert_eq!(
        taken, processes,
        "each process's thread, in the order asked"
    );
    assert!(kernel.suspend_tasks(&processes).next().is_none());
}

/// Returns the ids below [`PROBED`] the kernel has a process of, in order.
fn processes_of<K: Kernel>(kernel: &K) -> Vec<TaskId> {
    (1..PROBED)
        .map(TaskId)
        .filter(|&id| kernel.copy_process_context(id).is_some())
        .collect()
}

/// Checks what a container's first process tells of itself before it has run.
pub fn first_process_tells_of_itself<P: Process>(process: P) {
    assert_eq!(process.parent(), None, "the container's first process");
    assert!(process.mapped_memory() > 0, "its program is mapped");
    assert!(process.address_ceiling_distance() > 0);
    assert!(!process.is_inspected());
    assert_ne!(process.id(), ABSENT);
}

/// Checks that bytes written into the process's memory are read back, and that the process's
/// memory is still the same memory.
pub fn reads_back_what_it_writes<P: Process>(mut process: P) {
    let memory = process.memory_id();
    let range = first_written(&mut process);
    process
        .write_memory(range.clone(), &WRITTEN)
        .expect("a written page is written");
    assert_eq!(read(&mut process, range), WRITTEN);
    assert_eq!(
        process.memory_id(),
        memory,
        "written, it is the same memory"
    );
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

/// Checks that a process suspended through its handle leaves with its one thread, of its own id.
pub fn process_suspends_itself<P: Process>(process: P) {
    let id = process.id();
    let (_, tasks) = process.suspend_process();
    let ids: Vec<TaskId> = tasks.map(|(task, _)| task).collect();
    assert_eq!(ids, [id], "a process of one thread has its first thread");
}

/// Checks that a process that is not running uses no CPU time while the embedder reads and
/// writes its memory and watches it.
pub fn no_time_is_used_while_not_running<P: Process>(mut process: P) {
    let before = process.cpu_time();
    let range = first_written(&mut process);
    process.watch_memory(range.clone(), WatchesFor::Writes);
    process
        .write_memory(range.clone(), &WRITTEN)
        .expect("a written page is written");
    assert_eq!(read(&mut process, range.clone()), WRITTEN);
    process.watch_memory(range, WatchesFor::None);
    assert_eq!(process.cpu_time(), before, "the process has not run");
}

/// Checks that the process runs as it was started: serially and exactly.
pub fn runs_as_started<P: Process>(process: P) {
    let config = process.execution_config();
    assert_eq!(config.thread_ordering, ThreadOrdering::Serial);
    assert_eq!(config.replay_determinism, ReplayDeterminism::Exact);
}

/// Checks that the embedder's state of a process keeps what the embedder puts in it while the
/// embedder writes, reads and watches the process's memory and signals it.
pub fn state_keeps_what_is_put_in<P: Process>(mut process: P)
where
    P::State: From<u8> + PartialEq + core::fmt::Debug,
{
    let range = first_written(&mut process);
    let id = process.id();
    for value in WRITTEN {
        *process.state() = value.into();
        process.watch_memory(range.clone(), WatchesFor::Writes);
        process
            .write_memory(range.clone(), &WRITTEN)
            .expect("a written page is written");
        assert_eq!(read(&mut process, range.clone()), WRITTEN);
        process.watch_memory(range.clone(), WatchesFor::None);
        process.send_signal(
            id,
            SignalInformation::SentByKernel {
                signal: Signal::SIGCONT,
            },
        );
        assert_eq!(*process.state(), value.into());
    }
}

/// Checks that copying a process's context leaves the process as it was: its memory read and
/// written, and its thread there to take. What a copy holds is only seen by resuming it on a
/// kernel, which a process's handle cannot reach.
pub fn copy_leaves_the_process<P: Process>(mut process: P) {
    let range = first_written(&mut process);
    process
        .write_memory(range.clone(), &WRITTEN)
        .expect("a written page is written");
    let copy = process.copy_process_context();
    assert_eq!(read(&mut process, range), WRITTEN);
    let id = process.id();
    let (context, tasks) = process.suspend_process();
    let ids: Vec<TaskId> = tasks.map(|(task, _)| task).collect();
    assert_eq!(ids, [id], "the copied process still has its thread");
    drop((copy, context));
}

/// Checks that a thread taken off the kernel is not taken again, by itself or with its process.
pub fn taken_tasks_are_not_taken_again<P: Process>(mut process: P) {
    let id = process.id();
    let taken: Vec<TaskId> = process
        .suspend_threads(&[id])
        .map(|(task, _)| task)
        .collect();
    assert_eq!(taken, [id], "the process's thread, as asked for");
    assert!(process.suspend_threads(&[id]).next().is_none());
    let (_, tasks) = process.suspend_process();
    assert_eq!(tasks.count(), 0, "its one thread is already taken");
}

/// Checks that asking a process for every thread id but its own takes and returns nothing, though
/// its child's thread is among them, and that its own thread is still there to take.
pub fn threads_of_another_process_are_not_taken<P: Process>(mut process: P) {
    let id = process.id();
    let others: Vec<TaskId> = (1..PROBED).map(TaskId).filter(|&task| task != id).collect();
    assert!(process.suspend_threads(&others).next().is_none());
    let mut asked = others;
    asked.push(id);
    let taken: Vec<TaskId> = process
        .suspend_threads(&asked)
        .map(|(task, _)| task)
        .collect();
    assert_eq!(taken, [id], "only its own thread");
}

/// Checks that a signal whose default is to go on leaves a process that has not run as it was.
pub fn continue_signal_leaves_the_process<P: Process>(mut process: P) {
    let id = process.id();
    process.send_signal(
        id,
        SignalInformation::SentByKernel {
            signal: Signal::SIGCONT,
        },
    );
    let range = first_written(&mut process);
    process
        .write_memory(range.clone(), &WRITTEN)
        .expect("a written page is written");
    assert_eq!(read(&mut process, range), WRITTEN);
    let (_, tasks) = process.suspend_process();
    assert_eq!(
        tasks.count(),
        1,
        "the signalled process still has its thread"
    );
}

/// Checks that the embedder still reads and writes memory it watches, and after the watch ends.
pub fn watched_memory_is_read_and_written<P: Process>(mut process: P) {
    let range = first_written(&mut process);
    for watches in [WatchesFor::All, WatchesFor::Writes, WatchesFor::None] {
        process.watch_memory(range.clone(), watches);
        process
            .write_memory(range.clone(), &WRITTEN)
            .expect("a watched page is written by the embedder");
        assert_eq!(read(&mut process, range.clone()), WRITTEN);
    }
}

/// Returns the range of [`WRITTEN`]'s length at the start of the first range of pages written for
/// the process since its start, which wrote its program's arguments at least.
fn first_written<P: Process>(process: &mut P) -> core::ops::Range<u64> {
    let written = process.take_dirty_memory();
    let first = written
        .first()
        .expect("the start wrote the program's arguments")
        .start;
    first..first + WRITTEN.len() as u64
}

/// Returns the bytes of a range of the process's memory, which must be readable.
fn read<P: Process>(process: &mut P, range: core::ops::Range<u64>) -> Vec<u8> {
    process
        .read_memory(range)
        .expect("a written range is read")
        .flatten()
        .copied()
        .collect()
}

/// Runs each suite function above as its own test, and each group's where the implementation has it.
#[macro_export]
macro_rules! tests {
    (@Process yes ($value:expr)) => {
        $crate::tests!(@Process yes ($value) ($value));
    };
    (@Process yes ($base:expr) ($value:expr)) => {
        mod process {
            #[test]
            fn first_process_tells_of_itself() {
                $crate::first_process_tells_of_itself($value);
            }
            #[test]
            fn reads_back_what_it_writes() {
                $crate::reads_back_what_it_writes($value);
            }
            #[test]
            fn bad_reads_are_refused() {
                $crate::bad_reads_are_refused($value);
            }
            #[test]
            fn process_suspends_itself() {
                $crate::process_suspends_itself($value);
            }
            #[test]
            fn no_time_is_used_while_not_running() {
                $crate::no_time_is_used_while_not_running($value);
            }
            #[test]
            fn runs_as_started() {
                $crate::runs_as_started($value);
            }
            #[test]
            fn state_keeps_what_is_put_in() {
                $crate::state_keeps_what_is_put_in($value);
            }
            #[test]
            fn copy_leaves_the_process() {
                $crate::copy_leaves_the_process($value);
            }
            #[test]
            fn taken_tasks_are_not_taken_again() {
                $crate::taken_tasks_are_not_taken_again($value);
            }
            #[test]
            fn threads_of_another_process_are_not_taken() {
                $crate::threads_of_another_process_are_not_taken($value);
            }
            #[test]
            fn continue_signal_leaves_the_process() {
                $crate::continue_signal_leaves_the_process($value);
            }
            #[test]
            fn watched_memory_is_read_and_written() {
                $crate::watched_memory_is_read_and_written($value);
            }
        }
    };
    (@Process no ($value:expr)) => {
        mod process {
            #[test]
            fn lacks_process() {
                struct Probe<T>(::core::marker::PhantomData<T>);
                impl<T> ::core::clone::Clone for Probe<T> {
                    fn clone(&self) -> Self {
                        Probe(::core::marker::PhantomData)
                    }
                }
                struct Implemented;
                #[allow(dead_code)]
                impl<T: ::wcontainers_kernel::Process> Probe<T> {
                    fn clone(&self) -> Implemented {
                        Implemented
                    }
                }
                fn probe_of<T>(_value: &T) -> Probe<T> {
                    Probe(::core::marker::PhantomData)
                }
                let _: Probe<_> = probe_of(&$value).clone();
            }
        }
    };
    ($value:expr; Process: $process:ident $(($process_value:expr))?) => {
        #[test]
        fn suspend_no_tasks() {
            $crate::suspend_no_tasks($value);
        }
        #[test]
        fn resume_no_tasks() {
            $crate::resume_no_tasks($value);
        }
        #[test]
        fn absent_process_is_not_found() {
            $crate::absent_process_is_not_found($value);
        }
        #[test]
        fn tasks_of_several_processes_are_taken_at_once() {
            $crate::tasks_of_several_processes_are_taken_at_once($value);
        }
        $crate::tests!(@Process $process ($value) $(($process_value))?);
    };
}
