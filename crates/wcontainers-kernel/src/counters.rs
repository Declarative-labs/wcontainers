//! Named counters for every slow, missing or approximated path, each also reported as a warning.

use alloc::collections::BTreeMap;

use wcontainers_abi::{CounterReading, Host, Warning};

/// The fixed reasons the kernel counts.
pub mod reason {
    /// A system call Linux has and the kernel does not yet. Detail: the call number.
    pub const NOT_IMPLEMENTED: &str = "system call not implemented";
    /// A system call number Linux does not have on ARM64. Detail: the call number.
    pub const UNKNOWN_SYSCALL: &str = "system call number unknown";
    /// A program that names a loader for shared libraries, which the kernel cannot start yet.
    pub const DYNAMIC_PROGRAM: &str = "dynamically linked program not supported";
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
