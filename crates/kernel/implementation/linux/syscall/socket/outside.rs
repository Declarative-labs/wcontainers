//! Internet sockets whose other end is outside the container (plan section 9): a TCP connection
//! or listener is one of the system's, waited on through its readiness, and a UDP socket has one
//! host connection for each outside destination it sends to. Bytes that arrive are pulled into
//! the socket's queue when the system says they are there, so readiness is the kernel's to give.
//! Name queries sent to the container's name server are answered here through the system's
//! resolver.

use alloc::collections::BTreeMap;
use alloc::rc::{Rc, Weak};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::RefCell;
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use wcontainers_abi::{ConnectionId, HostError};
use wcontainers_internet_protocol::{
    BindAddress, Ipv6Families, LocalBinding, LocalPort, Port, PortChoice, PortRange, Report,
    TransmitReportKey, TransmitReportRequest,
};
use wcontainers_kernel::UserId;
use wcontainers_readiness::{Readiness, ReadyFor};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpClose, TcpConnections, TcpContinuation, TcpFastOpen, TcpListeners,
    TcpProtocol, TcpReceiveRequest, TcpRecordBoundary, TcpSendRequest, TcpShutdown, TcpUrgency,
};

use super::inet::{Endpoint, mapped, v4_of};
use super::{Packet, RCV_SHUTDOWN, SHUTDOWN_MASK, Socket, SocketRef, State, channel};
use crate::linux::general::*;
use crate::linux::net::{IP_RECVERR, IPV6_RECVERR, SOL_IP, SOL_IPV6};
use crate::waiting::{Answer, WaitFor, Waiting, waiting_call};
use crate::{Context, Errno, Kernel, network};

/// The most bytes pulled from the host at once.
const PIECE: usize = 65536;

/// A TCP connection or listener of the system.
enum Handle<C, L> {
    Connection(C),
    Listener(L),
}

type HandleOf<S> = Handle<<S as TcpConnections>::Connection, <S as TcpListeners>::Listener>;

/// One connection or listener to the outside.
struct Link {
    socket: Weak<RefCell<Socket>>,
    /// The destination a UDP flow carries; none for TCP.
    flow: Option<Endpoint>,
    /// The system's `HandleOf<S>`, kept without its type as the kernel is not generic over its
    /// system; none for a UDP flow, which the abi's host carries.
    handle: Option<Rc<dyn Any>>,
    /// What the wait kept for it asks; empty when none is kept.
    asked: ReadyFor,
    /// A receive or accept found nothing waiting.
    drained: bool,
    /// A TCP connect not finished yet.
    connecting: bool,
}

impl Link {
    fn new(socket: &SocketRef, flow: Option<Endpoint>, handle: Option<Rc<dyn Any>>) -> Self {
        Self {
            socket: Rc::downgrade(socket),
            flow,
            handle,
            asked: ReadyFor::empty(),
            drained: false,
            connecting: false,
        }
    }
}

/// What is let go and not yet handed back to the system.
enum Closing {
    Flow(u32),
    Handle(Rc<dyn Any>),
}

/// The connections to the outside and the name lookups under way.
#[derive(Default)]
pub(crate) struct Outside {
    last: u32,
    connections: BTreeMap<u32, Link>,
    /// Listeners the system accepts connections for from outside.
    listeners: BTreeMap<u32, Link>,
    last_lookup: u64,
    lookups: BTreeMap<u64, Lookup>,
    /// Connections let go where no system was at hand, closed at the next flush.
    closing: Vec<Closing>,
}

/// A name query waiting for the system's resolver, which is asked as a waiting call of its number.
struct Lookup {
    socket: Weak<RefCell<Socket>>,
    /// The server's address it asked at.
    server: Endpoint,
    query: Vec<u8>,
}

/// The address bytes the host is given: 4 for IPv4, else 16.
fn host_address(endpoint: &Endpoint) -> Vec<u8> {
    match v4_of(&endpoint.address) {
        Some(v4) => v4.to_vec(),
        None => endpoint.address.to_vec(),
    }
}

/// An endpoint as the system takes it.
fn socket_address(endpoint: &Endpoint) -> SocketAddr {
    match v4_of(&endpoint.address) {
        Some(v4) => SocketAddr::new(Ipv4Addr::from(v4).into(), endpoint.port),
        None => SocketAddr::new(Ipv6Addr::from(endpoint.address).into(), endpoint.port),
    }
}

