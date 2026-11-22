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

use domain::base::{Name, Question};
use wcontainers_abi::{ConnectionId, Event, HostError};
use wcontainers_kernel::errno::Errno;
use wcontainers_readiness::ReadyFor;

/// Docker Desktop's name and address for the machine it runs on.
pub const DOCKER_HOST_NAME: &str = "host.docker.internal";
pub const DOCKER_HOST: Ipv4Addr = Ipv4Addr::new(192, 168, 65, 254);

/// A UDP flow, by the kernel's number of it.
struct Flow {
    socket: UdpSocket,
    /// A receive or send answered "would block", so the next change is told.
    read_armed: bool,
    write_armed: bool,
}

/// A TCP connection, by this host's number of it.
struct Stream {
    /// None when the connect failed.
    socket: Option<TcpStream>,
    /// The error the connect failed with, given to the next receive.
    error: Option<i32>,
    /// A wait asks for this, so the next change is told.
    read_armed: bool,
    write_armed: bool,
}

/// An event owed to the kernel, the answer to the name question of a number, or what a TCP
/// connection or listener of a number became ready for.
pub enum Owed {
    Ready(u32),
    Answered(u64, Vec<u8>),
    TcpReady(u32, ReadyFor),
}

impl Owed {
    /// Hands the event to `deliver` in the boundary's form; none for an answer.
    pub fn with(&self, deliver: impl FnOnce(Event<'_>)) {
        match self {
            Owed::Ready(id) => deliver(Event::ConnectionReady {
                connection: ConnectionId(*id),
            }),
            Owed::Answered(..) | Owed::TcpReady(..) => {}
        }
    }
}

/// A container port this machine accepts connections for.
struct Listener {
    socket: TcpListener,
    /// The container's port.
    port: u16,
    /// A wait asks for a connection, so the next one is told.
    armed: bool,
}

#[derive(Default)]
pub struct Network {
    flows: BTreeMap<u32, Flow>,
    streams: BTreeMap<u32, Stream>,
    listeners: BTreeMap<u32, Listener>,
    owed: VecDeque<Owed>,
    /// Ports of this machine's loopback that reach the container's ports, by the container's
    /// port, as `docker run -p 127.0.0.1:outer:inner` gives.
    pub published: BTreeMap<u16, u16>,
    /// Listening sockets of published ports an earlier host held, taken again by a listen.
    kept: BTreeMap<u16, TcpListener>,
}

/// The address a guest names as this machine sees it.
fn seen_here(address: SocketAddr) -> SocketAddr {
    match address {
        SocketAddr::V4(v4) if *v4.ip() == DOCKER_HOST => {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), v4.port())
        }
        other => other,
    }
}

