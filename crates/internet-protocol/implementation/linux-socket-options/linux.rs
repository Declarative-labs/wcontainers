//! The shared settings of crates/internet-protocol as Linux socket options, and the raw option calls under them.

use std::ffi::c_int;
use std::io::IoSliceMut;
use std::mem::size_of;
use std::net::{IpAddr, SocketAddr, SocketAddrV6};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};
use std::time::Duration;

use linux_raw_sys::net as uapi;
use nix::sys::socket::{MsgFlags, SockaddrLike, recvmsg};
use nix::unistd::{Uid, setfsuid};
use rustix::io::Errno;
use rustix::net::{Protocol, SocketType};
use socket2::{Domain, SockAddr, Socket};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AddressSharing, BindAddress, BindingSetting, FlowLabelChoice, IpPacketSetting, IpSocketSetting,
    IpVersion, Ipv6Families, LocalBinding, LocalPort, MobileAddressPreference, NonlocalAddresses,
    PathMtuDiscovery, Port, PortChoice, PortRange, PortSharing, RouteScope, SocketFilter,
    TemporaryAddressPreference, Timestamping,
};
use wcontainers_kernel::UserId;
use zerocopy::IntoBytes;

use crate::{
    Shortfalls, host_error, io_host_error, set_bytes, set_flag, set_int, set_option, sock,
};

const NO_MEMBERSHIPS: &str = "multicast and anycast memberships are not mapped to the host";

const NO_FIXED_FLOW_LABEL: &str = "a fixed IPv6 flow label is not mapped to the host";

/// The width of the ECN field at the bottom of the traffic class octet, [RFC 3168](https://www.rfc-editor.org/rfc/rfc3168#section-5).
const ECN_FIELD_BITS: u32 = 2;

/// Binds a socket to an address, answering Linux's errno.
pub fn bind_to(socket: BorrowedFd<'_>, address: SocketAddr) -> Result<(), Errno> {
    sock(&socket)
        .bind(&SockAddr::from(address))
        .map_err(|error| Errno::from_io_error(&error).unwrap_or(Errno::IO))
}

/// Gives the flags of `SO_TIMESTAMPING` that a socket's receive timestamping asks for.
pub fn receive_timestamping_flags(timestamping: &Timestamping) -> u32 {
    let mut flags = u32::MIN;
    if timestamping.receive_software == wcontainers_internet_protocol::Report::Requested {
        flags |= uapi::SOF_TIMESTAMPING_RX_SOFTWARE as u32 | uapi::SOF_TIMESTAMPING_SOFTWARE as u32;
    }
    if timestamping.receive_hardware == wcontainers_internet_protocol::Report::Requested {
        flags |=
            uapi::SOF_TIMESTAMPING_RX_HARDWARE as u32 | uapi::SOF_TIMESTAMPING_RAW_HARDWARE as u32;
    }
    flags
}

/// The `bind_phc` of `SO_TIMESTAMPING` when no hardware clock is bound.
///
/// https://docs.kernel.org/networking/timestamping.html
const NO_HARDWARE_CLOCK: c_int = 0;

/// Sets `SO_TIMESTAMPING` to the flags, bound to the hardware clock if one is given.
pub fn set_timestamping(
    socket: BorrowedFd<'_>,
    flags: u32,
    bound_clock: Option<wcontainers_internet_protocol::PtpHardwareClockIndex>,
) -> Result<(), HostError> {
    let (flags, bind_phc) = match bound_clock {
        Some(clock) => (
            flags | uapi::SOF_TIMESTAMPING_BIND_PHC as u32,
            c_int::try_from(clock.0).map_err(|_| HostError::Errno(libc::EINVAL))?,
        ),
        None => (flags, NO_HARDWARE_CLOCK),
    };
    let value = crate::datagram::send::Timestamping {
        flags: flags as c_int,
        bind_phc,
    };
    set_option(socket, uapi::SOL_SOCKET, uapi::SO_TIMESTAMPING, &value)
}