/// The unspecified address of a family, as the system binds it.
fn any_address(v4: bool) -> BindAddress {
    if v4 {
        BindAddress::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        BindAddress::V6 {
            address: Ipv6Addr::UNSPECIFIED,
            families: Ipv6Families::DualStack,
        }
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

/// The system's handle kept for a link.
fn handle_of<S: crate::System>(handle: &Option<Rc<dyn Any>>) -> Option<&HandleOf<S>> {
    handle.as_deref()?.downcast_ref::<HandleOf<S>>()
}

/// The system's handle of connection `id`, if it is one.
fn connection_of<S: crate::System>(
    kernel: &Kernel,
    id: u32,
) -> Option<&<S as TcpConnections>::Connection> {
    let link = kernel.sockets.inet.outside.connections.get(&id)?;
    match handle_of::<S>(&link.handle)? {
        Handle::Connection(connection) => Some(connection),
        Handle::Listener(_) => None,
    }
}

/// Hands a let-go handle back to the system, once no wait holds it.
fn close_handle<S: crate::System>(system: &S, handle: Rc<dyn Any>) {
    let Ok(handle) = handle.downcast::<HandleOf<S>>() else {
        return;
    };
    match Rc::try_unwrap(handle) {
        Ok(Handle::Connection(connection)) => {
            TcpConnections::close(system, connection, TcpClose::Orderly);
        }
        Ok(Handle::Listener(listener)) => TcpListeners::stop_listening(system, listener),
        Err(_) => debug_assert!(false, "a handle let go while a wait holds it"),
    }
}

/// Starts a host connection for a UDP `socket` to `to`; its number.
fn open_flow<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    to: Endpoint,
) -> Result<u32, Errno> {
    let id = next_id(&mut kernel.sockets.inet.outside);
    let address = host_address(&to);
    if let Err(error) = wcontainers_abi::Host::connect(cx.host, ConnectionId(id), &address, to.port)
    {
        return Err(refusal(kernel, cx, error));
    }
    let link = Link::new(socket, Some(to), None);
    kernel.sockets.inet.outside.connections.insert(id, link);
    Ok(id)
}

/// Starts the system's TCP connection for `socket` to `to`; its number. Its first wait starts
/// when the kernel next settles, so the connect's call is over before its answer is taken.
pub(super) fn open<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    to: Endpoint,
) -> Result<u32, Errno> {
    let v4 = v4_of(&to.address).is_some();
    let (first, last) = (
        *super::inet::EPHEMERAL.start(),
        *super::inet::EPHEMERAL.end(),
    );
    let local = LocalBinding {
        address: any_address(v4),
        port: LocalPort::ChosenByExitNode(PortChoice {
            range: PortRange {
                first: Port(first),
                last: Port(last),
            },
            reserved: &[],
        }),
    };
    let owner = UserId(socket.borrow().uid);
    let connected = TcpConnections::connect(
        &**cx.system,
        TcpProtocol::Tcp,
        owner,
        local,
        socket_address(&to),
        &[],
        TcpFastOpen::Off,
    );
    let connected = match connected {
        Ok(connected) => connected,
        Err(error) => return Err(refusal(kernel, cx, error)),
    };
    let id = next_id(&mut kernel.sockets.inet.outside);
    let handle: HandleOf<S> = Handle::Connection(connected.connection);
    let mut link = Link::new(socket, None, Some(Rc::new(handle)));
    link.connecting = true;
    kernel.sockets.inet.outside.connections.insert(id, link);
    watch(kernel, cx, id);
    Ok(id)
}

/// Lets go of a link: its wait is dropped and its handle closed at the next flush.
fn let_go(kernel: &mut Kernel, id: u32, link: Link) {
    kernel
        .waiting
        .drop_where(|about| *about == WaitFor::Ready(id));
    let closing = match link.handle {
        Some(handle) => Closing::Handle(handle),
        None => Closing::Flow(id),
    };
    kernel.sockets.inet.outside.closing.push(closing);
}

/// Ends every host connection of a socket whose last descriptor went.
pub(super) fn released(kernel: &mut Kernel, socket: &SocketRef) {
    let ids: Vec<u32> = kernel
        .sockets
        .inet
        .outside
        .connections
        .iter()
        .filter(|(_, link)| core::ptr::eq(link.socket.as_ptr(), Rc::as_ptr(socket)))
        .map(|(&id, _)| id)
        .collect();
    for id in ids {
        if let Some(link) = kernel.sockets.inet.outside.connections.remove(&id) {
            let_go(kernel, id, link);
        }
    }
    stop_listening(kernel, socket);
}

