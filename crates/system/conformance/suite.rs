//! The tests every implementation of wcontainers-system runs, written against its traits alone.

use core::cell::{Cell, RefCell};
use core::future::Future;
use core::mem::size_of;
use core::pin::pin;
use core::task::{Context, Poll, Waker};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::task::Wake;
use std::time::Duration;

pub use wcontainers_readiness_conformance;

use domain::base::iana::{Class, Rtype};
use domain::base::{Name, Question};
use wcontainers_abi::HostError;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_kernel::TaskId;
use wcontainers_readiness::{ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_readiness_conformance::Waiting;
use wcontainers_system::{DomainNameResolver, FuseServer, StreamTransport, TaskIdAllocator};
use wcontainers_volatile_bytes::VolatileBytes;

/// The id today's kernel gives as the one before its first.
const BEFORE_FIRST_TASK_ID: TaskId = TaskId(u32::MIN);

/// Checks that an id handed out and not freed is not handed out next.
pub fn allocate_task_ids_apart<S: TaskIdAllocator>(source: S) {
    let first = source.allocate_task_id(BEFORE_FIRST_TASK_ID);
    let second = source.allocate_task_id(first);
    assert_ne!(first, second);
}

/// Checks that freeing one id does not let the next allocation hand out another still held.
pub fn free_task_id_alone<S: TaskIdAllocator>(source: S) {
    let first = source.allocate_task_id(BEFORE_FIRST_TASK_ID);
    let second = source.allocate_task_id(first);
    source.free_task_id(first);
    assert_ne!(source.allocate_task_id(second), second);
}

/// Calls each method of `FuseServer` as a placeholder that fails until the real checks are written.
pub fn call_fuse_server_methods<F: FuseServer>(server: F) {
    drop(server.send_request(&[]));
    todo!("this is a placeholder: the real checks of FuseServer are not written yet");
}

/// Calls each method of `DomainNameResolver` as a placeholder that fails until the real checks are written.
pub fn call_domain_name_resolver_methods<R: DomainNameResolver>(resolver: R) {
    let question = Question::new(Name::root_slice(), Rtype::A, Class::IN);
    drop(resolver.resolve_domain(question));
    todo!("this is a placeholder: the real checks of DomainNameResolver are not written yet");
}

/// The length of a short message sent on a stream, the size of a `u128`.
const SHORT_MESSAGE_LENGTH: usize = size_of::<u128>();

/// The length of the smallest buffer, one byte.
const ONE_BYTE: usize = size_of::<u8>();

/// The largest page of the hosts the suite runs on, [Apple silicon's 16 KiB](https://developer.apple.com/documentation/apple-silicon/addressing-architectural-differences-in-your-macos-code#Fix-problems-caused-by-the-page-size-change).
const LARGEST_PAGE_LENGTH: usize = 1 << 14;

/// The lengths sent and received: one byte, a short message, and one byte more than a page.
const STREAM_LENGTHS: &[usize] = &[ONE_BYTE, SHORT_MESSAGE_LENGTH, LARGEST_PAGE_LENGTH + 1];

/// The lengths a check receives into when it takes a message in parts: one byte, and half a short message.
const PART_LENGTHS: &[usize] = &[ONE_BYTE, SHORT_MESSAGE_LENGTH / size_of::<u16>()];

/// How many bytes a check sends before it takes a transport to have no limit, Linux's default largest pipe.
///
/// https://man7.org/linux/man-pages/man7/pipe.7.html#:~:text=pipe%2Dmax%2Dsize
const ROOM_PROBE_LENGTH: usize = 1 << 20;

/// How long a check waits for a transport to wake a wait it polled, as long as the readiness suite waits.
///
/// https://github.com/stack1ng/wcontainers/blob/main/crates/readiness/conformance/conformance.rs
const WAKE_PATIENCE: Duration = Duration::from_secs(1);

/// The wakes a wait polled by hand has been given, counted.
#[derive(Default)]
struct Wakes {
    count: Mutex<usize>,
    changed: Condvar,
}

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        *self.count.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        self.changed.notify_all();
    }
}

