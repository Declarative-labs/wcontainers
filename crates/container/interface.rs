//! Running a container, and interrupting it from another thread.

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
    Idle { until: Option<u64> },
    Finished { status: i32 },
}

/// The machine a container appears to run on.
#[derive(Clone, Debug, Default)]
pub struct GuestMachine<Offer> {
    pub offer: Offer,
}
