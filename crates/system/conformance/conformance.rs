//! The tests every implementation of wcontainers-system runs, written against its traits alone.

use core::cell::{Cell, RefCell};
use core::mem::size_of;
use core::pin::pin;
use core::task::{Context, Poll, Waker};

use domain::base::iana::{Class, Rtype};
use domain::base::{Name, Question};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_kernel::TaskId;
use wcontainers_readiness::{ReadinessReport, ReadyFor};
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

/// The largest page of the hosts the suite runs on, Apple silicon's 16 KiB (four of x86's).
const LARGEST_PAGE_LENGTH: usize = 1 << 14;

/// The lengths sent and received whole: one byte, a short message, and one byte more than a page.
const STREAM_LENGTHS: &[usize] = &[1, SHORT_MESSAGE_LENGTH, LARGEST_PAGE_LENGTH + 1];

/// Bytes held one by one in cells, which a source or a sink reaches only through a buffer of their own, as bytes
/// another thread may change are; they count how they were reached.
struct CellBytes {
    cells: Vec<Cell<u8>>,
    /// Calls of `read_to` and `write_from`, each of which moves all the bytes.
    whole_moves: Cell<usize>,
    /// Calls of `read` and `write`, which move bytes piece by piece.
    piecewise_moves: Cell<usize>,
}

impl CellBytes {
    fn new(bytes: &[u8]) -> Self {
        Self {
            cells: bytes.iter().copied().map(Cell::new).collect(),
            whole_moves: Cell::new(usize::MIN),
            piecewise_moves: Cell::new(usize::MIN),
        }
    }

    /// Returns a copy of the bytes held, without counting it as a move.
    fn contents(&self) -> Vec<u8> {
        self.cells.iter().map(Cell::get).collect()
    }

    /// Returns the number of whole moves and of piecewise moves made so far.
    fn moves(&self) -> (usize, usize) {
        (self.whole_moves.get(), self.piecewise_moves.get())
    }
}

impl VolatileBytes for CellBytes {
    fn len(&self) -> u64 {
        self.cells.len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        self.piecewise_moves.set(self.piecewise_moves.get() + 1);
        into.iter_mut()
            .zip(&self.cells)
            .map(|(byte, cell)| *byte = cell.get())
            .count()
    }

    fn write(&self, bytes: &[u8]) -> usize {
        self.piecewise_moves.set(self.piecewise_moves.get() + 1);
        self.cells
            .iter()
            .zip(bytes)
            .map(|(cell, &byte)| cell.set(byte))
            .count()
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        self.whole_moves.set(self.whole_moves.get() + 1);
        sink.write(&self.contents())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        self.whole_moves.set(self.whole_moves.get() + 1);
        let mut buffer = vec![u8::MIN; self.cells.len()];
        let count = source.read(&mut buffer)?;
        self.cells
            .iter()
            .zip(&buffer[..count])
            .for_each(|(cell, &byte)| cell.set(byte));
        Ok(count)
    }
}

/// Returns bytes that differ from their neighbors, every byte value in turn.
fn message(length: usize) -> Vec<u8> {
    (u8::MIN..=u8::MAX).cycle().take(length).collect()
}

/// Returns the endpoint's readiness, which the transport must know at the first poll.
fn readiness_at_once<T: StreamTransport>(
    transport: &T,
    endpoint: &T::Endpoint,
    interest: ReadyFor,
) -> ReadinessReport {
    let waiting = pin!(transport.wait_until_ready(endpoint, interest));
    match waiting.poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(report) => report.expect("the transport must say how ready its endpoint is"),
        Poll::Pending => {
            panic!("the transport did not know at once that what was sent can be received")
        }
    }
}

/// Checks that the receiving endpoint is readable and receives in one call into all of `into`, and returns the
/// number of bytes received.
fn receive_whole<T: StreamTransport>(
    transport: &T,
    receiving: &T::Endpoint,
    into: &impl VolatileBytes,
) -> usize {
    let report = readiness_at_once(transport, receiving, ReadyFor::READABLE);
    assert!(
        report.ready_for.contains(ReadyFor::READABLE),
        "what was sent is not ready to receive"
    );
    let received = transport
        .receive_stream(receiving, into)
        .expect("the transport must receive what was sent");
    assert_eq!(
        received as u64,
        into.len(),
        "a receive of {} bytes sent was not one call for all of them",
        into.len()
    );
    received
}

