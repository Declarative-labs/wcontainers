//! The readiness's held events, which the host's poll does not see.
#![cfg(unix)]

use std::net::UdpSocket;
use std::os::fd::OwnedFd;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use wcontainers_internet_protocol_linux_socket_options::readiness::{NO_EVENTS, Readiness};
use wcontainers_kernel::signal_information::PollEvents;

#[derive(Default)]
struct Wakes(AtomicUsize);

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

const READABLE: PollEvents = PollEvents {
    data_to_read: true,
    ..NO_EVENTS
};

const ERROR: PollEvents = PollEvents {
    error_condition: true,
    ..NO_EVENTS
};

/// A call already waiting when an error is held is woken once, and then finishes with the error.
#[test]
fn a_held_error_wakes_a_waiting_call() {
    let readiness = Readiness::new().expect("the readiness starts");
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket binds");
    let watched = readiness
        .watch(OwnedFd::from(socket))
        .expect("the socket is watched");
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(wakes.clone());
    let mut context = Context::from_waker(&waker);
    let mut wait = pin!(watched.wait_for_events(READABLE));
    assert!(wait.as_mut().poll(&mut context).is_pending());
    watched.hold(ERROR);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    match wait.as_mut().poll(&mut context) {
        Poll::Ready(Ok(events)) => assert!(events.error_condition, "{events:?}"),
        other => panic!("the wait finishes with the error, not {other:?}"),
    }
    watched.hold(NO_EVENTS);
    let wait = pin!(watched.wait_for_events(READABLE));
    assert!(wait.poll(&mut context).is_pending());
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
}
