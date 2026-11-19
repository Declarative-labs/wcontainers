//! The local system's network: the container's connections to the world outside are sockets of the
//! host, names are looked up by the host's resolver, and a container's TCP port is reached from the
//! host where it is published. A thread of its own waits for the sockets and raises the stop flag
//! when one changes, as a lookup's thread does when it finishes.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{
    IpAddr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket,
};
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use polling::{Event as Interest, Events, Poller};
use socket2::{Domain, Protocol, Socket as RawSocket, Type};
use wcontainers_abi::{ConnectionId, Event, HostError};
use wcontainers_kernel::linux::errno as linux_errno;

use crate::arrivals::{Arrival, Arrivals};
use crate::errno;

/// The names of the network's threads.
const NETWORK_THREAD: &str = "wcontainers network";
const LOOKUP_THREAD: &str = "wcontainers lookup";

enum Socket {
    Stream(TcpStream),
    Datagram(UdpSocket),
}

impl Socket {
    fn fd(&self) -> std::os::fd::BorrowedFd<'_> {
        match self {
            Socket::Stream(stream) => stream.as_fd(),
            Socket::Datagram(socket) => socket.as_fd(),
        }
    }
}

struct Connection {
    socket: Socket,
    /// Still connecting: the next change is the connection's result.
    connecting: bool,
    /// A receive or send answered "would block", so the next change is told.
    read_armed: bool,
    write_armed: bool,
}

struct Listener {
    socket: TcpListener,
    armed: bool,
}

/// The network of one container.
pub(crate) struct Network {
    arrivals: Arc<Arrivals>,
    poller: Arc<Poller>,
    thread: Option<JoinHandle<()>>,
    gone: Arc<AtomicBool>,
    connections: HashMap<u32, Connection>,
    listeners: HashMap<u32, Listener>,
    /// Where on the host each published container port is reached.
    published: HashMap<u16, SocketAddr>,
}

/// Linux's number for a host error, or `otherwise` when Linux has none.
fn linux_error(error: &std::io::Error, otherwise: u32) -> i32 {
    errno::of_io(error).unwrap_or(otherwise) as i32
}

/// The address a guest names, as the host takes it.
fn address(bytes: &[u8], port: u16) -> Option<SocketAddr> {
    let ip = match bytes.len() {
        len if len == size_of::<std::net::Ipv4Addr>() => {
            IpAddr::from(<[u8; size_of::<std::net::Ipv4Addr>()]>::try_from(bytes).ok()?)
        }
        _ => IpAddr::from(<[u8; size_of::<Ipv6Addr>()]>::try_from(bytes).ok()?),
    };
    Some(SocketAddr::new(ip, port))
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
                id: event.key as u32,
                readable: event.readable,
                writable: event.writable,
            });
        }
    }
}

impl Network {
    pub(crate) fn new(arrivals: Arc<Arrivals>) -> Self {
        let poller = Arc::new(Poller::new().expect("the host's readiness poller"));
        let gone = Arc::new(AtomicBool::new(false));
        let thread = {
            let (poller, arrivals, gone) = (poller.clone(), arrivals.clone(), gone.clone());
            std::thread::Builder::new()
                .name(NETWORK_THREAD.to_string())
                .spawn(move || watch(&poller, &arrivals, &gone))
                .expect("a network thread")
        };
        Self {
            arrivals,
            poller,
            thread: Some(thread),
            gone,
            connections: HashMap::new(),
            listeners: HashMap::new(),
            published: HashMap::new(),
        }
    }

    pub(crate) fn publish(&mut self, port: u16, at: SocketAddr) {
        self.published.insert(port, at);
    }

    /// Asks the poller for the next change of a connection, as it is armed now.
    fn arm(&self, id: u32) {
        if let Some(connection) = self.connections.get(&id) {
            let interest = Interest::new(
                id as usize,
                connection.read_armed,
                connection.write_armed || connection.connecting,
            );
            let _ = self.poller.modify(connection.socket.fd(), interest);
        } else if let Some(listener) = self.listeners.get(&id) {
            let interest = Interest::new(id as usize, listener.armed, false);
            let _ = self.poller.modify(&listener.socket, interest);
        }
    }

