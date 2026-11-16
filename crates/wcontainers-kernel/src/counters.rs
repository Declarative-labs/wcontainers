//! Named counters for every slow, missing or approximated path, each also reported as a warning.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::RefCell;

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
    pub const CORE_DUMP_NOT_WRITTEN: &str = "core dump not written";
    /// A signal whose default action stops the process; stopping comes with job control. Detail: the
    /// signal.
    pub const PROCESS_STOP_NOT_IMPLEMENTED: &str = "process stop not implemented";
    /// File locks through `fcntl`. Detail: the command.
    pub const LOCKS_NOT_IMPLEMENTED: &str = "file locks not implemented";
    /// An `fcntl` command the kernel does not know. Detail: the command.
    pub const FCNTL_UNKNOWN: &str = "fcntl command unknown";
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
    /// `mincore`, `getrusage`, `sysinfo` or `/proc/meminfo` reported every mapped page as in
    /// memory, or `cachestat` or `statfs` counted a mapped file's pages without those its faults
    /// brought in: which pages a program has touched is not known here. Detail: none.
    pub const RESIDENCY_NOT_TRACKED: &str = "page residency not tracked";
    /// A seccomp filter asked for a listener, through which another process would answer the calls it
    /// stops; not built. Detail: the flags.
    pub const SECCOMP_LISTENER_NOT_IMPLEMENTED: &str = "seccomp listener not implemented";
    /// A `/proc` entry Linux has and this kernel does not build was looked up; it answers ENOENT.
    /// Detail: the directory's index in Linux's list of entries, times 1024, plus the name's.
    pub const PROC_ENTRY_NOT_BUILT: &str = "proc entry not built";
    /// A `/proc` file was made whose fields this kernel does not track all of (those show 0 or are
    /// left out). Detail: which file (0 `stat`, 1 `statm`, 2 `status`, 3 `mounts`, 4 `meminfo`).
    pub const PROC_FIELDS_NOT_TRACKED: &str = "proc fields not tracked";
    /// A file changed that a private mapping holds a copy of, made when the mapping was, or a file
    /// is mapped both shared and private: Linux's private pages not yet written would show the file's
    /// later bytes, and the copy does not. Detail: the inode number.
    pub const PRIVATE_MAPPING_COPY: &str = "private file mapping is a copy";
    /// An event arrived that no thread was waiting for.
    pub const EVENT_UNEXPECTED: &str = "event with no waiter";
}

#[derive(Default)]
pub(crate) struct Counters {
    counts: BTreeMap<(&'static str, u64), u64>,
    /// Occurrences seen where the host cannot be reached (a path walk), counted at the next flush.
    noted: RefCell<Vec<(&'static str, u64)>>,
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

    /// Notes an occurrence to count at the next [`Counters::flush`].
    pub fn note(&self, reason: &'static str, detail: u64) {
        self.noted.borrow_mut().push((reason, detail));
    }

    /// Counts and warns of every noted occurrence.
    pub fn flush(&mut self, host: &mut dyn Host) {
        for (reason, detail) in core::mem::take(self.noted.get_mut()) {
            self.count(host, reason, detail);
        }
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
