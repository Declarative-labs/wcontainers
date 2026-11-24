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
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Duration;

use domain::base::{Name, Question};
use rustix::event::{PollFd, PollFlags, Timespec};
use wcontainers_abi::HostError;
use wcontainers_kernel::errno::Errno;
use wcontainers_readiness::{ReadinessReport, ReadyFor, UnconditionalEvents};

/// Docker Desktop's name and address for the machine it runs on.
pub const DOCKER_HOST_NAME: &str = "host.docker.internal";
pub const DOCKER_HOST: Ipv4Addr = Ipv4Addr::new(192, 168, 65, 254);

/// A UDP socket, by this host's number of it, connected to one outside address.
struct Flow {
    socket: UdpSocket,
    /// The address it is connected to, as the guest names it.
    peer: SocketAddr,
    /// A wait is kept, which hears an error or hang-up, and what more it asks for.
    armed: bool,
    read_armed: bool,
    write_armed: bool,
}

/// A TCP connection, by this host's number of it.
struct Stream {
    /// None when the connect failed.
    socket: Option<TcpStream>,
    /// The error the connect failed with, given to the next receive.
    error: Option<i32>,
    /// A wait is kept, which hears an error or hang-up, and what more it asks for.
    armed: bool,
    read_armed: bool,
    write_armed: bool,
}

/// What is owed to the kernel's waits: the answer to the name question of a number, or what a
/// socket of a number became ready for.
pub enum Owed {
    Answered(u64, Vec<u8>),
    Ready(u32, ReadinessReport),
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

fn errno(error: &std::io::Error) -> i32 {
    match error.kind() {
        ErrorKind::ConnectionRefused => Errno::ECONNREFUSED as i32,
        ErrorKind::TimedOut => Errno::ETIMEDOUT as i32,
        ErrorKind::NetworkUnreachable => Errno::ENETUNREACH as i32,
        ErrorKind::HostUnreachable => Errno::EHOSTUNREACH as i32,
        _ => Errno::ECONNREFUSED as i32,
    }
}

/// The unspecified address of an address's family, port 0, for a local address the machine
/// chooses.
fn unspecified_of(address: SocketAddr) -> SocketAddr {
    match address {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), u16::MIN),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), u16::MIN),
    }
}

/// The events `poll` asks of a socket whose wait asks to read, to write, or neither (an error or
/// hang-up is always told).
fn asked(read: bool, write: bool) -> PollFlags {
    let mut events = PollFlags::empty();
    if read {
        events |= PollFlags::IN;
    }
    if write {
        events |= PollFlags::OUT;
    }
    events
}

/// What a socket is ready for, from the events `poll` gave for it.
fn ready_for(revents: PollFlags) -> ReadinessReport {
    let mut ready = report(ReadyFor::empty(), UnconditionalEvents::empty());
    for (bit, flag) in [
        (PollFlags::IN, ReadyFor::READABLE),
        (PollFlags::OUT, ReadyFor::WRITABLE),
    ] {
        if revents.contains(bit) {
            ready.ready_for |= flag;
        }
    }
    for (bit, flag) in [
        (PollFlags::ERR, UnconditionalEvents::ERROR),
        (PollFlags::HUP, UnconditionalEvents::HANG_UP),
    ] {
        if revents.contains(bit) {
            ready.unconditional |= flag;
        }
    }
    ready
}

