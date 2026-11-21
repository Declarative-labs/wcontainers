//! Internet sockets whose other end is outside the container, carried by the host's network hooks
//! (plan section 9): a TCP connection is one host connection, a UDP socket has one host
//! connection for each outside destination it sends to. Bytes that arrive are pulled into the
//! socket's queue when the host says they are there, so readiness is the kernel's to give. Name
//! queries sent to the container's name server are answered here through the host's lookup.

use alloc::collections::BTreeMap;
use alloc::rc::{Rc, Weak};
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use wcontainers_abi::{ConnectionId, HostError};

use super::inet::{Endpoint, Protocol, mapped, v4_of};
use super::{Packet, RCV_SHUTDOWN, SHUTDOWN_MASK, Socket, SocketRef, State, channel};
use crate::linux::general::*;
use crate::linux::net::{IP_RECVERR, IPV6_RECVERR, SOL_IP, SOL_IPV6};
use crate::{Context, Errno, Kernel, network};

/// The most bytes pulled from the host at once.
const PIECE: usize = 65536;

/// The connections to the outside and the name lookups under way.
#[derive(Default)]
pub(crate) struct Outside {
    last: u32,
    /// Each connection's socket, and for UDP the destination it carries.
    connections: BTreeMap<u32, (Weak<RefCell<Socket>>, Option<Endpoint>)>,
    /// Listeners the host accepts connections for from outside.
    listeners: BTreeMap<u32, Weak<RefCell<Socket>>>,
    last_lookup: u64,
    lookups: BTreeMap<u64, Lookup>,
    /// Connections let go where no host was at hand, closed at the next flush.
    closing: Vec<u32>,
}

/// A name query waiting for the host's lookup.
struct Lookup {
    socket: Weak<RefCell<Socket>>,
    /// The server's address it asked at.
    server: Endpoint,
    query: Vec<u8>,
    name: String,
}

/// The address bytes the host is given: 4 for IPv4, else 16.
fn host_address(endpoint: &Endpoint) -> Vec<u8> {
    match v4_of(&endpoint.address) {
        Some(v4) => v4.to_vec(),
        None => endpoint.address.to_vec(),
    }
}

/// A host's refusal as the error the guest sees: a host that cannot reach outside is a network
/// without a route, which is counted.
fn refusal<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    error: HostError,
) -> Errno {
    match error {
        HostError::Errno(number) => Errno(number as u32),
        HostError::Closed => Errno::ECONNREFUSED,
        HostError::WouldBlock => Errno::EAGAIN,
        HostError::NotSupported => {
            kernel
                .counters
                .count(cx.host, crate::reason::HOST_CANNOT, 0);
            Errno::ENETUNREACH
        }
    }
}

/// A number for a new host connection or listener.
fn next_id(outside: &mut Outside) -> u32 {
    outside.last = outside.last.wrapping_add(1).max(1);
    outside.last
}

/// Starts a host connection for `socket` to `to`; its number.
pub(super) fn open<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    to: Endpoint,
    stream: bool,
) -> Result<u32, Errno> {
    let id = next_id(&mut kernel.sockets.inet.outside);
    let address = host_address(&to);
    if let Err(error) = cx.host.connect(ConnectionId(id), &address, to.port, stream) {
        return Err(refusal(kernel, cx, error));
    }
    let flow = (!stream).then_some(to);
    kernel
        .sockets
        .inet
        .outside
        .connections
        .insert(id, (Rc::downgrade(socket), flow));
    Ok(id)
}

/// Ends every host connection of a socket whose last descriptor went.
pub(super) fn released(kernel: &mut Kernel, socket: &SocketRef) {
    let ids: Vec<u32> = kernel
        .sockets
        .inet
        .outside
        .connections
        .iter()
        .filter(|(_, (owner, _))| core::ptr::eq(owner.as_ptr(), Rc::as_ptr(socket)))
        .map(|(&id, _)| id)
        .collect();
    for id in ids {
        kernel.sockets.inet.outside.connections.remove(&id);
        kernel.sockets.inet.outside.closing.push(id);
    }
    stop_listening(kernel, socket);
}

