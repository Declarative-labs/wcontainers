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
    pub const CORE_DUMP_NOT_WRITTEN: &str = "core dump not written";
    /// An `fcntl` command the kernel does not build, known to Linux or not; it answers EINVAL, as
    /// Linux does for one it does not know or has not configured. Detail: the command.
    pub const FCNTL_NOT_BUILT: &str = "fcntl command not built";
    /// Opening a device the kernel has no driver for. Detail: the device number.
    pub const DEVICE_NOT_BUILT: &str = "device not built";
    /// A `prctl` option or form the kernel does not build, known to Linux or not; it answers EINVAL,
    /// as Linux does for one it does not know. Detail: the option.
    pub const PRCTL_NOT_BUILT: &str = "prctl option not built";
    /// A flag or form of a call that the kernel does not build, answered as Linux answers without
    /// it. Detail: the call number times 2^32, plus the flag bits.
    pub const FLAG_NOT_BUILT: &str = "call flag not built";
    /// `madvise` advice Linux has that the kernel does not build; it answers EINVAL, as a Linux
    /// without that advice does. Detail: the advice.
    pub const ADVICE_NOT_BUILT: &str = "madvise advice not built";
    /// An `ioctl` request of a file whose Linux driver has requests this kernel does not build; it
    /// answers as that driver does for a request it does not know. Detail: the request.
    pub const IOCTL_NOT_BUILT: &str = "ioctl request not built";
    /// A process's memory, files or other state reached from another process in a way not built.
    /// Detail: the call number.
    pub const CROSS_PROCESS_NOT_BUILT: &str = "call on another process not built";
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
    /// A fault's syndrome in a handler's frame: which translation level faulted and the decoded
    /// instruction are Linux's page tables' and CPU's own, so a level 3 fault with no instruction
    /// detail is given. Detail: none.
    pub const FAULT_SYNDROME_APPROXIMATED: &str = "fault syndrome approximated";
    /// The time between a thread being woken and it running, measured, not a slow path. Detail: none.
    pub const WAKE_TO_RUN: &str = "wake to run";
    /// An event arrived that no thread was waiting for.
    pub const EVENT_UNEXPECTED: &str = "event with no waiter";

    /// Every reason that means something Linux has is not built here, each answered through
    /// `Counters::refuse`: the status file's list of what is missing comes
    /// from these names.
    pub const NOT_BUILT: &[&str] = &[
        NOT_IMPLEMENTED,
        FLAG_NOT_BUILT,
        FCNTL_NOT_BUILT,
        PRCTL_NOT_BUILT,
        IOCTL_NOT_BUILT,
        ADVICE_NOT_BUILT,
        DEVICE_NOT_BUILT,
        PROC_ENTRY_NOT_BUILT,
        SECCOMP_LISTENER_NOT_IMPLEMENTED,
        CROSS_PROCESS_NOT_BUILT,
    ];
}

#[derive(Default)]
pub(crate) struct Counters {
    counts: BTreeMap<(&'static str, u64), u64>,
    /// Measured times: how many and their sum in nanoseconds.
    times: BTreeMap<(&'static str, u64), (u64, u64)>,
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

    /// The one path for something Linux has that is not built here: counts it under one of
    /// [`reason::NOT_BUILT`], warns, and gives back the error Linux answers when it lacks the
    /// feature.
    pub fn refuse(
        &mut self,
        host: &mut dyn Host,
        reason: &'static str,
        detail: u64,
        answer: crate::Errno,
    ) -> crate::Errno {
        debug_assert!(reason::NOT_BUILT.contains(&reason), "{reason}");
        self.count(host, reason, detail);
        answer
    }

    /// A flag or form of call `number` that is not built, as [`Counters::refuse`] counts it.
    pub fn refuse_flag(
        &mut self,
        host: &mut dyn Host,
        number: u32,
        flags: u64,
        answer: crate::Errno,
    ) -> crate::Errno {
        let detail = (u64::from(number) << 32) | (flags & 0xffff_ffff);
        self.refuse(host, reason::FLAG_NOT_BUILT, detail, answer)
    }

    /// Adds a measured time under a reason, without a warning: a measurement, not a slow path.
    pub fn measure(&mut self, reason: &'static str, detail: u64, nanoseconds: u64) {
        let entry = self.times.entry((reason, detail)).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += nanoseconds;
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
        let counted = self
            .counts
            .iter()
            .map(|(&(reason, detail), &count)| CounterReading {
                reason,
                detail,
                count,
                nanoseconds: 0,
            });
        let measured =
            self.times.iter().map(
                |(&(reason, detail), &(count, nanoseconds))| CounterReading {
                    reason,
                    detail,
                    count,
                    nanoseconds,
                },
            );
        counted.chain(measured)
    }
}
