//! The kernel's [`Host`] and standard streams through the link: each call is a request the monitor answers by
//! calling the host's services.

use alloc::vec::Vec;

use spin::Mutex;
use wcontainers_abi::{Host, HostError, Warning};
use wcontainers_clock::Clock;
use wcontainers_random_source::RandomSource;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_system::StreamTransport;
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
    /// The bytes the kernel sends on the endpoint numbered `stream`, all of which the monitor takes.
    Output { stream } -> ();
    /// The kernel let go of the endpoint numbered `stream`.
    CloseStream { stream } -> ();
}

/// One number as an answer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct Value {
    pub value: u64,
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

impl<R: Ring + Send> Clock for LinkHost<R> {
    fn monotonic_nanoseconds(&self) -> u64 {
        self.ring.lock().ask(&MonotonicNanoseconds {}, &[]).value
    }

    fn realtime_nanoseconds(&self) -> u64 {
        self.ring.lock().ask(&RealtimeNanoseconds {}, &[]).value
    }
}

impl<R: Ring + Send> RandomSource for LinkHost<R> {
    fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError> {
        let mut ring = self.ring.lock();
        ring.ask(
            &Random {
                length: into.len() as u64,
            },
            &[],
        );
        ring.take(into);
        Ok(())
    }
}

impl<R: Ring + Send> Host for LinkHost<R> {
    fn warn(&self, warning: Warning<'_>) {
        self.ring.lock().ask(
            &Warn {
                count: warning.count,
                detail: warning.detail,
            },
            warning.reason.as_bytes(),
        );
    }
}

/// The standard streams: the monitor takes all the kernel sends, and gives no input.
impl<R: Ring + Send> StreamTransport for LinkHost<R> {
    type Endpoint = u32;

    fn send_stream(&self, peer: &u32, bytes: &[u8]) -> Result<usize, HostError> {
        self.ring.lock().ask(
            &Output {
                stream: (*peer).into(),
            },
            bytes,
        );
        Ok(bytes.len())
    }

    fn receive_stream(&self, _peer: &u32, _into: &mut [u8]) -> Result<usize, HostError> {
        Err(HostError::NotSupported)
    }

    fn close_stream(&self, peer: &u32) {
        self.ring.lock().ask(
            &CloseStream {
                stream: (*peer).into(),
            },
            &[],
        );
    }
}

/// Every endpoint can always take more and the monitor never lets go of one, so a wait for anything else never
/// ends.
impl<R: Ring + Send> Readiness<u32> for LinkHost<R> {
    async fn wait_until_ready(
        &self,
        _peer: &u32,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        if interest.contains(ReadyFor::WRITABLE) {
            return Ok(ReadinessReport {
                ready_for: ReadyFor::WRITABLE,
                unconditional: UnconditionalEvents::empty(),
            });
        }
        core::future::pending().await
    }
}