/// Asks the host to accept connections from outside for a listener on its port. A host that
/// cannot (or whose port is taken) leaves it listening inside the container alone, counted.
pub(super) fn listen<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
) {
    let id = next_id(&mut kernel.sockets.inet.outside);
    if ask_listen(kernel, cx, socket, id) {
        let outside = &mut kernel.sockets.inet.outside;
        outside.listeners.insert(id, Rc::downgrade(socket));
    }
}

/// The host's `listen` for a listener's port; whether it took it.
fn ask_listen<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    id: u32,
) -> bool {
    let port = socket
        .borrow()
        .inet
        .as_deref()
        .map_or(0, |inet| inet.local.port);
    match cx.host.listen(ConnectionId(id), port) {
        Ok(()) => true,
        Err(_) => {
            kernel.counters.count(
                cx.host,
                crate::reason::OUTSIDE_LISTEN_FAILED,
                u64::from(port),
            );
            false
        }
    }
}

/// A listener stopped listening: the host stops accepting for it.
pub(super) fn stop_listening(kernel: &mut Kernel, socket: &SocketRef) {
    let outside = &mut kernel.sockets.inet.outside;
    let ids: Vec<u32> = outside
        .listeners
        .iter()
        .filter(|(_, owner)| core::ptr::eq(owner.as_ptr(), Rc::as_ptr(socket)))
        .map(|(&id, _)| id)
        .collect();
    for id in ids {
        outside.listeners.remove(&id);
        outside.closing.push(id);
    }
}

/// Takes the connections waiting at the host for a listener into its backlog while it has room.
/// Each is the listener's end of a connection from the gateway (as Docker's published ports show
/// it), at eth0's address.
pub(super) fn take<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    listener: &SocketRef,
) {
    let Some(listener_id) = kernel
        .sockets
        .inet
        .outside
        .listeners
        .iter()
        .find(|(_, owner)| core::ptr::eq(owner.as_ptr(), Rc::as_ptr(listener)))
        .map(|(&id, _)| id)
    else {
        return;
    };
    while !super::inet::backlog_full(&listener.borrow()) {
        let id = next_id(&mut kernel.sockets.inet.outside);
        match cx.host.accept(ConnectionId(listener_id), ConnectionId(id)) {
            Ok(()) => {}
            Err(HostError::WouldBlock) => return,
            Err(_) => {
                let port = listener
                    .borrow()
                    .inet
                    .as_deref()
                    .map_or(0, |inet| inet.local.port);
                kernel.counters.count(
                    cx.host,
                    crate::reason::OUTSIDE_LISTEN_FAILED,
                    u64::from(port),
                );
                return;
            }
        }
        let mut port = [0u8; 2];
        cx.host.random(&mut port);
        let (low, high) = (
            *super::inet::EPHEMERAL.start(),
            *super::inet::EPHEMERAL.end(),
        );
        let remote = Endpoint {
            address: mapped(network::GATEWAY),
            port: low + u16::from_le_bytes(port) % (high - low + 1),
        };
        if super::inet::deferral_seconds(&listener.borrow()) > 0 {
            kernel
                .counters
                .count(cx.host, crate::reason::OUTSIDE_NOT_DEFERRED, 0);
        }
        let embryo = super::inet::embryo(kernel, listener, mapped(network::ADDRESS), remote);
        if let Some(inet) = embryo.borrow_mut().inet.as_deref_mut() {
            inet.outside = Some(id);
            inet.room = true;
        }
        super::inet::made_outside(kernel, &embryo, false);
        kernel
            .sockets
            .inet
            .outside
            .connections
            .insert(id, (Rc::downgrade(&embryo), None));
        pull(kernel, cx, &embryo, id);
        listener.borrow_mut().backlog.push_back(embryo);
        super::data_ready(kernel, cx, listener, EPOLLIN | EPOLLRDNORM);
    }
}

