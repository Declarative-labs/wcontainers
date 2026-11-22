//! The TCP sockets of a system other than Linux: what socket2 covers, and a counted refusal for what only Linux gives.

use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::net::{RecvFlags, SendFlags};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    AddressSharing, BindAddress, BindingSetting, ByteCount, DropCount, InterfaceIndex,
    IpSocketSetting, IpVersion, Ipv6Families, LocalBinding, LocalPort, PacketMark, PacketPriority,
    PortSharing, ReceiveTimestamps, ReceivedPacketFields, Report, TransmitReportRequest,
};
use wcontainers_internet_protocol_linux_socket_options::errno::errno_of;
use wcontainers_internet_protocol_linux_socket_options::{host_error, io_host_error, sock};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};
use wcontainers_tcp_sockets::authentication::TcpAuthenticationCounters;
use wcontainers_tcp_sockets::repair::TcpTimestampClock;
use wcontainers_tcp_sockets::settings::{TcpListenerSetting, TcpQuickAcknowledgement, TcpSetting};
use wcontainers_tcp_sockets::state::{
    TcpErrorQueueEntry, TcpListenerQuery, TcpListenerReading, TcpQuery, TcpReading,
};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpAccepted, TcpBound, TcpClose, TcpConnectStart, TcpConnected, TcpConnections,
    TcpContinuation, TcpDisconnected, TcpFastOpen, TcpListeners, TcpListening, TcpLocalEnd,
    TcpLocalEndError, TcpProtocol, TcpPushOverride, TcpReceiveRequest, TcpReceived,
    TcpRecordBoundary, TcpSendRequest, TcpShutdown, TcpSockets, TcpUrgency,
};

use super::{
    ABORT_CLOSED_IN_ORDER, HostTcpBoundSocket, HostTcpConnection, HostTcpListener, HostTcpSockets,
};

/// The reasons the host could not honour a request, each counted.
const MULTIPATH: &str = "a Multipath TCP socket, which only Linux has";
const OWNER_NOT_THE_PROCESS: &str =
    "a socket opened as a user other than the process's, which only Linux's filesystem user allows";
const PORT_CHOSEN_BY_EXIT_NODE: &str =
    "a port chosen by the exit node from its ranges, which only Linux's IP_LOCAL_PORT_RANGE binds";
const FAST_OPEN: &str = "TCP Fast Open, which socket2 makes only on Linux";
const MORE_TO_COME: &str = "a send with more to come, Linux's MSG_MORE";
const TRANSMIT_REPORTS: &str =
    "transmit reports and statistics, which only Linux's error queue gives";
const PEEK_AT_AN_OFFSET: &str = "a peek at an offset, Linux's SO_PEEK_OFF";
const ERROR_QUEUE: &str = "a socket's error queue, which only Linux has";
const NO_SYNCHRONIZE: &str =
    "an accepted connection's SYN, which only Linux keeps (TCP_SAVED_SYN): none is given";
const STATE: &str = "reading a TCP socket's state, which only the Linux build maps";
const PUSH_PAST_CORK_AND_NAGLE: &str =
    "a push past Nagle's algorithm, which only Linux makes by TCP_NODELAY";
const DELAYED_ACKNOWLEDGEMENT: &str = "sending a delayed acknowledgement, Linux's TCP_QUICKACK";
const KEEP_ALIVE_REARM: &str =
    "rearming the keep-alive timer, which only Linux's TCP_KEEPIDLE does";
const REPAIR: &str = "TCP repair, which only Linux has";
const AUTHENTICATION: &str = "the TCP Authentication Option, which only Linux has";
const NEGATIVE_ROUTE_ADVICE: &str = "negative route advice, which only Linux's sk_dst_reset gives";
const SETTING: &str = "a setting that only the Linux build makes";
const DISCONNECT: &str =
    "a disconnect, Linux's connect to AF_UNSPEC: the socket is closed, freeing its port";
const NO_SYNCHRONIZE_INTERFACE: &str = "no host interface holds an accepted connection's local address, the one its SYN is taken to have come in on";

fn refuse(sockets: &HostTcpSockets, reason: &'static str) -> HostError {
    sockets.shortfalls.refuse(reason)
}