/// Attaches the classic BPF program with one option or detaches it with the other, detaching none being no error.
///
/// The options are `SO_ATTACH_FILTER` and `SO_DETACH_FILTER`, through socket2, or `SO_ATTACH_REUSEPORT_CBPF`, through
/// nix, and `SO_DETACH_REUSEPORT_BPF`.
pub fn set_filter(
    socket: BorrowedFd<'_>,
    attach: u32,
    detach: u32,
    filter: Option<SocketFilter<'_>>,
) -> Result<(), HostError> {
    let io = |error: std::io::Error| io_host_error(&error);
    match filter {
        Some(SocketFilter(instructions)) if attach == uapi::SO_ATTACH_FILTER => {
            let program: Vec<socket2::SockFilter> = instructions
                .iter()
                .map(|instruction| {
                    socket2::SockFilter::new(
                        instruction.operation.0,
                        instruction.jump_if_true.0,
                        instruction.jump_if_false.0,
                        instruction.operand.0,
                    )
                })
                .collect();
            sock(&socket).attach_filter(&program).map_err(io)
        }
        Some(SocketFilter(instructions)) => {
            let program: Vec<libc::sock_filter> = instructions
                .iter()
                .map(|instruction| libc::sock_filter {
                    code: instruction.operation.0,
                    jt: instruction.jump_if_true.0,
                    jf: instruction.jump_if_false.0,
                    k: instruction.operand.0,
                })
                .collect();
            // nix's typed call takes the program's address in the structure; `program` outlives the call.
            let value = libc::sock_fprog {
                len: u16::try_from(program.len()).map_err(|_| HostError::Errno(libc::EINVAL))?,
                filter: program.as_ptr().cast_mut(),
            };
            nix::sys::socket::setsockopt(
                &socket,
                nix::sys::socket::sockopt::AttachReusePortCbpf,
                &value,
            )
            .map_err(|errno| HostError::Errno(errno as i32))
        }
        None if detach == uapi::SO_DETACH_FILTER => match sock(&socket).detach_filter() {
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => Ok(()),
            other => other.map_err(io),
        },
        None => match set_int(socket, uapi::SOL_SOCKET, detach, DETACH_VALUE) {
            Err(HostError::Errno(libc::ENOENT)) => Ok(()),
            other => other,
        },
    }
}

/// Sets a setting of the socket's IP layer; `report_flags` are the `SO_TIMESTAMPING` flags the implementation keeps on.
pub fn set_ip_socket_setting(
    socket: BorrowedFd<'_>,
    family: IpVersion,
    setting: &IpSocketSetting<'_>,
    report_flags: u32,
    shortfalls: &Shortfalls,
) -> Result<(), HostError> {
    match *setting {
        IpSocketSetting::MulticastV4(_)
        | IpSocketSetting::MulticastV6(_)
        | IpSocketSetting::Anycast(_) => Err(shortfalls.refuse(NO_MEMBERSHIPS)),
        IpSocketSetting::MaximumPacingRate(rate) => set_option(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_MAX_PACING_RATE,
            &rate.map_or(u64::MAX, |rate| rate.0),
        ),
        IpSocketSetting::SendBuffer(bytes) => sock(&socket)
            .set_send_buffer_size(bytes.0)
            .map_err(|error| io_host_error(&error)),
        IpSocketSetting::ReceiveBuffer(bytes) => sock(&socket)
            .set_recv_buffer_size(bytes.0)
            .map_err(|error| io_host_error(&error)),
        IpSocketSetting::Timestamping(timestamping) => set_timestamping(
            socket,
            receive_timestamping_flags(&timestamping) | report_flags,
            timestamping.bound_clock,
        ),
        IpSocketSetting::MulticastLoopback(on) => match family {
            IpVersion::V4 => sock(&socket).set_multicast_loop_v4(on),
            IpVersion::V6 => sock(&socket).set_multicast_loop_v6(on),
        }
        .map_err(|error| io_host_error(&error)),
        IpSocketSetting::MulticastAllGroups { version, enabled } => match version {
            IpVersion::V4 => sock(&socket).set_multicast_all_v4(enabled),
            IpVersion::V6 => sock(&socket).set_multicast_all_v6(enabled),
        }
        .map_err(|error| io_host_error(&error)),
    }
}

