//! The guest's wait status, in Linux's encoding (`kernel/exit.c`, the C library's `W*` macros).

/// Where an exit status sits in a wait status.
pub const WAIT_EXIT_SHIFT: u32 = 8;
/// The bits of the signal that ended a process; none for one that exited.
pub const WAIT_SIGNAL_MASK: i32 = 0x7f;
/// Set when the ending signal dumped core.
pub const WAIT_CORE_DUMPED: i32 = 0x80;
/// The low bits of a stopped child's status, with the stopping signal above them.
pub const WAIT_STOPPED: i32 = 0x7f;
/// The whole status of a continued child.
pub const WAIT_CONTINUED: i32 = 0xffff;

/// How a process ended, as its wait status says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    Exited(i32),
    Killed { signal: i32, core_dumped: bool },
}

/// The wait status of a process that exited with `status`: its low byte, shifted.
pub fn exited(status: i32) -> i32 {
    (status & i32::from(u8::MAX)) << WAIT_EXIT_SHIFT
}

/// The wait status of a child stopped by `signal`.
pub fn stopped(signal: i32) -> i32 {
    (signal << WAIT_EXIT_SHIFT) | WAIT_STOPPED
}

/// How the process whose wait status is `status` ended.
pub fn decode(status: i32) -> Ended {
    match status & WAIT_SIGNAL_MASK {
        0 => Ended::Exited((status >> WAIT_EXIT_SHIFT) & i32::from(u8::MAX)),
        signal => Ended::Killed {
            signal,
            core_dumped: status & WAIT_CORE_DUMPED != 0,
        },
    }
}
