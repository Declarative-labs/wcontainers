#![no_std]
#![allow(async_fn_in_trait)]

use bitflags::bitflags;
use wcontainers_abi::HostError;

pub trait Readiness<Handle> {
    /// Waits until the handle is ready for one of the things asked for or has an unconditional event, and returns both.
    async fn wait_until_ready(
        &self,
        handle: &Handle,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadinessReport {
    pub ready_for: ReadyFor,
    pub unconditional: UnconditionalEvents,
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ReadyFor: u8 {
        const READABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        /// The peer sent [urgent data](https://man7.org/linux/man-pages/man7/tcp.7.html#:~:text=TCP%20supports%20urgent%20data), a byte marked to be noticed ahead of the stream.
        const URGENT_DATA = 1 << 2;
        /// The peer [shut down its sending side](https://man7.org/linux/man-pages/man2/shutdown.2.html) and can still receive.
        const PEER_FINISHED_SENDING = 1 << 3;
    }
}

bitflags! {
    /// The events of a handle that every wait reports, as [`poll`](https://man7.org/linux/man-pages/man2/poll.2.html#:~:text=only%20returned%20in%20revents) does.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct UnconditionalEvents: u8 {
        const ERROR = 1 << 0;
        /// The other end has closed; data it already sent can still be read.
        const HANG_UP = 1 << 1;
    }
}