/// The address a guest names as 4 or 16 bytes, as this machine sees it.
fn local(address: &[u8], port: u16) -> SocketAddr {
    let ip = match <[u8; size_of::<Ipv4Addr>()]>::try_from(address) {
        Ok(v4) => IpAddr::V4(Ipv4Addr::from(v4)),
        Err(_) => {
            let bytes: [u8; size_of::<Ipv6Addr>()] = address.try_into().expect("16 bytes");
            IpAddr::V6(Ipv6Addr::from(bytes))
        }
    };
    seen_here(SocketAddr::new(ip, port))
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

/// How long a connect to the outside may take.
const CONNECT_PATIENCE: Duration = Duration::from_secs(10);

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

    /// Accepts connections to a published port as listener `number`; nothing reaches a port not
    /// published.
    pub fn listen(&mut self, number: u32, port: u16) -> Result<(), HostError> {
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
            armed: false,
        };
        self.listeners.insert(number, listener);
        Ok(())
    }

    /// Takes a connection waiting on a listener as connection `number`.
    pub fn accept(&mut self, listener: u32, number: u32) -> Result<(), HostError> {
        let listener = self.listeners.get_mut(&listener).ok_or(HostError::Closed)?;
        match listener.socket.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).expect("nonblocking");
                let stream = Stream {
                    socket: Some(stream),
                    error: None,
                    read_armed: false,
                    write_armed: false,
                };
                self.streams.insert(number, stream);
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
            Err(_) => Err(HostError::Errno(Errno::ECONNABORTED as i32)),
        }
    }

    /// Connects TCP connection `number` to `to`; its end is owed as its readiness.
    pub fn tcp_connect(&mut self, number: u32, to: SocketAddr) {
        let mut stream = Stream {
            socket: None,
            error: None,
            read_armed: false,
            write_armed: false,
        };
        let ready = match TcpStream::connect_timeout(&seen_here(to), CONNECT_PATIENCE) {
            Ok(socket) => {
                socket.set_nonblocking(true).expect("nonblocking");
                stream.socket = Some(socket);
                ReadyFor::WRITABLE
            }
            Err(error) => {
                stream.error = Some(errno(&error));
                ReadyFor::ERROR | ReadyFor::HANG_UP
            }
        };
        self.streams.insert(number, stream);
        self.owed.push_back(Owed::TcpReady(number, ready));
    }

    pub fn tcp_send(&mut self, number: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let stream = self.streams.get_mut(&number).ok_or(HostError::Closed)?;
        let socket = stream.socket.as_mut().ok_or(HostError::Closed)?;
        match socket.write(bytes) {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
            Err(_) => Err(HostError::Closed),
        }
    }

    /// Receives or peeks; zero bytes is the end of the stream.
    pub fn tcp_receive(
        &mut self,
        number: u32,
        into: &mut [u8],
        peek: bool,
    ) -> Result<usize, HostError> {
        let stream = self.streams.get_mut(&number).ok_or(HostError::Closed)?;
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

    pub fn tcp_finish(&mut self, number: u32) {
        if let Some(socket) = self
            .streams
            .get(&number)
            .and_then(|stream| stream.socket.as_ref())
        {
            let _ = socket.shutdown(Shutdown::Write);
        }
    }

    pub fn tcp_close(&mut self, number: u32) {
        self.streams.remove(&number);
        self.listeners.remove(&number);
    }

    /// Tells the next change a wait on TCP connection or listener `number` asks for.
    pub fn arm(&mut self, number: u32, interest: ReadyFor) {
        if let Some(stream) = self.streams.get_mut(&number) {
            stream.read_armed = interest.contains(ReadyFor::READABLE);
            stream.write_armed = interest.contains(ReadyFor::WRITABLE);
        } else if let Some(listener) = self.listeners.get_mut(&number) {
            listener.armed = interest.contains(ReadyFor::READABLE);
        }
    }

    /// Starts UDP flow `id` to an outside address.
    pub fn connect(&mut self, id: u32, address: &[u8], port: u16) -> Result<(), HostError> {
        let to = local(address, port);
        let bind = if to.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
        let socket =
            UdpSocket::bind(bind).map_err(|_| HostError::Errno(Errno::ENETUNREACH as i32))?;
        socket
            .connect(to)
            .map_err(|error| HostError::Errno(errno(&error)))?;
        socket.set_nonblocking(true).expect("nonblocking");
        let flow = Flow {
            socket,
            read_armed: true,
            write_armed: false,
        };
        self.flows.insert(id, flow);
        Ok(())
    }

    pub fn send(&mut self, id: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let flow = self.flows.get_mut(&id).ok_or(HostError::Closed)?;
        match flow.socket.send(bytes) {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                flow.write_armed = true;
                Err(HostError::WouldBlock)
            }
            Err(_) => Err(HostError::Closed),
        }
    }

    pub fn receive(&mut self, id: u32, into: &mut [u8]) -> Result<usize, HostError> {
        let flow = self.flows.get_mut(&id).ok_or(HostError::Closed)?;
        match flow.socket.recv(into) {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                flow.read_armed = true;
                Err(HostError::WouldBlock)
            }
            Err(_) => Err(HostError::Closed),
        }
    }

    pub fn close(&mut self, id: u32) {
        self.flows.remove(&id);
    }

    /// Looks up the name of question `number` with this machine's resolver; its answer is owed.
    pub fn resolve(&mut self, number: u64, question: Question<&Name<[u8]>>) {
        let name = question.qname().to_string();
        let found: Option<Vec<IpAddr>> = if name == DOCKER_HOST_NAME {
            Some(vec![IpAddr::V4(DOCKER_HOST)])
        } else {
            (name.as_str(), 0)
                .to_socket_addrs()
                .ok()
                .map(|addresses| addresses.map(|address| address.ip()).collect())
        };
        let answer =
            wcontainers_runtime_single_kernel::names::address_answer(question, found.as_deref());
        self.owed.push_back(Owed::Answered(number, answer));
    }

    /// Whether anything may still come: an event owed, or a flow, connection or listener waiting
    /// for a change.
    pub fn waiting(&self) -> bool {
        !self.owed.is_empty()
            || self
                .flows
                .values()
                .any(|flow| flow.read_armed || flow.write_armed)
            || self
                .streams
                .values()
                .any(|stream| stream.socket.is_some() && (stream.read_armed || stream.write_armed))
            || self.listeners.values().any(|listener| listener.armed)
    }

    /// The events owed, after waiting up to `timeout` for a socket to change when none is.
    pub fn take(&mut self, timeout: Duration) -> Vec<Owed> {
        /// What a socket watched is.
        #[derive(Clone, Copy)]
        enum Watched {
            Flow(u32),
            Stream(u32),
            Listener(u32),
        }
        let events = |read: bool, write: bool| {
            let mut events = 0;
            if read {
                events |= libc::POLLIN;
            }
            if write {
                events |= libc::POLLOUT;
            }
            events
        };
        let mut armed: Vec<(Watched, i32, i16)> = self
            .flows
            .iter()
            .filter(|(_, flow)| flow.read_armed || flow.write_armed)
            .map(|(&id, flow)| {
                let wanted = events(flow.read_armed, flow.write_armed);
                (Watched::Flow(id), flow.socket.as_raw_fd(), wanted)
            })
            .collect();
        armed.extend(self.streams.iter().filter_map(|(&number, stream)| {
            let socket = stream.socket.as_ref()?;
            let wanted = events(stream.read_armed, stream.write_armed);
            (wanted != 0).then(|| (Watched::Stream(number), socket.as_raw_fd(), wanted))
        }));
        armed.extend(
            self.listeners
                .iter()
                .filter(|(_, listener)| listener.armed)
                .map(|(&number, listener)| {
                    let watched = Watched::Listener(number);
                    (watched, listener.socket.as_raw_fd(), libc::POLLIN)
                }),
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
        for (&(watched, _, _), fd) in armed.iter().zip(&fds) {
            let revents = fd.revents;
            if revents == 0 {
                continue;
            }
            match watched {
                Watched::Listener(number) => {
                    if let Some(listener) = self.listeners.get_mut(&number) {
                        listener.armed = false;
                    }
                    self.owed
                        .push_back(Owed::TcpReady(number, ReadyFor::READABLE));
                }
                Watched::Stream(number) => {
                    let stream = self.streams.get_mut(&number).expect("an armed connection");
                    stream.read_armed = false;
                    stream.write_armed = false;
                    let mut ready = ReadyFor::empty();
                    for (bit, flag) in [
                        (libc::POLLIN, ReadyFor::READABLE),
                        (libc::POLLOUT, ReadyFor::WRITABLE),
                        (libc::POLLERR, ReadyFor::ERROR),
                        (libc::POLLHUP, ReadyFor::HANG_UP),
                    ] {
                        if revents & bit != 0 {
                            ready |= flag;
                        }
                    }
                    self.owed.push_back(Owed::TcpReady(number, ready));
                }
                Watched::Flow(id) => {
                    let flow = self.flows.get_mut(&id).expect("an armed flow");
                    if revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                        flow.read_armed = false;
                    }
                    if revents & (libc::POLLOUT | libc::POLLHUP | libc::POLLERR) != 0 {
                        flow.write_armed = false;
                    }
                    self.owed.push_back(Owed::Ready(id));
                }
            }
        }
        self.owed.drain(..).collect()
    }
}
