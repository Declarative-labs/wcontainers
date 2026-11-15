//! Calls that end threads and processes.

use wcontainers_abi::Syscall;

use super::Outcome;
use crate::process::Tid;
use crate::{Context, Kernel};

/// `exit(status)`: ends the calling thread, and its process when it is the last thread.
pub(super) fn exit(kernel: &mut Kernel, cx: &mut Context<'_>, tid: Tid, call: Syscall) -> Outcome {
    let status = call.arguments[0] as i32;
    let pid = kernel.threads[&tid].process;
    if kernel.processes[&pid].threads.len() == 1 {
        kernel.end_process(cx, pid, exit_status(status));
    } else {
        let thread = kernel.threads.remove(&tid).expect("thread exists");
        cx.cpu.thread_drop(thread.cpu);
        let process = kernel.processes.get_mut(&pid).expect("process exists");
        process.threads.retain(|other| *other != tid);
    }
    Outcome::Gone
}

/// `exit_group(status)`: ends every thread of the calling process.
pub(super) fn exit_group(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    call: Syscall,
) -> Outcome {
    let status = call.arguments[0] as i32;
    let pid = kernel.threads[&tid].process;
    kernel.end_process(cx, pid, exit_status(status));
    Outcome::Gone
}

/// The wait status of a process that exited with `status`: its low byte, shifted, as Linux reports it.
fn exit_status(status: i32) -> i32 {
    (status & 0xff) << 8
}
