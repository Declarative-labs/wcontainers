//! The shared settings of crates/internet-protocol as Linux socket options, and the raw option calls under them.
#![cfg(target_os = "linux")]

pub mod datagram;
pub mod icmp_error;

use std::collections::BTreeMap;
use std::ffi::{c_int, c_void};
use std::mem::{MaybeUninit, size_of};
use std::net::{IpAddr, SocketAddr, SocketAddrV6};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};
use std::sync::Mutex;
use std::time::Duration;

use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use rustix::net::{AddressFamily, Protocol, SocketFlags, SocketType, sockopt};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_internet_protocol::{
    AddressSharing, BindAddress, FilterInstruction, FlowLabelChoice, IpPacketSetting,
    IpSocketSetting, IpVersion, Ipv6Families, LocalBinding, LocalPort, MobileAddressPreference,
    NonlocalAddresses, PathMtuDiscovery, Port, PortChoice, PortRange, PortSetting, PortSharing,
    RouteScope, SocketFilter, TemporaryAddressPreference, Timestamping,
};
use wcontainers_kernel::UserId;

const NO_MEMBERSHIPS: &str = "multicast and anycast memberships are not mapped to the host";

const NO_FIXED_FLOW_LABEL: &str = "a fixed IPv6 flow label is not mapped to the host";

/// The width of the ECN field at the bottom of the traffic class octet, [RFC 3168](https://www.rfc-editor.org/rfc/rfc3168#section-5).
const ECN_FIELD_BITS: u32 = 2;

pub fn host_error(errno: Errno) -> HostError {
    if errno == Errno::AGAIN {
        HostError::WouldBlock
    } else {
        HostError::Errno(errno.raw_os_error())
    }
}

/// The error of the last libc call on this thread.
pub fn last_host_error() -> HostError {
    host_error(Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::IO))
}

/// The requests a host implementation could not honour, counted by reason, each reason warned of at its first count.
#[derive(Debug, Default)]
pub struct Shortfalls {
    counts: Mutex<BTreeMap<&'static str, u64>>,
}

impl Shortfalls {
    pub fn count(&self, reason: &'static str) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = counts.entry(reason).or_default();
        *count += 1;
        if *count == 1 {
            eprintln!("warning: {reason}");
        }
    }

    /// Counts the reason and answers that the host does not support the request.
    pub fn refuse(&self, reason: &'static str) -> HostError {
        self.count(reason);
        HostError::NotSupported
    }

    pub fn readings(&self) -> Vec<CounterReading> {
        let counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        counts
            .iter()
            .map(|(&reason, &count)| CounterReading {
                reason,
                detail: 0,
                count,
                nanoseconds: 0,
            })
            .collect()
    }
}

/// Sets a socket option to the bytes of a value.
pub fn set_option<T>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: &T,
) -> Result<(), HostError> {
    set_raw(
        socket,
        level,
        name,
        (value as *const T).cast(),
        size_of::<T>(),
    )
}

pub fn set_bytes(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    bytes: &[u8],
) -> Result<(), HostError> {
    set_raw(socket, level, name, bytes.as_ptr().cast(), bytes.len())
}

pub fn set_int(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: c_int,
) -> Result<(), HostError> {
    set_option(socket, level, name, &value)
}

pub fn set_flag(socket: BorrowedFd<'_>, level: u32, name: u32, on: bool) -> Result<(), HostError> {
    set_int(socket, level, name, c_int::from(on))
}

fn set_raw(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: *const c_void,
    length: usize,
) -> Result<(), HostError> {
    let length = libc::socklen_t::try_from(length).map_err(|_| HostError::Errno(libc::EINVAL))?;
    // SAFETY: the callers give a pointer to `length` readable bytes; the descriptor is borrowed and open.
    let result = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            level as c_int,
            name as c_int,
            value,
            length,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(last_host_error())
    }
}