/// Sets how the socket holds its local address and port.
pub fn set_binding_setting(
    socket: BorrowedFd<'_>,
    family: IpVersion,
    setting: &BindingSetting<'_>,
) -> Result<(), HostError> {
    match *setting {
        BindingSetting::AddressSharing(sharing) => sock(&socket)
            .set_reuse_address(sharing == AddressSharing::Shared)
            .map_err(|error| io_host_error(&error)),
        BindingSetting::PortSharing(sharing) => sock(&socket)
            .set_reuse_port(sharing == PortSharing::Shared)
            .map_err(|error| io_host_error(&error)),
        BindingSetting::NonlocalAddresses(permission) => {
            let transparent = permission == NonlocalAddresses::Transparent;
            let bindable = permission == NonlocalAddresses::Bindable;
            let socket = sock(&socket);
            match family {
                IpVersion::V4 => socket
                    .set_ip_transparent_v4(transparent)
                    .and_then(|()| socket.set_freebind_v4(bindable)),
                IpVersion::V6 => socket
                    .set_ip_transparent_v6(transparent)
                    .and_then(|()| socket.set_freebind_v6(bindable)),
            }
            .map_err(|error| io_host_error(&error))
        }
        BindingSetting::BoundInterface(interface) => {
            let index = interface.map(|interface| interface.0);
            match family {
                IpVersion::V4 => sock(&socket).bind_device_by_index_v4(index),
                IpVersion::V6 => sock(&socket).bind_device_by_index_v6(index),
            }
            .map_err(|error| io_host_error(&error))
        }
        BindingSetting::GroupFilter(filter) => set_filter(
            socket,
            uapi::SO_ATTACH_REUSEPORT_CBPF,
            uapi::SO_DETACH_REUSEPORT_BPF,
            filter,
        ),
        BindingSetting::IncomingProcessor(Some(processor)) => sock(&socket)
            .set_cpu_affinity(
                usize::try_from(processor.0).map_err(|_| HostError::Errno(libc::EINVAL))?,
            )
            .map_err(|error| io_host_error(&error)),
        // socket2 sets only a processor; none is Linux's -1.
        BindingSetting::IncomingProcessor(None) => set_int(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_INCOMING_CPU,
            NO_PROCESSOR,
        ),
    }
}

/// The value a filter's detach takes, which Linux ignores.
///
/// https://man7.org/linux/man-pages/man7/socket.7.html
const DETACH_VALUE: c_int = 0;

/// The `SO_INCOMING_CPU` value that ties the socket to no processor.
///
/// https://man7.org/linux/man-pages/man7/socket.7.html
pub const NO_PROCESSOR: c_int = -1;

/// The hop limit value that takes the route's.
///
/// https://man7.org/linux/man-pages/man7/ipv6.7.html
const ROUTE_HOP_LIMIT: c_int = -1;

fn path_mtu_discovery(mode: PathMtuDiscovery) -> u32 {
    match mode {
        PathMtuDiscovery::Dont => uapi::IP_PMTUDISC_DONT,
        PathMtuDiscovery::Want => uapi::IP_PMTUDISC_WANT,
        PathMtuDiscovery::Do => uapi::IP_PMTUDISC_DO,
        PathMtuDiscovery::Probe => uapi::IP_PMTUDISC_PROBE,
        PathMtuDiscovery::Interface => uapi::IP_PMTUDISC_INTERFACE,
        PathMtuDiscovery::Omit => uapi::IP_PMTUDISC_OMIT,
    }
}

