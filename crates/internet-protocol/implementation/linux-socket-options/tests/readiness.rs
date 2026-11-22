//! The watcher's held facts, which the host's poll does not see.
#![cfg(unix)]

use std::net::UdpSocket;
use std::os::fd::OwnedFd;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use wcontainers_abi::readiness::Readiness;
use wcontainers_internet_protocol_linux_socket_options::readiness::Watcher;

#[derive(Default)]
struct Wakes(AtomicUsize);

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// A call already waiting when an error is held is woken once, and then finishes with the error.
#[test]
fn a_held_error_wakes_a_waiting_call() {
    let watcher = Watcher::new().expect("the watcher starts");
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a socket binds");
    let watched = watcher
        .watch(OwnedFd::from(socket))
        .expect("the socket is watched");
    let wakes = Arc::new(Wakes::default());
    let waker = Waker::from(wakes.clone());
    let mut context = Context::from_waker(&waker);
    let mut wait = pin!(watched.wait_for_events(Readiness::READABLE));
    assert!(wait.as_mut().poll(&mut context).is_pending());
    watched.hold(Readiness::ERROR);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    match wait.as_mut().poll(&mut context) {
        Poll::Ready(Ok(events)) => assert!(events.contains(Readiness::ERROR), "{events:?}"),
        other => panic!("the wait finishes with the error, not {other:?}"),
    }
    watched.hold(Readiness::empty());
    let wait = pin!(watched.wait_for_events(Readiness::READABLE));
    assert!(wait.poll(&mut context).is_pending());
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
}