/// Asks the system to accept connections from outside for a listener on its port. A system that
/// cannot (or whose port is taken) leaves it listening inside the container alone, counted.
pub(super) fn listen<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
) {
    let id = next_id(&mut kernel.sockets.inet.outside);
    ask_listen(kernel, cx, socket, id);
}

/// The system's `listen` for a listener's port, kept under `id` and waited on if it took it.
fn ask_listen<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    id: u32,
) {
    let (port, v4, backlog, owner) = {
        let own = socket.borrow();
        let port = own.inet.as_deref().map_or(0, |inet| inet.local.port);
        (
            port,
            own.family == crate::linux::net::AF_INET,
            own.max_backlog,
            own.uid,
        )
    };
    let local = LocalBinding {
        address: any_address(v4),
        port: LocalPort::Requested(Port(port)),
    };
    let listening = TcpListeners::listen(
        &**cx.system,
        TcpProtocol::Tcp,
        UserId(owner),
        local,
        ListenBacklog(backlog),
        &[],
    );
    match listening {
        Ok(listening) => {
            let handle: HandleOf<S> = Handle::Listener(listening.listener);
            let mut link = Link::new(socket, None, Some(Rc::new(handle)));
            link.drained = true;
            kernel.sockets.inet.outside.listeners.insert(id, link);
            watch(kernel, cx, id);
        }
        Err(_) => {
            kernel.counters.count(
                cx.host,
                crate::reason::OUTSIDE_LISTEN_FAILED,
                u64::from(port),
            );
        }
    }
}

/// A listener stopped listening: the system stops accepting for it.
pub(super) fn stop_listening(kernel: &mut Kernel, socket: &SocketRef) {
    let ids: Vec<u32> = kernel
        .sockets
        .inet
        .outside
        .listeners
        .iter()
        .filter(|(_, link)| core::ptr::eq(link.socket.as_ptr(), Rc::as_ptr(socket)))
        .map(|(&id, _)| id)
        .collect();
    for id in ids {
        if let Some(link) = kernel.sockets.inet.outside.listeners.remove(&id) {
            let_go(kernel, id, link);
        }
    }
}

/// Takes the connections waiting at the system for a listener into its backlog while it has
/// room. Each is the listener's end of a connection from the gateway (as Docker's published ports
/// show it), at eth0's address.
pub(super) fn take<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    listener: &SocketRef,
) {
    let Some((listener_id, handle)) = kernel
        .sockets
        .inet
        .outside
        .listeners
        .iter()
        .find(|(_, link)| core::ptr::eq(link.socket.as_ptr(), Rc::as_ptr(listener)))
        .map(|(&id, link)| (id, link.handle.clone()))
    else {
        return;
    };
    let Some(Handle::Listener(system_listener)) = handle_of::<S>(&handle) else {
        return;
    };
    while !super::inet::backlog_full(&listener.borrow()) {
        let accepted = match TcpListeners::accept(&**cx.system, system_listener, &mut []) {
            Ok(accepted) => accepted.connection,
            Err(HostError::WouldBlock) => {
                if let Some(link) = kernel.sockets.inet.outside.listeners.get_mut(&listener_id) {
                    link.drained = true;
                }
                watch(kernel, cx, listener_id);
                return;
            }
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
        };
        let id = next_id(&mut kernel.sockets.inet.outside);
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
        let handle: HandleOf<S> = Handle::Connection(accepted);
        let link = Link::new(&embryo, None, Some(Rc::new(handle)));
        kernel.sockets.inet.outside.connections.insert(id, link);
        pull_and_wait(kernel, cx, &embryo, id);
        listener.borrow_mut().backlog.push_back(embryo);
        super::data_ready(kernel, cx, listener, EPOLLIN | EPOLLRDNORM);
    }
}

/// Hands the system the connections and listeners released since the last call.
pub(super) fn flush<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>) {
    for closing in core::mem::take(&mut kernel.sockets.inet.outside.closing) {
        match closing {
            Closing::Flow(id) => wcontainers_abi::Host::close(cx.host, ConnectionId(id)),
            Closing::Handle(handle) => close_handle(&**cx.system, handle),
        }
    }
}

