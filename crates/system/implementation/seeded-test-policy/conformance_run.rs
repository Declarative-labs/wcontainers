//! The system suite's stream tests on the policy, over services whose one stream loops back to them.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

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
    state: Mutex<LoopState>,
}

/// The bytes in flight, whether the sending endpoint was closed, and the waits, each in a slot of its own.
#[derive(Default)]
struct LoopState {
    queue: VecDeque<u8>,
    sending_closed: bool,
    waits: Vec<Option<(u32, Waker)>>,
}

impl LoopServices {
    fn state(&self) -> MutexGuard<'_, LoopState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wakes every wait on `peer`, each once.
    fn wake(&self, peer: u32) {
        let woken: Vec<Waker> = self
            .state()
            .waits
            .iter_mut()
            .filter(|wait| wait.as_ref().is_some_and(|(on, _)| *on == peer))
            .filter_map(Option::take)
            .map(|(_, waker)| waker)
            .collect();
        woken.into_iter().for_each(Waker::wake);
    }
}

impl LoopState {
    /// What `peer` is ready for: sending always has room, receiving holds bytes or hangs up once sending is closed.
    fn report(&self, peer: u32) -> ReadinessReport {
        let mut report = ReadinessReport {
            ready_for: ReadyFor::empty(),
            unconditional: UnconditionalEvents::empty(),
        };
        match peer {
            SENDING => report.ready_for |= ReadyFor::WRITABLE,
            RECEIVING => {
                if !self.queue.is_empty() {
                    report.ready_for |= ReadyFor::READABLE;
                }
                if self.sending_closed {
                    report.unconditional |= UnconditionalEvents::HANG_UP;
                }
            }
            _ => {}
        }
        report
    }
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
        let taken = {
            let mut state = self.state();
            if state.sending_closed {
                return Err(HostError::Closed);
            }
            payload
                .read_to(&mut state.queue)
                .map_err(|_| HostError::WouldBlock)?
        };
        if taken > usize::MIN {
            self.wake(RECEIVING);
        }
        Ok(taken)
    }

    /// Gives what is held; with nothing held, the end once sending is closed, else that a receive would block.
    fn receive_stream(&self, peer: &u32, into: &impl VolatileBytes) -> Result<usize, HostError> {
        if *peer != RECEIVING {
            return Err(HostError::NotSupported);
        }
        let mut state = self.state();
        if state.queue.is_empty() {
            return match state.sending_closed {
                true => Ok(usize::MIN),
                false => Err(HostError::WouldBlock),
            };
        }
        match into.write_from(&mut state.queue) {
            Ok(count) => Ok(count),
            Err(never) => match never {},
        }
    }

    /// Closing sending ends the stream after what it sent and wakes the waits on receiving; closing receiving drops
    /// what it had yet to receive.
    fn close_stream(&self, peer: &u32) {
        match *peer {
            SENDING => {
                self.state().sending_closed = true;
                self.wake(RECEIVING);
            }
            RECEIVING => self.state().queue.clear(),
            _ => {}
        }
    }
}

/// A wait on one endpoint of the loop, kept in one slot of its waits until it finishes or is dropped.
struct Wait<'a> {
    services: &'a LoopServices,
    peer: u32,
    interest: ReadyFor,
    slot: Option<usize>,
}

impl Future for Wait<'_> {
    type Output = Result<ReadinessReport, HostError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut state = this.services.state();
        let report = state.report(this.peer);
        if report.ready_for.intersects(this.interest) || !report.unconditional.is_empty() {
            return Poll::Ready(Ok(report));
        }
        let waiting = Some((this.peer, context.waker().clone()));
        match this.slot.filter(|&slot| slot < state.waits.len()) {
            Some(slot) => state.waits[slot] = waiting,
            None => {
                this.slot = Some(state.waits.len());
                state.waits.push(waiting);
            }
        }
        Poll::Pending
    }
}

impl Drop for Wait<'_> {
    /// A dropped wait is withdrawn, so it is never woken.
    fn drop(&mut self) {
        if let Some(slot) = self.slot
            && let Some(wait) = self.services.state().waits.get_mut(slot)
        {
            *wait = None;
        }
    }
}

impl Readiness<u32> for LoopServices {
    async fn wait_until_ready(
        &self,
        peer: &u32,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        Wait {
            services: self,
            peer: *peer,
            interest,
            slot: None,
        }
        .await
    }
}

wcontainers_system_conformance::stream_tests!(
    transport: TestPolicy::new(LoopServices::default(), u64::MIN, REPEATABLE_START_NANOSECONDS),
    sending: SENDING,
    receiving: RECEIVING
);