/// Opens a non-blocking TCP socket of the family as the owner the process runs as.
fn open(
    sockets: &HostTcpSockets,
    family: IpVersion,
    protocol: TcpProtocol,
    owner: UserId,
) -> Result<OwnedFd, HostError> {
    if protocol == TcpProtocol::Multipath {
        return Err(refuse(sockets, MULTIPATH));
    }
    if owner.0 != rustix::process::getuid().as_raw() {
        return Err(refuse(sockets, OWNER_NOT_THE_PROCESS));
    }
    let domain = match family {
        IpVersion::V4 => Domain::IPV4,
        IpVersion::V6 => Domain::IPV6,
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(|error| io_host_error(&error))?;
    prepare(&socket)?;
    Ok(OwnedFd::from(socket))
}

/// Opens the socket a connect or listen starts from, binding it when it is not bound yet.
fn local_end(
    sockets: &HostTcpSockets,
    local: TcpLocalEnd<'_, HostTcpBoundSocket>,
) -> Result<HostTcpBoundSocket, TcpLocalEndError<HostTcpBoundSocket>> {
    match local {
        TcpLocalEnd::Bound(bound) => Ok(bound),
        TcpLocalEnd::Unbound {
            protocol,
            owner,
            local,
        } => sockets
            .bind(protocol, owner, local, &[])
            .map(|bound| bound.bound_socket)
            .map_err(TcpLocalEndError::Unbound),
    }
}

/// Gives the error of a connect or listen that failed before it started, with the socket it was given back.
fn failed(
    bound: Option<HostTcpBoundSocket>,
    error: HostError,
) -> TcpLocalEndError<HostTcpBoundSocket> {
    match bound {
        Some(bound_socket) => TcpLocalEndError::Bound {
            bound_socket,
            error,
        },
        None => TcpLocalEndError::Unbound(error),
    }
}

/// Gives the index of the host interface that holds the address.
fn interface_holding(address: IpAddr) -> Option<InterfaceIndex> {
    nix::ifaddrs::getifaddrs()
        .ok()?
        .find(|entry| {
            entry.address.as_ref().and_then(|held| {
                held.as_sockaddr_in()
                    .map(|held| IpAddr::V4(held.ip()))
                    .or_else(|| held.as_sockaddr_in6().map(|held| IpAddr::V6(held.ip())))
            }) == Some(address)
        })
        .and_then(|entry| nix::net::if_::if_nametoindex(entry.interface_name.as_str()).ok())
        .and_then(NonZeroU32::new)
        .map(InterfaceIndex)
}

/// Binds the socket to the local binding's address and port.
fn bind(
    sockets: &HostTcpSockets,
    socket: BorrowedFd<'_>,
    local: &LocalBinding<'_>,
) -> Result<(), HostError> {
    let host = sock(&socket);
    let address = match local.address {
        BindAddress::V4(address) => IpAddr::V4(address),
        BindAddress::V6 { address, families } => {
            host.set_only_v6(families == Ipv6Families::V6Only)
                .map_err(|error| io_host_error(&error))?;
            IpAddr::V6(address)
        }
    };
    let port = match local.port {
        LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => port.0,
        LocalPort::ChosenByExitNode(_) => return Err(refuse(sockets, PORT_CHOSEN_BY_EXIT_NODE)),
    };
    host.bind(&SockAddr::from(SocketAddr::new(address, port)))
        .map_err(|error| io_host_error(&error))
}

/// Sets a setting of either kind of socket on the host socket.
fn set(
    sockets: &HostTcpSockets,
    socket: BorrowedFd<'_>,
    family: IpVersion,
    setting: TcpSetting<'_>,
) -> Result<(), HostError> {
    let host = sock(&socket);
    let set = match setting {
        TcpSetting::Ip(IpSocketSetting::SendBuffer(bytes)) => host.set_send_buffer_size(bytes.0),
        TcpSetting::Ip(IpSocketSetting::ReceiveBuffer(bytes)) => host.set_recv_buffer_size(bytes.0),
        TcpSetting::Ip(IpSocketSetting::MulticastLoopback(on)) if family == IpVersion::V6 => {
            host.set_multicast_loop_v6(on)
        }
        TcpSetting::Ip(IpSocketSetting::MulticastLoopback(on)) => host.set_multicast_loop_v4(on),
        TcpSetting::Binding(BindingSetting::AddressSharing(sharing)) => {
            host.set_reuse_address(sharing == AddressSharing::Shared)
        }
        TcpSetting::Binding(BindingSetting::PortSharing(sharing)) => {
            host.set_reuse_port(sharing == PortSharing::Shared)
        }
        TcpSetting::KeepAlive(on) => host.set_keepalive(on),
        TcpSetting::NoDelay(on) => host.set_tcp_nodelay(on),
        TcpSetting::UrgentInline(on) => host.set_out_of_band_inline(on),
        _ => return Err(refuse(sockets, SETTING)),
    };
    set.map_err(|error| io_host_error(&error))
}

/// Makes a host socket non-blocking and, where the system has no `MSG_NOSIGNAL`, free of `SIGPIPE`.
fn prepare(socket: &Socket) -> Result<(), HostError> {
    socket
        .set_nonblocking(true)
        .map_err(|error| io_host_error(&error))?;
    #[cfg(target_vendor = "apple")]
    socket
        .set_nosigpipe(true)
        .map_err(|error| io_host_error(&error))?;
    Ok(())
}

/// Gives the flags every send carries: it never blocks and, where the system has the flag, raises no `SIGPIPE`.
fn send_flags() -> SendFlags {
    #[cfg(not(target_vendor = "apple"))]
    return SendFlags::DONTWAIT | SendFlags::NOSIGNAL;
    #[cfg(target_vendor = "apple")]
    return SendFlags::DONTWAIT;
}

/// Gives an address the host gives, as an internet address with an IPv4 one an IPv6 socket holds unmapped.
fn internet_address(address: std::io::Result<SockAddr>) -> Result<SocketAddr, HostError> {
    let address = address
        .map_err(|error| io_host_error(&error))?
        .as_socket()
        .ok_or(HostError::Errno(
            rustix::io::Errno::AFNOSUPPORT.raw_os_error(),
        ))?;
    Ok(match address {
        SocketAddr::V6(v6) => v6
            .ip()
            .to_ipv4_mapped()
            .map_or(address, |v4| SocketAddr::new(IpAddr::V4(v4), v6.port())),
        SocketAddr::V4(_) => address,
    })
}

/// Gives a peer an IPv6 socket reaches, an IPv4 one mapped.
fn peer_for(family: IpVersion, peer: SocketAddr) -> SocketAddr {
    match (family, peer) {
        (IpVersion::V6, SocketAddr::V4(v4)) => {
            SocketAddr::new(IpAddr::V6(v4.ip().to_ipv6_mapped()), v4.port())
        }
        _ => peer,
    }
}

/// Gives the family of the socket a binding asks for.
fn family_of(address: &BindAddress) -> IpVersion {
    match address {
        BindAddress::V4(_) => IpVersion::V4,
        BindAddress::V6 { .. } => IpVersion::V6,
    }
}

/// Tells whether a transmit report request asks for any report.
fn asks_for_reports(request: &TransmitReportRequest) -> bool {
    [
        request.scheduled,
        request.software,
        request.hardware,
        request.acknowledgement,
        request.completion,
        request.wireless_status,
        request.packet,
    ]
    .contains(&Report::Requested)
}

impl TcpConnections for HostTcpSockets {
    type Connection = HostTcpConnection;
    type BoundSocket = HostTcpBoundSocket;

    fn bind(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        settings: &[BindingSetting<'_>],
    ) -> Result<TcpBound<Self::BoundSocket>, HostError> {
        let family = family_of(&local.address);
        let socket = open(self, family, protocol, owner)?;
        for setting in settings {
            set(self, socket.as_fd(), family, TcpSetting::Binding(*setting))?;
        }
        bind(self, socket.as_fd(), &local)?;
        let address = internet_address(sock(&socket.as_fd()).local_addr())?;
        Ok(TcpBound {
            bound_socket: HostTcpBoundSocket {
                socket,
                family,
                protocol,
                bound_by_number: !matches!(local.port, LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) if port.0 == u16::MIN),
            },
            local: address,
        })
    }

    fn connect(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        peer: SocketAddr,
        settings: &[TcpSetting<'_>],
        fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, TcpLocalEndError<Self::BoundSocket>> {
        let unbound = matches!(local, TcpLocalEnd::Unbound { .. });
        let bound = local_end(self, local)?;
        let give_back = |bound| (!unbound).then_some(bound);
        if fast_open != TcpFastOpen::Off {
            return Err(failed(give_back(bound), refuse(self, FAST_OPEN)));
        }
        let HostTcpBoundSocket {
            socket,
            family,
            protocol,
            bound_by_number,
        } = bound;
        let connection = HostTcpConnection {
            socket: self
                .readiness
                .watch(socket, ReadyFor::all())
                .map_err(TcpLocalEndError::Unbound)?,
            family,
            protocol,
            bound_by_number,
        };
        let started = settings
            .iter()
            .try_for_each(|setting| self.configure(&connection, setting.clone()))
            .and_then(|()| {
                match sock(&connection.socket.fd()).connect(&SockAddr::from(peer_for(family, peer)))
                {
                    Ok(()) => Ok(()),
                    Err(error)
                        if error.raw_os_error()
                            == Some(rustix::io::Errno::INPROGRESS.raw_os_error()) =>
                    {
                        Ok(())
                    }
                    Err(error) => Err(io_host_error(&error)),
                }
            });
        if let Err(error) = started {
            return Err(failed(give_back(connection.into_bound()), error));
        }
        let local = match internet_address(sock(&connection.socket.fd()).local_addr()) {
            Ok(local) => local,
            Err(error) => {
                self.close(connection, TcpClose::Abort);
                return Err(TcpLocalEndError::Unbound(error));
            }
        };
        Ok(TcpConnected {
            connection,
            local,
            start: TcpConnectStart::SynchronizeSent {
                fast_open_data_taken: ByteCount(usize::MIN),
            },
        })
    }

    fn send(
        &self,
        connection: &Self::Connection,
        request: TcpSendRequest,
        bytes: &[u8],
    ) -> Result<ByteCount, HostError> {
        if request.continuation == TcpContinuation::MoreToCome {
            return Err(refuse(self, MORE_TO_COME));
        }
        if asks_for_reports(&request.transmit_reports)
            || request.transmit_statistics == Report::Requested
        {
            return Err(refuse(self, TRANSMIT_REPORTS));
        }
        let mut flags = send_flags();
        if request.urgency == TcpUrgency::Urgent {
            flags |= SendFlags::OOB;
        }
        if request.record == TcpRecordBoundary::Ended {
            flags |= SendFlags::EOR;
        }
        rustix::net::send(connection.socket.fd(), bytes, flags)
            .map(ByteCount)
            .map_err(host_error)
    }

    fn receive(
        &self,
        connection: &Self::Connection,
        request: TcpReceiveRequest,
        into: &mut [u8],
    ) -> Result<TcpReceived, HostError> {
        let flags = match request {
            TcpReceiveRequest::Read => RecvFlags::DONTWAIT,
            TcpReceiveRequest::Peek { offset } if offset.0 == usize::MIN => {
                RecvFlags::DONTWAIT | RecvFlags::PEEK
            }
            TcpReceiveRequest::Peek { .. } => return Err(refuse(self, PEEK_AT_AN_OFFSET)),
            TcpReceiveRequest::ReadUrgentByte => RecvFlags::DONTWAIT | RecvFlags::OOB,
            TcpReceiveRequest::PeekUrgentByte => {
                RecvFlags::DONTWAIT | RecvFlags::OOB | RecvFlags::PEEK
            }
        };
        let (taken, _) =
            rustix::net::recv(connection.socket.fd(), into, flags).map_err(host_error)?;
        Ok(TcpReceived {
            length: ByteCount(taken),
            timestamps: ReceiveTimestamps {
                software: None,
                hardware: None,
            },
            unread_after: None,
        })
    }

    fn take_error_queue_entry(
        &self,
        _connection: &Self::Connection,
        _into: &mut [u8],
    ) -> Result<TcpErrorQueueEntry, HostError> {
        Err(refuse(self, ERROR_QUEUE))
    }

    fn read_state<'a>(
        &self,
        connection: &Self::Connection,
        query: TcpQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
        match query {
            TcpQuery::TakePendingError => {
                take_pending_error(connection.socket.fd()).map(TcpReading::PendingError)
            }
            _ => Err(refuse(self, STATE)),
        }
    }

    fn shut_down(
        &self,
        connection: &Self::Connection,
        directions: TcpShutdown,
    ) -> Result<(), HostError> {
        let how = match directions {
            TcpShutdown::Read => std::net::Shutdown::Read,
            TcpShutdown::Write => std::net::Shutdown::Write,
            TcpShutdown::ReadAndWrite => std::net::Shutdown::Both,
        };
        sock(&connection.socket.fd())
            .shutdown(how)
            .map_err(|error| io_host_error(&error))
    }

    fn close(&self, connection: Self::Connection, manner: TcpClose) {
        if manner == TcpClose::Abort
            && sock(&connection.socket.fd())
                .set_linger(Some(core::time::Duration::ZERO))
                .is_err()
        {
            self.shortfalls.count(ABORT_CLOSED_IN_ORDER);
        }
        drop(connection);
    }

    fn disconnect(&self, connection: Self::Connection) -> TcpDisconnected<Self::BoundSocket> {
        let pending_error = take_pending_error(connection.socket.fd()).unwrap_or(None);
        self.shortfalls.count(DISCONNECT);
        self.close(connection, TcpClose::Abort);
        TcpDisconnected {
            bound_socket: None,
            pending_error,
        }
    }

    fn release(&self, bound_socket: Self::BoundSocket) {
        drop(bound_socket);
    }

    fn push_queued_data(
        &self,
        _connection: &Self::Connection,
        ignoring: TcpPushOverride,
    ) -> Result<(), HostError> {
        // Without a cork or more-to-come on this system, the host already sends what windows and Nagle allow.
        match ignoring {
            TcpPushOverride::Neither => Ok(()),
            TcpPushOverride::CorkAndNagle => Err(refuse(self, PUSH_PAST_CORK_AND_NAGLE)),
        }
    }

    fn send_delayed_acknowledgement(
        &self,
        _connection: &Self::Connection,
        _afterwards: TcpQuickAcknowledgement,
    ) -> Result<(), HostError> {
        Err(refuse(self, DELAYED_ACKNOWLEDGEMENT))
    }

    fn rearm_keep_alive_timer(&self, _connection: &Self::Connection) -> Result<(), HostError> {
        Err(refuse(self, KEEP_ALIVE_REARM))
    }

    fn discard_error_queue_entries(&self, _connection: &Self::Connection) -> Result<(), HostError> {
        Err(refuse(self, ERROR_QUEUE))
    }

    fn send_window_probe(&self, _connection: &Self::Connection) -> Result<(), HostError> {
        Err(refuse(self, REPAIR))
    }

    fn set_timestamp_clock(
        &self,
        _connection: &Self::Connection,
        _reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        Err(refuse(self, REPAIR))
    }

    fn set_authentication_counters(
        &self,
        _connection: &Self::Connection,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        Err(refuse(self, AUTHENTICATION))
    }
}

impl TcpListeners for HostTcpSockets {
    type Listener = HostTcpListener;

    fn listen(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        backlog: ListenBacklog,
        settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, TcpLocalEndError<Self::BoundSocket>> {
        let unbound = matches!(local, TcpLocalEnd::Unbound { .. });
        let HostTcpBoundSocket {
            socket,
            family,
            protocol,
            bound_by_number,
        } = local_end(self, local)?;
        let listener = HostTcpListener {
            socket: self
                .readiness
                .watch(socket, ReadyFor::all())
                .map_err(TcpLocalEndError::Unbound)?,
            family,
        };
        let mut backlog = backlog;
        let started = settings
            .iter()
            .try_for_each(|setting| match setting {
                TcpListenerSetting::Backlog(last) => {
                    backlog = *last;
                    Ok(())
                }
                other => self.configure(&listener, other.clone()),
            })
            .and_then(|()| {
                sock(&listener.socket.fd())
                    .listen(i32::try_from(backlog.0).unwrap_or(i32::MAX))
                    .map_err(|error| io_host_error(&error))
            });
        if let Err(error) = started {
            let bound = HostTcpBoundSocket {
                socket: listener.socket.unwatch(),
                family,
                protocol,
                bound_by_number,
            };
            return Err(failed((!unbound).then_some(bound), error));
        }
        match internet_address(sock(&listener.socket.fd()).local_addr()) {
            Ok(local) => Ok(TcpListening { listener, local }),
            Err(error) => Err(TcpLocalEndError::Unbound(error)),
        }
    }

    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError> {
        let (socket, peer) = match sock(&listener.socket.fd()).accept() {
            Ok(accepted) => accepted,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                return Err(HostError::WouldBlock);
            }
            Err(error) => return Err(io_host_error(&error)),
        };
        prepare(&socket)?;
        let peer = internet_address(Ok(peer))?;
        let local = internet_address(socket.local_addr())?;
        let interface =
            interface_holding(local.ip()).ok_or_else(|| refuse(self, NO_SYNCHRONIZE_INTERFACE))?;
        let connection = HostTcpConnection {
            socket: self
                .readiness
                .watch(OwnedFd::from(socket), ReadyFor::all())?,
            family: listener.family,
            protocol: TcpProtocol::Tcp,
            bound_by_number: false,
        };
        self.shortfalls.count(NO_SYNCHRONIZE);
        Ok(TcpAccepted {
            connection,
            peer,
            local,
            synchronize: ReceivedPacketFields {
                length: usize::MIN,
                interface,
                specific_destination: None,
                largest_fragment: None,
                link_layer_length: None,
                timestamps: ReceiveTimestamps {
                    software: None,
                    hardware: None,
                },
                mark: PacketMark(u32::MIN),
                priority: PacketPriority(u32::MIN),
                drops_since_previous: DropCount(u32::MIN),
            },
            synchronize_header: &synchronize_header[..usize::MIN],
        })
    }

    fn read_listener_state<'a>(
        &self,
        listener: &Self::Listener,
        query: TcpListenerQuery<'a>,
    ) -> Result<TcpListenerReading<'a>, HostError> {
        match query {
            TcpListenerQuery::TakePendingError => {
                take_pending_error(listener.socket.fd()).map(TcpListenerReading::PendingError)
            }
            _ => Err(refuse(self, STATE)),
        }
    }

    fn set_listener_authentication_counters(
        &self,
        _listener: &Self::Listener,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        Err(refuse(self, AUTHENTICATION))
    }

    fn set_listener_timestamp_clock(
        &self,
        _listener: &Self::Listener,
        _reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        Err(refuse(self, REPAIR))
    }

    fn stop_listening(&self, listener: Self::Listener) {
        drop(listener);
    }
}

