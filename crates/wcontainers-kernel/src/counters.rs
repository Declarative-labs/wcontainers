//! Named counters for every slow, missing or approximated path, each also reported as a warning.

use alloc::collections::BTreeMap;

use wcontainers_abi::{CounterReading, Host, Warning};

/// The fixed reasons the kernel counts.
pub mod reason {
    /// A system call Linux has and the kernel does not yet. Detail: the call number.
    pub const NOT_IMPLEMENTED: &str = "system call not implemented";
    /// A system call number Linux does not have on ARM64. Detail: the call number.
    pub const UNKNOWN_SYSCALL: &str = "system call number unknown";
    /// A program started without an auxiliary vector entry Linux passes. Detail: the `AT_*` key.
    pub const AUXILIARY_MISSING: &str = "auxiliary vector entry missing";
    /// A signal the program set a handler for arrived or was to be let through; handlers do not run
    /// yet, so the call was refused, or for a fault the process died. Detail: the signal.
    pub const SIGNAL_HANDLER_NOT_RUN: &str = "signal handler not run";
    /// A signal whose default action stops the process; stopping comes with job control. Detail: the
    /// signal.
    pub const PROCESS_STOP_NOT_IMPLEMENTED: &str = "process stop not implemented";
    /// A change of owner or mode to a `/proc` entry outside the process directories, which Linux keeps
    /// and this kernel refuses.
    pub const PROC_CHANGE_NOT_KEPT: &str = "proc attribute change not kept";
    /// File locks through `fcntl`. Detail: the command.
    pub const LOCKS_NOT_IMPLEMENTED: &str = "file locks not implemented";
    /// An `fcntl` command the kernel does not know. Detail: the command.
    pub const FCNTL_UNKNOWN: &str = "fcntl command unknown";
    /// An `ioctl` request on a device the kernel has no answer for. Detail: the request.
    pub const IOCTL_UNKNOWN: &str = "ioctl request unknown";
    /// Opening a device the kernel has no driver for yet. Detail: the device number.
    pub const DEVICE_NOT_IMPLEMENTED: &str = "device not implemented";
    /// Opening a named pipe, which needs pipes. Detail: the inode number.
    pub const NAMED_PIPE_NOT_IMPLEMENTED: &str = "named pipe not implemented";
    /// A shared mapping of a file, which needs shared pages.
    pub const SHARED_FILE_MAPPING: &str = "shared file mapping not implemented";
    /// A `prctl` option the kernel does not know. Detail: the option.
    pub const PRCTL_UNKNOWN: &str = "prctl option unknown";
    /// A flag of a memory call the kernel does not implement. Detail: the flag bits.
    pub const MEMORY_FLAG_NOT_IMPLEMENTED: &str = "memory call flag not implemented";
    /// An event arrived that no thread was waiting for.
    pub const EVENT_UNEXPECTED: &str = "event with no waiter";
}

#[derive(Default)]
pub(crate) struct Counters {
    counts: BTreeMap<(&'static str, u64), u64>,
}

impl Counters {
    /// Counts one occurrence and warns the host.
    pub fn count(&mut self, host: &mut dyn Host, reason: &'static str, detail: u64) {
        let count = self.counts.entry((reason, detail)).or_insert(0);
        *count += 1;
        host.warn(Warning {
            reason,
            count: *count,
            detail,
        });
    }

    pub fn readings(&self) -> impl Iterator<Item = CounterReading> + '_ {
        self.counts
            .iter()
            .map(|(&(reason, detail), &count)| CounterReading {
                reason,
                detail,
                count,
                nanoseconds: 0,
            })
    }
}
