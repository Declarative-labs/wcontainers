//! The watcher's held facts, which the host's poll does not see.
#![cfg(unix)]

use std::net::UdpSocket;
use std::os::fd::OwnedFd;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use wcontainers_internet_protocol_linux_socket_options::readiness::{NOTHING, Watcher};
use wcontainers_readiness::{ReadinessReport, ReadyFor, UnconditionalEvents};

#[derive(Default)]
struct Wakes(AtomicUsize);

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Checks that a call already waiting when an error is held is woken once, and then finishes with the error.
#[test]
fn a_held_error_wakes_a_waiting_call() {
    let watcher = Watcher::new().expect("the watcher starts");
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket binds");
    let watched = watcher
        .watch(OwnedFd::from(socket), ReadyFor::all())
        .expect("the socket is watched");
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(wakes.clone());
    let mut context = Context::from_waker(&waker);
    let mut wait = pin!(watched.wait_until_ready(ReadyFor::READABLE));
    assert!(wait.as_mut().poll(&mut context).is_pending());
    watched.hold(ReadinessReport {
        unconditional: UnconditionalEvents::ERROR,
        ..NOTHING
    });
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    match wait.as_mut().poll(&mut context) {
        Poll::Ready(Ok(report)) => assert_eq!(
            report.unconditional,
            UnconditionalEvents::ERROR,
            "{report:?}"
        ),
        other => panic!("the wait finishes with the error, not {other:?}"),
    }
    watched.hold(NOTHING);
    let wait = pin!(watched.wait_until_ready(ReadyFor::READABLE));
    assert!(wait.poll(&mut context).is_pending());
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
}
