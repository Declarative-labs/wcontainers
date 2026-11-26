//! A host socket with a plain sender on loopback, and waits on it that count their wakes.

use core::cell::RefCell;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::pin::{Pin, pin};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use wcontainers_internet_protocol::{BindAddress, IpVersion, LocalBinding, LocalPort, Port};
use wcontainers_kernel::UserId;
use wcontainers_readiness::{Readiness, ReadyFor};
use wcontainers_udp_sockets::{UdpProtocol, UdpSockets};
use wcontainers_udp_sockets_host::{HostUdpSocket, HostUdpSockets};

/// How long one wake may take to come.
pub const PATIENCE: Duration = Duration::from_secs(1);

pub const PAYLOAD: &[u8] = b"a datagram waited for";

/// How many datagrams each check sends, waits for or receives.
pub const DATAGRAMS: usize = 1200;

#[derive(Default)]
pub struct Wakes {
    pub count: Mutex<usize>,
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
    pub fn reach(&self, count: usize) -> bool {
        let wakes = self.count.lock().expect("the wakes");
        let (wakes, _) = self
            .changed
            .wait_timeout_while(wakes, PATIENCE, |wakes| *wakes < count)
            .expect("the wakes");
        *wakes >= count
    }
}

/// A host socket on loopback with a plain socket that sends to it.
pub fn pair(sockets: &HostUdpSockets) -> (HostUdpSocket, UdpSocket) {
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

pub fn poll_with<F: Future>(call: Pin<&mut F>, wakes: &Arc<Wakes>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::clone(wakes));
    call.poll(&mut Context::from_waker(&waker))
}

/// One datagram waited for, then taken.
pub fn wait_and_take(
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
        Poll::Ready(Ok(events)) => assert!(events.ready_for.contains(ReadyFor::READABLE)),
        other => panic!("the wait did not finish: {other:?}"),
    }
    assert!(sockets.receive(socket, &RefCell::new(&mut *into)).is_ok());
}