/// Sets a field the socket's stack gives every packet it sends.
pub fn set_packet_setting(
    socket: BorrowedFd<'_>,
    setting: &IpPacketSetting<'_>,
    shortfalls: &Shortfalls,
) -> Result<(), HostError> {
    let default_or = |limit: Option<wcontainers_internet_protocol::HopLimit>| {
        limit.map_or(ROUTE_HOP_LIMIT, |limit| c_int::from(limit.0))
    };
    match setting {
        IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit,
        } => set_int(socket, uapi::SOL_IP, uapi::IP_TTL, default_or(*limit)),
        IpPacketSetting::HopLimit {
            version: IpVersion::V6,
            limit,
        } => set_int(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_UNICAST_HOPS,
            default_or(*limit),
        ),
        IpPacketSetting::TrafficClass { version, dscp } => {
            let class = u32::from(dscp.value()) << ECN_FIELD_BITS;
            match version {
                IpVersion::V4 => sock(&socket).set_tos_v4(class),
                IpVersion::V6 => sock(&socket).set_tclass_v6(class),
            }
            .map_err(|error| io_host_error(&error))
        }
        IpPacketSetting::PathMtuDiscovery {
            version: IpVersion::V4,
            mode,
        } => set_int(
            socket,
            uapi::SOL_IP,
            uapi::IP_MTU_DISCOVER,
            path_mtu_discovery(*mode) as c_int,
        ),
        IpPacketSetting::PathMtuDiscovery {
            version: IpVersion::V6,
            mode,
        } => set_int(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_MTU_DISCOVER,
            path_mtu_discovery(*mode) as c_int,
        ),
        IpPacketSetting::Ipv4Options(options) => {
            set_bytes(socket, uapi::SOL_IP, uapi::IP_OPTIONS, options.as_slice())
        }
        IpPacketSetting::HopByHopOptions(header) => set_bytes(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_HOPOPTS,
            header.as_ref().map_or(&[], |h| h.slice()),
        ),
        IpPacketSetting::RoutingDestinationOptions(header) => set_bytes(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_RTHDRDSTOPTS,
            header.as_ref().map_or(&[], |h| h.slice()),
        ),
        IpPacketSetting::RoutingHeader(header) => set_bytes(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_RTHDR,
            header.as_ref().map_or(&[], |h| h.slice()),
        ),
        IpPacketSetting::DestinationOptions(header) => set_bytes(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_DSTOPTS,
            header.as_ref().map_or(&[], |h| h.slice()),
        ),
        IpPacketSetting::FlowLabel(FlowLabelChoice::Label(_)) => {
            Err(shortfalls.refuse(NO_FIXED_FLOW_LABEL))
        }
        IpPacketSetting::FlowLabel(choice) => {
            set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_FLOWINFO_SEND, false)?;
            set_flag(
                socket,
                uapi::SOL_IPV6,
                uapi::IPV6_AUTOFLOWLABEL,
                *choice == FlowLabelChoice::Automatic,
            )
        }
        IpPacketSetting::Mark(mark) => sock(&socket)
            .set_mark(mark.0)
            .map_err(|error| io_host_error(&error)),
        IpPacketSetting::Priority(priority) => sock(&socket)
            .set_priority(priority.0)
            .map_err(|error| io_host_error(&error)),
        IpPacketSetting::RouteScope(scope) => set_flag(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_DONTROUTE,
            *scope == RouteScope::Link,
        ),
        IpPacketSetting::SourcePreferences(preferences) => {
            let temporary = match preferences.temporary {
                TemporaryAddressPreference::SystemDefault => uapi::IPV6_PREFER_SRC_PUBTMP_DEFAULT,
                TemporaryAddressPreference::Temporary => uapi::IPV6_PREFER_SRC_TMP,
                TemporaryAddressPreference::Public => uapi::IPV6_PREFER_SRC_PUBLIC,
            };
            let mobile = match preferences.mobile {
                MobileAddressPreference::Home => uapi::IPV6_PREFER_SRC_HOME,
                MobileAddressPreference::CareOf => uapi::IPV6_PREFER_SRC_COA,
            };
            set_option(
                socket,
                uapi::SOL_IPV6,
                uapi::IPV6_ADDR_PREFERENCES,
                &(temporary | mobile),
            )
        }
    }
}

/// `SCM_TSTAMP_SND`, the stage of a transmit timestamp taken as the packet left, from [linux/errqueue.h](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h), which neither libc nor linux-raw-sys carries.
pub const TIMESTAMP_STAGE_SENT: u32 = 0;
/// `SCM_TSTAMP_SCHED`, taken as the packet entered the queueing discipline.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h
pub const TIMESTAMP_STAGE_SCHEDULED: u32 = 1;
/// `SCM_TSTAMP_ACK`, taken as the peer acknowledged the data.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h
pub const TIMESTAMP_STAGE_ACKNOWLEDGED: u32 = 2;
/// `SCM_TSTAMP_COMPLETION`, taken as the device completed the transmission.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h
pub const TIMESTAMP_STAGE_COMPLETED: u32 = 3;