/// Reads a socket option into the buffer, returning the length the kernel gave.
pub fn get_bytes(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    into: &mut [u8],
) -> Result<usize, HostError> {
    let mut length =
        libc::socklen_t::try_from(into.len()).map_err(|_| HostError::Errno(libc::EINVAL))?;
    // SAFETY: the buffer is writable for `length` bytes and the kernel writes no more than that.
    let result = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            level as c_int,
            name as c_int,
            into.as_mut_ptr().cast(),
            &mut length,
        )
    };
    if result == 0 {
        Ok(length as usize)
    } else {
        Err(last_host_error())
    }
}

/// Reads a socket option whose value is plain data that every bit pattern is valid for.
pub fn get_option<T: Copy>(socket: BorrowedFd<'_>, level: u32, name: u32) -> Result<T, HostError> {
    let mut value = MaybeUninit::<T>::zeroed();
    // SAFETY: a zeroed T is a valid T for the plain uapi types this is used with, and the buffer is its bytes.
    let bytes =
        unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast::<u8>(), size_of::<T>()) };
    get_bytes(socket, level, name, bytes)?;
    // SAFETY: zeroed and then written by the kernel, so initialised.
    Ok(unsafe { value.assume_init() })
}

pub fn get_int(socket: BorrowedFd<'_>, level: u32, name: u32) -> Result<c_int, HostError> {
    get_option(socket, level, name)
}

/// The flags of `SO_TIMESTAMPING` that a socket's receive timestamping asks for.
pub fn receive_timestamping_flags(timestamping: &Timestamping) -> u32 {
    let mut flags = 0;
    if timestamping.receive_software == wcontainers_internet_protocol::Report::Requested {
        flags |= uapi::SOF_TIMESTAMPING_RX_SOFTWARE as u32 | uapi::SOF_TIMESTAMPING_SOFTWARE as u32;
    }
    if timestamping.receive_hardware == wcontainers_internet_protocol::Report::Requested {
        flags |=
            uapi::SOF_TIMESTAMPING_RX_HARDWARE as u32 | uapi::SOF_TIMESTAMPING_RAW_HARDWARE as u32;
    }
    flags
}

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
        None => (flags, 0),
    };
    let value = uapi::so_timestamping {
        flags: flags as c_int,
        bind_phc,
    };
    set_option(socket, uapi::SOL_SOCKET, uapi::SO_TIMESTAMPING, &value)
}

/// Attaches the classic BPF program with one option or detaches it with the other, detaching none being no error.
pub fn set_filter(
    socket: BorrowedFd<'_>,
    attach: u32,
    detach: u32,
    filter: Option<SocketFilter<'_>>,
) -> Result<(), HostError> {
    match filter {
        Some(SocketFilter(instructions)) => {
            let program: Vec<libc::sock_filter> = instructions
                .iter()
                .map(
                    |&FilterInstruction {
                         operation,
                         jump_if_true,
                         jump_if_false,
                         operand,
                     }| {
                        libc::sock_filter {
                            code: operation.0,
                            jt: jump_if_true.0,
                            jf: jump_if_false.0,
                            k: operand.0,
                        }
                    },
                )
                .collect();
            let value = libc::sock_fprog {
                len: u16::try_from(program.len()).map_err(|_| HostError::Errno(libc::EINVAL))?,
                filter: program.as_ptr().cast_mut(),
            };
            set_option(socket, uapi::SOL_SOCKET, attach, &value)
        }
        None => match set_int(socket, uapi::SOL_SOCKET, detach, 0) {
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
        IpSocketSetting::SendBuffer(bytes) => {
            sockopt::set_socket_send_buffer_size(socket, bytes.0).map_err(host_error)
        }
        IpSocketSetting::ReceiveBuffer(bytes) => {
            sockopt::set_socket_recv_buffer_size(socket, bytes.0).map_err(host_error)
        }
        IpSocketSetting::Timestamping(timestamping) => set_timestamping(
            socket,
            receive_timestamping_flags(&timestamping) | report_flags,
            timestamping.bound_clock,
        ),
        IpSocketSetting::MulticastLoopback(on) => match family {
            IpVersion::V4 => set_flag(socket, uapi::SOL_IP, uapi::IP_MULTICAST_LOOP, on),
            IpVersion::V6 => set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_MULTICAST_LOOP, on),
        },
        IpSocketSetting::MulticastAllGroups { version, enabled } => match version {
            IpVersion::V4 => set_flag(socket, uapi::SOL_IP, uapi::IP_MULTICAST_ALL, enabled),
            IpVersion::V6 => set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_MULTICAST_ALL, enabled),
        },
    }
}