/// A readiness report of what a handle is ready for and its unconditional events.
pub(crate) fn report(ready_for: ReadyFor, unconditional: UnconditionalEvents) -> ReadinessReport {
    ReadinessReport {
        ready_for,
        unconditional,
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
                    armed: false,
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
            armed: false,
            read_armed: false,
            write_armed: false,
        };
        let ready = match TcpStream::connect_timeout(&seen_here(to), CONNECT_PATIENCE) {
            Ok(socket) => {
                socket.set_nonblocking(true).expect("nonblocking");
                stream.socket = Some(socket);
                report(ReadyFor::WRITABLE, UnconditionalEvents::empty())
            }
            Err(error) => {
                stream.error = Some(errno(&error));
                report(
                    ReadyFor::empty(),
                    UnconditionalEvents::ERROR | UnconditionalEvents::HANG_UP,
                )
            }
        };
        self.streams.insert(number, stream);
        self.owed.push_back(Owed::Ready(number, ready));
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

    /// Closes TCP connection `number` with a reset, as a close lingering for no time does.
    pub fn tcp_reset(&mut self, number: u32) {
        let Some(socket) = self
            .streams
            .remove(&number)
            .and_then(|stream| stream.socket)
        else {
            return;
        };
        let _ = socket2::SockRef::from(&socket).set_linger(Some(Duration::ZERO));
    }

    /// Tells the next change a wait on socket `number` asks for, or an error or hang-up, which
    /// every wait hears.
    pub fn arm(&mut self, number: u32, interest: ReadyFor) {
        if let Some(flow) = self.flows.get_mut(&number) {
            flow.armed = true;
            flow.read_armed = interest.contains(ReadyFor::READABLE);
            flow.write_armed = interest.contains(ReadyFor::WRITABLE);
        } else if let Some(stream) = self.streams.get_mut(&number) {
            stream.armed = true;
            stream.read_armed = interest.contains(ReadyFor::READABLE);
            stream.write_armed = interest.contains(ReadyFor::WRITABLE);
        } else if let Some(listener) = self.listeners.get_mut(&number) {
            listener.armed = interest.contains(ReadyFor::READABLE);
        }
    }

    /// Opens UDP socket `number` connected to `to`, an outside address as the guest names it.
    pub fn udp_connect(&mut self, number: u32, to: SocketAddr) -> Result<(), HostError> {
        let here = seen_here(to);
        let any = unspecified_of(here);
        let socket =
            UdpSocket::bind(any).map_err(|_| HostError::Errno(Errno::ENETUNREACH as i32))?;
        socket
            .connect(here)
            .map_err(|error| HostError::Errno(errno(&error)))?;
        socket.set_nonblocking(true).expect("nonblocking");
        let flow = Flow {
            socket,
            peer: to,
            armed: false,
            read_armed: false,
            write_armed: false,
        };
        self.flows.insert(number, flow);
        Ok(())
    }

    pub fn udp_send(&mut self, number: u32, bytes: &[u8]) -> Result<usize, HostError> {
        let flow = self.flows.get_mut(&number).ok_or(HostError::Closed)?;
        match flow.socket.send(bytes) {
            Ok(count) => Ok(count),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
            Err(_) => Err(HostError::Closed),
        }
    }

    /// Receives one datagram as the whole packet from its IP header; the packet's length.
    pub fn udp_receive(&mut self, number: u32, into: &mut [u8]) -> Result<usize, HostError> {
        let flow = self.flows.get_mut(&number).ok_or(HostError::Closed)?;
        let local = flow
            .socket
            .local_addr()
            .unwrap_or_else(|_| unspecified_of(flow.peer));
        wcontainers_runtime_single_kernel::udp::receive_packet(flow.peer, local, into, |payload| {
            match flow.socket.recv(payload) {
                Ok(count) => Ok(count),
                Err(error) if error.kind() == ErrorKind::WouldBlock => Err(HostError::WouldBlock),
                Err(_) => Err(HostError::Closed),
            }
        })
    }

    pub fn udp_close(&mut self, number: u32) {
        self.flows.remove(&number);
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

    /// Whether anything may still come: an answer or readiness owed, or a socket waiting for a
    /// change.
    pub fn waiting(&self) -> bool {
        !self.owed.is_empty()
            || self.flows.values().any(|flow| flow.armed)
            || self
                .streams
                .values()
                .any(|stream| stream.socket.is_some() && stream.armed)
            || self.listeners.values().any(|listener| listener.armed)
    }

    /// What socket `number`'s kept wait would hear now, asked of the socket without waiting, as a
    /// host's wait asks when polled; the watch then ends, as a report of [`Self::take`] ends it.
    pub fn ready_now(&mut self, number: u32) -> Option<ReadinessReport> {
        let (fd, wanted) = if let Some(flow) = self.flows.get(&number).filter(|flow| flow.armed) {
            let wanted = asked(flow.read_armed, flow.write_armed);
            (flow.socket.as_fd(), wanted)
        } else if let Some(stream) = self.streams.get(&number).filter(|stream| stream.armed) {
            let wanted = asked(stream.read_armed, stream.write_armed);
            (stream.socket.as_ref()?.as_fd(), wanted)
        } else {
            let listener = self
                .listeners
                .get(&number)
                .filter(|listener| listener.armed)?;
            (listener.socket.as_fd(), PollFlags::IN)
        };
        let mut fds = [PollFd::from_borrowed_fd(fd, wanted)];
        let _ = rustix::event::poll(&mut fds, Some(&Timespec::default()));
        let revents = fds[0].revents();
        if revents.is_empty() {
            return None;
        }
        if let Some(flow) = self.flows.get_mut(&number) {
            (flow.armed, flow.read_armed, flow.write_armed) = (false, false, false);
            return Some(ready_for(revents));
        }
        if let Some(stream) = self.streams.get_mut(&number) {
            (stream.armed, stream.read_armed, stream.write_armed) = (false, false, false);
            return Some(ready_for(revents));
        }
        let listener = self.listeners.get_mut(&number)?;
        listener.armed = false;
        Some(report(ReadyFor::READABLE, UnconditionalEvents::empty()))
    }

    /// What is owed, after waiting up to `timeout` for a socket to change when nothing is.
    pub fn take(&mut self, timeout: Duration) -> Vec<Owed> {
        /// What a socket watched is.
        #[derive(Clone, Copy)]
        enum Watched {
            Flow(u32),
            Stream(u32),
            Listener(u32),
        }
        let mut armed: Vec<(Watched, BorrowedFd<'_>, PollFlags)> = self
            .flows
            .iter()
            .filter(|(_, flow)| flow.armed)
            .map(|(&id, flow)| {
                let wanted = asked(flow.read_armed, flow.write_armed);
                (Watched::Flow(id), flow.socket.as_fd(), wanted)
            })
            .collect();
        armed.extend(self.streams.iter().filter_map(|(&number, stream)| {
            let socket = stream.socket.as_ref().filter(|_| stream.armed)?;
            let wanted = asked(stream.read_armed, stream.write_armed);
            Some((Watched::Stream(number), socket.as_fd(), wanted))
        }));
        armed.extend(
            self.listeners
                .iter()
                .filter(|(_, listener)| listener.armed)
                .map(|(&number, listener)| {
                    let watched = Watched::Listener(number);
                    (watched, listener.socket.as_fd(), PollFlags::IN)
                }),
        );
        let mut fds: Vec<PollFd<'_>> = armed
            .iter()
            .map(|&(_, fd, events)| PollFd::from_borrowed_fd(fd, events))
            .collect();
        // Whole milliseconds, as a poll in milliseconds waits.
        let wait = if self.owed.is_empty() {
            Duration::from_millis(timeout.as_millis().min(i32::MAX as u128) as u64)
        } else {
            Duration::ZERO
        };
        if !fds.is_empty() {
            let wait = Timespec::try_from(wait).expect("a wait of at most a few weeks");
            // A wait cut short by a signal reports nothing, and is asked again.
            let _ = rustix::event::poll(&mut fds, Some(&wait));
        }
        let changed: Vec<(Watched, PollFlags)> = armed
            .iter()
            .zip(&fds)
            .map(|(&(watched, _, _), fd)| (watched, fd.revents()))
            .filter(|(_, revents)| !revents.is_empty())
            .collect();
        drop(fds);
        drop(armed);
        for (watched, revents) in changed {
            match watched {
                Watched::Listener(number) => {
                    if let Some(listener) = self.listeners.get_mut(&number) {
                        listener.armed = false;
                    }
                    self.owed.push_back(Owed::Ready(
                        number,
                        report(ReadyFor::READABLE, UnconditionalEvents::empty()),
                    ));
                }
                Watched::Stream(number) => {
                    let stream = self.streams.get_mut(&number).expect("an armed connection");
                    stream.armed = false;
                    stream.read_armed = false;
                    stream.write_armed = false;
                    self.owed.push_back(Owed::Ready(number, ready_for(revents)));
                }
                Watched::Flow(number) => {
                    let flow = self.flows.get_mut(&number).expect("an armed flow");
                    flow.armed = false;
                    flow.read_armed = false;
                    flow.write_armed = false;
                    self.owed.push_back(Owed::Ready(number, ready_for(revents)));
                }
            }
        }
        self.owed.drain(..).collect()
    }
}