impl Wakes {
    /// Returns whether the wait has been woken `count` times before the patience of a check ends.
    fn reached(&self, count: usize) -> bool {
        let wakes = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        let (wakes, _) = self
            .changed
            .wait_timeout_while(wakes, WAKE_PATIENCE, |wakes| *wakes < count)
            .unwrap_or_else(PoisonError::into_inner);
        *wakes >= count
    }
}

/// Polls a wait on the endpoint until it finishes, each poll after a wake, and returns its report.
fn readiness_soon<T: StreamTransport>(
    transport: &T,
    endpoint: &T::Endpoint,
    interest: ReadyFor,
) -> ReadinessReport {
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&wakes));
    let mut wait = pin!(transport.wait_until_ready(endpoint, interest));
    let mut pending = usize::MIN;
    loop {
        match wait.as_mut().poll(&mut Context::from_waker(&waker)) {
            Poll::Ready(report) => {
                return report.expect("the transport must say how ready its endpoint is");
            }
            Poll::Pending => {
                pending += 1;
                assert!(
                    wakes.reached(pending),
                    "a wait for {interest:?} was never woken"
                );
            }
        }
    }
}

/// Bytes in cells, reached only through a buffer of their own as bytes another thread may change are.
struct CellBytes {
    cells: Vec<Cell<u8>>,
}

impl CellBytes {
    fn new(bytes: &[u8]) -> Self {
        Self {
            cells: bytes.iter().copied().map(Cell::new).collect(),
        }
    }

    /// Returns a copy of the bytes held.
    fn contents(&self) -> Vec<u8> {
        self.cells.iter().map(Cell::get).collect()
    }
}

impl VolatileBytes for CellBytes {
    fn len(&self) -> u64 {
        self.cells.len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        into.iter_mut()
            .zip(&self.cells)
            .map(|(byte, cell)| *byte = cell.get())
            .count()
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.cells
            .iter()
            .zip(bytes)
            .map(|(cell, &byte)| cell.set(byte))
            .count()
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        sink.write(&self.contents())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut buffer = vec![u8::MIN; self.cells.len()];
        let count = source.read(&mut buffer)?;
        self.write(&buffer[..count]);
        Ok(count)
    }
}

/// What holds the bytes a check sends from and receives into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Holder {
    /// A slice in a `RefCell`, as the kernel gives them.
    Buffer,
    /// Each byte in a cell, reached only through `VolatileBytes`, which no slice can stand for.
    Cells,
}

/// Returns bytes that differ from their neighbors, every byte value in turn.
fn message(length: usize) -> Vec<u8> {
    (u8::MIN..=u8::MAX).cycle().take(length).collect()
}

/// Sends `bytes` once from the holder, and returns how many the transport took, after checking it took no more.
fn send_once<T: StreamTransport>(
    transport: &T,
    sending: &T::Endpoint,
    bytes: &[u8],
    holder: Holder,
) -> Result<usize, HostError> {
    let taken = match holder {
        Holder::Buffer => {
            let mut buffer = bytes.to_vec();
            transport.send_stream(sending, &RefCell::new(&mut buffer[..]))
        }
        Holder::Cells => transport.send_stream(sending, &CellBytes::new(bytes)),
    }?;
    assert!(
        taken <= bytes.len(),
        "a send of {} bytes says it took {taken}",
        bytes.len()
    );
    Ok(taken)
}

/// Receives once into a holder of `length` bytes, and returns what came, after checking it is no more than asked.
fn receive_once<T: StreamTransport>(
    transport: &T,
    receiving: &T::Endpoint,
    length: usize,
    holder: Holder,
) -> Result<Vec<u8>, HostError> {
    let mut buffer = vec![u8::MIN; length];
    let (count, bytes) = match holder {
        Holder::Buffer => {
            let count = transport.receive_stream(receiving, &RefCell::new(&mut buffer[..]))?;
            (count, buffer)
        }
        Holder::Cells => {
            let cells = CellBytes::new(&buffer);
            let count = transport.receive_stream(receiving, &cells)?;
            (count, cells.contents())
        }
    };
    assert!(
        count <= length,
        "a receive into {length} bytes says it gave {count}"
    );
    Ok(bytes[..count].to_vec())
}

