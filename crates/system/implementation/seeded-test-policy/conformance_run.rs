//! The system suite's stream tests on the policy, over services whose one stream loops back to them.

use std::collections::VecDeque;
use std::future::poll_fn;
use std::sync::{Mutex, PoisonError};
use std::task::Poll;

use wcontainers_abi::{Host, HostError, Warning};
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_system::StreamTransport;
use wcontainers_system_seeded_test_policy::{REPEATABLE_START_NANOSECONDS, TestPolicy};
use wcontainers_volatile_bytes::VolatileBytes;

/// The endpoint the loop's bytes are sent to.
const SENDING: u32 = 1;

/// The endpoint the loop's bytes are received from.
const RECEIVING: u32 = 2;

/// Services whose one stream loops back: what is sent to `SENDING` waits in a queue to be received from `RECEIVING`.
#[derive(Default)]
struct LoopServices {
    queue: Mutex<VecDeque<u8>>,
}

impl Host for LoopServices {
    fn warn(&self, warning: Warning<'_>) {
        panic!("the loop's services have no slow path, yet warned of {warning:?}");
    }
}

impl StreamTransport for LoopServices {
    type Endpoint = u32;

    fn send_stream(&self, peer: &u32, payload: &impl VolatileBytes) -> Result<usize, HostError> {
        if *peer != SENDING {
            return Err(HostError::NotSupported);
        }
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        payload
            .read_to(&mut *queue)
            .map_err(|_| HostError::WouldBlock)
    }

    fn receive_stream(&self, peer: &u32, into: &impl VolatileBytes) -> Result<usize, HostError> {
        if *peer != RECEIVING {
            return Err(HostError::NotSupported);
        }
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        match into.write_from(&mut *queue) {
            Ok(count) => Ok(count),
            Err(never) => match never {},
        }
    }

    /// Drops what the receiving end had yet to receive when it is the one closed.
    fn close_stream(&self, peer: &u32) {
        if *peer == RECEIVING {
            self.queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear();
        }
    }
}

impl Readiness<u32> for LoopServices {
    async fn wait_until_ready(
        &self,
        peer: &u32,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        // Nothing else happens on the loop while a wait is pending, so a wait not ready at once is never woken.
        poll_fn(|_| {
            let queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
            let ready_for = match *peer {
                SENDING => ReadyFor::WRITABLE,
                RECEIVING if !queue.is_empty() => ReadyFor::READABLE,
                _ => ReadyFor::empty(),
            } & interest;
            if ready_for.is_empty() {
                return Poll::Pending;
            }
            Poll::Ready(Ok(ReadinessReport {
                ready_for,
                unconditional: UnconditionalEvents::empty(),
            }))
        })
        .await
    }
}

wcontainers_system_conformance::stream_tests!(
    transport: TestPolicy::new(LoopServices::default(), u64::MIN, REPEATABLE_START_NANOSECONDS),
    sending: SENDING,
    receiving: RECEIVING
);
