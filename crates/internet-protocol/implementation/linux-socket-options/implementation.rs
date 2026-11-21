//! The shared settings of crates/internet-protocol as Linux socket options, and the raw option calls under them.
#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::ffi::{c_int, c_void};
use std::mem::{MaybeUninit, size_of};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::Mutex;
use std::time::Duration;

use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use rustix::net::sockopt;
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_internet_protocol::{
    AddressSharing, FilterInstruction, FlowLabelChoice, IpPacketSetting, IpSocketSetting,
    IpVersion, MobileAddressPreference, NonlocalAddresses, PathMtuDiscovery, PortSetting,
    PortSharing, RouteScope, SocketFilter, TemporaryAddressPreference, TimestampSource,
    Timestamping, TransmitEvent, TransmitStage,
};

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
const TIMESTAMPING_TIMES: usize = 3;

/// The room for the control messages of one receive: timestamps, an extended error with its offender, packet information.
const CONTROL_ROOM: usize = 512;

/// The control messages of one `recvmsg`, in words so that they are aligned as `cmsghdr` needs.
pub struct ControlBuffer([usize; CONTROL_ROOM / size_of::<usize>()]);

impl Default for ControlBuffer {
    fn default() -> Self {
        Self([0; CONTROL_ROOM / size_of::<usize>()])
    }
}

/// What one `recvmsg` gave: the bytes it wrote, its flags, and the control messages as level, type and data.
pub struct ReceivedMessage<'a> {
    pub length: usize,
    pub flags: c_int,
    pub control: Vec<(c_int, c_int, &'a [u8])>,
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
    Ok(ReceivedMessage {
        length: length as usize,
        flags: header.msg_flags,
        control: messages,
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

/// One entry of a socket's error queue.
pub struct ErrorQueueMessage {
    pub error: libc::sock_extended_err,
    pub offender: Option<SocketAddr>,
    pub software: Option<Duration>,
    pub hardware: Option<Duration>,
    pub length: usize,
}

fn socket_address(address: *const libc::sockaddr) -> Option<SocketAddr> {
    // SAFETY: the address is the offender right after a sock_extended_err in its control message; read by family.
    unsafe {
        match c_int::from((*address).sa_family) {
            libc::AF_INET => {
                let address = address.cast::<libc::sockaddr_in>().read_unaligned();
                Some(SocketAddr::from((
                    Ipv4Addr::from(u32::from_be(address.sin_addr.s_addr)),
                    u16::from_be(address.sin_port),
                )))
            }
            libc::AF_INET6 => {
                let address = address.cast::<libc::sockaddr_in6>().read_unaligned();
                Some(SocketAddr::from((
                    Ipv6Addr::from(address.sin6_addr.s6_addr),
                    u16::from_be(address.sin6_port),
                )))
            }
            _ => None,
        }
    }
}

/// Takes the oldest entry of the socket's error queue, its payload written into the buffer.
pub fn take_error_queue_message(
    socket: BorrowedFd<'_>,
    into: &mut [u8],
) -> Result<ErrorQueueMessage, HostError> {
    let mut control = ControlBuffer::default();
    let message = receive_message(
        socket,
        into,
        libc::MSG_ERRQUEUE | libc::MSG_DONTWAIT,
        &mut control,
    )?;
    let mut entry = ErrorQueueMessage {
        // SAFETY: sock_extended_err is plain data.
        error: unsafe { std::mem::zeroed() },
        offender: None,
        software: None,
        hardware: None,
        length: message.length,
    };
    for &(level, kind, data) in &message.control {
        let extended_error = (level == libc::SOL_IP && kind == libc::IP_RECVERR)
            || (level == libc::SOL_IPV6 && kind == libc::IPV6_RECVERR);
        if extended_error && data.len() >= size_of::<libc::sock_extended_err>() {
            let error = data.as_ptr().cast::<libc::sock_extended_err>();
            // SAFETY: the data holds a sock_extended_err, the offender's address after it.
            unsafe {
                entry.error = error.read_unaligned();
                if data.len() > size_of::<libc::sock_extended_err>() {
                    entry.offender = socket_address(libc::SO_EE_OFFENDER(error).cast_const());
                }
            }
        } else if level == libc::SOL_SOCKET && kind == libc::SCM_TIMESTAMPING {
            (entry.software, entry.hardware) = timestamps(data);
        }
    }
    Ok(entry)
}

/// The transmit event an error queue entry reports, if it is one.
pub fn transmit_event(entry: &ErrorQueueMessage) -> Option<TransmitEvent> {
    match entry.error.ee_origin {
        libc::SO_EE_ORIGIN_TIMESTAMPING => {
            let stage = match entry.error.ee_info {
                TIMESTAMP_STAGE_SENT => TransmitStage::Sent,
                TIMESTAMP_STAGE_SCHEDULED => TransmitStage::Scheduled,
                TIMESTAMP_STAGE_ACKNOWLEDGED => TransmitStage::Acknowledged,
                TIMESTAMP_STAGE_COMPLETED => TransmitStage::Completed,
                _ => return None,
            };
            let (source, time) = match (entry.hardware, entry.software) {
                (Some(time), _) => (TimestampSource::Hardware, time),
                (None, Some(time)) => (TimestampSource::Software, time),
                (None, None) => return None,
            };
            Some(TransmitEvent::Timestamp {
                stage,
                source,
                time,
            })
        }
        _ => None,
    }
}
