//! The shared settings of crates/internet-protocol as Linux socket options, and the raw option calls under them.
#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::ffi::{c_int, c_void};
use std::mem::{MaybeUninit, size_of};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::Mutex;

use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use rustix::net::sockopt;
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_internet_protocol::{
    AddressSharing, FilterInstruction, FlowLabelChoice, IpPacketSetting, IpSocketSetting,
    IpVersion, MobileAddressPreference, NonlocalAddresses, PathMtuDiscovery, PortSetting,
    PortSharing, RouteScope, SocketFilter, TemporaryAddressPreference, Timestamping,
};

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
        | IpSocketSetting::Anycast(_) => {
            Err(shortfalls.refuse("multicast and anycast memberships are not mapped to the host"))
        }
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
            Err(shortfalls.refuse("a fixed IPv6 flow label is not mapped to the host"))
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