/// The times of an `SCM_TIMESTAMPING` message, `struct scm_timestamping`: software, deprecated, hardware.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h
pub const TIMESTAMPING_TIMES: usize = 3;

/// The places of the software and hardware times among them; the one between is deprecated.
const SOFTWARE_TIME: usize = 0;
const HARDWARE_TIME: usize = 2;

/// The room for the control messages of one receive: timestamps, an extended error, packet information, statistics.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-shared-crates-own-choices
const CONTROL_ROOM: usize = 1024;

/// Room for the control messages of a `recvmsg`, in words so that it is aligned as `cmsghdr` needs.
///
/// Before each receive it holds zeros, so that a walk of the control messages the receive writes ends where they end.
pub struct ControlRoom<const WORDS: usize> {
    words: [usize; WORDS],
    written: bool,
}

impl<const WORDS: usize> ControlRoom<WORDS> {
    pub fn new() -> Self {
        Self {
            words: [usize::MIN; WORDS],
            written: false,
        }
    }
}

impl<const WORDS: usize> Default for ControlRoom<WORDS> {
    fn default() -> Self {
        Self::new()
    }
}

/// The control messages of one stream receive.
pub type ControlBuffer = ControlRoom<{ CONTROL_ROOM / size_of::<usize>() }>;

/// What one `recvmsg` gave: the bytes it wrote, its flags (`MSG_*`), the sender, and the room with its control
/// messages, zeros after them.
pub struct Received<'a, T> {
    pub length: usize,
    pub flags: c_int,
    pub source: Option<T>,
    pub control: &'a [u8],
}

/// Makes one `recvmsg` into `vectors` and the room for control messages, the sender read as `S` and given as
/// `source` makes of it; `flags` are `MSG_*` flags.
///
/// Made with nix rather than socket2, whose `RecvFlags` gives no `MSG_CTRUNC`, or rustix, which hands out only the
/// control messages it parses; the messages are read from the room's bytes, not through nix's parsing, which
/// allocates for each kind it does not know.
#[inline]
pub fn receive_vectored<'a, S: SockaddrLike, T, const WORDS: usize>(
    socket: BorrowedFd<'_>,
    vectors: &mut [IoSliceMut<'_>],
    flags: c_int,
    control: &'a mut ControlRoom<WORDS>,
    source: impl FnOnce(&S) -> Option<T>,
) -> Result<Received<'a, T>, HostError> {
    if control.written {
        control.words.fill(usize::MIN);
    }
    control.written = true;
    let received = recvmsg::<S>(
        socket.as_raw_fd(),
        vectors,
        Some(control.words.as_mut_bytes()),
        MsgFlags::from_bits_retain(flags),
    )
    .map_err(|errno| host_error(Errno::from_raw_os_error(errno as c_int)))?;
    let (length, flags) = (received.bytes, received.flags.bits());
    let source = received.address.as_ref().and_then(source);
    Ok(Received {
        length,
        flags,
        source,
        control: control.words.as_bytes(),
    })
}

/// How many bytes of the control messages of a receive, read up to the zeros after them, they take.
pub fn written_length(control: &[u8]) -> usize {
    let mut at = 0;
    while let Some(length) = header_field(control.get(at..).unwrap_or_default(), CONTROL_LENGTH_AT)
    {
        let length = usize::from_ne_bytes(length);
        if length < CONTROL_DATA_AT {
            break;
        }
        at = at
            .saturating_add(control_aligned(length))
            .min(control.len());
    }
    at
}

/// What one stream `recvmsg` gave: the bytes it wrote, its flags and its control messages.
pub struct ReceivedMessage<'a> {
    pub length: usize,
    pub flags: c_int,
    pub control: &'a [u8],
}

impl<'a> ReceivedMessage<'a> {
    pub fn messages(&self) -> ControlMessages<'a> {
        ControlMessages::new(self.control)
    }
}

