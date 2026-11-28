#![no_std]

use core::future::Future;

use bitflags::bitflags;
use vstd::prelude::*;
use wcontainers_abi::HostError;

verus! {

pub trait Readiness<Handle> {
    // VERUS: no `async fn` in a trait, which is an `impl Trait` return, so the wait is a named type.
    type Wait<'wait>: Future<Output = Result<ReadinessReport, HostError>>
    where
        Self: 'wait,
        Handle: 'wait;

    /// Waits until the handle is ready for one of the things asked for or has an unconditional event, and returns both.
    fn wait_until_ready<'wait>(
        &'wait self,
        handle: &'wait Handle,
        interest: ReadyFor,
    ) -> Self::Wait<'wait>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadinessReport {
    pub ready_for: ReadyFor,
    pub unconditional: UnconditionalEvents,
}

// VERUS: the items a macro makes are outside `verus!` even inside it, so each set of flags is declared here
// and bitflags gives it its operations below, as bitflags' externally defined flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadyFor(u8);

/// The events of a handle that every wait reports, as [`poll`](https://man7.org/linux/man-pages/man2/poll.2.html#:~:text=only%20returned%20in%20revents) does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnconditionalEvents(u8);

} // verus!

bitflags! {
    impl ReadyFor: u8 {
        const READABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        /// The peer sent [urgent data](https://man7.org/linux/man-pages/man7/tcp.7.html#:~:text=TCP%20supports%20urgent%20data), a byte marked to be noticed ahead of the stream.
        const URGENT_DATA = 1 << 2;
        /// The peer [shut down its sending side](https://man7.org/linux/man-pages/man2/shutdown.2.html) and can still receive.
        const PEER_FINISHED_SENDING = 1 << 3;
    }
}

bitflags! {
    impl UnconditionalEvents: u8 {
        const ERROR = 1 << 0;
        /// The other end has closed; data it already sent can still be read.
        const HANG_UP = 1 << 1;
    }
}