    fn insert(&mut self, id: u32, connection: Connection) {
        // SAFETY: the socket stays registered only while it is held here; `close` deletes it first.
        let _ = unsafe {
            self.poller
                .add(&connection.socket.fd(), Interest::none(id as usize))
        };
        self.connections.insert(id, connection);
        self.arm(id);
    }

    pub(crate) fn connect(
        &mut self,
        id: u32,
        bytes: &[u8],
        port: u16,
        stream: bool,
    ) -> Result<(), HostError> {
        let to = address(bytes, port).ok_or(HostError::Errno(linux_errno::EINVAL as i32))?;
        let (kind, protocol) = if stream {
            (Type::STREAM, Protocol::TCP)
        } else {
            (Type::DGRAM, Protocol::UDP)
        };
        let socket = RawSocket::new(Domain::for_address(to), kind, Some(protocol))
            .map_err(|e| HostError::Errno(linux_error(&e, linux_errno::ENETUNREACH)))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| HostError::Errno(linux_error(&e, linux_errno::ENETUNREACH)))?;
        let connecting = match socket.connect(&to.into()) {
            Ok(()) => false,
            Err(error)
                if stream
                    && (error.kind() == ErrorKind::WouldBlock
                        || rustix::io::Errno::from_io_error(&error)
                            == Some(rustix::io::Errno::INPROGRESS)) =>
            {
                true
            }
            Err(error) => {
                let number = linux_error(&error, linux_errno::ECONNREFUSED);
                if stream {
                    self.arrivals.push(Arrival::Connected(id, number));
                    return Ok(());
                }
                return Err(HostError::Errno(number));
            }
        };
        let socket = if stream {
            Socket::Stream(socket.into())
        } else {
            Socket::Datagram(socket.into())
        };
        self.insert(
            id,
            Connection {
                socket,
                connecting,
                read_armed: !connecting,
                write_armed: false,
            },
        );
        if !connecting {
            self.arrivals.push(Arrival::Connected(id, 0));
        }
        Ok(())
    }

    pub(crate) fn listen(&mut self, id: u32, port: u16) -> Result<(), HostError> {
        let Some(&at) = self.published.get(&port) else {
            // Nothing outside reaches a port that is not published.
            return Ok(());
        };
        let socket = TcpListener::bind(at)
            .map_err(|e| HostError::Errno(linux_error(&e, linux_errno::EADDRINUSE)))?;
        socket
            .set_nonblocking(true)
            .map_err(|e| HostError::Errno(linux_error(&e, linux_errno::EADDRINUSE)))?;
        // SAFETY: as in `insert`.
        let _ = unsafe { self.poller.add(&socket, Interest::none(id as usize)) };
        self.listeners.insert(
            id,
            Listener {
                socket,
                armed: true,
            },
        );
        self.arm(id);
        Ok(())
    }

    pub(crate) fn accept(&mut self, listener: u32, id: u32) -> Result<(), HostError> {
        let socket = self.listeners.get_mut(&listener).ok_or(HostError::Closed)?;
        match socket.socket.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(true)
                    .map_err(|e| HostError::Errno(linux_error(&e, linux_errno::ECONNABORTED)))?;
                self.insert(
                    id,
                    Connection {
                        socket: Socket::Stream(stream),
                        connecting: false,
                        read_armed: true,
                        write_armed: false,
                    },
                );
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                socket.armed = true;
                self.arm(listener);
                Err(HostError::WouldBlock)
            }
            Err(error) => Err(HostError::Errno(linux_error(
                &error,
                linux_errno::ECONNABORTED,
            ))),
        }
    }

    pub(crate) fn send(&mut self, id: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let connection = self.connections.get_mut(&id).ok_or(HostError::Closed)?;
        let result = match &mut connection.socket {
            Socket::Stream(stream) => stream.write(bytes),
            Socket::Datagram(socket) => socket.send(bytes),
        };
        match result {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                connection.write_armed = true;
                self.arm(id);
                Err(HostError::WouldBlock)
            }
            Err(_) => Err(HostError::Closed),
        }
    }

    pub(crate) fn receive(&mut self, id: u32, into: &mut [u8]) -> Result<usize, HostError> {
        let connection = self.connections.get_mut(&id).ok_or(HostError::Closed)?;
        let result = match &mut connection.socket {
            Socket::Stream(stream) => stream.read(into),
            Socket::Datagram(socket) => socket.recv(into),
        };
        match result {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                connection.read_armed = true;
                self.arm(id);
                Err(HostError::WouldBlock)
            }
            Err(_) => Err(HostError::Closed),
        }
    }

    pub(crate) fn finish(&mut self, id: u32) {
        if let Some(Connection {
            socket: Socket::Stream(stream),
            ..
        }) = self.connections.get(&id)
        {
            let _ = stream.shutdown(Shutdown::Write);
        }
    }

    pub(crate) fn close(&mut self, id: u32) {
        if let Some(connection) = self.connections.remove(&id) {
            let _ = self.poller.delete(connection.socket.fd());
        }
        if let Some(listener) = self.listeners.remove(&id) {
            let _ = self.poller.delete(&listener.socket);
        }
    }

    pub(crate) fn look_up(&mut self, request: u64, name: &str) {
        let arrivals = self.arrivals.clone();
        let name = name.to_string();
        let started = std::thread::Builder::new()
            .name(LOOKUP_THREAD.to_string())
            .spawn(move || {
                let arrival = match (name.as_str(), 0).to_socket_addrs() {
                    Ok(found) => {
                        let mut bytes = Vec::new();
                        for address in found {
                            let mapped = match address.ip() {
                                IpAddr::V4(v4) => v4.to_ipv6_mapped(),
                                IpAddr::V6(v6) => v6,
                            };
                            bytes.extend_from_slice(&mapped.octets());
                        }
                        Arrival::LookedUp(request, bytes, 0)
                    }
                    Err(_) => Arrival::LookedUp(request, Vec::new(), linux_errno::ENOENT as i32),
                };
                arrivals.push(arrival);
            });
        if started.is_err() {
            self.arrivals.push(Arrival::LookedUp(
                request,
                Vec::new(),
                linux_errno::EAGAIN as i32,
            ));
        }
    }

    /// The event the kernel is told for one arrival; none when it tells nothing new.
    pub(crate) fn event<'a>(&mut self, arrival: &'a Arrival) -> Option<Event<'a>> {
        match *arrival {
            // The standard input's, which the system hands over itself.
            Arrival::Input(_) | Arrival::InputEnded(_) => None,
            Arrival::Connected(id, error) => Some(Event::Connected {
                connection: ConnectionId(id),
                error,
            }),
            Arrival::LookedUp(request, ref addresses, error) => Some(Event::LookedUp {
                request,
                addresses,
                error,
            }),
            Arrival::Ready {
                id,
                readable,
                writable,
            } => {
                if let Some(listener) = self.listeners.get_mut(&id) {
                    listener.armed = false;
                    return Some(Event::ConnectionReady {
                        connection: ConnectionId(id),
                    });
                }
                let connection = self.connections.get_mut(&id)?;
                if connection.connecting {
                    connection.connecting = false;
                    let error = match &connection.socket {
                        Socket::Stream(stream) => stream.take_error(),
                        Socket::Datagram(socket) => socket.take_error(),
                    };
                    let error = match error {
                        Ok(None) => 0,
                        Ok(Some(error)) | Err(error) => {
                            linux_error(&error, linux_errno::ECONNREFUSED)
                        }
                    };
                    connection.read_armed = error == 0;
                    self.arm(id);
                    return Some(Event::Connected {
                        connection: ConnectionId(id),
                        error,
                    });
                }
                // A hang-up or error reads as both.
                connection.read_armed &= !readable;
                connection.write_armed &= !writable;
                self.arm(id);
                Some(Event::ConnectionReady {
                    connection: ConnectionId(id),
                })
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