/// Makes one `recvmsg` into the buffer with room for control messages; `flags` are `MSG_*` flags.
pub fn receive_message<'a>(
    socket: BorrowedFd<'_>,
    into: &mut [u8],
    flags: c_int,
    control: &'a mut ControlBuffer,
) -> Result<ReceivedMessage<'a>, HostError> {
    let received = receive_vectored(
        socket,
        &mut [IoSliceMut::new(into)],
        flags,
        control,
        |_: &()| None::<()>,
    )?;
    Ok(ReceivedMessage {
        length: received.length,
        flags: received.flags,
        control: received.control,
    })
}

/// Rounds a length up to the alignment of control messages, a `size_t`'s (`CMSG_ALIGN`).
const fn control_aligned(length: usize) -> usize {
    length.next_multiple_of(size_of::<uapi::__kernel_size_t>())
}

/// Where a control message's data starts, after its header (`CMSG_DATA`).
const CONTROL_DATA_AT: usize = control_aligned(size_of::<uapi::cmsghdr>());

/// Where a control message's header holds its length, data included (`cmsg_len`).
const CONTROL_LENGTH_AT: usize = std::mem::offset_of!(uapi::cmsghdr, cmsg_len);

/// The room one control message of `length` bytes of data takes (`CMSG_SPACE`).
pub const fn control_space(length: usize) -> usize {
    control_aligned(length) + CONTROL_DATA_AT
}

/// The bytes of one field of the control message header at the start of `bytes`.
fn header_field<const N: usize>(bytes: &[u8], at: usize) -> Option<[u8; N]> {
    bytes.get(at..at + N)?.try_into().ok()
}

/// The control messages of a receive as level, type and data, read from the bytes the kernel wrote.
pub struct ControlMessages<'a> {
    rest: &'a [u8],
}

impl<'a> ControlMessages<'a> {
    pub fn new(control: &'a [u8]) -> Self {
        Self { rest: control }
    }
}

impl<'a> Iterator for ControlMessages<'a> {
    type Item = (c_int, c_int, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let rest = std::mem::take(&mut self.rest);
        let length = usize::from_ne_bytes(header_field(rest, CONTROL_LENGTH_AT)?);
        let level = c_int::from_ne_bytes(header_field(
            rest,
            std::mem::offset_of!(uapi::cmsghdr, cmsg_level),
        )?);
        let kind = c_int::from_ne_bytes(header_field(
            rest,
            std::mem::offset_of!(uapi::cmsghdr, cmsg_type),
        )?);
        // As CMSG_NXTHDR, a header that claims less than itself or more than the bytes there ends the walk.
        let data = rest.get(CONTROL_DATA_AT..length)?;
        self.rest = rest.get(control_aligned(length)..).unwrap_or_default();
        Some((level, kind, data))
    }
}

/// Reads the `timespec` at an index of the data, absent when it is zero.
fn timespec(data: &[u8], index: usize) -> Option<Duration> {
    let at = index * size_of::<libc::timespec>();
    let seconds = at + std::mem::offset_of!(libc::timespec, tv_sec);
    let nanoseconds = at + std::mem::offset_of!(libc::timespec, tv_nsec);
    let seconds = libc::time_t::from_ne_bytes(
        data.get(seconds..seconds + size_of::<libc::time_t>())?
            .try_into()
            .ok()?,
    );
    let nanoseconds = libc::c_long::from_ne_bytes(
        data.get(nanoseconds..nanoseconds + size_of::<libc::c_long>())?
            .try_into()
            .ok()?,
    );
    let duration = Duration::new(
        u64::try_from(seconds).ok()?,
        u32::try_from(nanoseconds).ok()?,
    );
    (!duration.is_zero()).then_some(duration)
}

/// Gives the software and hardware times of an `SCM_TIMESTAMPING` control message, each absent when it is zero.
pub fn timestamps(data: &[u8]) -> (Option<Duration>, Option<Duration>) {
    if data.len() < size_of::<[libc::timespec; TIMESTAMPING_TIMES]>() {
        return (None, None);
    }
    (timespec(data, SOFTWARE_TIME), timespec(data, HARDWARE_TIME))
}