/// Sets how the socket holds its local address and port.
pub fn set_port_setting(
    socket: BorrowedFd<'_>,
    family: IpVersion,
    setting: &PortSetting<'_>,
) -> Result<(), HostError> {
    match *setting {
        PortSetting::AddressSharing(sharing) => {
            sockopt::set_socket_reuseaddr(socket, sharing == AddressSharing::Shared)
                .map_err(host_error)
        }
        PortSetting::PortSharing(sharing) => {
            sockopt::set_socket_reuseport(socket, sharing == PortSharing::Shared)
                .map_err(host_error)
        }
        PortSetting::NonlocalAddresses(permission) => {
            let (level, free_bind, transparent) = match family {
                IpVersion::V4 => (uapi::SOL_IP, uapi::IP_FREEBIND, uapi::IP_TRANSPARENT),
                IpVersion::V6 => (uapi::SOL_IPV6, uapi::IPV6_FREEBIND, uapi::IPV6_TRANSPARENT),
            };
            set_flag(
                socket,
                level,
                transparent,
                permission == NonlocalAddresses::Transparent,
            )?;
            set_flag(
                socket,
                level,
                free_bind,
                permission == NonlocalAddresses::Bindable,
            )
        }
        PortSetting::BoundInterface(interface) => set_option(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_BINDTOIFINDEX,
            &interface.map_or(0, |interface| interface.0),
        ),
        PortSetting::GroupFilter(filter) => set_filter(
            socket,
            uapi::SO_ATTACH_REUSEPORT_CBPF,
            uapi::SO_DETACH_REUSEPORT_BPF,
            filter,
        ),
        PortSetting::IncomingProcessor(processor) => set_int(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_INCOMING_CPU,
            match processor {
                Some(processor) => {
                    c_int::try_from(processor.0).map_err(|_| HostError::Errno(libc::EINVAL))?
                }
                None => -1,
            },
        ),
    }
}

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
    family: IpVersion,
    setting: &IpPacketSetting<'_>,
    shortfalls: &Shortfalls,
) -> Result<(), HostError> {
    let default_or = |limit: Option<wcontainers_internet_protocol::HopLimit>| {
        limit.map_or(-1, |limit| c_int::from(limit.0))
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
        IpPacketSetting::TrafficClass { version, dscp, ecn } => {
            let class = c_int::from(dscp.value()) << ECN_FIELD_BITS | c_int::from(ecn.value());
            match version {
                IpVersion::V4 => set_int(socket, uapi::SOL_IP, uapi::IP_TOS, class),
                IpVersion::V6 => set_int(socket, uapi::SOL_IPV6, uapi::IPV6_TCLASS, class),
            }
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
        IpPacketSetting::Mark(mark) => set_option(socket, uapi::SOL_SOCKET, uapi::SO_MARK, &mark.0),
        IpPacketSetting::Priority(priority) => {
            set_option(socket, uapi::SOL_SOCKET, uapi::SO_PRIORITY, &priority.0)
        }
        IpPacketSetting::UnicastInterface(interface) => {
            let index = interface.map_or(0, |interface| interface.0);
            match family {
                // IPv4 takes the index in network byte order.
                IpVersion::V4 => {
                    set_option(socket, uapi::SOL_IP, uapi::IP_UNICAST_IF, &index.to_be())
                }
                IpVersion::V6 => set_option(socket, uapi::SOL_IPV6, uapi::IPV6_UNICAST_IF, &index),
            }
        }
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
pub const TIMESTAMP_STAGE_SCHEDULED: u32 = 1;
/// `SCM_TSTAMP_ACK`, taken as the peer acknowledged the data.
pub const TIMESTAMP_STAGE_ACKNOWLEDGED: u32 = 2;
/// `SCM_TSTAMP_COMPLETION`, taken as the device completed the transmission.
pub const TIMESTAMP_STAGE_COMPLETED: u32 = 3;

/// The times of an `SCM_TIMESTAMPING` message, `struct scm_timestamping` of linux/errqueue.h: software, deprecated, hardware.
pub const TIMESTAMPING_TIMES: usize = 3;

/// The room for the control messages of one receive: timestamps, an extended error with its offender, packet
/// information and the TCP statistics of a transmit report.
const CONTROL_ROOM: usize = 1024;

/// The control messages of one `recvmsg`, in words so that they are aligned as `cmsghdr` needs.
pub struct ControlBuffer([usize; CONTROL_ROOM / size_of::<usize>()]);

impl Default for ControlBuffer {
    fn default() -> Self {
        Self([0; CONTROL_ROOM / size_of::<usize>()])
    }
}

/// What one `recvmsg` gave: the bytes it wrote, its flags, and the control messages as level, type and data and whole.
pub struct ReceivedMessage<'a> {
    pub length: usize,
    pub flags: c_int,
    pub control: Vec<(c_int, c_int, &'a [u8])>,
    pub control_bytes: &'a [u8],
}

/// One `recvmsg` into the buffer with room for control messages; `flags` are `MSG_*` flags.
pub fn receive_message<'a>(
    socket: BorrowedFd<'_>,
    into: &mut [u8],
    flags: c_int,
    control: &'a mut ControlBuffer,
) -> Result<ReceivedMessage<'a>, HostError> {
    let mut vector = libc::iovec {
        iov_base: into.as_mut_ptr().cast(),
        iov_len: into.len(),
    };
    // SAFETY: msghdr is plain data; the fields set below are the only ones the call reads.
    let mut header: libc::msghdr = unsafe { std::mem::zeroed() };
    header.msg_iov = &mut vector;
    header.msg_iovlen = 1;
    header.msg_control = control.0.as_mut_ptr().cast();
    header.msg_controllen = CONTROL_ROOM as _;
    // SAFETY: the vector and the control buffer stay alive and writable for the call.
    let length = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut header, flags) };
    if length < 0 {
        return Err(last_host_error());
    }
    let mut messages = Vec::new();
    // SAFETY: the header was filled by recvmsg, so the CMSG walk stays inside the control buffer.
    unsafe {
        let mut message = libc::CMSG_FIRSTHDR(&header);
        while !message.is_null() {
            let data = libc::CMSG_DATA(message);
            let used = data.offset_from(message.cast::<u8>()) as usize;
            let data_length = ((*message).cmsg_len as usize).saturating_sub(used);
            messages.push((
                (*message).cmsg_level,
                (*message).cmsg_type,
                std::slice::from_raw_parts(data, data_length),
            ));
            message = libc::CMSG_NXTHDR(&header, message);
        }
    }
    // SAFETY: the kernel wrote `msg_controllen` bytes of control messages into the buffer.
    let control_bytes = unsafe {
        std::slice::from_raw_parts(
            control.0.as_ptr().cast::<u8>(),
            header.msg_controllen as usize,
        )
    };
    Ok(ReceivedMessage {
        length: length as usize,
        flags: header.msg_flags,
        control: messages,
        control_bytes,
    })
}

