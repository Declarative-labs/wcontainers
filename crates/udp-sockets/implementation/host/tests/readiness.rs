//! What waiting costs the host implementation: its watcher thread's wakes and the allocations of a wait.
#![cfg(target_os = "linux")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{BindAddress, IpVersion, LocalBinding, LocalPort, Port};
use wcontainers_internet_protocol_linux_socket_options::readiness::WATCHER_THREAD;
use wcontainers_kernel::UserId;
use wcontainers_readiness::{Readiness, ReadyFor};
use wcontainers_udp_sockets::{UdpProtocol, UdpSockets};
use wcontainers_udp_sockets_host::{HostUdpSocket, HostUdpSockets};

/// An allocator that counts the allocations of the whole test process.
struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// The allocations of this thread alone, which the test harness's other threads do not add to.
    static ALLOCATIONS_HERE: Cell<usize> = const { Cell::new(0) };
}

fn count_allocation() {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    // A thread being torn down has no count left, and none is wanted of it.
    let _ = ALLOCATIONS_HERE.try_with(|count| count.set(count.get() + 1));
}

// SAFETY: every call is passed on to the system's allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        // SAFETY: as the caller of alloc promised.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: as the caller of dealloc promised.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        // SAFETY: as the caller of realloc promised.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Taken by each test, so that one counts only its own wakes and allocations.
static ALONE: Mutex<()> = Mutex::new(());

/// How many datagrams each measurement sends.
const DATAGRAMS: usize = 1200;

/// How many waits warm a socket's waiter list before allocations are counted.
const WARM_UP: usize = 10;

/// How long one wake may take to come.
const PATIENCE: Duration = Duration::from_secs(1);

/// How long the watcher is given to show a wake it was sent.
const SETTLE: Duration = Duration::from_millis(50);

/// How long a wait is left waiting for an event its socket lacks.
const LACKING: Duration = Duration::from_millis(300);

/// The most processor time the watcher may take over that time: a few looks, far from running throughout.
const LOOKING: Duration = Duration::from_millis(15);

const PAYLOAD: &[u8] = b"a datagram waited for";

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
        *self.count.lock().expect("the wakes") += 1;
        self.changed.notify_all();
    }
}

impl Wakes {
    fn reach(&self, count: usize) -> bool {
        let wakes = self.count.lock().expect("the wakes");
        let (wakes, _) = self
            .changed
            .wait_timeout_while(wakes, PATIENCE, |wakes| *wakes < count)
            .expect("the wakes");
        *wakes >= count
    }
}

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

/// A host socket on loopback with a plain socket that sends to it.
fn pair(sockets: &HostUdpSockets) -> (HostUdpSocket, UdpSocket) {
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, UserId(u32::MIN))
        .expect("open");
    let address = sockets
        .bind(
            &socket,
            LocalBinding {
                address: BindAddress::V4(Ipv4Addr::LOCALHOST),
                port: LocalPort::ChosenByKernel(Port(u16::MIN)),
            },
        )
        .expect("bind");
    let sender = UdpSocket::bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), u16::MIN))
        .expect("a plain sender");
    sender.connect(address).expect("connect the sender");
    (socket, sender)
}

fn poll_with<F: Future>(call: core::pin::Pin<&mut F>, wakes: &Arc<Wakes>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::clone(wakes));
    call.poll(&mut Context::from_waker(&waker))
}

/// One datagram waited for, then taken.
fn wait_and_take(
    sockets: &HostUdpSockets,
    socket: &HostUdpSocket,
    sender: &UdpSocket,
    wakes: &Arc<Wakes>,
    woken: usize,
    into: &mut [u8],
) {
    let mut wait = pin!(sockets.wait_until_ready(socket, ReadyFor::READABLE));
    assert!(poll_with(wait.as_mut(), wakes).is_pending());
    sender.send(PAYLOAD).expect("send");
    assert!(wakes.reach(woken), "woken by the datagram");
    match poll_with(wait.as_mut(), wakes) {
        Poll::Ready(Ok(events)) => assert!(events.contains(ReadyFor::READABLE)),
        other => panic!("the wait did not finish: {other:?}"),
    }
    assert!(sockets.receive(socket, into).is_ok());
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

/// A call that has to wait allocates nothing once its socket has been waited on.
#[test]
fn a_wait_allocates_nothing_once_warm() {
    let _alone = ALONE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let sockets = HostUdpSockets::new().expect("make the host's UDP sockets");
    let (socket, sender) = pair(&sockets);
    let mut into = vec![u8::MIN; usize::from(u16::MAX)];
    let wakes = Arc::new(Wakes::default());
    for woken in 1..=WARM_UP {
        wait_and_take(&sockets, &socket, &sender, &wakes, woken, &mut into);
    }
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for woken in WARM_UP + 1..=WARM_UP + DATAGRAMS {
        wait_and_take(&sockets, &socket, &sender, &wakes, woken, &mut into);
    }
    let allocated = ALLOCATIONS.load(Ordering::Relaxed) - before;
    assert_eq!(allocated, 0, "allocations over {DATAGRAMS} waits");
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

/// A receive with nothing queued allocates nothing.
#[test]
fn a_receive_that_would_block_allocates_nothing() {
    let _alone = ALONE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let sockets = HostUdpSockets::new().expect("make the host's UDP sockets");
    let (socket, _sender) = pair(&sockets);
    let mut into = vec![u8::MIN; usize::from(u16::MAX)];
    assert_eq!(
        sockets.receive(&socket, &mut into),
        Err(HostError::WouldBlock)
    );
    let before = ALLOCATIONS_HERE.with(Cell::get);
    for _ in 0..DATAGRAMS {
        assert_eq!(
            sockets.receive(&socket, &mut into),
            Err(HostError::WouldBlock)
        );
    }
    let allocated = ALLOCATIONS_HERE.with(Cell::get) - before;
    assert_eq!(
        allocated, 0,
        "allocations over {DATAGRAMS} receives that would block"
    );
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
        Poll::Ready(Ok(events)) if events.contains(ReadyFor::READABLE)
    ));
}
