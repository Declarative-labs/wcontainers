//! The local system's network: the container's connections to the world outside are sockets of the
//! host, names are looked up by the host's resolver, and a container's TCP port is reached from the
//! host where it is published. A thread of its own waits for the sockets and raises the stop flag
//! when one changes, as a lookup's thread does when it finishes.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Poll, Waker};
use std::thread::JoinHandle;

use polling::{Event as Interest, Events, Poller};
use socket2::{Domain, Protocol, Socket as RawSocket, Type};
use wcontainers_abi::{Event, HostError};
use wcontainers_kernel::errno::Errno;
use wcontainers_readiness::ReadyFor;

use crate::arrivals::{Arrival, Arrivals};
use crate::errno;

/// The names of the network's threads.
const NETWORK_THREAD: &str = "wcontainers network";

/// A UDP socket, numbered here.
struct Datagram {
    socket: UdpSocket,
    /// The address it is connected to.
    peer: Option<SocketAddr>,
    /// What the poller said it is ready for, not yet given to a wait.
    ready: ReadyFor,
    waker: Option<Waker>,
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
    ready: ReadyFor,
    waker: Option<Waker>,
}

/// A TCP listener, numbered here; one for a port not published has no socket and never accepts.
struct Listener {
    socket: Option<TcpListener>,
    ready: bool,
    waker: Option<Waker>,
}