/// Moves `payload` from `sending` to `receiving`, at most `part` bytes per receive, and returns what arrived.
///
/// It sends and receives as often as the transport takes, and waits on readiness when neither moves.
fn transfer<T: StreamTransport>(
    transport: &T,
    sending: &T::Endpoint,
    receiving: &T::Endpoint,
    payload: &[u8],
    part: usize,
    holder: Holder,
) -> Vec<u8> {
    let mut sent = usize::MIN;
    let mut arrived = Vec::with_capacity(payload.len());
    while arrived.len() < payload.len() {
        let mut moved = false;
        if sent < payload.len() {
            match send_once(transport, sending, &payload[sent..], holder) {
                Ok(taken) => {
                    sent += taken;
                    moved |= taken > usize::MIN;
                }
                Err(HostError::WouldBlock) => {}
                Err(error) => panic!("a send to a held stream failed with {error:?}"),
            }
        }
        let wanted = part.min(payload.len() - arrived.len());
        match receive_once(transport, receiving, wanted, holder) {
            Ok(bytes) => {
                assert!(
                    !bytes.is_empty(),
                    "a receive gave the end of a stream whose sending endpoint is held"
                );
                arrived.extend_from_slice(&bytes);
                moved = true;
            }
            Err(HostError::WouldBlock) => {}
            Err(error) => panic!("a receive from a held stream failed with {error:?}"),
        }
        assert!(
            arrived.len() <= sent,
            "{} bytes arrived of {sent} sent",
            arrived.len()
        );
        if !moved {
            let (endpoint, interest) = if sent < payload.len() {
                (sending, ReadyFor::WRITABLE)
            } else {
                (receiving, ReadyFor::READABLE)
            };
            readiness_soon(transport, endpoint, interest);
        }
    }
    arrived
}

/// Checks that bytes sent from a buffer to `sending` arrive whole and in order at `receiving`.
pub fn send_and_receive_through_buffers<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    for &length in STREAM_LENGTHS {
        let payload = message(length);
        let arrived = transfer(
            &transport,
            &sending,
            &receiving,
            &payload,
            length,
            Holder::Buffer,
        );
        assert_eq!(
            arrived, payload,
            "{length} bytes received differ from those sent"
        );
    }
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
}

/// Checks that bytes sent from and received into cells that are not a slice arrive whole and in order.
pub fn send_and_receive_bytes_that_are_not_a_slice<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    for &length in STREAM_LENGTHS {
        let payload = message(length);
        let arrived = transfer(
            &transport,
            &sending,
            &receiving,
            &payload,
            length,
            Holder::Cells,
        );
        assert_eq!(
            arrived, payload,
            "{length} bytes received differ from those sent"
        );
    }
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
}

/// Checks that receives shorter than what was sent each take the next part, and together all of it in order.
pub fn receive_part_and_keep_the_rest<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    let payload = message(SHORT_MESSAGE_LENGTH);
    for &part in PART_LENGTHS {
        let arrived = transfer(
            &transport,
            &sending,
            &receiving,
            &payload,
            part,
            Holder::Buffer,
        );
        assert_eq!(
            arrived, payload,
            "receives of {part} bytes at a time took other bytes than were sent"
        );
    }
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
}

/// Checks that a receive with nothing to give says it would block, never the end, while the sending endpoint is held.
pub fn a_receive_with_nothing_sent_would_block<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    let nothing = receive_once(&transport, &receiving, ONE_BYTE, Holder::Buffer);
    assert_eq!(
        nothing,
        Err(HostError::WouldBlock),
        "a receive before anything was sent"
    );
    let payload = message(SHORT_MESSAGE_LENGTH);
    transfer(
        &transport,
        &sending,
        &receiving,
        &payload,
        SHORT_MESSAGE_LENGTH,
        Holder::Buffer,
    );
    let drained = receive_once(&transport, &receiving, ONE_BYTE, Holder::Buffer);
    assert_eq!(
        drained,
        Err(HostError::WouldBlock),
        "a receive after everything sent was received"
    );
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
}