/// Checks that bytes sent from a buffer to `sending` arrive whole and in order at `receiving`, which has them at once,
/// in one receive into a buffer, also when longer than a page.
pub fn send_and_receive_through_buffers<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    for &length in STREAM_LENGTHS {
        let mut payload = message(length);
        let sent = transport
            .send_stream(&sending, &RefCell::new(&mut payload[..]))
            .expect("the transport must send what it is given");
        assert_eq!(
            sent, length,
            "a send of {length} bytes did not send them all"
        );
        let mut buffer = vec![u8::MIN; length];
        receive_whole(&transport, &receiving, &RefCell::new(&mut buffer[..]));
        assert_eq!(
            buffer, payload,
            "{length} bytes received differ from those sent"
        );
    }
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
}

/// Checks that bytes which are not a slice are sent to `sending` in one read to a sink and received at `receiving`,
/// which has them at once, in one write from a source, whole and in order, also when longer than a page.
pub fn send_and_receive_bytes_that_are_not_a_slice<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    for &length in STREAM_LENGTHS {
        let payload = CellBytes::new(&message(length));
        let sent = transport
            .send_stream(&sending, &payload)
            .expect("the transport must send what it is given");
        assert_eq!(
            sent, length,
            "a send of {length} bytes did not send them all"
        );
        assert_eq!(
            payload.moves(),
            (1, usize::MIN),
            "a send of {length} bytes was not one read to a sink"
        );
        let into = CellBytes::new(&vec![u8::MIN; length]);
        receive_whole(&transport, &receiving, &into);
        assert_eq!(
            into.moves(),
            (1, usize::MIN),
            "a receive of {length} bytes was not one write from a source"
        );
        assert_eq!(
            into.contents(),
            payload.contents(),
            "{length} bytes received differ from those sent"
        );
    }
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
}

/// Checks that a receive at `receiving` shorter than what was sent to `sending` takes the start, and the next the rest.
pub fn receive_part_and_keep_the_rest<T: StreamTransport>(
    transport: T,
    sending: T::Endpoint,
    receiving: T::Endpoint,
) {
    let mut payload = message(SHORT_MESSAGE_LENGTH);
    let sent = transport
        .send_stream(&sending, &RefCell::new(&mut payload[..]))
        .expect("the transport must send what it is given");
    assert_eq!(
        sent, SHORT_MESSAGE_LENGTH,
        "a send did not send all its bytes"
    );
    let (start, rest) = payload.split_at(SHORT_MESSAGE_LENGTH / size_of::<u16>());
    let mut buffer = vec![u8::MIN; start.len()];
    receive_whole(&transport, &receiving, &RefCell::new(&mut buffer[..]));
    assert_eq!(
        buffer, start,
        "the first receive did not take the start of what was sent"
    );
    let into = CellBytes::new(&vec![u8::MIN; rest.len()]);
    receive_whole(&transport, &receiving, &into);
    assert_eq!(
        into.contents(),
        rest,
        "the second receive did not take the rest of what was sent"
    );
    transport.close_stream(&sending);
    transport.close_stream(&receiving);
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

/// Runs each stream suite function above as its own test on the transport and the two endpoints of one stream an
/// implementation gives, made anew for every test.
#[macro_export]
macro_rules! stream_tests {
    (transport: $transport:expr, sending: $sending:expr, receiving: $receiving:expr) => {
        #[test]
        fn send_and_receive_through_buffers() {
            $crate::send_and_receive_through_buffers($transport, $sending, $receiving);
        }

        #[test]
        fn send_and_receive_bytes_that_are_not_a_slice() {
            $crate::send_and_receive_bytes_that_are_not_a_slice($transport, $sending, $receiving);
        }

        #[test]
        fn receive_part_and_keep_the_rest() {
            $crate::receive_part_and_keep_the_rest($transport, $sending, $receiving);
        }
    };
}