pub fn family_of(address: &BindAddress) -> IpVersion {
    match address {
        BindAddress::V4(_) => IpVersion::V4,
        BindAddress::V6 { .. } => IpVersion::V6,
    }
}

pub fn socket_address(address: rustix::net::SocketAddrAny) -> Result<SocketAddr, HostError> {
    SocketAddr::try_from(address).map_err(host_error)
}

/// Makes a non-blocking socket owned by the given user, acting as that user for the call when the host's thread is another.
pub fn open_socket(
    family: IpVersion,
    kind: SocketType,
    protocol: Protocol,
    owner: UserId,
) -> Result<OwnedFd, HostError> {
    let domain = match family {
        IpVersion::V4 => Domain::IPV4,
        IpVersion::V6 => Domain::IPV6,
    };
    let kind = socket2::Type::from(kind.as_raw().cast_signed()).nonblocking();
    let protocol = socket2::Protocol::from(protocol.as_raw().get().cast_signed());
    // socket2 makes every socket close on exec.
    let open = || {
        Socket::new(domain, kind, Some(protocol))
            .map(OwnedFd::from)
            .map_err(|error| io_host_error(&error))
    };
    // An invalid id changes nothing and answers the thread's filesystem user.
    let unchanged = Uid::from_raw(libc::uid_t::MAX);
    let previous = setfsuid(unchanged);
    if previous.as_raw() == owner.0 {
        return open();
    }
    setfsuid(Uid::from_raw(owner.0));
    let socket = if setfsuid(unchanged).as_raw() == owner.0 {
        open()
    } else {
        Err(HostError::Errno(libc::EPERM))
    };
    setfsuid(previous);
    socket
}

/// Gives the ports of a choice outside its reserved ranges, as ranges.
pub fn allowed_ranges(choice: &PortChoice<'_>) -> Vec<PortRange> {
    let mut reserved: Vec<PortRange> = choice.reserved.to_vec();
    reserved.sort_by_key(|range| range.first.0);
    let mut allowed = Vec::new();
    let mut next = u32::from(choice.range.first.0);
    let last = u32::from(choice.range.last.0);
    for range in reserved {
        let (first, end) = (u32::from(range.first.0), u32::from(range.last.0));
        if first > next && next <= last {
            allowed.push((next, (first - 1).min(last)));
        }
        next = next.max(end + 1);
    }
    if next <= last {
        allowed.push((next, last));
    }
    allowed
        .into_iter()
        .map(|(first, last)| PortRange {
            first: Port(first as u16),
            last: Port(last as u16),
        })
        .collect()
}

pub fn set_port_range(socket: BorrowedFd<'_>, range: PortRange) -> Result<(), HostError> {
    let value = u32::from(range.last.0) << u16::BITS | u32::from(range.first.0);
    set_option(socket, uapi::SOL_IP, uapi::IP_LOCAL_PORT_RANGE, &value)
}

/// Runs an operation that takes a local port once per range the exit node may choose from, until one has a free port.
pub fn in_port_ranges<T>(
    socket: BorrowedFd<'_>,
    choice: Option<&PortChoice<'_>>,
    mut operation: impl FnMut() -> Result<T, HostError>,
) -> Result<T, HostError> {
    let Some(choice) = choice else {
        return operation();
    };
    let mut outcome = Err(HostError::Errno(libc::EADDRNOTAVAIL));
    for range in allowed_ranges(choice) {
        set_port_range(socket, range)?;
        outcome = operation();
        match outcome {
            Err(HostError::Errno(libc::EADDRNOTAVAIL | libc::EADDRINUSE)) => continue,
            _ => break,
        }
    }
    outcome
}

/// Binds the socket as the binding asks, returning the choice the exit node makes its port from, if it does.
///
/// `choose_now` chooses that port at once, as a listener or a datagram socket needs, not when connecting.
pub fn bind<'a>(
    socket: BorrowedFd<'_>,
    local: &LocalBinding<'a>,
    choose_now: bool,
) -> Result<Option<PortChoice<'a>>, HostError> {
    let address = bind_address(socket, &local.address)?;
    match local.port {
        LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => {
            bind_to(socket, SocketAddr::new(address, port.0)).map_err(host_error)?;
            Ok(None)
        }
        LocalPort::ChosenByExitNode(choice) => {
            let any = SocketAddr::new(address, u16::MIN);
            if choose_now {
                in_port_ranges(socket, Some(&choice), || {
                    bind_to(socket, any).map_err(host_error)
                })?;
            } else if !address.is_unspecified() {
                set_flag(socket, uapi::SOL_IP, uapi::IP_BIND_ADDRESS_NO_PORT, true)?;
                bind_to(socket, any).map_err(host_error)?;
            }
            Ok(Some(choice))
        }
    }
}