/// Sends up to `length` bytes, closes `sending`, and checks how `receiving` ends.
///
/// It must hang up, still give everything sent before, and then only end.
fn close_after_sending<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
    length: usize,
) {
    let payload = message(length);
    let mut sent = usize::MIN;
    while sent < length {
        match send_once(&transport, &sending, &payload[sent..], Holder::Buffer) {
            Ok(taken) if taken > usize::MIN => sent += taken,
            Ok(_) | Err(HostError::WouldBlock) => break,
            Err(error) => panic!("a send to a held stream failed with {error:?}"),
        }
    }
    transport.close_stream(&sending);
    assert!(
        length == usize::MIN || sent > usize::MIN,
        "an empty stream took none of {length} bytes"
    );
    let report = readiness_soon(&transport, &receiving, ReadyFor::READABLE);
    assert!(
        report.unconditional.contains(UnconditionalEvents::HANG_UP),
        "a stream whose sending endpoint was closed reports {report:?}"
    );
    let mut arrived = Vec::with_capacity(sent);
    while arrived.len() < sent {
        match receive_once(&transport, &receiving, sent - arrived.len(), Holder::Buffer) {
            Ok(bytes) if !bytes.is_empty() => arrived.extend_from_slice(&bytes),
            ended => panic!(
                "a stream ended with {ended:?} after {} of the {sent} bytes sent before it was closed",
                arrived.len()
            ),
        }
    }
    assert_eq!(
        arrived,
        payload[..sent],
        "bytes sent before closing arrived changed"
    );
    let end = receive_once(&transport, &receiving, ONE_BYTE, Holder::Buffer);
    let ended = match &end {
        Ok(bytes) => bytes.is_empty(),
        Err(error) => *error != HostError::WouldBlock,
    };
    assert!(
        ended,
        "a stream whose sending endpoint was closed gave {end:?} after all it held"
    );
    transport.close_stream(&receiving);
}

/// Checks that once `sending` is closed after a short message, `receiving` hangs up, gives the message, then ends.
pub fn closing_the_sending_endpoint_ends_the_stream_after_what_was_sent<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    close_after_sending(transport, sending, receiving, SHORT_MESSAGE_LENGTH);
}

/// Checks that once `sending` is closed with nothing sent, `receiving` hangs up and ends at once.
pub fn closing_the_sending_endpoint_with_nothing_sent_ends_the_stream<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    close_after_sending(transport, sending, receiving, usize::MIN);
}

/// Checks that a stream that took less than it was given, once its receiving endpoint takes it all, is writable again.
pub fn a_send_without_room_is_ready_after_a_receive<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    let page = message(LARGEST_PAGE_LENGTH);
    let mut sent = Vec::new();
    let mut refused = false;
    while sent.len() < ROOM_PROBE_LENGTH && !refused {
        let taken = match send_once(&transport, &sending, &page, Holder::Buffer) {
            Ok(taken) => taken,
            Err(HostError::WouldBlock) => usize::MIN,
            Err(error) => panic!("a send to a held stream failed with {error:?}"),
        };
        sent.extend_from_slice(&page[..taken]);
        refused = taken < page.len();
    }
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(Arc::clone(&wakes));
    let mut wait = pin!(transport.wait_until_ready(&sending, ReadyFor::WRITABLE));
    let early = wait.as_mut().poll(&mut Context::from_waker(&waker));
    let mut pending = usize::MIN;
    if early.is_pending() {
        pending += 1;
    }
    let mut arrived = Vec::with_capacity(sent.len());
    while arrived.len() < sent.len() {
        match receive_once(
            &transport,
            &receiving,
            sent.len() - arrived.len(),
            Holder::Buffer,
        ) {
            Ok(bytes) => {
                assert!(
                    !bytes.is_empty(),
                    "a receive gave the end of a stream whose sending endpoint is held"
                );
                arrived.extend_from_slice(&bytes);
            }
            Err(HostError::WouldBlock) => {
                readiness_soon(&transport, &receiving, ReadyFor::READABLE);
            }
            Err(error) => panic!("a receive from a held stream failed with {error:?}"),
        }
    }
    assert!(
        arrived == sent,
        "bytes sent until the stream was full arrived changed"
    );
    let report = match early {
        Poll::Ready(report) => report.expect("the transport must say how ready its endpoint is"),
        Poll::Pending => {
            assert!(
                wakes.reached(pending),
                "a wait for room was not woken when everything sent was received"
            );
            match wait.as_mut().poll(&mut Context::from_waker(&waker)) {
                Poll::Ready(report) => {
                    report.expect("the transport must say how ready its endpoint is")
                }
                Poll::Pending => panic!("a woken wait for room on an empty stream did not finish"),
            }
        }
    };
    if refused {
        assert!(
            report.ready_for.contains(ReadyFor::WRITABLE),
            "an empty stream that was full reports {report:?}"
        );
    }
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
}

