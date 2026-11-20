//! Running a container, and interrupting it from another thread.

use std::time::Duration;

/// A container a runtime made, run by whoever holds it.
pub trait Container {
    fn run(&mut self) -> StopReason;
}

/// A handle that interrupts a container's run from any thread.
pub trait Interrupter: Send + Sync {
    fn interrupt(&self);
}

/// The reason a run of a container returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Interrupted,
    Suspended,
    Idle { until: Option<Duration> },
    Finished { status: i32 },
}

/// The machine as a container's programs see it.
#[derive(Clone, Debug, Default)]
pub struct GuestMachine<Offer> {
    pub offer: Offer,
}