impl Outside {
    /// Hands the host the connections released since the last call.
    pub(crate) fn flush<H: wcontainers_abi::Host>(&mut self, host: &H) {
        for id in core::mem::take(&mut self.closing) {
            host.close(ConnectionId(id));
        }
    }
}

/// Pulls what the host has for a connection into its socket's queue, up to its receive buffer:
/// bytes or datagrams, the end of a stream, or a reset.
pub(super) fn pull<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    id: u32,
) {
    let Some(&(_, flow)) = kernel.sockets.inet.outside.connections.get(&id) else {
        return;
    };
    let stream = flow.is_none();
    loop {
        let (held, room) = {
            let own = socket.borrow();
            let held: u64 = own.queue.iter().map(|packet| packet.truesize).sum();
            (held, own.rcvbuf)
        };
        if stream && held >= room {
            return;
        }
        if stream && socket.borrow().shutdown & RCV_SHUTDOWN != 0 {
            return;
        }
        let size = if stream {
            ((room - held) as usize).min(PIECE)
        } else {
            PIECE
        };
        let mut buffer = alloc::vec![0u8; size];
        match cx.host.receive(ConnectionId(id), &mut buffer) {
            Ok(0) if stream => {
                super::inet::carry_outside(kernel, socket, 0, true, false);
                let mut own = socket.borrow_mut();
                own.shutdown |= RCV_SHUTDOWN;
                if let Some(inet) = own.inet.as_deref_mut() {
                    inet.done = true;
                }
                let ended = own.shutdown == SHUTDOWN_MASK;
                drop(own);
                kernel.wake_poll(channel(socket), EPOLLIN | EPOLLRDNORM | EPOLLRDHUP);
                // `tcp_fin` tells the end, then `tcp_data_ready` the data.
                let reason = if ended { POLL_HUP } else { POLL_IN };
                super::wake_async(kernel, cx, socket, super::Wake::Data, reason);
                super::data_ready(kernel, cx, socket, EPOLLIN | EPOLLRDNORM);
                return;
            }
            Ok(count) => {
                buffer.truncate(count);
                let truesize = if stream {
                    count as u64
                } else {
                    super::inet::datagram_truesize(count as u64)
                };
                if !stream && held > room {
                    continue;
                }
                let packet = Packet {
                    data: buffer,
                    consumed: 0,
                    files: Vec::new(),
                    cred: super::Cred {
                        pid: 0,
                        uid: 65534,
                        gid: 65534,
                    },
                    sender: Weak::new(),
                    truesize,
                    source: flow,
                    arrival: (!stream)
                        .then(|| super::inet::prepared(socket, super::inet::arrival_outside())),
                    time: super::stamp(kernel, cx, socket),
                    urgent: false,
                    owner: 0,
                };
                if stream {
                    super::inet::carry_outside(kernel, socket, count as u64, false, false);
                }
                let mut own = socket.borrow_mut();
                if let Some(inet) = own.inet.as_deref_mut() {
                    inet.received = true;
                }
                own.queue.push_back(packet);
                drop(own);
                super::data_ready(kernel, cx, socket, EPOLLIN | EPOLLRDNORM);
            }
            Err(HostError::WouldBlock) | Err(HostError::NotSupported) => return,
            Err(HostError::Closed) | Err(HostError::Errno(_)) if !stream => {
                // The destination refused: a socket connected to it hears of it.
                let connected_to = {
                    let own = socket.borrow();
                    (own.state == State::Connected)
                        .then(|| own.inet.as_deref().and_then(|inet| inet.remote))
                        .flatten()
                };
                let told = connected_to == flow || unqueued_error(kernel, cx, socket, flow);
                if told {
                    socket.borrow_mut().error = Errno::ECONNREFUSED.0 as i32;
                }
                forget_flow(kernel, socket, id);
                if told {
                    super::error_report(kernel, cx, socket);
                } else {
                    kernel.wake_poll(channel(socket), EPOLLERR);
                }
                return;
            }
            Err(HostError::Closed) => {
                reset(kernel, cx, socket, Errno::ECONNRESET);
                return;
            }
            Err(HostError::Errno(number)) => {
                reset(kernel, cx, socket, Errno(number as u32));
                return;
            }
        }
    }
}