/// The monitor's half: answers `request`, whose whole bytes are `bytes`, by calling `host`; gives the answer's
/// bytes.
pub fn answer<H: Host + Clock + RandomSource + StreamTransport<Endpoint = u32>>(
    request: Request,
    link: &mut Link,
    bytes: &[u8],
    host: &H,
) -> Vec<u8> {
    let mut answer = Vec::new();
    match request {
        Request::MonotonicNanoseconds => {
            link.put(&Value {
                value: host.monotonic_nanoseconds(),
            });
        }
        Request::RealtimeNanoseconds => {
            link.put(&Value {
                value: host.realtime_nanoseconds(),
            });
        }
        Request::Random => {
            let Random { length } = link.get();
            answer.resize(length as usize, 0);
            // The link has no error for it: a monitor without randomness cannot run a kernel.
            host.fill_random_bytes(&mut answer)
                .expect("the monitor's random source works");
        }
        Request::Warn => {
            let Warn { count, detail } = link.get();
            let reason = core::str::from_utf8(bytes).expect("a warning's reason is text");
            host.warn(Warning {
                reason,
                count,
                detail,
            });
        }
        Request::Output => {
            let Output { stream } = link.get();
            let _ = host.send_stream(&(stream as u32), bytes);
        }
        Request::CloseStream => {
            let CloseStream { stream } = link.get();
            host.close_stream(&(stream as u32));
        }
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

    use wcontainers_abi::{Host, HostError, Warning};
    use wcontainers_clock::Clock;
    use wcontainers_random_source::RandomSource;
    use wcontainers_system::StreamTransport;
    use zerocopy::FromZeros;

    use super::{LinkHost, Request};
    use crate::link::{LINK_BYTES, Link, Parts, Ring};

    /// A host that records what it is given and answers from what it holds.
    #[derive(Default)]
    struct Recording {
        calls: Mutex<Vec<(String, Vec<u8>)>>,
        /// What a random request gives.
        given: Vec<u8>,
    }

    impl Recording {
        fn record(&self, call: &str, bytes: &[u8]) {
            self.calls
                .lock()
                .unwrap()
                .push((call.into(), bytes.to_vec()));
        }
    }

    /// Each call's name as recorded.
    const MONOTONIC: &str = "monotonic";
    const REALTIME: &str = "realtime";
    const RANDOM: &str = "random";
    const WARN: &str = "warn";
    const OUTPUT: &str = "output";
    const CLOSE_STREAM: &str = "close stream";

    /// The clocks' readings.
    const MONOTONIC_NANOSECONDS: u64 = 123_456_789;
    const REALTIME_NANOSECONDS: u64 = 1_700_000_000_000_000_000;

    impl Clock for Recording {
        fn monotonic_nanoseconds(&self) -> u64 {
            self.record(MONOTONIC, &[]);
            MONOTONIC_NANOSECONDS
        }
        fn realtime_nanoseconds(&self) -> u64 {
            self.record(REALTIME, &[]);
            REALTIME_NANOSECONDS
        }
    }

    impl RandomSource for Recording {
        fn fill_random_bytes(&self, into: &mut [u8]) -> Result<(), HostError> {
            self.record(RANDOM, &[]);
            into.copy_from_slice(&self.given[..into.len()]);
            Ok(())
        }
    }

    impl Host for Recording {
        fn warn(&self, warning: Warning<'_>) {
            let mut bytes = warning.reason.as_bytes().to_vec();
            bytes.extend_from_slice(&warning.count.to_le_bytes());
            bytes.extend_from_slice(&warning.detail.to_le_bytes());
            self.record(WARN, &bytes);
        }
    }

    impl StreamTransport for Recording {
        type Endpoint = u32;

        fn send_stream(&self, peer: &u32, bytes: &[u8]) -> Result<usize, HostError> {
            let mut all = peer.to_le_bytes().to_vec();
            all.extend_from_slice(bytes);
            self.record(OUTPUT, &all);
            Ok(bytes.len())
        }
        fn receive_stream(&self, _peer: &u32, _into: &mut [u8]) -> Result<usize, HostError> {
            Err(HostError::NotSupported)
        }
        fn close_stream(&self, peer: &u32) {
            self.record(CLOSE_STREAM, &peer.to_le_bytes());
        }
    }

    impl wcontainers_readiness::Readiness<u32> for Recording {
        async fn wait_until_ready(
            &self,
            _peer: &u32,
            _interest: wcontainers_readiness::ReadyFor,
        ) -> Result<wcontainers_readiness::ReadinessReport, HostError> {
            Err(HostError::NotSupported)
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

    /// An endpoint the kernel sends on.
    const ERROR_END: u32 = 2;

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
            assert_eq!(host.send_stream(&ERROR_END, &bytes), Ok(length));
            assert_eq!(
                host.ring.lock().rung - rung,
                length.div_ceil(LINK_BYTES).max(1),
                "one request a link's worth"
            );
            let mut expected = ERROR_END.to_le_bytes().to_vec();
            expected.extend_from_slice(&bytes);
            assert_eq!(last(&host), (OUTPUT.into(), expected), "output of {length}");

            let given: Vec<u8> = long(length).into_iter().rev().collect();
            host.ring.lock().host.given = given.clone();
            let mut into = vec![0; length];
            assert_eq!(host.fill_random_bytes(&mut into), Ok(()));
            assert_eq!(into, given, "random of {length}");
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
        host.close_stream(&ERROR_END);
        assert_eq!(
            last(&host),
            (CLOSE_STREAM.into(), ERROR_END.to_le_bytes().to_vec())
        );
    }
}
