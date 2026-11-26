//! The local system's network: the container's connections to the world outside are sockets of the
//! host, names are looked up by the host's resolver, and a container's TCP port is reached from the
//! host where it is published. A thread of its own waits for the sockets and raises the stop flag
//! when one changes, as a lookup's thread does when it finishes.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Poll, Waker};
use std::thread::JoinHandle;

use mio::unix::SourceFd;
use mio::{Events, Interest, Poll as Poller, Registry, Token};
use socket2::{Domain, Protocol, Socket as RawSocket, Type};
use wcontainers_abi::HostError;
use wcontainers_kernel::errno::Errno;
use wcontainers_readiness::{ReadinessReport, ReadyFor, UnconditionalEvents};

use crate::arrivals::{Arrival, Arrivals};
use crate::errno;

/// The names of the network's threads.
const NETWORK_THREAD: &str = "wcontainers network";

/// The token of the poller's waker, which no socket's number gives.
const WAKE: Token = Token(usize::MAX);

/// The most changes taken from the host in one wait; the rest come with the next.
const EVENTS_AT_ONCE: usize = 1024;

/// What the poller is asked for when neither reading nor writing is, as it takes at least one: on Linux urgent data,
/// which a socket here has only beside an error; elsewhere readable, as kqueue reports errors and hang-ups only with a
/// filter.
#[cfg(target_os = "linux")]
const LEAST: Interest = Interest::PRIORITY;
#[cfg(not(target_os = "linux"))]
const LEAST: Interest = Interest::READABLE;

/// A report of nothing ready.
const NOTHING: ReadinessReport = ReadinessReport {
    ready_for: ReadyFor::empty(),
    unconditional: UnconditionalEvents::empty(),
};

/// Whether `ready` ends a wait for `interest`: something asked, an error or a hang-up.
fn ends_wait(ready: &ReadinessReport, interest: ReadyFor) -> bool {
    ready.ready_for.intersects(interest) || !ready.unconditional.is_empty()
}

/// A UDP socket, numbered here.
struct Datagram {
    socket: UdpSocket,
    /// The address it is connected to.
    peer: Option<SocketAddr>,
    /// What the poller said it is ready for, not yet given to a wait.
    ready: ReadinessReport,
    waker: Option<Waker>,
    /// Whether the poller reports its next change.
    armed: bool,
}

/// A TCP connection, numbered here.
struct Stream {
    /// None when the connect failed at once.
    socket: Option<TcpStream>,
    /// Still connecting: the next change is the connection's result.
    connecting: bool,
    /// The error the connect failed with, given to the next receive.
    error: Option<i32>,
    /// What the poller said it is ready for, not yet given to a wait.
    ready: ReadinessReport,
    waker: Option<Waker>,
    /// Whether the poller reports its next change.
    armed: bool,
}

/// A TCP listener, numbered here; one for a port not published has no socket and never accepts.
struct Listener {
    socket: Option<TcpListener>,
    ready: bool,
    waker: Option<Waker>,
    /// Whether the poller reports its next change.
    armed: bool,
}

/// The network of one container.
pub(crate) struct Network {
    registry: Registry,
    waker: mio::Waker,
    thread: Option<JoinHandle<()>>,
    gone: Arc<AtomicBool>,
    datagrams: HashMap<u32, Datagram>,
    streams: HashMap<u32, Stream>,
    listeners: HashMap<u32, Listener>,
    /// The number of the last socket made: a TCP connection or listener, or a UDP socket.
    last: u32,
    /// Where on the host each published container port is reached.
    published: HashMap<u16, SocketAddr>,
}

/// The poller's token of a socket.
fn token(id: u32) -> Token {
    Token(id as usize)
}