/// A UDP socket's host connection to one destination is gone.
fn forget_flow(kernel: &mut Kernel, socket: &SocketRef, id: u32) {
    kernel.sockets.inet.outside.connections.remove(&id);
    kernel.sockets.inet.outside.closing.push(id);
    if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
        inet.flows.retain(|_, flow| *flow != id);
    }
}

/// Whether a socket that asked for queued errors (IP_RECVERR or IPV6_RECVERR for the way the
/// datagram went) hears of an outside destination's refusal; the host does not say what the
/// refused datagram held, so nothing is queued, which is counted.
fn unqueued_error<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    flow: Option<Endpoint>,
) -> bool {
    let v4 = flow.is_some_and(|to| v4_of(&to.address).is_some());
    let (level, name) = if v4 {
        (SOL_IP, IP_RECVERR)
    } else {
        (SOL_IPV6, IPV6_RECVERR)
    };
    let asked = socket
        .borrow()
        .options
        .get(&(level, name))
        .is_some_and(|value| value.iter().any(|&byte| byte != 0));
    if asked {
        kernel
            .counters
            .count(cx.host, crate::reason::OUTSIDE_ERROR_NOT_QUEUED, 0);
    }
    asked
}

/// A TCP connection to the outside was reset or failed: closed, with the error to report.
fn reset<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    error: Errno,
) {
    {
        let mut own = socket.borrow_mut();
        let close_wait = own.shutdown & RCV_SHUTDOWN != 0 && own.state == State::Connected;
        own.error = if error == Errno::ECONNRESET && close_wait {
            Errno::EPIPE.0 as i32
        } else {
            error.0 as i32
        };
        own.state = State::Closed;
        own.shutdown = SHUTDOWN_MASK;
        if let Some(inet) = own.inet.as_deref_mut() {
            inet.connecting = None;
            inet.room = false;
        }
    }
    kernel.wake(channel(socket));
    super::wake_async(kernel, cx, socket, super::Wake::Io, POLL_ERR);
}

/// Sends bytes of a TCP connection to the outside, as much as the host takes now.
pub(super) fn send<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    id: u32,
    bytes: &[u8],
) -> Result<usize, Errno> {
    match cx.host.send(ConnectionId(id), bytes) {
        Ok(count) => {
            if count < bytes.len()
                && let Some(inet) = socket.borrow_mut().inet.as_deref_mut()
            {
                inet.room = false;
            }
            Ok(count)
        }
        Err(HostError::WouldBlock) => {
            if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
                inet.room = false;
            }
            Ok(0)
        }
        Err(HostError::Closed) => {
            reset(kernel, cx, socket, Errno::ECONNRESET);
            Err(Errno::ECONNRESET)
        }
        Err(error) => Err(refusal(kernel, cx, error)),
    }
}

/// Sends one datagram to an outside destination through the socket's host connection for it.
pub(super) fn send_datagram<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    to: Endpoint,
    data: &[u8],
) -> Result<(), Errno> {
    let known = socket
        .borrow()
        .inet
        .as_deref()
        .and_then(|inet| inet.flows.get(&to).copied());
    let id = match known {
        Some(id) => id,
        None => {
            let id = open(kernel, cx, socket, to, false)?;
            if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
                inet.flows.insert(to, id);
            }
            id
        }
    };
    match cx.host.send(ConnectionId(id), data) {
        Ok(_) => Ok(()),
        Err(HostError::WouldBlock) => Err(Errno::EAGAIN),
        Err(HostError::Closed) => {
            forget_flow(kernel, socket, id);
            Err(Errno::ECONNREFUSED)
        }
        Err(error) => Err(refusal(kernel, cx, error)),
    }
}

