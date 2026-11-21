//! The test host's network hooks: connections to the outside are real sockets of the machine the
//! tests run on, and names are looked up by its resolver. The reference reaches this machine as
//! Docker Desktop's `host.docker.internal`; here that name and its address mean this machine's
//! loopback, so a test server on this machine serves both.

use std::collections::{BTreeMap, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::net::{
    IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs,
    UdpSocket,
};
use std::os::fd::AsRawFd;
use std::time::Duration;

use wcontainers_abi::{ConnectionId, Event, HostError};
use wcontainers_kernel::errno::Errno;

/// Docker Desktop's name and address for the machine it runs on.
pub const DOCKER_HOST_NAME: &str = "host.docker.internal";
pub const DOCKER_HOST: Ipv4Addr = Ipv4Addr::new(192, 168, 65, 254);

enum Socket {
    Stream(TcpStream),
    Datagram(UdpSocket),
}

impl Socket {
    fn fd(&self) -> i32 {
        match self {
            Socket::Stream(stream) => stream.as_raw_fd(),
            Socket::Datagram(socket) => socket.as_raw_fd(),
        }
    }
}

struct Connection {
    socket: Socket,
    /// A receive or send answered "would block", so the next change is told.
    read_armed: bool,
    write_armed: bool,
}

/// An event owed to the kernel.
pub enum Owed {
    Connected(u32, i32),
    Ready(u32),
    LookedUp(u64, Vec<u8>, i32),
}

impl Owed {
    /// Hands the event to `deliver` in the boundary's form.
    pub fn with(&self, deliver: impl FnOnce(Event<'_>)) {
        match self {
            Owed::Connected(id, error) => deliver(Event::Connected {
                connection: ConnectionId(*id),
                error: *error,
            }),
            Owed::Ready(id) => deliver(Event::ConnectionReady {
                connection: ConnectionId(*id),
            }),
            Owed::LookedUp(request, addresses, error) => deliver(Event::LookedUp {
                request: *request,
                addresses,
                error: *error,
            }),
        }
    }
}

/// A container port this machine accepts connections for.
struct Listener {
    socket: TcpListener,
    /// The container's port.
    port: u16,
    /// An accept answered "would block", so the next connection is told.
    armed: bool,
}

#[derive(Default)]
pub struct Network {
    connections: BTreeMap<u32, Connection>,
    listeners: BTreeMap<u32, Listener>,
    owed: VecDeque<Owed>,
    /// Ports of this machine's loopback that reach the container's ports, by the container's
    /// port, as `docker run -p 127.0.0.1:outer:inner` gives.
    pub published: BTreeMap<u16, u16>,
    /// Listening sockets of published ports an earlier host held, taken again by a listen.
    kept: BTreeMap<u16, TcpListener>,
}

/// The address a guest names as this machine sees it.
fn local(address: &[u8], port: u16) -> SocketAddr {
    let ip = match address.len() {
        4 => {
            let v4 = Ipv4Addr::new(address[0], address[1], address[2], address[3]);
            if v4 == DOCKER_HOST {
                IpAddr::V4(Ipv4Addr::LOCALHOST)
            } else {
                IpAddr::V4(v4)
            }
        }
        _ => {
            let bytes: [u8; 16] = address.try_into().expect("16 bytes");
            IpAddr::V6(Ipv6Addr::from(bytes))
        }
    };
    SocketAddr::new(ip, port)
}

fn errno(error: &std::io::Error) -> i32 {
    match error.kind() {
        ErrorKind::ConnectionRefused => Errno::ECONNREFUSED as i32,
        ErrorKind::TimedOut => Errno::ETIMEDOUT as i32,
        ErrorKind::NetworkUnreachable => Errno::ENETUNREACH as i32,
        ErrorKind::HostUnreachable => Errno::EHOSTUNREACH as i32,
        _ => Errno::ECONNREFUSED as i32,
    }
}

impl Network {
    /// The network a host brought back from a save starts with: the same published ports, still
    /// held, and no connections.
    pub fn moved(&mut self) -> Network {
        let mut kept = std::mem::take(&mut self.kept);
        for listener in std::mem::take(&mut self.listeners).into_values() {
            kept.insert(listener.port, listener.socket);
        }
        Network {
            published: self.published.clone(),
            kept,
            ..Default::default()
        }
    }

    /// Accepts connections to a published port; nothing reaches a port not published.
    pub fn listen(&mut self, id: u32, port: u16) -> Result<(), HostError> {
        let Some(&outer) = self.published.get(&port) else {
            return Ok(());
        };
        let socket = match self.kept.remove(&port) {
            Some(socket) => socket,
            None => TcpListener::bind((Ipv4Addr::LOCALHOST, outer))
                .map_err(|_| HostError::Errno(Errno::EADDRINUSE as i32))?,
        };
        socket.set_nonblocking(true).expect("nonblocking");
        let listener = Listener {
            socket,
            port,
            armed: true,
        };
        self.listeners.insert(id, listener);
        Ok(())
    }

    pub fn accept(&mut self, listener: u32, id: u32) -> Result<(), HostError> {
        let listener = self.listeners.get_mut(&listener).ok_or(HostError::Closed)?;
        match listener.socket.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).expect("nonblocking");
                let connection = Connection {
                    socket: Socket::Stream(stream),
                    read_armed: true,
                    write_armed: false,
                };
                self.connections.insert(id, connection);
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                listener.armed = true;
                Err(HostError::WouldBlock)
            }
            Err(_) => Err(HostError::Errno(Errno::ECONNABORTED as i32)),
        }
    }

    pub fn connect(
        &mut self,
        id: u32,
        address: &[u8],
        port: u16,
        stream: bool,
    ) -> Result<(), HostError> {
        let to = local(address, port);
        let socket = if stream {
            match TcpStream::connect_timeout(&to, Duration::from_secs(10)) {
                Ok(stream) => Socket::Stream(stream),
                Err(error) => {
                    self.owed.push_back(Owed::Connected(id, errno(&error)));
                    return Ok(());
                }
            }
        } else {
            let bind = if to.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
            let socket =
                UdpSocket::bind(bind).map_err(|_| HostError::Errno(Errno::ENETUNREACH as i32))?;
            socket
                .connect(to)
                .map_err(|error| HostError::Errno(errno(&error)))?;
            Socket::Datagram(socket)
        };
        match &socket {
            Socket::Stream(stream) => stream.set_nonblocking(true),
            Socket::Datagram(socket) => socket.set_nonblocking(true),
        }
        .expect("nonblocking");
        self.connections.insert(
            id,
            Connection {
                socket,
                read_armed: true,
                write_armed: false,
            },
        );
        self.owed.push_back(Owed::Connected(id, 0));
        Ok(())
    }

    pub fn send(&mut self, id: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let connection = self.connections.get_mut(&id).ok_or(HostError::Closed)?;
        let result = match &mut connection.socket {
            Socket::Stream(stream) => stream.write(bytes),
            Socket::Datagram(socket) => socket.send(bytes),
        };
        match result {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                connection.write_armed = true;
                Err(HostError::WouldBlock)
            }
            Err(_) => Err(HostError::Closed),
        }
    }

    pub fn receive(&mut self, id: u32, into: &mut [u8]) -> Result<usize, HostError> {
        let connection = self.connections.get_mut(&id).ok_or(HostError::Closed)?;
        let result = match &mut connection.socket {
            Socket::Stream(stream) => stream.read(into),
            Socket::Datagram(socket) => socket.recv(into),
        };
        match result {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                connection.read_armed = true;
                Err(HostError::WouldBlock)
            }
            Err(_) => Err(HostError::Closed),
        }
    }

    pub fn finish(&mut self, id: u32) {
        if let Some(Connection {
            socket: Socket::Stream(stream),
            ..
        }) = self.connections.get(&id)
        {
            let _ = stream.shutdown(Shutdown::Write);
        }
    }

    pub fn close(&mut self, id: u32) {
        self.connections.remove(&id);
        self.listeners.remove(&id);
    }

    pub fn look_up(&mut self, request: u64, name: &str) {
        let found: Result<Vec<IpAddr>, ()> = if name == DOCKER_HOST_NAME {
            Ok(vec![IpAddr::V4(DOCKER_HOST)])
        } else {
            (name, 0)
                .to_socket_addrs()
                .map(|addresses| addresses.map(|address| address.ip()).collect())
                .map_err(|_| ())
        };
        let owed = match found {
            Ok(addresses) => {
                let mut bytes = Vec::new();
                for address in addresses {
                    let mapped = match address {
                        IpAddr::V4(v4) => v4.to_ipv6_mapped(),
                        IpAddr::V6(v6) => v6,
                    };
                    bytes.extend_from_slice(&mapped.octets());
                }
                Owed::LookedUp(request, bytes, 0)
            }
            Err(()) => Owed::LookedUp(request, Vec::new(), Errno::ENOENT as i32),
        };
        self.owed.push_back(owed);
    }

    /// Whether anything may still come: an event owed, or a connection or listener waiting for a
    /// change.
    pub fn waiting(&self) -> bool {
        !self.owed.is_empty()
            || self
                .connections
                .values()
                .any(|connection| connection.read_armed || connection.write_armed)
            || self.listeners.values().any(|listener| listener.armed)
    }

    /// The events owed, after waiting up to `timeout` for a connection to change when none is.
    pub fn take(&mut self, timeout: Duration) -> Vec<Owed> {
        let mut armed: Vec<(u32, i32, i16)> = self
            .connections
            .iter()
            .filter(|(_, connection)| connection.read_armed || connection.write_armed)
            .map(|(&id, connection)| {
                let mut events = 0;
                if connection.read_armed {
                    events |= libc::POLLIN;
                }
                if connection.write_armed {
                    events |= libc::POLLOUT;
                }
                (id, connection.socket.fd(), events)
            })
            .collect();
        armed.extend(
            self.listeners
                .iter()
                .filter(|(_, listener)| listener.armed)
                .map(|(&id, listener)| (id, listener.socket.as_raw_fd(), libc::POLLIN)),
        );
        let mut fds: Vec<libc::pollfd> = armed
            .iter()
            .map(|&(_, fd, events)| libc::pollfd {
                fd,
                events,
                revents: 0,
            })
            .collect();
        let wait = if self.owed.is_empty() {
            timeout.as_millis() as i32
        } else {
            0
        };
        if !fds.is_empty() {
            // SAFETY: the descriptors are this host's own open sockets.
            unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, wait) };
        }
        for (&(id, _, _), fd) in armed.iter().zip(&fds) {
            if fd.revents == 0 {
                continue;
            }
            if let Some(listener) = self.listeners.get_mut(&id) {
                listener.armed = false;
                self.owed.push_back(Owed::Ready(id));
                continue;
            }
            let connection = self.connections.get_mut(&id).expect("an armed connection");
            if fd.revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                connection.read_armed = false;
            }
            if fd.revents & (libc::POLLOUT | libc::POLLHUP | libc::POLLERR) != 0 {
                connection.write_armed = false;
            }
            self.owed.push_back(Owed::Ready(id));
        }
        self.owed.drain(..).collect()
    }
}