impl<'a> Configurable<HostTcpConnection, TcpSetting<'a>> for HostTcpSockets {
    fn configure(
        &self,
        connection: &HostTcpConnection,
        setting: TcpSetting<'a>,
    ) -> Result<(), HostError> {
        set(self, connection.socket.fd(), connection.family, setting)
    }
}

impl<'a> Configurable<HostTcpListener, TcpListenerSetting<'a>> for HostTcpSockets {
    fn configure(
        &self,
        listener: &HostTcpListener,
        setting: TcpListenerSetting<'a>,
    ) -> Result<(), HostError> {
        let socket = listener.socket.fd();
        match setting {
            TcpListenerSetting::Socket(setting) => set(self, socket, listener.family, setting),
            TcpListenerSetting::Backlog(backlog) => sock(&socket)
                .listen(i32::try_from(backlog.0).unwrap_or(i32::MAX))
                .map_err(|error| io_host_error(&error)),
            _ => Err(refuse(self, SETTING)),
        }
    }
}

impl NegativeRouteAdvisable<HostTcpConnection> for HostTcpSockets {
    fn give_negative_route_advice(&self, _connection: &HostTcpConnection) -> Result<(), HostError> {
        Err(refuse(self, NEGATIVE_ROUTE_ADVICE))
    }
}