/// Finishes sending on a TCP connection to the outside.
pub(super) fn finish<S: crate::System>(cx: &mut Context<'_, S>, id: u32) {
    cx.host.finish(ConnectionId(id));
}

/// Hands an event about a connection or a lookup to its socket. Returns false for one nothing
/// waits for.
pub(crate) fn event<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    event: &wcontainers_abi::Event<'_>,
) -> bool {
    use wcontainers_abi::Event;
    match *event {
        Event::Connected { connection, error } => {
            let Some(socket) = owner(kernel, connection.0) else {
                return false;
            };
            let stream = inet_protocol(&socket) == Some(Protocol::Tcp);
            if !stream {
                if error != 0 {
                    let flow = kernel.sockets.inet.outside.connections[&connection.0].1;
                    let connected_to = socket.borrow().inet.as_deref().and_then(|inet| inet.remote);
                    let told = (connected_to == flow && socket.borrow().state == State::Connected)
                        || unqueued_error(kernel, cx, &socket, flow);
                    if told {
                        socket.borrow_mut().error = error;
                    }
                    forget_flow(kernel, &socket, connection.0);
                    if told {
                        super::error_report(kernel, cx, &socket);
                    } else {
                        kernel.wake_poll(channel(&socket), EPOLLERR);
                    }
                }
                return true;
            }
            if error == 0 {
                let mut own = socket.borrow_mut();
                own.state = State::Connected;
                own.error = 0;
                let inet = own.inet.as_deref_mut().expect("an internet socket");
                inet.connecting = None;
                inet.room = true;
                drop(own);
                super::inet::made_outside(kernel, &socket, true);
                kernel.wake_poll(channel(&socket), EPOLLOUT | EPOLLWRNORM);
                super::wake_async(kernel, cx, &socket, super::Wake::Io, POLL_OUT);
            } else {
                kernel
                    .sockets
                    .inet
                    .outside
                    .connections
                    .remove(&connection.0);
                cx.host.close(connection);
                super::inet::failed(kernel, cx, &socket, Errno(error as u32));
            }
            true
        }
        Event::ConnectionReady { connection } => {
            let listener = kernel
                .sockets
                .inet
                .outside
                .listeners
                .get(&connection.0)
                .and_then(Weak::upgrade);
            if let Some(listener) = listener {
                take(kernel, cx, &listener);
                return true;
            }
            let Some(socket) = owner(kernel, connection.0) else {
                return false;
            };
            if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
                inet.room = true;
            }
            pull(kernel, cx, &socket, connection.0);
            super::write_space(kernel, cx, &socket);
            true
        }
        Event::LookedUp {
            request,
            addresses,
            error,
        } => {
            let Some(lookup) = kernel.sockets.inet.outside.lookups.remove(&request) else {
                return false;
            };
            if let Some(socket) = lookup.socket.upgrade() {
                let answer = super::resolver::answer(&lookup.query, addresses, error);
                deliver(kernel, cx, &socket, lookup.server, answer);
            }
            true
        }
        _ => false,
    }
}

fn owner(kernel: &Kernel, id: u32) -> Option<SocketRef> {
    let (socket, _) = kernel.sockets.inet.outside.connections.get(&id)?;
    socket.upgrade().filter(|socket| !socket.borrow().dead)
}

fn inet_protocol(socket: &SocketRef) -> Option<Protocol> {
    socket.borrow().inet.as_deref().map(|inet| inet.protocol)
}