/// What a link's wait should ask for now: to read once a receive or accept found nothing, to
/// send once the system took less than it was given, and a connect's end.
fn interest(link: &Link) -> ReadyFor {
    let Some(socket) = link.socket.upgrade() else {
        return ReadyFor::empty();
    };
    if link.connecting {
        return ReadyFor::WRITABLE;
    }
    let mut interest = ReadyFor::empty();
    if link.drained {
        interest |= ReadyFor::READABLE;
    }
    let room = socket.borrow().inet.as_deref().is_none_or(|inet| inet.room);
    if !room {
        interest |= ReadyFor::WRITABLE;
    }
    interest
}

/// Keeps the wait of link `id` asking for what it needs now: none, the one kept, or a new one in
/// place of the one kept. A new wait's first poll comes when the kernel next settles.
fn watch<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, id: u32) {
    let outside = &mut kernel.sockets.inet.outside;
    let Some(link) = outside
        .connections
        .get_mut(&id)
        .or_else(|| outside.listeners.get_mut(&id))
    else {
        return;
    };
    let Some(handle) = link.handle.clone() else {
        return;
    };
    let wanted = interest(link);
    if wanted == link.asked {
        return;
    }
    let had = !link.asked.is_empty();
    link.asked = wanted;
    if had {
        kernel
            .waiting
            .drop_where(|about| *about == WaitFor::Ready(id));
    }
    if wanted.is_empty() {
        return;
    }
    let call: crate::waiting::Call = waiting_call!(ready_of(cx.system.clone(), handle, wanted));
    if Waiting::on_heap(&call) {
        kernel
            .counters
            .count(cx.host, crate::reason::WAITING_CALL_ON_HEAP, u64::from(id));
    }
    kernel.waiting.start_woken(WaitFor::Ready(id), call);
}

/// The system's wait on a connection or listener, owning what it waits on.
async fn ready_of<S: crate::System>(
    system: Arc<S>,
    handle: Rc<dyn Any>,
    interest: ReadyFor,
) -> Answer {
    let ready = match handle.downcast_ref::<HandleOf<S>>() {
        Some(Handle::Connection(connection)) => {
            Readiness::<<S as TcpConnections>::Connection>::wait_until_ready(
                &*system, connection, interest,
            )
            .await
        }
        Some(Handle::Listener(listener)) => {
            Readiness::<<S as TcpListeners>::Listener>::wait_until_ready(
                &*system, listener, interest,
            )
            .await
        }
        None => Err(HostError::NotSupported),
    };
    Answer::Ready(ready)
}

/// The error a connect that failed ended with, as the system's connection reports it.
fn connect_error<S: crate::System>(kernel: &Kernel, cx: &Context<'_, S>, id: u32) -> Errno {
    let Some(connection) = connection_of::<S>(kernel, id) else {
        return Errno::ECONNREFUSED;
    };
    let mut byte = [0u8; 1];
    let peek = TcpReceiveRequest::Peek {
        offset: wcontainers_internet_protocol::ByteCount(0),
    };
    match TcpConnections::receive(&**cx.system, connection, peek, &mut byte) {
        Err(HostError::Errno(number)) => Errno(number as u32),
        _ => Errno::ECONNREFUSED,
    }
}

/// The system's wait on connection or listener `id` ended: a listener takes what waits, a
/// connect ends, and a connection pulls what arrived and hears of room to send.
pub(super) fn ready<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    id: u32,
    ready: Result<ReadyFor, HostError>,
) {
    let ready = ready.unwrap_or(ReadyFor::ERROR);
    if let Some(link) = kernel.sockets.inet.outside.listeners.get_mut(&id) {
        link.asked = ReadyFor::empty();
        link.drained = false;
        if let Some(listener) = link.socket.upgrade() {
            take(kernel, cx, &listener);
        }
        return;
    }
    let Some(link) = kernel.sockets.inet.outside.connections.get_mut(&id) else {
        return;
    };
    link.asked = ReadyFor::empty();
    let connecting = core::mem::take(&mut link.connecting);
    let Some(socket) = owner(kernel, id) else {
        return;
    };
    if connecting {
        if ready.contains(ReadyFor::ERROR) {
            let error = connect_error(kernel, cx, id);
            if let Some(link) = kernel.sockets.inet.outside.connections.remove(&id) {
                let_go(kernel, id, link);
            }
            flush(kernel, cx);
            super::inet::failed(kernel, cx, &socket, error);
            return;
        }
        {
            let mut own = socket.borrow_mut();
            own.state = State::Connected;
            own.error = 0;
            let inet = own.inet.as_deref_mut().expect("an internet socket");
            inet.connecting = None;
            inet.room = true;
        }
        if let Some(link) = kernel.sockets.inet.outside.connections.get_mut(&id) {
            link.drained = true;
        }
        super::inet::made_outside(kernel, &socket, true);
        kernel.wake_poll(channel(&socket), EPOLLOUT | EPOLLWRNORM);
        super::wake_async(kernel, cx, &socket, super::Wake::Io, POLL_OUT);
        watch(kernel, cx, id);
        return;
    }
    if ready.intersects(ReadyFor::WRITABLE | ReadyFor::ERROR | ReadyFor::HANG_UP)
        && let Some(inet) = socket.borrow_mut().inet.as_deref_mut()
    {
        inet.room = true;
    }
    if let Some(link) = kernel.sockets.inet.outside.connections.get_mut(&id) {
        link.drained = false;
    }
    pull_and_wait(kernel, cx, &socket, id);
    super::write_space(kernel, cx, &socket);
}

