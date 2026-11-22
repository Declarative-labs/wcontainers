#![no_std]
#![allow(async_fn_in_trait)]

use bitflags::bitflags;
use wcontainers_abi::HostError;

pub trait Readiness<Handle> {
    /// Waits until the handle is ready for one of the things asked for, and returns everything it is ready for.
    async fn wait_until_ready(
        &self,
        handle: &Handle,
        interest: ReadyFor,
    ) -> Result<ReadyFor, HostError>;
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ReadyFor: u8 {
        const READABLE = 1 << 0;
        const WRITABLE = 1 << 1;
        const ERROR = 1 << 2;
        /// The other end has closed; data it already sent can still be read.
        const HANG_UP = 1 << 3;
        /// The peer sent [urgent data](https://man7.org/linux/man-pages/man7/tcp.7.html), a byte marked to be noticed ahead of the stream.
        const URGENT_DATA = 1 << 4;
        /// The peer [shut down its sending side](https://man7.org/linux/man-pages/man2/shutdown.2.html) and can still receive.
        const PEER_FINISHED_SENDING = 1 << 5;
    }
}