fn timespec(time: &libc::timespec) -> Option<Duration> {
    let seconds = u64::try_from(time.tv_sec).ok()?;
    let nanoseconds = u32::try_from(time.tv_nsec).ok()?;
    let duration = Duration::new(seconds, nanoseconds);
    (!duration.is_zero()).then_some(duration)
}

/// The software and hardware times of an `SCM_TIMESTAMPING` control message, each absent when it is zero.
pub fn timestamps(data: &[u8]) -> (Option<Duration>, Option<Duration>) {
    if data.len() < size_of::<[libc::timespec; TIMESTAMPING_TIMES]>() {
        return (None, None);
    }
    // SAFETY: the data holds the three timespecs of scm_timestamping, read unaligned.
    let times = unsafe {
        data.as_ptr()
            .cast::<[libc::timespec; TIMESTAMPING_TIMES]>()
            .read_unaligned()
    };
    let [software, _, hardware] = times;
    (timespec(&software), timespec(&hardware))
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
        IpVersion::V4 => AddressFamily::INET,
        IpVersion::V6 => AddressFamily::INET6,
    };
    let open = || {
        rustix::net::socket_with(
            domain,
            kind,
            SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
            Some(protocol),
        )
        .map_err(host_error)
    };
    // An invalid id changes nothing and answers the thread's filesystem user.
    // SAFETY: setfsuid only changes this thread's filesystem user, which is restored before returning.
    let previous = unsafe { libc::setfsuid(libc::uid_t::MAX) } as libc::uid_t;
    if previous == owner.0 {
        return open();
    }
    // SAFETY: as above.
    let acting = unsafe {
        libc::setfsuid(owner.0);
        libc::setfsuid(libc::uid_t::MAX) as libc::uid_t
    };
    let socket = if acting == owner.0 {
        open()
    } else {
        Err(HostError::Errno(libc::EPERM))
    };
    // SAFETY: as above.
    unsafe { libc::setfsuid(previous) };
    socket
}

