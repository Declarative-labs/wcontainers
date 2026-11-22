//! The kernel's [`Host`] through the link: each call is a request the monitor answers by calling the host's
//! services.

use alloc::vec::Vec;

use spin::Mutex;
use wcontainers_abi::{ConnectionId, End, Host, HostError, Warning};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::link::{Link, Ring};

crate::requests! {
    first: crate::routines::Request::END;
    MonotonicNanoseconds {} -> Value;
    RealtimeNanoseconds {} -> Value;
    /// `length` random bytes, in the answer's bytes.
    Random { length } -> ();
    /// A slow path: its reason in the bytes.
    Warn { count, detail } -> ();
    /// The bytes the kernel sends on the end numbered `stream`.
    Output { stream } -> ();
    /// A UDP flow to the address in the bytes.
    Connect { connection, port } -> Outcome;
    SendBytes { connection } -> Outcome;
    /// Up to `length` bytes, in the answer's bytes.
    Receive { connection, length } -> Outcome;
    Close { connection } -> ();
    /// The kernel may take `bytes` more on the end numbered `stream`.
    CreditStream { stream, bytes } -> ();
    /// The kernel let go of the end numbered `stream`.
    CloseStream { stream } -> ();
}

/// One number as an answer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Value {
    pub value: u64,
}

/// A `Result<usize, HostError>` as words.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Outcome {
    /// An [`OutcomeKind`]'s word.
    pub kind: u64,
    /// The count when done, the error number for [`HostError::Errno`].
    pub value: u64,
}

/// What an [`Outcome`] says.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeKind {
    Done,
    NotSupported,
    WouldBlock,
    Closed,
    Errno,
}

impl Outcome {
    pub fn of(result: Result<usize, HostError>) -> Outcome {
        let (kind, value) = match result {
            Ok(count) => (OutcomeKind::Done, count as u64),
            Err(HostError::NotSupported) => (OutcomeKind::NotSupported, 0),
            Err(HostError::WouldBlock) => (OutcomeKind::WouldBlock, 0),
            Err(HostError::Closed) => (OutcomeKind::Closed, 0),
            Err(HostError::Errno(errno)) => (OutcomeKind::Errno, i64::from(errno) as u64),
        };
        Outcome {
            kind: kind as u64,
            value,
        }
    }

    pub fn result(self) -> Result<usize, HostError> {
        match self.kind {
            k if k == OutcomeKind::Done as u64 => Ok(self.value as usize),
            k if k == OutcomeKind::WouldBlock as u64 => Err(HostError::WouldBlock),
            k if k == OutcomeKind::Closed as u64 => Err(HostError::Closed),
            k if k == OutcomeKind::Errno as u64 => Err(HostError::Errno(self.value as i64 as i32)),
            _ => Err(HostError::NotSupported),
        }
    }

    fn unit(self) -> Result<(), HostError> {
        self.result().map(drop)
    }
}

/// The kernel's [`Host`] whose services are the monitor's, through a link, which one call holds at a time.
pub struct LinkHost<R> {
    pub ring: Mutex<R>,
}

impl<R> LinkHost<R> {
    pub fn new(ring: R) -> Self {
        Self {
            ring: Mutex::new(ring),
        }
    }
}

impl<R: Ring + Send> Host for LinkHost<R> {
    fn monotonic_nanoseconds(&self) -> u64 {
        self.ring.lock().ask(&MonotonicNanoseconds {}, &[]).value
    }

    fn realtime_nanoseconds(&self) -> u64 {
        self.ring.lock().ask(&RealtimeNanoseconds {}, &[]).value
    }

    fn random(&self, into: &mut [u8]) {
        let mut ring = self.ring.lock();
        ring.ask(
            &Random {
                length: into.len() as u64,
            },
            &[],
        );
        ring.take(into);
    }

    fn warn(&self, warning: Warning<'_>) {
        self.ring.lock().ask(
            &Warn {
                count: warning.count,
                detail: warning.detail,
            },
            warning.reason.as_bytes(),
        );
    }

    fn send_stream(&self, end: End, bytes: &[u8]) {
        self.ring.lock().ask(
            &Output {
                stream: end.0.into(),
            },
            bytes,
        );
    }

    fn credit_stream(&self, end: End, bytes: u64) {
        let message = CreditStream {
            stream: end.0.into(),
            bytes,
        };
        self.ring.lock().ask(&message, &[]);
    }