/// Makes the readiness suite's value: `receiving`, waited on to be readable, made so by a byte sent to `sending`.
pub fn receiving_made_readable_by_a_send<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) -> Waiting<T, T::Endpoint, impl FnOnce(&T, &T::Endpoint)> {
    Waiting {
        readiness: transport,
        handle: receiving,
        interest: ReadyFor::READABLE,
        make_ready: move |transport: &T, _: &T::Endpoint| {
            let taken = send_once(transport, &sending, &message(ONE_BYTE), Holder::Buffer);
            assert_eq!(taken, Ok(ONE_BYTE), "an empty stream did not take one byte");
        },
    }
}

/// Makes the readiness suite's failing value: `receiving`, waited on to be readable, hung up by closing `sending`.
pub fn receiving_hung_up_by_closing_the_sending_endpoint<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) -> Waiting<T, T::Endpoint, impl FnOnce(&T, &T::Endpoint)> {
    Waiting {
        readiness: transport,
        handle: receiving,
        interest: ReadyFor::READABLE,
        make_ready: move |transport: &T, _: &T::Endpoint| transport.close_stream(&sending),
    }
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn allocate_task_ids_apart() {
            $crate::allocate_task_ids_apart($value);
        }

        #[test]
        fn free_task_id_alone() {
            $crate::free_task_id_alone($value);
        }

        #[test]
        fn call_fuse_server_methods() {
            $crate::call_fuse_server_methods($value);
        }

        #[test]
        fn call_domain_name_resolver_methods() {
            $crate::call_domain_name_resolver_methods($value);
        }
    };
}

/// Runs each stream suite function and the readiness suite as tests on a transport and one stream's two endpoints.
///
/// Each test makes the transport and endpoints anew; the readiness suite waits on the receiving endpoint.
#[macro_export]
macro_rules! stream_tests {
    (transport: $transport:expr, sending: $sending:expr, receiving: $receiving:expr) => {
        $crate::stream_tests!(@each ($transport, $sending, $receiving);
            send_and_receive_through_buffers
            send_and_receive_bytes_that_are_not_a_slice
            receive_part_and_keep_the_rest
            a_receive_with_nothing_sent_would_block
            closing_the_sending_endpoint_ends_the_stream_after_what_was_sent
            closing_the_sending_endpoint_with_nothing_sent_ends_the_stream
            a_send_without_room_is_ready_after_a_receive
        );

        mod readiness {
            #[allow(
                unused_imports,
                reason = "the values an implementation gives may name items of the module that runs the suite, or none"
            )]
            use super::*;

            $crate::wcontainers_readiness_conformance::tests!(
                receiving,
                ready: $crate::receiving_made_readable_by_a_send($transport, $sending, $receiving),
                failing: $crate::receiving_hung_up_by_closing_the_sending_endpoint(
                    $transport, $sending, $receiving
                )
            );
        }
    };
    (@each ($transport:expr, $sending:expr, $receiving:expr); $($name:ident)*) => {
        $(
            #[test]
            fn $name() {
                $crate::$name($transport, $sending, $receiving);
            }
        )*
    };
}