/// A datagram from `from` into a UDP socket's queue, if its buffer has room.
pub(super) fn deliver<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    from: Endpoint,
    data: Vec<u8>,
) {
    let truesize = super::inet::datagram_truesize(data.len() as u64);
    let held: u64 = socket
        .borrow()
        .queue
        .iter()
        .map(|packet| packet.truesize)
        .sum();
    if held > socket.borrow().rcvbuf {
        return;
    }
    let packet = Packet {
        data,
        consumed: 0,
        files: Vec::new(),
        cred: super::Cred {
            pid: 0,
            uid: 65534,
            gid: 65534,
        },
        sender: Weak::new(),
        truesize,
        source: Some(from),
        // The container's name server answers on loopback; the host does not say how.
        arrival: Some(super::inet::prepared(
            socket,
            super::inet::Arrival {
                destination: super::inet::mapped([127, 0, 0, 1]),
                index: network::LOOPBACK_INDEX,
                ..super::inet::arrival_outside()
            },
        )),
        time: None,
        urgent: false,
        owner: 0,
    };
    socket.borrow_mut().queue.push_back(packet);
    super::data_ready(kernel, cx, socket, EPOLLIN | EPOLLRDNORM);
}

/// A name query sent to the container's name server: answered at once when it asks for nothing
/// the host can look up, else once the host's lookup comes back.
pub(super) fn query<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    server: Endpoint,
    data: &[u8],
) {
    match super::resolver::question(data) {
        super::resolver::Question::Answer(answer) => deliver(kernel, cx, socket, server, answer),
        super::resolver::Question::Unanswerable(answer, kind) => {
            kernel.counters.count(
                cx.host,
                crate::reason::NAME_QUERY_NOT_ANSWERED,
                u64::from(kind),
            );
            deliver(kernel, cx, socket, server, answer);
        }
        super::resolver::Question::LookUp(name) => {
            let outside = &mut kernel.sockets.inet.outside;
            outside.last_lookup += 1;
            let request = outside.last_lookup;
            outside.lookups.insert(
                request,
                Lookup {
                    socket: Rc::downgrade(socket),
                    server,
                    query: data.to_vec(),
                    name: name.clone(),
                },
            );
            if let Err(error) = cx.host.look_up(request, &name) {
                kernel.sockets.inet.outside.lookups.remove(&request);
                let errno = refusal(kernel, cx, error);
                let answer = super::resolver::answer(data, &[], errno.0 as i32);
                deliver(kernel, cx, socket, server, answer);
            }
        }
        super::resolver::Question::Dropped => {}
    }
}

/// After a restore on a new host: connections to the outside are reported closed (plan section
/// 12), listeners listen again, and lookups under way are asked again.
pub(crate) fn restored<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>) {
    let listeners = core::mem::take(&mut kernel.sockets.inet.outside.listeners);
    for (id, socket) in listeners {
        if let Some(socket) = socket.upgrade().filter(|socket| !socket.borrow().dead)
            && ask_listen(kernel, cx, &socket, id)
        {
            kernel
                .sockets
                .inet
                .outside
                .listeners
                .insert(id, Rc::downgrade(&socket));
        }
    }
    let connections = core::mem::take(&mut kernel.sockets.inet.outside.connections);
    kernel.sockets.inet.outside.closing.clear();
    for (id, (socket, flow)) in connections {
        let Some(socket) = socket.upgrade() else {
            continue;
        };
        if flow.is_some() {
            if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
                inet.flows.retain(|_, held| *held != id);
            }
            continue;
        }
        let connecting = socket
            .borrow()
            .inet
            .as_deref()
            .is_some_and(|inet| inet.connecting.is_some());
        if connecting {
            super::inet::failed(kernel, cx, &socket, Errno::ECONNREFUSED);
        } else {
            reset(kernel, cx, &socket, Errno::ECONNRESET);
        }
    }
    let requests: Vec<(u64, String)> = kernel
        .sockets
        .inet
        .outside
        .lookups
        .iter()
        .map(|(&request, lookup)| (request, lookup.name.clone()))
        .collect();
    for (request, name) in requests {
        if cx.host.look_up(request, &name).is_err()
            && let Some(lookup) = kernel.sockets.inet.outside.lookups.remove(&request)
            && let Some(socket) = lookup.socket.upgrade()
        {
            let answer = super::resolver::answer(&lookup.query, &[], Errno::EAGAIN.0 as i32);
            deliver(kernel, cx, &socket, lookup.server, answer);
        }
    }
}