/// Asks the poller for the socket's next change of what is asked, which [`Network::ready`] takes, so one is told at most
/// once per asking; one ready already is told at once.
fn arm(
    registry: &Registry,
    socket: BorrowedFd<'_>,
    id: u32,
    armed: &mut bool,
    read: bool,
    write: bool,
) {
    let interest = [
        read.then_some(Interest::READABLE),
        write.then_some(Interest::WRITABLE),
    ]
    .into_iter()
    .flatten()
    .reduce(Interest::add)
    .unwrap_or(LEAST);
    let source = &mut SourceFd(&socket.as_raw_fd());
    let asked = if *armed {
        registry.reregister(source, token(id), interest)
    } else {
        registry.register(source, token(id), interest)
    };
    *armed = asked.is_ok();
}

/// Takes the socket out of the poller's set, which reports nothing more of it until it is armed again.
fn disarm(registry: &Registry, socket: BorrowedFd<'_>, armed: &mut bool) {
    if *armed
        && registry
            .deregister(&mut SourceFd(&socket.as_raw_fd()))
            .is_ok()
    {
        *armed = false;
    }
}

/// Linux's number for a host error, or `otherwise` when Linux has none.
fn linux_error(error: &std::io::Error, otherwise: u32) -> i32 {
    errno::of_io(error).unwrap_or(otherwise) as i32
}

/// Waits for the sockets armed, and tells of each change.
fn watch(mut poller: Poller, arrivals: &Arrivals, gone: &AtomicBool) {
    let mut events = Events::with_capacity(EVENTS_AT_ONCE);
    loop {
        if poller.poll(&mut events, None).is_err() || gone.load(Ordering::SeqCst) {
            return;
        }
        // An error or a hang-up makes a socket both readable and writable, as a read or write then answers at once.
        for event in events.iter().filter(|event| event.token() != WAKE) {
            arrivals.push(Arrival::Ready {
                key: event.token().0,
                readable: event.is_readable() || event.is_read_closed() || event.is_error(),
                writable: event.is_writable() || event.is_write_closed() || event.is_error(),
            });
        }
    }
}

/// Keeps the waker of a wait, made again only when it would wake another.
fn keep_waker(kept: &mut Option<Waker>, waker: &Waker) {
    if !kept.as_ref().is_some_and(|kept| kept.will_wake(waker)) {
        *kept = Some(waker.clone());
    }
}

impl Network {
    pub(crate) fn new(arrivals: Arc<Arrivals>) -> Self {
        let poller = Poller::new().expect("the host's readiness poller");
        let registry = poller
            .registry()
            .try_clone()
            .expect("the host's readiness poller");
        let waker = mio::Waker::new(&registry, WAKE).expect("the host's readiness poller");
        let gone = Arc::new(AtomicBool::new(false));
        let thread = {
            let gone = gone.clone();
            std::thread::Builder::new()
                .name(NETWORK_THREAD.to_string())
                .spawn(move || watch(poller, &arrivals, &gone))
                .expect("a network thread")
        };
        Self {
            registry,
            waker,
            thread: Some(thread),
            gone,
            datagrams: HashMap::new(),
            streams: HashMap::new(),
            listeners: HashMap::new(),
            last: 0,
            published: HashMap::new(),
        }
    }

    pub(crate) fn publish(&mut self, port: u16, at: SocketAddr) {
        self.published.insert(port, at);
    }

    fn next_number(&mut self) -> u32 {
        self.last = self.last.wrapping_add(1);
        self.last
    }