/// Pulls what the system has for a connection into its socket's queue, up to its receive buffer,
/// and waits for more once it has none: bytes or datagrams, the end of a stream, or a reset.
pub(super) fn pull_and_wait<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    id: u32,
) {
    pull(kernel, cx, socket, id);
    watch(kernel, cx, id);
}

/// Receives from connection `id`: the system's TCP connection, or the host's UDP flow.
fn receive<S: crate::System>(
    kernel: &Kernel,
    cx: &Context<'_, S>,
    id: u32,
    into: &mut [u8],
) -> Result<usize, HostError> {
    match connection_of::<S>(kernel, id) {
        Some(connection) => {
            TcpConnections::receive(&**cx.system, connection, TcpReceiveRequest::Read, into)
                .map(|received| received.length.0)
        }
        None => wcontainers_abi::Host::receive(cx.host, ConnectionId(id), into),
    }
}

/// What [`pull_and_wait`] takes now, without waiting.
fn pull<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    id: u32,
) {
    let Some(flow) = kernel
        .sockets
        .inet
        .outside
        .connections
        .get(&id)
        .map(|link| link.flow)
    else {
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
        match receive(kernel, cx, id, &mut buffer) {
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
            Err(HostError::WouldBlock) => {
                if let Some(link) = kernel.sockets.inet.outside.connections.get_mut(&id) {
                    link.drained = true;
                }
                return;
            }
            Err(HostError::NotSupported) => return,
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
    if let Some(link) = kernel.sockets.inet.outside.connections.remove(&id) {
        let_go(kernel, id, link);
    }
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

/// The flags of a plain send, with nothing reported.
const PLAIN_SEND: TcpSendRequest = TcpSendRequest {
    urgency: TcpUrgency::Normal,
    continuation: TcpContinuation::Push,
    record: TcpRecordBoundary::Open,
    transmit_reports: TransmitReportRequest {
        key: TransmitReportKey(0),
        scheduled: Report::NotRequested,
        software: Report::NotRequested,
        hardware: Report::NotRequested,
        acknowledgement: Report::NotRequested,
        completion: Report::NotRequested,
        wireless_status: Report::NotRequested,
        packet: Report::NotRequested,
    },
    transmit_statistics: Report::NotRequested,
};

/// Sends bytes of a TCP connection to the outside, as much as the system takes now; once it
/// takes less, the connection waits for room.
pub(super) fn send<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    id: u32,
    bytes: &[u8],
) -> Result<usize, Errno> {
    let sent = match connection_of::<S>(kernel, id) {
        Some(connection) => {
            TcpConnections::send(&**cx.system, connection, PLAIN_SEND, bytes).map(|count| count.0)
        }
        None => Err(HostError::Closed),
    };
    match sent {
        Ok(count) => {
            if count < bytes.len() {
                if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
                    inet.room = false;
                }
                watch(kernel, cx, id);
            }
            Ok(count)
        }
        Err(HostError::WouldBlock) => {
            if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
                inet.room = false;
            }
            watch(kernel, cx, id);
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
            let id = open_flow(kernel, cx, socket, to)?;
            if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
                inet.flows.insert(to, id);
            }
            id
        }
    };
    match wcontainers_abi::Host::send(cx.host, ConnectionId(id), data) {
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
pub(super) fn finish<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, id: u32) {
    if let Some(connection) = connection_of::<S>(kernel, id)
        && let Err(error) = TcpConnections::shut_down(&**cx.system, connection, TcpShutdown::Write)
    {
        refusal(kernel, cx, error);
    }
}