impl Readiness<HostTcpConnection> for HostTcpSockets {
    async fn wait_until_ready(
        &self,
        connection: &HostTcpConnection,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        connection.socket.wait_until_ready(interest).await
    }
}

impl Readiness<HostTcpListener> for HostTcpSockets {
    async fn wait_until_ready(
        &self,
        listener: &HostTcpListener,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        listener.socket.wait_until_ready(interest).await
    }
}

impl TcpSockets for HostTcpSockets {}

/// Takes the socket's `SO_ERROR`, as Linux's number for it.
fn take_pending_error(
    socket: BorrowedFd<'_>,
) -> Result<Option<wcontainers_kernel::errno::Errno>, HostError> {
    match sock(&socket)
        .take_error()
        .map_err(|error| io_host_error(&error))?
    {
        None => Ok(None),
        Some(error) => match io_host_error(&error) {
            HostError::Errno(number) => Ok(errno_of(number)),
            other => Err(other),
        },
    }
}

impl HostTcpConnection {
    /// Gives the connection's socket back as only bound, no longer watched.
    fn into_bound(self) -> HostTcpBoundSocket {
        HostTcpBoundSocket {
            family: self.family,
            protocol: self.protocol,
            bound_by_number: self.bound_by_number,
            socket: self.socket.unwatch(),
        }
    }
}