    fn close_stream(&self, end: End) {
        self.ring.lock().ask(
            &CloseStream {
                stream: end.0.into(),
            },
            &[],
        );
    }

    fn connect(
        &self,
        connection: ConnectionId,
        address: &[u8],
        port: u16,
    ) -> Result<(), HostError> {
        let message = Connect {
            connection: connection.0.into(),
            port: port.into(),
        };
        self.ring.lock().ask(&message, address).unit()
    }

    fn send(&self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError> {
        let message = SendBytes {
            connection: connection.0.into(),
        };
        self.ring.lock().ask(&message, bytes).result()
    }

    fn receive(&self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError> {
        let message = Receive {
            connection: connection.0.into(),
            length: into.len() as u64,
        };
        let mut ring = self.ring.lock();
        let received = ring.ask(&message, &[]).result()?;
        let length = received.min(into.len());
        ring.take(&mut into[..length]);
        Ok(received)
    }

    fn close(&self, connection: ConnectionId) {
        self.ring.lock().ask(
            &Close {
                connection: connection.0.into(),
            },
            &[],
        );
    }
}

/// A connection's number as the link carries it.
fn connection(word: u64) -> ConnectionId {
    ConnectionId(word as u32)
}

/// The monitor's half: answers `request`, whose whole bytes are `bytes`, by calling `host`; gives the answer's
/// bytes.
pub fn answer(request: Request, link: &mut Link, bytes: &[u8], host: &dyn Host) -> Vec<u8> {
    let mut answer = Vec::new();
    let outcome = match request {
        Request::MonotonicNanoseconds => {
            link.put(&Value {
                value: host.monotonic_nanoseconds(),
            });
            None
        }
        Request::RealtimeNanoseconds => {
            link.put(&Value {
                value: host.realtime_nanoseconds(),
            });
            None
        }
        Request::Random => {
            let Random { length } = link.get();
            answer.resize(length as usize, 0);
            host.random(&mut answer);
            None
        }
        Request::Warn => {
            let Warn { count, detail } = link.get();
            let reason = core::str::from_utf8(bytes).expect("a warning's reason is text");
            host.warn(Warning {
                reason,
                count,
                detail,
            });
            None
        }
        Request::Output => {
            let Output { stream } = link.get();
            host.send_stream(End(stream as u32), bytes);
            None
        }
        Request::CreditStream => {
            let CreditStream { stream, bytes } = link.get();
            host.credit_stream(End(stream as u32), bytes);
            None
        }
        Request::CloseStream => {
            let CloseStream { stream } = link.get();
            host.close_stream(End(stream as u32));
            None
        }
        Request::Connect => {
            let Connect {
                connection: id,
                port,
            } = link.get();
            let result = host.connect(connection(id), bytes, port as u16);
            Some(result.map(|()| 0))
        }
        Request::SendBytes => {
            let SendBytes { connection: id } = link.get();
            Some(host.send(connection(id), bytes))
        }
        Request::Receive => {
            let Receive {
                connection: id,
                length,
            } = link.get();
            answer.resize(length as usize, 0);
            let result = host.receive(connection(id), &mut answer);
            answer.truncate(*result.as_ref().unwrap_or(&0));
            Some(result)
        }
        Request::Close => {
            let Close { connection: id } = link.get();
            host.close(connection(id));
            None
        }
    };
    if let Some(result) = outcome {
        link.put(&Outcome::of(result));
    }
    answer
}

#[cfg(test)]
mod tests {
    extern crate std;

    use alloc::boxed::Box;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    use std::sync::Mutex;

    use wcontainers_abi::{ConnectionId, End, Host, HostError, Warning};
    use zerocopy::FromZeros;

    use super::{LinkHost, Request};
    use crate::link::{LINK_BYTES, Link, Parts, Ring};

    /// A host that records what it is given and answers from what it holds.
    #[derive(Default)]
    struct Recording {
        calls: Mutex<Vec<(String, Vec<u8>)>>,
        error: Option<HostError>,
        /// What a receive or a random request gives.
        given: Vec<u8>,
    }

    impl Recording {
        fn record(&self, call: &str, bytes: &[u8]) {
            self.calls
                .lock()
                .unwrap()
                .push((call.into(), bytes.to_vec()));
        }

        fn outcome(&self, count: usize) -> Result<usize, HostError> {
            self.error.map_or(Ok(count), Err)
        }
    }

    /// Each call's name as recorded.
    const MONOTONIC: &str = "monotonic";
    const REALTIME: &str = "realtime";
    const RANDOM: &str = "random";
    const WARN: &str = "warn";
    const OUTPUT: &str = "output";
    const CREDIT: &str = "credit";
    const CLOSE_STREAM: &str = "close stream";
    const CONNECT: &str = "connect";
    const SEND: &str = "send";
    const RECEIVE: &str = "receive";
    const CLOSE: &str = "close";

    /// The clocks' readings.
    const MONOTONIC_NANOSECONDS: u64 = 123_456_789;
    const REALTIME_NANOSECONDS: u64 = 1_700_000_000_000_000_000;

    impl Host for Recording {
        fn monotonic_nanoseconds(&self) -> u64 {
            self.record(MONOTONIC, &[]);
            MONOTONIC_NANOSECONDS
        }
        fn realtime_nanoseconds(&self) -> u64 {
            self.record(REALTIME, &[]);
            REALTIME_NANOSECONDS
        }
        fn random(&self, into: &mut [u8]) {
            self.record(RANDOM, &[]);
            into.copy_from_slice(&self.given[..into.len()]);
        }
        fn warn(&self, warning: Warning<'_>) {
            let mut bytes = warning.reason.as_bytes().to_vec();
            bytes.extend_from_slice(&warning.count.to_le_bytes());
            bytes.extend_from_slice(&warning.detail.to_le_bytes());
            self.record(WARN, &bytes);
        }
        fn send_stream(&self, end: End, bytes: &[u8]) {
            let mut all = end.0.to_le_bytes().to_vec();
            all.extend_from_slice(bytes);
            self.record(OUTPUT, &all);
        }
        fn credit_stream(&self, end: End, bytes: u64) {
            let mut all = end.0.to_le_bytes().to_vec();
            all.extend_from_slice(&bytes.to_le_bytes());
            self.record(CREDIT, &all);
        }
        fn close_stream(&self, end: End) {
            self.record(CLOSE_STREAM, &end.0.to_le_bytes());
        }
        fn connect(
            &self,
            connection: ConnectionId,
            address: &[u8],
            port: u16,
        ) -> Result<(), HostError> {
            let mut bytes = connection.0.to_le_bytes().to_vec();
            bytes.extend_from_slice(&port.to_le_bytes());
            bytes.extend_from_slice(address);
            self.record(CONNECT, &bytes);
            self.outcome(0).map(drop)
        }
        fn send(&self, connection: ConnectionId, bytes: &[u8]) -> Result<usize, HostError> {
            let mut all = connection.0.to_le_bytes().to_vec();
            all.extend_from_slice(bytes);
            self.record(SEND, &all);
            self.outcome(bytes.len())
        }
        fn receive(&self, connection: ConnectionId, into: &mut [u8]) -> Result<usize, HostError> {
            self.record(RECEIVE, &connection.0.to_le_bytes());
            let count = self.given.len().min(into.len());
            into[..count].copy_from_slice(&self.given[..count]);
            self.outcome(count)
        }
        fn close(&self, connection: ConnectionId) {
            self.record(CLOSE, &connection.0.to_le_bytes());
        }
    }

    /// The link and the monitor's half in one place: ringing answers at once.
    struct Loopback {
        link: Box<Link>,
        parts: Parts,
        host: Recording,
        rung: usize,
    }

    impl Ring for Loopback {
        fn link(&mut self) -> &mut Link {
            &mut self.link
        }
        fn ring(&mut self) {
            self.rung += 1;
            let Some(bytes) = self.parts.request(&mut self.link) else {
                return;
            };
            let request =
                Request::from_word(self.link.head.request).expect("a request of the host's set");
            let answer = super::answer(request, &mut self.link, &bytes, &self.host);
            self.parts.answer(&mut self.link, answer);
        }
    }

    fn host() -> LinkHost<Loopback> {
        LinkHost::new(Loopback {
            link: Box::new(Link::new_zeroed()),
            parts: Parts::default(),
            host: Recording::default(),
            rung: 0,
        })
    }

    /// Bytes no single link holds, each different from its neighbours.
    fn long(length: usize) -> Vec<u8> {
        (0..length)
            .map(|i| i as u8 ^ (i / LINK_BYTES) as u8)
            .collect()
    }

    /// An end the kernel sends on.
    const ERROR_END: End = End(2);

    /// Lengths around the link's size.
    const LENGTHS: [usize; 6] = [
        0,
        1,
        LINK_BYTES - 1,
        LINK_BYTES,
        LINK_BYTES + 1,
        3 * LINK_BYTES + 5,
    ];

    fn last(host: &LinkHost<Loopback>) -> (String, Vec<u8>) {
        let ring = host.ring.lock();
        let calls = ring.host.calls.lock().unwrap();
        calls.last().expect("a call").clone()
    }

    #[test]
    fn every_call_arrives_whole() {
        let host = host();
        assert_eq!(host.monotonic_nanoseconds(), MONOTONIC_NANOSECONDS);
        assert_eq!(host.realtime_nanoseconds(), REALTIME_NANOSECONDS);
        for length in LENGTHS {
            let bytes = long(length);
            let rung = host.ring.lock().rung;
            host.send_stream(ERROR_END, &bytes);
            assert_eq!(
                host.ring.lock().rung - rung,
                length.div_ceil(LINK_BYTES).max(1),
                "one request a link's worth"
            );
            let mut expected = ERROR_END.0.to_le_bytes().to_vec();
            expected.extend_from_slice(&bytes);
            assert_eq!(last(&host), (OUTPUT.into(), expected), "output of {length}");
            let id = ConnectionId(length as u32);
            assert_eq!(host.send(id, &bytes), Ok(length));
            let mut expected = id.0.to_le_bytes().to_vec();
            expected.extend_from_slice(&bytes);
            assert_eq!(last(&host), (SEND.into(), expected));

            let given: Vec<u8> = long(length).into_iter().rev().collect();
            host.ring.lock().host.given = given.clone();
            let mut into = vec![0; length];
            host.random(&mut into);
            assert_eq!(into, given, "random of {length}");
            let mut into = vec![0; length + 1];
            assert_eq!(host.receive(id, &mut into), Ok(length));
            assert_eq!(into[..length], given[..], "received {length}");
            assert_eq!(into[length], 0);
        }
        host.warn(Warning {
            reason: "a reason",
            count: 7,
            detail: 0x40_1000,
        });
        let mut expected = b"a reason".to_vec();
        expected.extend_from_slice(&7u64.to_le_bytes());
        expected.extend_from_slice(&0x40_1000u64.to_le_bytes());
        assert_eq!(last(&host), (WARN.into(), expected));
        let address = [10, 0, 0, 1];
        assert_eq!(host.connect(ConnectionId(3), &address, 8080), Ok(()));
        let mut expected = 3u32.to_le_bytes().to_vec();
        expected.extend_from_slice(&8080u16.to_le_bytes());
        expected.extend_from_slice(&address);
        assert_eq!(last(&host), (CONNECT.into(), expected));
        host.close(ConnectionId(6));
        assert_eq!(last(&host), (CLOSE.into(), 6u32.to_le_bytes().to_vec()));
        host.credit_stream(ERROR_END, wcontainers_abi::PAGE_SIZE);
        let mut expected = ERROR_END.0.to_le_bytes().to_vec();
        expected.extend_from_slice(&wcontainers_abi::PAGE_SIZE.to_le_bytes());
        assert_eq!(last(&host), (CREDIT.into(), expected));
        host.close_stream(ERROR_END);
        assert_eq!(
            last(&host),
            (CLOSE_STREAM.into(), ERROR_END.0.to_le_bytes().to_vec())
        );
    }

    #[test]
    fn every_error_comes_back() {
        let errors = [
            HostError::NotSupported,
            HostError::WouldBlock,
            HostError::Closed,
            HostError::Errno(i32::MIN),
            HostError::Errno(i32::MAX),
        ];
        let host = host();
        for error in errors {
            host.ring.lock().host.error = Some(error);
            let id = ConnectionId(1);
            assert_eq!(host.send(id, b"x"), Err(error));
            assert_eq!(host.receive(id, &mut [0; 4]), Err(error));
            assert_eq!(host.connect(id, &[0; 16], 1), Err(error));
        }
    }
}