/// The ports of a choice outside its reserved ranges, as ranges.
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

/// Binds the socket as the binding asks, returning the choice the exit node makes its port from, if it does;
/// `choose_now` chooses that port at once, as a listener or a datagram socket needs, not when connecting.
pub fn bind<'a>(
    socket: BorrowedFd<'_>,
    local: &LocalBinding<'a>,
    choose_now: bool,
) -> Result<Option<PortChoice<'a>>, HostError> {
    let address = match local.address {
        BindAddress::V4(address) => IpAddr::V4(address),
        BindAddress::V6 { address, families } => {
            sockopt::set_ipv6_v6only(socket, families == Ipv6Families::V6Only)
                .map_err(host_error)?;
            IpAddr::V6(address)
        }
    };
    match local.port {
        LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => {
            rustix::net::bind(socket, &SocketAddr::new(address, port.0)).map_err(host_error)?;
            Ok(None)
        }
        LocalPort::ChosenByExitNode(choice) => {
            let any = SocketAddr::new(address, 0);
            if choose_now {
                in_port_ranges(socket, Some(&choice), || {
                    rustix::net::bind(socket, &any).map_err(host_error)
                })?;
            } else if !address.is_unspecified() {
                set_flag(socket, uapi::SOL_IP, uapi::IP_BIND_ADDRESS_NO_PORT, true)?;
                rustix::net::bind(socket, &any).map_err(host_error)?;
            }
            Ok(Some(choice))
        }
    }
}

/// The peer as the socket's family takes it, an IPv4 peer of an IPv6 socket mapped into IPv6.
pub fn peer_for(family: IpVersion, peer: SocketAddr) -> SocketAddr {
    match (family, peer) {
        (IpVersion::V6, SocketAddr::V4(peer)) => SocketAddr::V6(SocketAddrV6::new(
            peer.ip().to_ipv6_mapped(),
            peer.port(),
            0,
            0,
        )),
        _ => peer,
    }
}