/// Hands an event about a UDP flow to its socket. Returns false for one nothing waits for.
pub(crate) fn event<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    event: &wcontainers_abi::Event<'_>,
) -> bool {
    let wcontainers_abi::Event::ConnectionReady { connection } = *event else {
        return false;
    };
    let Some(socket) = owner(kernel, connection.0) else {
        return false;
    };
    if let Some(inet) = socket.borrow_mut().inet.as_deref_mut() {
        inet.room = true;
    }
    pull_and_wait(kernel, cx, &socket, connection.0);
    super::write_space(kernel, cx, &socket);
    true
}

fn owner(kernel: &Kernel, id: u32) -> Option<SocketRef> {
    let link = kernel.sockets.inet.outside.connections.get(&id)?;
    link.socket.upgrade().filter(|socket| !socket.borrow().dead)
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
        super::resolver::Question::Ask(asked) => {
            let outside = &mut kernel.sockets.inet.outside;
            outside.last_lookup += 1;
            let number = outside.last_lookup;
            outside.lookups.insert(
                number,
                Lookup {
                    socket: Rc::downgrade(socket),
                    server,
                    query: data.to_vec(),
                },
            );
            ask(kernel, cx, number, asked);
        }
        super::resolver::Question::Dropped => {}
    }
}

/// After a restore on a new system: connections to the outside are reported closed (plan
/// section 12), listeners listen again, and lookups under way are asked again. The old system's
/// waits and handles are dropped unanswered.
pub(crate) fn restored<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>) {
    kernel.waiting.drop_where(|about| {
        matches!(
            about,
            crate::waiting::WaitFor::Name(_) | crate::waiting::WaitFor::Ready(_)
        )
    });
    let listeners = core::mem::take(&mut kernel.sockets.inet.outside.listeners);
    for (id, link) in listeners {
        if let Some(socket) = link.socket.upgrade().filter(|socket| !socket.borrow().dead) {
            ask_listen(kernel, cx, &socket, id);
        }
    }
    let connections = core::mem::take(&mut kernel.sockets.inet.outside.connections);
    kernel.sockets.inet.outside.closing.clear();
    for (id, link) in connections {
        let Some(socket) = link.socket.upgrade() else {
            continue;
        };
        if link.flow.is_some() {
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
    let asked: Vec<(u64, super::resolver::Question)> = kernel
        .sockets
        .inet
        .outside
        .lookups
        .iter()
        .map(|(&number, lookup)| (number, super::resolver::question(&lookup.query)))
        .collect();
    for (number, question) in asked {
        if let super::resolver::Question::Ask(asked) = question {
            ask(kernel, cx, number, asked);
        }
    }
}

/// Asks the system's resolver the question of lookup `number`, as a waiting call.
fn ask<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    number: u64,
    asked: super::resolver::Asked,
) {
    let call: crate::waiting::Call =
        crate::waiting::waiting_call!(resolve(cx.system.clone(), asked));
    if crate::waiting::Waiting::on_heap(&call) {
        kernel
            .counters
            .count(cx.host, crate::reason::WAITING_CALL_ON_HEAP, number);
    }
    let about = crate::waiting::WaitFor::Name(number);
    if let Some(crate::waiting::Answer::Name(answer)) = kernel.waiting.start(about, call) {
        answered(kernel, cx, number, answer);
    }
}

/// The system's answer to one question, owning what the question borrows.
async fn resolve<S: crate::System>(
    system: alloc::sync::Arc<S>,
    asked: super::resolver::Asked,
) -> crate::waiting::Answer {
    let question =
        domain::base::Question::new(asked.qname().for_slice(), asked.qtype(), asked.qclass());
    let answer = system.resolve_domain(question).await;
    crate::waiting::Answer::Name(answer.map(|message| message.as_slice().to_vec()))
}

/// The resolver answered lookup `number`: its reply goes to the socket that asked, if still open.
pub(super) fn answered<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    number: u64,
    answer: Result<Vec<u8>, HostError>,
) {
    let Some(lookup) = kernel.sockets.inet.outside.lookups.remove(&number) else {
        return;
    };
    let Some(socket) = lookup.socket.upgrade() else {
        return;
    };
    if let Err(error) = answer {
        refusal(kernel, cx, error);
    }
    let message = answer
        .as_deref()
        .ok()
        .and_then(|bytes| domain::base::Message::from_slice(bytes).ok());
    if let Some(reply) = super::resolver::answer(&lookup.query, message) {
        deliver(kernel, cx, &socket, lookup.server, reply);
    }
}