    /// Opens a UDP socket of IPv6 or IPv4, bound to a port the host chooses; its number.
    pub(crate) fn udp_open(&mut self, ipv6: bool) -> Result<u32, HostError> {
        let any = if ipv6 {
            SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), u16::MIN)
        } else {
            SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), u16::MIN)
        };
        let socket = UdpSocket::bind(any)
            .and_then(|socket| socket.set_nonblocking(true).map(|()| socket))
            .map_err(|e| HostError::Errno(linux_error(&e, Errno::ENETUNREACH as u32)))?;
        let id = self.next_number();
        let datagram = Datagram {
            socket,
            peer: None,
            ready: NOTHING,
            waker: None,
            armed: false,
        };
        self.datagrams.insert(id, datagram);
        Ok(id)
    }

    /// Connects a UDP socket to an outside address; its local address.
    pub(crate) fn udp_connect(&mut self, id: u32, to: SocketAddr) -> Result<SocketAddr, HostError> {
        let datagram = self.datagrams.get_mut(&id).ok_or(HostError::Closed)?;
        datagram
            .socket
            .connect(to)
            .map_err(|e| HostError::Errno(linux_error(&e, Errno::ECONNREFUSED as u32)))?;
        datagram.peer = Some(to);
        datagram
            .socket
            .local_addr()
            .map_err(|e| HostError::Errno(linux_error(&e, Errno::ENOTCONN as u32)))
    }

    pub(crate) fn udp_send(&mut self, id: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let datagram = self.datagrams.get_mut(&id).ok_or(HostError::Closed)?;
        match datagram.socket.send(bytes) {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
            Err(_) => Err(HostError::Closed),
        }
    }

    /// Receives one datagram as the whole packet from its IP header; the packet's length.
    pub(crate) fn udp_receive(&mut self, id: u32, into: &mut [u8]) -> Result<usize, HostError> {
        let datagram = self.datagrams.get_mut(&id).ok_or(HostError::Closed)?;
        let peer = datagram
            .peer
            .ok_or(HostError::Errno(Errno::ENOTCONN as i32))?;
        let local = datagram
            .socket
            .local_addr()
            .map_err(|e| HostError::Errno(linux_error(&e, Errno::ENOTCONN as u32)))?;
        let socket = &datagram.socket;
        crate::udp::receive_packet(peer, local, into, |payload| match socket.recv(payload) {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
            Err(_) => Err(HostError::Closed),
        })
    }

    pub(crate) fn udp_close(&mut self, id: u32) {
        if let Some(mut datagram) = self.datagrams.remove(&id) {
            disarm(&self.registry, datagram.socket.as_fd(), &mut datagram.armed);
        }
    }

    /// Starts a TCP connection to `to`, without waiting; its number.
    pub(crate) fn tcp_connect(&mut self, to: SocketAddr) -> Result<u32, HostError> {
        let socket = RawSocket::new(Domain::for_address(to), Type::STREAM, Some(Protocol::TCP))
            .and_then(|socket| socket.set_nonblocking(true).map(|()| socket))
            .map_err(|e| HostError::Errno(linux_error(&e, Errno::ENETUNREACH as u32)))?;
        let id = self.next_number();
        let mut stream = Stream {
            socket: None,
            connecting: false,
            error: None,
            ready: NOTHING,
            waker: None,
            armed: false,
        };
        match socket.connect(&to.into()) {
            Ok(()) => stream.ready.ready_for = ReadyFor::WRITABLE,
            Err(error)
                if error.kind() == ErrorKind::WouldBlock
                    || rustix::io::Errno::from_io_error(&error)
                        == Some(rustix::io::Errno::INPROGRESS) =>
            {
                stream.connecting = true;
            }
            Err(error) => {
                stream.error = Some(linux_error(&error, Errno::ECONNREFUSED as u32));
                stream.ready.unconditional =
                    UnconditionalEvents::ERROR | UnconditionalEvents::HANG_UP;
                self.streams.insert(id, stream);
                return Ok(id);
            }
        }
        stream.socket = Some(socket.into());
        self.streams.insert(id, stream);
        Ok(id)
    }

    pub(crate) fn tcp_send(&mut self, id: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let stream = self.streams.get_mut(&id).ok_or(HostError::Closed)?;
        let socket = stream.socket.as_mut().ok_or(HostError::Closed)?;
        match socket.write(bytes) {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
            Err(_) => Err(HostError::Closed),
        }
    }

    /// Receives or peeks; zero bytes is the end of the stream.
    pub(crate) fn tcp_receive(
        &mut self,
        id: u32,
        into: &mut [u8],
        peek: bool,
    ) -> Result<usize, HostError> {
        let stream = self.streams.get_mut(&id).ok_or(HostError::Closed)?;
        if let Some(error) = stream.error {
            return Err(HostError::Errno(error));
        }
        let socket = stream.socket.as_mut().ok_or(HostError::Closed)?;
        let result = if peek {
            socket.peek(into)
        } else {
            socket.read(into)
        };
        match result {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
            Err(_) => Err(HostError::Closed),
        }
    }

    pub(crate) fn tcp_shut_down(&mut self, id: u32, how: Shutdown) -> Result<(), HostError> {
        let stream = self.streams.get(&id).ok_or(HostError::Closed)?;
        let socket = stream.socket.as_ref().ok_or(HostError::Closed)?;
        socket
            .shutdown(how)
            .map_err(|e| HostError::Errno(linux_error(&e, Errno::ENOTCONN as u32)))
    }

    pub(crate) fn tcp_close(&mut self, id: u32) {
        if let Some(mut stream) = self.streams.remove(&id)
            && let Some(socket) = &stream.socket
        {
            disarm(&self.registry, socket.as_fd(), &mut stream.armed);
        }
        if let Some(mut listener) = self.listeners.remove(&id)
            && let Some(socket) = &listener.socket
        {
            disarm(&self.registry, socket.as_fd(), &mut listener.armed);
        }
    }

    /// Accepts connections to a container's port where it is published; a listener for a port
    /// not published is closed to an accept. Its number.
    pub(crate) fn tcp_listen(&mut self, port: u16) -> Result<u32, HostError> {
        let socket = match self.published.get(&port) {
            Some(&at) => {
                let socket = TcpListener::bind(at)
                    .and_then(|socket| socket.set_nonblocking(true).map(|()| socket))
                    .map_err(|e| HostError::Errno(linux_error(&e, Errno::EADDRINUSE as u32)))?;
                Some(socket)
            }
            None => None,
        };
        let id = self.next_number();
        let listener = Listener {
            socket,
            ready: false,
            waker: None,
            armed: false,
        };
        self.listeners.insert(id, listener);
        Ok(id)
    }

    /// Takes one connection waiting on a listener; its number.
    pub(crate) fn tcp_accept(&mut self, listener: u32) -> Result<u32, HostError> {
        let socket = self
            .listeners
            .get(&listener)
            .ok_or(HostError::Closed)?
            .socket
            .as_ref()
            .ok_or(HostError::Closed)?;
        let stream = match socket.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                return Err(HostError::WouldBlock);
            }
            Err(error) => {
                return Err(HostError::Errno(linux_error(
                    &error,
                    Errno::ECONNABORTED as u32,
                )));
            }
        };
        stream
            .set_nonblocking(true)
            .map_err(|e| HostError::Errno(linux_error(&e, Errno::ECONNABORTED as u32)))?;
        let id = self.next_number();
        self.streams.insert(
            id,
            Stream {
                socket: Some(stream),
                connecting: false,
                error: None,
                ready: NOTHING,
                waker: None,
                armed: false,
            },
        );
        Ok(id)
    }

    /// What a socket is ready for, once it is ready for something asked (an error or a hang-up
    /// always counts); until then the poller is asked and `waker` kept.
    pub(crate) fn poll_ready(
        &mut self,
        id: u32,
        interest: ReadyFor,
        waker: &Waker,
    ) -> Poll<Result<ReadinessReport, HostError>> {
        if let Some(datagram) = self.datagrams.get_mut(&id) {
            if ends_wait(&datagram.ready, interest) {
                return Poll::Ready(Ok(core::mem::replace(&mut datagram.ready, NOTHING)));
            }
            keep_waker(&mut datagram.waker, waker);
            let read = interest.contains(ReadyFor::READABLE);
            let write = interest.contains(ReadyFor::WRITABLE);
            let socket = datagram.socket.as_fd();
            arm(&self.registry, socket, id, &mut datagram.armed, read, write);
            return Poll::Pending;
        }
        if let Some(stream) = self.streams.get_mut(&id) {
            if ends_wait(&stream.ready, interest) {
                return Poll::Ready(Ok(core::mem::replace(&mut stream.ready, NOTHING)));
            }
            keep_waker(&mut stream.waker, waker);
            if let Some(socket) = &stream.socket {
                let read = interest.contains(ReadyFor::READABLE);
                let write = interest.contains(ReadyFor::WRITABLE) || stream.connecting;
                arm(
                    &self.registry,
                    socket.as_fd(),
                    id,
                    &mut stream.armed,
                    read,
                    write,
                );
            }
            return Poll::Pending;
        }
        let Some(listener) = self.listeners.get_mut(&id) else {
            return Poll::Ready(Err(HostError::Closed));
        };
        if listener.ready && interest.contains(ReadyFor::READABLE) {
            listener.ready = false;
            return Poll::Ready(Ok(ReadinessReport {
                ready_for: ReadyFor::READABLE,
                unconditional: UnconditionalEvents::empty(),
            }));
        }
        keep_waker(&mut listener.waker, waker);
        if let Some(socket) = &listener.socket {
            arm(
                &self.registry,
                socket.as_fd(),
                id,
                &mut listener.armed,
                true,
                false,
            );
        }
        Poll::Pending
    }

    /// Wakes the wait on a socket the poller says changed.
    fn ready(&mut self, id: u32, readable: bool, writable: bool) {
        if let Some(datagram) = self.datagrams.get_mut(&id) {
            disarm(&self.registry, datagram.socket.as_fd(), &mut datagram.armed);
            if readable {
                datagram.ready.ready_for |= ReadyFor::READABLE;
            }
            if writable {
                datagram.ready.ready_for |= ReadyFor::WRITABLE;
            }
            if let Some(waker) = datagram.waker.take() {
                waker.wake();
            }
            return;
        }
        if let Some(listener) = self.listeners.get_mut(&id) {
            if let Some(socket) = &listener.socket {
                disarm(&self.registry, socket.as_fd(), &mut listener.armed);
            }
            listener.ready = true;
            if let Some(waker) = listener.waker.take() {
                waker.wake();
            }
            return;
        }
        let Some(stream) = self.streams.get_mut(&id) else {
            return;
        };
        if let Some(socket) = &stream.socket {
            disarm(&self.registry, socket.as_fd(), &mut stream.armed);
        }
        if readable {
            stream.ready.ready_for |= ReadyFor::READABLE;
        }
        if writable {
            stream.ready.ready_for |= ReadyFor::WRITABLE;
        }
        if core::mem::take(&mut stream.connecting)
            && let Some(socket) = &stream.socket
        {
            let error = match socket.take_error() {
                Ok(None) => None,
                Ok(Some(error)) | Err(error) => {
                    Some(linux_error(&error, Errno::ECONNREFUSED as u32))
                }
            };
            if error.is_some() {
                stream.error = error;
                stream.ready.unconditional |=
                    UnconditionalEvents::ERROR | UnconditionalEvents::HANG_UP;
            }
        }
        if let Some(waker) = stream.waker.take() {
            waker.wake();
        }
    }

    /// Takes one arrival: a socket's wakes its wait, whose answer the kernel takes when it next settles; the
    /// standard input's are handed over by the system.
    pub(crate) fn arrived(&mut self, arrival: &Arrival) {
        if let Arrival::Ready {
            key,
            readable,
            writable,
        } = *arrival
        {
            self.ready(key as u32, readable, writable);
        }
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        self.gone.store(true, Ordering::SeqCst);
        let _ = self.waker.wake();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, TcpStream, UdpSocket};
    use std::sync::atomic::AtomicUsize;
    use std::task::Wake;
    use std::time::{Duration, Instant};

    use super::*;

    /// How long a change may take to arrive.
    const PATIENCE: Duration = Duration::from_secs(5);

    /// How long a check waits to see that nothing arrives.
    const SILENCE: Duration = Duration::from_millis(200);

    /// A waker that counts its wakes.
    #[derive(Default)]
    struct Counted(AtomicUsize);

    impl Wake for Counted {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Counted {
        fn count(&self) -> usize {
            self.0.load(Ordering::SeqCst)
        }
    }

    /// Hands the network what arrives, as the system does, until the waker has been woken `count` times.
    fn woken(network: &mut Network, arrivals: &Arrivals, waker: &Counted, count: usize) -> bool {
        let start = Instant::now();
        while waker.count() < count && start.elapsed() < PATIENCE {
            arrivals.wait(Some(PATIENCE));
            for arrival in arrivals.take() {
                network.arrived(&arrival);
            }
        }
        waker.count() >= count
    }

    /// Tells whether anything arrives before the silence ends.
    fn anything_arrives(arrivals: &Arrivals) -> bool {
        arrivals.wait(Some(SILENCE));
        !arrivals.take().is_empty()
    }

    #[test]
    fn a_datagram_wakes_its_wait_once() {
        let arrivals = Arc::new(Arrivals::default());
        let mut network = Network::new(arrivals.clone());
        let id = network.udp_open(false).expect("a UDP socket");
        let port = network.datagrams[&id]
            .socket
            .local_addr()
            .expect("its address")
            .port();
        let peer = UdpSocket::bind((Ipv4Addr::LOCALHOST, u16::MIN)).expect("a peer");
        network
            .udp_connect(id, peer.local_addr().expect("the peer's address"))
            .expect("connect");
        let counted = Arc::new(Counted::default());
        let waker = Waker::from(counted.clone());
        assert!(
            network
                .poll_ready(id, ReadyFor::READABLE, &waker)
                .is_pending()
        );
        assert!(!anything_arrives(&arrivals), "a change before the datagram");
        peer.send_to(b"a datagram", (Ipv4Addr::LOCALHOST, port))
            .expect("send");
        assert!(
            woken(&mut network, &arrivals, &counted, 1),
            "woken by the datagram"
        );
        // Told once per asking: the datagrams after it change nothing until the socket is waited on again.
        peer.send_to(b"another", (Ipv4Addr::LOCALHOST, port))
            .expect("send");
        assert!(
            !anything_arrives(&arrivals),
            "a change after the wait was woken"
        );
        match network.poll_ready(id, ReadyFor::READABLE, &waker) {
            Poll::Ready(Ok(report)) => assert!(report.ready_for.contains(ReadyFor::READABLE)),
            other => panic!("the wait did not finish: {other:?}"),
        }
        assert_eq!(counted.count(), 1);
        network.udp_close(id);
    }

    #[test]
    fn a_connection_wakes_the_listeners_wait_and_its_bytes_the_streams() {
        let arrivals = Arc::new(Arrivals::default());
        let mut network = Network::new(arrivals.clone());
        let published = TcpListener::bind((Ipv4Addr::LOCALHOST, u16::MIN))
            .expect("a free port")
            .local_addr()
            .expect("its address");
        network.publish(published.port(), published);
        let listener = network.tcp_listen(published.port()).expect("listen");
        let counted = Arc::new(Counted::default());
        let waker = Waker::from(counted.clone());
        assert!(
            network
                .poll_ready(listener, ReadyFor::READABLE, &waker)
                .is_pending()
        );
        let mut client = TcpStream::connect(published).expect("connect");
        assert!(
            woken(&mut network, &arrivals, &counted, 1),
            "woken by the connection"
        );
        let stream = network.tcp_accept(listener).expect("accept");
        assert!(
            network
                .poll_ready(stream, ReadyFor::READABLE, &waker)
                .is_pending()
        );
        client.write_all(b"bytes").expect("write");
        assert!(
            woken(&mut network, &arrivals, &counted, 2),
            "woken by the bytes"
        );
        let mut into = [u8::MIN; 16];
        assert_eq!(
            network.tcp_receive(stream, &mut into, false),
            Ok(b"bytes".len())
        );
        network.tcp_close(stream);
        network.tcp_close(listener);
    }
}
