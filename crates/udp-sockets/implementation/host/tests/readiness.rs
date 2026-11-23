//! What waiting costs the host implementation's watcher thread.
#![cfg(target_os = "linux")]

mod support;

use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use wcontainers_abi::HostError;
use wcontainers_internet_protocol_linux_socket_options::readiness::WATCHER_THREAD;
use wcontainers_readiness::{Readiness, ReadyFor};
use wcontainers_udp_sockets::UdpSockets;
use wcontainers_udp_sockets_host::HostUdpSockets;

use support::{PAYLOAD, Wakes, pair, poll_with, wait_and_take};

/// Taken by each test, so that one counts only its own wakes.
static ALONE: Mutex<()> = Mutex::new(());

/// How many datagrams each measurement sends.
const DATAGRAMS: usize = 1200;

/// How long the watcher is given to show a wake it was sent.
const SETTLE: Duration = Duration::from_millis(50);

/// How long a wait is left waiting for an event its socket lacks.
const LACKING: Duration = Duration::from_millis(300);

/// The most processor time the watcher may take over that time: a few looks, far from running throughout.
const LOOKING: Duration = Duration::from_millis(15);

/// The folders under /proc of this process's watcher threads.
fn watchers() -> impl Iterator<Item = std::path::PathBuf> {
    std::fs::read_dir("/proc/self/task")
        .expect("this process's threads")
        .map(|task| task.expect("a thread").path())
        .filter(|task| {
            std::fs::read_to_string(task.join("comm"))
                .is_ok_and(|name| name.trim_end() == WATCHER_THREAD)
        })
}

/// The voluntary context switches of this process's watcher threads, which count their wakes.
fn watcher_wakes() -> u64 {
    watchers()
        .map(|task| {
            let status = std::fs::read_to_string(task.join("status")).expect("its status");
            status
                .lines()
                .find_map(|line| line.strip_prefix("voluntary_ctxt_switches:"))
                .and_then(|count| count.trim().parse::<u64>().ok())
                .expect("its context switches")
        })
        .sum()
}

/// The processor time this process's watcher threads have run, the first count of their `schedstat`.
fn watcher_time() -> Duration {
    watchers()
        .map(|task| {
            let schedstat = std::fs::read_to_string(task.join("schedstat")).expect("its schedstat");
            schedstat
                .split_whitespace()
                .next()
                .and_then(|nanoseconds| nanoseconds.parse::<u64>().ok())
                .map(Duration::from_nanos)
                .expect("its time on a processor")
        })
        .sum()
}

/// Datagrams sent and received with no call waiting do not wake the watcher; one waited for does.
#[test]
fn a_socket_nobody_waits_on_costs_the_watcher_nothing() {
    let _alone = ALONE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let sockets = HostUdpSockets::new().expect("make the host's UDP sockets");
    let (socket, sender) = pair(&sockets);
    let mut into = vec![u8::MIN; usize::from(u16::MAX)];
    std::thread::sleep(SETTLE);
    let before = watcher_wakes();
    for _ in 0..DATAGRAMS {
        sender.send(PAYLOAD).expect("send");
        while sockets.receive(&socket, &mut into) == Err(HostError::WouldBlock) {
            std::thread::yield_now();
        }
    }
    std::thread::sleep(SETTLE);
    let unwaited = watcher_wakes() - before;
    assert_eq!(
        unwaited, 0,
        "watcher wakes over {DATAGRAMS} datagrams nobody waited for"
    );
    // After waits, the socket costs nothing again.
    let wakes = Arc::new(Wakes::default());
    let before = watcher_wakes();
    wait_and_take(&sockets, &socket, &sender, &wakes, 1, &mut into);
    std::thread::sleep(SETTLE);
    assert!(watcher_wakes() > before, "the watcher woke for a wait");
    let before = watcher_wakes();
    for _ in 0..DATAGRAMS {
        sender.send(PAYLOAD).expect("send");
        while sockets.receive(&socket, &mut into) == Err(HostError::WouldBlock) {
            std::thread::yield_now();
        }
    }
    std::thread::sleep(SETTLE);
    assert_eq!(
        watcher_wakes() - before,
        0,
        "watcher wakes after a wait ended"
    );
}

/// A wait for facts UDP never has asks the host for nothing but errors and hang-ups, so it does not keep the watcher
/// running while the socket has data.
#[test]
fn a_wait_for_an_event_the_socket_lacks_keeps_the_watcher_still() {
    let _alone = ALONE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let sockets = HostUdpSockets::new().expect("make the host's UDP sockets");
    let (socket, sender) = pair(&sockets);
    let wakes = Arc::new(Wakes::default());
    let mut readable = pin!(sockets.wait_until_ready(&socket, ReadyFor::READABLE));
    assert!(poll_with(readable.as_mut(), &wakes).is_pending());
    sender.send(PAYLOAD).expect("send");
    assert!(wakes.reach(1), "woken by the datagram");
    let lacking = ReadyFor::URGENT_DATA | ReadyFor::PEER_FINISHED_SENDING;
    let before = watcher_time();
    let mut wait = pin!(sockets.wait_until_ready(&socket, lacking));
    assert!(poll_with(wait.as_mut(), &wakes).is_pending());
    std::thread::sleep(LACKING);
    let taken = watcher_time() - before;
    assert!(taken < LOOKING, "the watcher ran {taken:?} of {LACKING:?}");
    assert!(poll_with(wait.as_mut(), &wakes).is_pending());
    assert_eq!(*wakes.count.lock().expect("the wakes"), 1);
}

/// A wait that was woken and is not yet polled again costs the watcher nothing for the datagrams after its own.
#[test]
fn a_woken_wait_costs_the_watcher_nothing_until_it_is_polled() {
    let _alone = ALONE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let sockets = HostUdpSockets::new().expect("make the host's UDP sockets");
    let (socket, sender) = pair(&sockets);
    let wakes = Arc::new(Wakes::default());
    let mut wait = pin!(sockets.wait_until_ready(&socket, ReadyFor::READABLE));
    assert!(poll_with(wait.as_mut(), &wakes).is_pending());
    sender.send(PAYLOAD).expect("send");
    assert!(wakes.reach(1), "woken by the datagram");
    std::thread::sleep(SETTLE);
    let before = watcher_wakes();
    for _ in 0..DATAGRAMS {
        sender.send(PAYLOAD).expect("send");
    }
    std::thread::sleep(SETTLE);
    assert_eq!(
        watcher_wakes() - before,
        0,
        "watcher wakes over {DATAGRAMS} datagrams after the wait was woken"
    );
    assert!(matches!(
        poll_with(wait, &wakes),
        Poll::Ready(Ok(events)) if events.ready_for.contains(ReadyFor::READABLE)
    ));
}