#[cfg(test)]
mod suite {
    use core::cell::RefCell;
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll, Waker};
    use std::collections::VecDeque;

    use wcontainers_abi::HostError;
    use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor, UnconditionalEvents};
    use wcontainers_system::StreamTransport;
    use wcontainers_volatile_bytes::VolatileBytes;

    use crate::{LARGEST_PAGE_LENGTH, ONE_BYTE};

    /// What a loop gets wrong, each of which a check must catch.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Flaw {
        None,
        /// A receive with nothing to give says the stream ended.
        EndsWhenEmpty,
        /// A receive says it gave one byte more than it did.
        CountsAByteMore,
        /// A send takes every byte but the first of each call.
        DropsTheFirstByte,
        /// A send keeps every byte but says it took one fewer.
        CountsAByteFewer,
        /// A receive shorter than what is held throws the rest away.
        LosesTheRestOfAPartialReceive,
        /// Closing the sending endpoint throws away what it sent.
        DropsWhatWasSentOnClose,
        /// Closing the sending endpoint is never reported as a hang-up.
        NoHangUpOnClose,
        /// After closing the sending endpoint and giving all it held, a receive still says it would block.
        WouldBlockAfterClose,
        /// A receive that makes room does not wake waits for it.
        NeverWakesWriters,
        /// A send does not wake waits for it.
        NeverWakesReaders,
        /// An empty stream reports itself readable.
        ReadableWhenEmpty,
        /// A dropped wait is still woken.
        KeepsDroppedWaits,
    }

    /// The two endpoints of a loop's one stream.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub(super) enum End {
        Sending,
        Receiving,
    }

    /// The bytes in flight and the waits on a loop.
    #[derive(Default)]
    struct State {
        queue: VecDeque<u8>,
        sending_closed: bool,
        waits: Vec<Option<(End, Waker)>>,
    }

    /// A transport whose one stream loops back, holding at most `capacity` bytes, with one flaw or none.
    pub(super) struct Loop {
        flaw: Flaw,
        capacity: usize,
        state: RefCell<State>,
    }

    impl Loop {
        pub(super) fn new(flaw: Flaw, capacity: usize) -> Self {
            Self {
                flaw,
                capacity,
                state: RefCell::new(State::default()),
            }
        }

        /// Wakes every wait on `end`.
        fn wake(&self, end: End) {
            let woken: Vec<Waker> = self
                .state
                .borrow_mut()
                .waits
                .iter_mut()
                .filter(|wait| wait.as_ref().is_some_and(|(on, _)| *on == end))
                .filter_map(Option::take)
                .map(|(_, waker)| waker)
                .collect();
            woken.into_iter().for_each(Waker::wake);
        }

        fn report(&self, end: End) -> ReadinessReport {
            let state = self.state.borrow();
            let mut report = ReadinessReport {
                ready_for: ReadyFor::empty(),
                unconditional: UnconditionalEvents::empty(),
            };
            match end {
                End::Sending => {
                    if state.queue.len() < self.capacity {
                        report.ready_for |= ReadyFor::WRITABLE;
                    }
                }
                End::Receiving => {
                    if !state.queue.is_empty() || self.flaw == Flaw::ReadableWhenEmpty {
                        report.ready_for |= ReadyFor::READABLE;
                    }
                    if state.sending_closed {
                        if self.flaw == Flaw::NoHangUpOnClose {
                            report.ready_for |= ReadyFor::READABLE;
                        } else {
                            report.unconditional |= UnconditionalEvents::HANG_UP;
                        }
                    }
                }
            }
            report
        }
    }

    impl StreamTransport for Loop {
        type Endpoint = End;

        fn send_stream(
            &self,
            peer: &End,
            payload: &impl VolatileBytes,
        ) -> Result<usize, HostError> {
            if *peer != End::Sending {
                return Err(HostError::NotSupported);
            }
            let taken = {
                let mut state = self.state.borrow_mut();
                if state.sending_closed {
                    return Err(HostError::Closed);
                }
                let room = self.capacity - state.queue.len();
                if room == usize::MIN {
                    return Err(HostError::WouldBlock);
                }
                let length = usize::try_from(payload.len())
                    .unwrap_or(usize::MAX)
                    .min(room);
                let mut bytes = vec![u8::MIN; length];
                let taken = payload.read(&mut bytes);
                let kept = if self.flaw == Flaw::DropsTheFirstByte {
                    bytes.get(ONE_BYTE..taken).unwrap_or_default()
                } else {
                    &bytes[..taken]
                };
                state.queue.extend(kept);
                taken
            };
            if taken > usize::MIN && self.flaw != Flaw::NeverWakesReaders {
                self.wake(End::Receiving);
            }
            if self.flaw == Flaw::CountsAByteFewer {
                return Ok(taken.saturating_sub(1));
            }
            Ok(taken)
        }

        fn receive_stream(
            &self,
            peer: &End,
            into: &impl VolatileBytes,
        ) -> Result<usize, HostError> {
            if *peer != End::Receiving {
                return Err(HostError::NotSupported);
            }
            let given = {
                let mut state = self.state.borrow_mut();
                if state.queue.is_empty() {
                    let ended = state.sending_closed && self.flaw != Flaw::WouldBlockAfterClose;
                    return if ended || self.flaw == Flaw::EndsWhenEmpty {
                        Ok(usize::MIN)
                    } else {
                        Err(HostError::WouldBlock)
                    };
                }
                let length = usize::try_from(into.len()).unwrap_or(usize::MAX);
                let count = length.min(state.queue.len());
                let bytes: Vec<u8> = state.queue.drain(..count).collect();
                into.write(&bytes);
                if self.flaw == Flaw::LosesTheRestOfAPartialReceive {
                    state.queue.clear();
                }
                count
            };
            if self.flaw != Flaw::NeverWakesWriters {
                self.wake(End::Sending);
            }
            if self.flaw == Flaw::CountsAByteMore {
                return Ok(given + 1);
            }
            Ok(given)
        }

        fn close_stream(&self, peer: &End) {
            if *peer == End::Sending {
                {
                    let mut state = self.state.borrow_mut();
                    state.sending_closed = true;
                    if self.flaw == Flaw::DropsWhatWasSentOnClose {
                        state.queue.clear();
                    }
                }
                self.wake(End::Receiving);
            }
        }
    }

    /// A wait on one endpoint of a loop, kept in one slot of its waits.
    struct Wait<'a> {
        transport: &'a Loop,
        end: End,
        interest: ReadyFor,
        slot: Option<usize>,
    }

    impl Future for Wait<'_> {
        type Output = Result<ReadinessReport, HostError>;

        fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();
            let report = this.transport.report(this.end);
            if report.ready_for.intersects(this.interest) || !report.unconditional.is_empty() {
                return Poll::Ready(Ok(report));
            }
            let mut state = this.transport.state.borrow_mut();
            let waiting = Some((this.end, context.waker().clone()));
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
        fn drop(&mut self) {
            if self.transport.flaw != Flaw::KeepsDroppedWaits
                && let Some(slot) = self.slot
                && let Some(wait) = self.transport.state.borrow_mut().waits.get_mut(slot)
            {
                *wait = None;
            }
        }
    }

    impl Readiness<End> for Loop {
        async fn wait_until_ready(
            &self,
            handle: &End,
            interest: ReadyFor,
        ) -> Result<ReadinessReport, HostError> {
            Wait {
                transport: self,
                end: *handle,
                interest,
                slot: None,
            }
            .await
        }
    }

    /// The room of a bounded loop, a page, fewer bytes than the longest stream check sends.
    pub(super) const BOUNDED_CAPACITY: usize = LARGEST_PAGE_LENGTH;

    mod unbounded {
        use super::{End, Flaw, Loop};

        crate::stream_tests!(
            transport: Loop::new(Flaw::None, usize::MAX),
            sending: End::Sending,
            receiving: End::Receiving
        );
    }

    mod bounded {
        use super::{BOUNDED_CAPACITY, End, Flaw, Loop};

        crate::stream_tests!(
            transport: Loop::new(Flaw::None, BOUNDED_CAPACITY),
            sending: End::Sending,
            receiving: End::Receiving
        );
    }

    #[test]
    #[should_panic(expected = "a receive before anything was sent")]
    fn catches_a_receive_that_ends_an_empty_stream() {
        crate::a_receive_with_nothing_sent_would_block(
            Loop::new(Flaw::EndsWhenEmpty, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "says it gave")]
    fn catches_a_receive_that_counts_a_byte_more() {
        crate::receive_part_and_keep_the_rest(
            Loop::new(Flaw::CountsAByteMore, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "was never woken")]
    fn catches_a_send_that_drops_the_first_byte() {
        crate::send_and_receive_through_buffers(
            Loop::new(Flaw::DropsTheFirstByte, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "bytes arrived of")]
    fn catches_a_send_that_counts_a_byte_fewer() {
        crate::send_and_receive_bytes_that_are_not_a_slice(
            Loop::new(Flaw::CountsAByteFewer, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "was never woken")]
    fn catches_a_partial_receive_that_loses_the_rest() {
        crate::receive_part_and_keep_the_rest(
            Loop::new(Flaw::LosesTheRestOfAPartialReceive, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "a stream ended with")]
    fn catches_a_close_that_drops_what_was_sent() {
        crate::closing_the_sending_endpoint_ends_the_stream_after_what_was_sent(
            Loop::new(Flaw::DropsWhatWasSentOnClose, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "a stream whose sending endpoint was closed reports")]
    fn catches_a_close_without_a_hang_up() {
        crate::closing_the_sending_endpoint_ends_the_stream_after_what_was_sent(
            Loop::new(Flaw::NoHangUpOnClose, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "after all it held")]
    fn catches_a_closed_stream_that_would_block() {
        crate::closing_the_sending_endpoint_ends_the_stream_after_what_was_sent(
            Loop::new(Flaw::WouldBlockAfterClose, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "after all it held")]
    fn catches_a_closed_empty_stream_that_would_block() {
        crate::closing_the_sending_endpoint_with_nothing_sent_ends_the_stream(
            Loop::new(Flaw::WouldBlockAfterClose, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "a stream whose sending endpoint was closed reports")]
    fn catches_a_close_of_an_empty_stream_without_a_hang_up() {
        crate::closing_the_sending_endpoint_with_nothing_sent_ends_the_stream(
            Loop::new(Flaw::NoHangUpOnClose, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "a wait for room was not woken")]
    fn catches_a_receive_that_never_wakes_writers() {
        crate::a_send_without_room_is_ready_after_a_receive(
            Loop::new(Flaw::NeverWakesWriters, BOUNDED_CAPACITY),
            End::Sending,
            End::Receiving,
        );
    }

    #[test]
    #[should_panic(expected = "woken when the handle became ready")]
    fn catches_a_send_that_never_wakes_readers() {
        wcontainers_readiness_conformance::a_wait_finishes_when_the_handle_becomes_ready(
            crate::receiving_made_readable_by_a_send(
                Loop::new(Flaw::NeverWakesReaders, BOUNDED_CAPACITY),
                End::Sending,
                End::Receiving,
            ),
        );
    }

    #[test]
    #[should_panic(expected = "finished before the handle was ready")]
    fn catches_an_empty_stream_reported_readable() {
        wcontainers_readiness_conformance::a_wait_does_not_finish_before_the_handle_is_ready(
            crate::receiving_made_readable_by_a_send(
                Loop::new(Flaw::ReadableWhenEmpty, BOUNDED_CAPACITY),
                End::Sending,
                End::Receiving,
            ),
        );
    }

    #[test]
    #[should_panic(expected = "a dropped wait was woken")]
    fn catches_a_dropped_wait_still_woken() {
        wcontainers_readiness_conformance::a_dropped_wait_is_withdrawn(
            crate::receiving_made_readable_by_a_send(
                Loop::new(Flaw::KeepsDroppedWaits, BOUNDED_CAPACITY),
                End::Sending,
                End::Receiving,
            ),
        );
    }

    #[test]
    #[should_panic(expected = "UnconditionalEvents(0x0)")]
    fn catches_a_close_never_reported_as_a_failure() {
        wcontainers_readiness_conformance::a_failure_ends_a_wait_that_did_not_ask_for_it(
            crate::receiving_hung_up_by_closing_the_sending_endpoint(
                Loop::new(Flaw::NoHangUpOnClose, BOUNDED_CAPACITY),
                End::Sending,
                End::Receiving,
            ),
        );
    }
}