/// Gives the address of a binding, with the IPv6 families of an IPv6 one set on the socket.
pub fn bind_address(socket: BorrowedFd<'_>, address: &BindAddress) -> Result<IpAddr, HostError> {
    match *address {
        BindAddress::V4(address) => Ok(IpAddr::V4(address)),
        BindAddress::V6 { address, families } => {
            sock(&socket)
                .set_only_v6(families == Ipv6Families::V6Only)
                .map_err(|error| io_host_error(&error))?;
            Ok(IpAddr::V6(address))
        }
    }
}

pub const PORTS_OUTSIDE_THE_HOSTS_RANGE: &str =
    "ports an exit node chooses from outside the host's own range, tried one at a time";

/// Where the host keeps its own range of ports to choose from, for the caller's network.
const HOST_PORT_RANGE: &str = "/proc/sys/net/ipv4/ip_local_port_range";

/// Reads the host's own range of ports to choose from, `net.ipv4.ip_local_port_range`.
pub fn host_port_range() -> Result<PortRange, HostError> {
    let text = std::fs::read_to_string(HOST_PORT_RANGE)
        .map_err(|error| HostError::Errno(error.raw_os_error().unwrap_or(libc::EIO)))?;
    let mut ports = text.split_whitespace().map(str::parse::<u16>);
    match (ports.next(), ports.next()) {
        (Some(Ok(first)), Some(Ok(last))) => Ok(PortRange {
            first: Port(first),
            last: Port(last),
        }),
        _ => Err(HostError::Errno(libc::EIO)),
    }
}

/// Binds at once to a free port of the first allowed range of a choice that has one.
///
/// The kernel chooses in the part of a range inside the host's own range (Linux ignores a socket's range outside it),
/// and each port of the rest is tried in turn, counted.
pub fn bind_chosen_by_exit_node(
    socket: BorrowedFd<'_>,
    address: IpAddr,
    choice: &PortChoice<'_>,
    shortfalls: &Shortfalls,
) -> Result<(), HostError> {
    let host = host_port_range()?;
    let in_host = host.first.0..=host.last.0;
    for range in allowed_ranges(choice) {
        let (first, last) = (
            range.first.0.max(host.first.0),
            range.last.0.min(host.last.0),
        );
        if first <= last {
            set_port_range(
                socket,
                PortRange {
                    first: Port(first),
                    last: Port(last),
                },
            )?;
            match bind_to(socket, SocketAddr::new(address, u16::MIN)) {
                Err(Errno::ADDRINUSE) => {}
                other => return other.map_err(host_error),
            }
        }
        let mut outside = (range.first.0..=range.last.0).filter(|port| !in_host.contains(port));
        if let Some(port) = outside.next() {
            shortfalls.count(PORTS_OUTSIDE_THE_HOSTS_RANGE);
            for port in core::iter::once(port).chain(outside) {
                match bind_to(socket, SocketAddr::new(address, port)) {
                    Err(Errno::ADDRINUSE) => {}
                    other => return other.map_err(host_error),
                }
            }
        }
    }
    Err(HostError::Errno(libc::EADDRINUSE))
}

/// Gives the peer as the socket's family takes it, an IPv4 peer of an IPv6 socket mapped into IPv6.
pub fn peer_for(family: IpVersion, peer: SocketAddr) -> SocketAddr {
    match (family, peer) {
        (IpVersion::V6, SocketAddr::V4(peer)) => SocketAddr::V6(SocketAddrV6::new(
            peer.ip().to_ipv6_mapped(),
            peer.port(),
            u32::MIN,
            u32::MIN,
        )),
        _ => peer,
    }
}