/// The network of one container.
pub(crate) struct Network {
    poller: Arc<Poller>,
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

/// The poller's key of a socket.
fn key(id: u32) -> usize {
    id as usize
}

/// Linux's number for a host error, or `otherwise` when Linux has none.
fn linux_error(error: &std::io::Error, otherwise: u32) -> i32 {
    errno::of_io(error).unwrap_or(otherwise) as i32
}

/// Waits for the sockets armed, and tells of each change.
fn watch(poller: &Poller, arrivals: &Arrivals, gone: &AtomicBool) {
    let mut events = Events::new();
    loop {
        events.clear();
        if poller.wait(&mut events, None).is_err() || gone.load(Ordering::SeqCst) {
            return;
        }
        for event in events.iter() {
            arrivals.push(Arrival::Ready {
                key: event.key,
                readable: event.readable,
                writable: event.writable,
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
        let poller = Arc::new(Poller::new().expect("the host's readiness poller"));
        let gone = Arc::new(AtomicBool::new(false));
        let thread = {
            let (poller, gone) = (poller.clone(), gone.clone());
            std::thread::Builder::new()
                .name(NETWORK_THREAD.to_string())
                .spawn(move || watch(&poller, &arrivals, &gone))
                .expect("a network thread")
        };
        Self {
            poller,
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
        // SAFETY: the socket stays registered only while it is held here; closing deletes it first.
        let _ = unsafe { self.poller.add(&socket.as_fd(), Interest::none(key(id))) };
        let datagram = Datagram {
            socket,
            peer: None,
            ready: ReadyFor::empty(),
            waker: None,
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
        if let Some(datagram) = self.datagrams.remove(&id) {
            let _ = self.poller.delete(datagram.socket.as_fd());
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
            ready: ReadyFor::empty(),
            waker: None,
        };
        match socket.connect(&to.into()) {
            Ok(()) => stream.ready = ReadyFor::WRITABLE,
            Err(error)
                if error.kind() == ErrorKind::WouldBlock
                    || rustix::io::Errno::from_io_error(&error)
                        == Some(rustix::io::Errno::INPROGRESS) =>
            {
                stream.connecting = true;
            }
            Err(error) => {
                stream.error = Some(linux_error(&error, Errno::ECONNREFUSED as u32));
                stream.ready = ReadyFor::ERROR | ReadyFor::HANG_UP;
                self.streams.insert(id, stream);
                return Ok(id);
            }
        }
        let socket: TcpStream = socket.into();
        // SAFETY: as in `connect`.
        let _ = unsafe { self.poller.add(&socket.as_fd(), Interest::none(key(id))) };
        stream.socket = Some(socket);
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
        if let Some(socket) = self.streams.remove(&id).and_then(|stream| stream.socket) {
            let _ = self.poller.delete(socket.as_fd());
        }
        if let Some(socket) = self
            .listeners
            .remove(&id)
            .and_then(|listener| listener.socket)
        {
            let _ = self.poller.delete(&socket);
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
        if let Some(socket) = &socket {
            // SAFETY: as in `connect`.
            let _ = unsafe { self.poller.add(socket, Interest::none(key(id))) };
        }
        let listener = Listener {
            socket,
            ready: false,
            waker: None,
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
        // SAFETY: as in `connect`.
        let _ = unsafe { self.poller.add(&stream.as_fd(), Interest::none(key(id))) };
        self.streams.insert(
            id,
            Stream {
                socket: Some(stream),
                connecting: false,
                error: None,
                ready: ReadyFor::empty(),
                waker: None,
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
    ) -> Poll<Result<ReadyFor, HostError>> {
        let counted = interest | ReadyFor::ERROR | ReadyFor::HANG_UP;
        if let Some(datagram) = self.datagrams.get_mut(&id) {
            if datagram.ready.intersects(counted) {
                return Poll::Ready(Ok(core::mem::replace(
                    &mut datagram.ready,
                    ReadyFor::empty(),
                )));
            }
            keep_waker(&mut datagram.waker, waker);
            let read = interest.contains(ReadyFor::READABLE);
            let write = interest.contains(ReadyFor::WRITABLE);
            let _ = self
                .poller
                .modify(datagram.socket.as_fd(), Interest::new(key(id), read, write));
            return Poll::Pending;
        }
        if let Some(stream) = self.streams.get_mut(&id) {
            if stream.ready.intersects(counted) {
                return Poll::Ready(Ok(core::mem::replace(&mut stream.ready, ReadyFor::empty())));
            }
            keep_waker(&mut stream.waker, waker);
            if let Some(socket) = &stream.socket {
                let read = interest.contains(ReadyFor::READABLE);
                let write = interest.contains(ReadyFor::WRITABLE) || stream.connecting;
                let _ = self
                    .poller
                    .modify(socket.as_fd(), Interest::new(key(id), read, write));
            }
            return Poll::Pending;
        }
        let Some(listener) = self.listeners.get_mut(&id) else {
            return Poll::Ready(Err(HostError::Closed));
        };
        if listener.ready && interest.contains(ReadyFor::READABLE) {
            listener.ready = false;
            return Poll::Ready(Ok(ReadyFor::READABLE));
        }
        keep_waker(&mut listener.waker, waker);
        if let Some(socket) = &listener.socket {
            let interest = Interest::new(key(id), true, false);
            let _ = self.poller.modify(socket, interest);
        }
        Poll::Pending
    }

    /// Wakes the wait on a socket the poller says changed.
    fn ready(&mut self, id: u32, readable: bool, writable: bool) {
        if let Some(datagram) = self.datagrams.get_mut(&id) {
            if readable {
                datagram.ready |= ReadyFor::READABLE;
            }
            if writable {
                datagram.ready |= ReadyFor::WRITABLE;
            }
            if let Some(waker) = datagram.waker.take() {
                waker.wake();
            }
            return;
        }
        if let Some(listener) = self.listeners.get_mut(&id) {
            listener.ready = true;
            if let Some(waker) = listener.waker.take() {
                waker.wake();
            }
            return;
        }
        let Some(stream) = self.streams.get_mut(&id) else {
            return;
        };
        if readable {
            stream.ready |= ReadyFor::READABLE;
        }
        if writable {
            stream.ready |= ReadyFor::WRITABLE;
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
                stream.ready |= ReadyFor::ERROR | ReadyFor::HANG_UP;
            }
        }
        if let Some(waker) = stream.waker.take() {
            waker.wake();
        }
    }

    /// The event the kernel is told for one arrival: none, as the standard input's are handed
    /// over by the system and a socket's wakes its wait, whose answer the kernel takes when it
    /// next settles.
    pub(crate) fn event<'a>(&mut self, arrival: &'a Arrival) -> Option<Event<'a>> {
        match *arrival {
            Arrival::Input(_) | Arrival::InputEnded(_) | Arrival::Answered => None,
            Arrival::Ready {
                key,
                readable,
                writable,
            } => {
                self.ready(key as u32, readable, writable);
                None
            }
        }
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        self.gone.store(true, Ordering::SeqCst);
        let _ = self.poller.notify();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
