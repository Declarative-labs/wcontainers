//! What the kernel asks of the host for everything that is not running instructions.
//!
//! Nothing here waits. A request whose answer takes time returns at once and the answer arrives later as an
//! [`Event`], which the driver hands to the kernel.

/// The host's clocks count nanoseconds: how many make a second.
pub const NANOSECONDS_PER_SECOND: u64 = core::time::Duration::from_secs(1).as_nanos() as u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostError {
    /// This host cannot do this at all.
    NotSupported,
    /// Nothing to give or no room to take right now. An [`Event`] will say when to try again.
    WouldBlock,
    /// The other end is gone.
    Closed,
    /// Any other failure, as a Linux error number.
    Errno(i32),
}

/// One slow path that was taken, reported so that it is never silent.
#[derive(Clone, Copy, Debug)]
pub struct Warning<'a> {
    /// The fixed name of the reason, such as "system call not implemented".
    pub reason: &'a str,
    /// How many times it has happened so far.
    pub count: u64,
    /// The guest address or the number concerned, if there is one.
    pub detail: u64,
}

/// The state of one named counter.
#[derive(Clone, Copy, Debug)]
pub struct CounterReading {
    pub reason: &'static str,
    /// The guest address or the number concerned, as in [`Warning::detail`]; one counter per value.
    pub detail: u64,
    pub count: u64,
    pub nanoseconds: u64,
}

/// Something that happened outside, handed to the kernel by the driver. If a guest thread is running when
/// one arrives, the driver raises that thread's stop flag first and delivers the event once it comes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// The time given in [`crate::Next::Idle`] has come.
    Timer,
    /// The backend sent a FUSE notice that was not asked for, such as "this file changed".
    BackendNotice { notice: &'a [u8] },
    /// The terminal of the first process's standard streams changed size.
    TerminalResized { columns: u16, rows: u16 },
}

/// What the kernel tells the host: its warnings.
pub trait Host: Sync {
    fn warn(&self, warning: Warning<'_>);
}
