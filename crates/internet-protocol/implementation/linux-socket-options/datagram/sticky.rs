//! The socket options a socket keeps between calls, written only when a call wants another value.

use std::mem::size_of;
use std::os::fd::BorrowedFd;

use libc::c_int;
use linux_raw_sys::net as uapi;
use nix::sys::socket::{TimestampingFlag, setsockopt, sockopt};
use rustix::io::Errno;
use wcontainers_abi::HostError;
use wcontainers_linux_socket_options::LinuxSocketOptions;
use zerocopy::{Immutable, IntoBytes};

use crate::{LINUX, host_error, io_host_error, set_flag, set_int, sock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    TypeOfService,
    Ipv4Options,
    Ipv4PathMtuDiscovery,
    Ipv4UnicastInterface,
    Ipv4Transparent,
    Ipv4Freebind,
    TrafficClass,
    Ipv6PathMtuDiscovery,
    Ipv6DontFragment,
    Ipv6FragmentMtu,
    Ipv6UnicastInterface,
    Ipv6Transparent,
    Ipv6Freebind,
    HopByHopOptions,
    RoutingDestinationOptions,
    RoutingHeader,
    DestinationOptions,
    AddressPreferences,
    Mark,
    Priority,
    DontRoute,
    Broadcast,
    Timestamping,
    TransmitTime,
    NoChecksum,
    NoChecksumIpv6,
    ChecksumCoverage,
}

/// What a slot holds before the implementation first writes it.
enum Start {
    Zero,
    Empty,
    Unknown,
}

const SLOTS: usize = Slot::ChecksumCoverage as usize + 1;

/// `UDPLITE_SEND_CSCOV` of `linux/udp.h`, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub const UDP_LITE_SEND_COVERAGE: u32 = 10;
/// `UDPLITE_RECV_CSCOV` of `linux/udp.h`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub const UDP_LITE_RECEIVE_COVERAGE: u32 = 11;

impl Slot {
    fn option(self) -> (u32, u32, Start) {
        use Start::{Empty, Unknown, Zero};
        let (ip, ipv6, socket) = (uapi::SOL_IP, uapi::SOL_IPV6, uapi::SOL_SOCKET);
        match self {
            Slot::TypeOfService => (ip, uapi::IP_TOS, Zero),
            Slot::Ipv4Options => (ip, uapi::IP_OPTIONS, Empty),
            Slot::Ipv4PathMtuDiscovery => (ip, uapi::IP_MTU_DISCOVER, Unknown),
            Slot::Ipv4UnicastInterface => (ip, uapi::IP_UNICAST_IF, Zero),
            Slot::Ipv4Transparent => (ip, uapi::IP_TRANSPARENT, Zero),
            Slot::Ipv4Freebind => (ip, uapi::IP_FREEBIND, Zero),
            Slot::TrafficClass => (ipv6, uapi::IPV6_TCLASS, Zero),
            Slot::Ipv6PathMtuDiscovery => (ipv6, uapi::IPV6_MTU_DISCOVER, Unknown),
            Slot::Ipv6DontFragment => (ipv6, uapi::IPV6_DONTFRAG, Zero),
            Slot::Ipv6FragmentMtu => (ipv6, uapi::IPV6_MTU, Zero),
            Slot::Ipv6UnicastInterface => (ipv6, uapi::IPV6_UNICAST_IF, Zero),
            Slot::Ipv6Transparent => (ipv6, uapi::IPV6_TRANSPARENT, Zero),
            Slot::Ipv6Freebind => (ipv6, uapi::IPV6_FREEBIND, Zero),
            Slot::HopByHopOptions => (ipv6, uapi::IPV6_HOPOPTS, Empty),
            Slot::RoutingDestinationOptions => (ipv6, uapi::IPV6_RTHDRDSTOPTS, Empty),
            Slot::RoutingHeader => (ipv6, uapi::IPV6_RTHDR, Empty),
            Slot::DestinationOptions => (ipv6, uapi::IPV6_DSTOPTS, Empty),
            Slot::AddressPreferences => (ipv6, uapi::IPV6_ADDR_PREFERENCES, Unknown),
            Slot::Mark => (socket, uapi::SO_MARK, Zero),
            Slot::Priority => (socket, uapi::SO_PRIORITY, Zero),
            Slot::DontRoute => (socket, uapi::SO_DONTROUTE, Zero),
            Slot::Broadcast => (socket, uapi::SO_BROADCAST, Zero),
            Slot::Timestamping => (socket, uapi::SO_TIMESTAMPING, Unknown),
            Slot::TransmitTime => (socket, uapi::SO_TXTIME, Empty),
            Slot::NoChecksum => (socket, uapi::SO_NO_CHECK, Zero),
            Slot::NoChecksumIpv6 => (libc::SOL_UDP as u32, libc::UDP_NO_CHECK6_TX as u32, Zero),
            Slot::ChecksumCoverage => (uapi::IPPROTO_UDPLITE as u32, UDP_LITE_SEND_COVERAGE, Zero),
        }
    }
}

/// The last value written to each slot of one socket.
pub struct Sticky {
    values: [Value; SLOTS],
}

enum Value {
    /// What Linux gives a new socket.
    Start,
    Written(Vec<u8>),
    /// Written by another path, to a value not kept here.
    Unknown,
}

impl Sticky {
    pub fn new() -> Self {
        Self {
            values: [const { Value::Start }; SLOTS],
        }
    }

    pub fn ensure_bytes(
        &mut self,
        fd: BorrowedFd<'_>,
        slot: Slot,
        wanted: &[u8],
    ) -> Result<(), HostError> {
        self.ensure_with(slot, wanted, || write(fd, slot, wanted))
    }

    /// Writes a slot's option with `write` unless the slot already holds `wanted`.
    fn ensure_with(
        &mut self,
        slot: Slot,
        wanted: &[u8],
        write: impl FnOnce() -> Result<(), HostError>,
    ) -> Result<(), HostError> {
        let (_, _, start) = slot.option();
        let unchanged = match &self.values[slot as usize] {
            Value::Written(current) => current.as_slice() == wanted,
            Value::Unknown => false,
            Value::Start => match start {
                Start::Zero => wanted.iter().all(|byte| *byte == u8::MIN),
                Start::Empty => wanted.is_empty(),
                Start::Unknown => false,
            },
        };
        if unchanged {
            return Ok(());
        }
        write()?;
        self.values[slot as usize] = Value::Written(wanted.to_vec());
        Ok(())
    }

    pub fn ensure<T: IntoBytes + Immutable>(
        &mut self,
        fd: BorrowedFd<'_>,
        slot: Slot,
        wanted: &T,
    ) -> Result<(), HostError> {
        self.ensure_bytes(fd, slot, wanted.as_bytes())
    }

    pub fn ensure_int(
        &mut self,
        fd: BorrowedFd<'_>,
        slot: Slot,
        wanted: c_int,
    ) -> Result<(), HostError> {
        self.ensure_with(slot, wanted.as_bytes(), || {
            write(fd, slot, wanted.as_bytes())
        })
    }

    pub fn ensure_flag(
        &mut self,
        fd: BorrowedFd<'_>,
        slot: Slot,
        on: bool,
    ) -> Result<(), HostError> {
        self.ensure_int(fd, slot, c_int::from(on))
    }

    /// Forgets a slot's value after another path wrote its option, so that the next call writes it again.
    pub fn forget(&mut self, slot: Slot) {
        self.values[slot as usize] = Value::Unknown;
    }
}

/// The bytes of a slot's value as the two 4-byte words of `struct so_timestamping` or `struct sock_txtime`.
fn words(value: &[u8]) -> Result<(u32, u32), HostError> {
    let word = |at: usize| {
        value
            .get(at..at + size_of::<u32>())
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_ne_bytes)
    };
    match (value.len(), word(usize::MIN), word(size_of::<u32>())) {
        (TWO_WORDS, Some(first), Some(second)) => Ok((first, second)),
        _ => Err(HostError::Errno(libc::EINVAL)),
    }
}

/// The length of a slot's value of two words.
const TWO_WORDS: usize = 2 * size_of::<u32>();

/// Writes a slot's option with the value's bytes, through the typed call that covers the option.
fn write(fd: BorrowedFd<'_>, slot: Slot, value: &[u8]) -> Result<(), HostError> {
    let int = || {
        value
            .try_into()
            .map(c_int::from_ne_bytes)
            .map_err(|_| HostError::Errno(libc::EINVAL))
    };
    let io = |result: std::io::Result<()>| result.map_err(|error| io_host_error(&error));
    let nix = |result: nix::Result<()>| {
        result.map_err(|errno| host_error(Errno::from_raw_os_error(errno as c_int)))
    };
    let socket = sock(&fd);
    let (level, name, _) = slot.option();
    match slot {
        Slot::TypeOfService => io(socket.set_tos_v4(int()?.cast_unsigned())),
        Slot::TrafficClass => io(socket.set_tclass_v6(int()?.cast_unsigned())),
        Slot::Mark => io(socket.set_mark(int()?.cast_unsigned())),
        Slot::Priority => io(socket.set_priority(int()?.cast_unsigned())),
        Slot::Broadcast => io(socket.set_broadcast(int()? != c_int::default())),
        Slot::Ipv4Freebind => io(socket.set_freebind_v4(int()? != c_int::default())),
        Slot::Ipv6Freebind => io(socket.set_freebind_v6(int()? != c_int::default())),
        Slot::Ipv4Transparent => io(socket.set_ip_transparent_v4(int()? != c_int::default())),
        Slot::Ipv6Transparent => io(socket.set_ip_transparent_v6(int()? != c_int::default())),
        Slot::Ipv6DontFragment => nix(setsockopt(
            &fd,
            sockopt::Ipv6DontFrag,
            &(int()? != c_int::default()),
        )),
        Slot::DontRoute => set_flag(fd, level, name, int()? != c_int::default()),
        Slot::Ipv4PathMtuDiscovery | Slot::Ipv6PathMtuDiscovery => set_int(fd, level, name, int()?),
        Slot::Ipv6FragmentMtu => LINUX.set_ipv6_mtu(fd, int()?),
        Slot::Ipv4UnicastInterface => LINUX.set_ip_unicast_if(fd, int()?.cast_unsigned()),
        Slot::Ipv6UnicastInterface => LINUX.set_ipv6_unicast_if(fd, int()?.cast_unsigned()),
        Slot::AddressPreferences => LINUX.set_ipv6_addr_preferences(fd, int()?.cast_unsigned()),
        Slot::NoChecksum => LINUX.set_so_no_check(fd, int()? != c_int::default()),
        Slot::NoChecksumIpv6 => LINUX.set_udp_no_check6_tx(fd, int()? != c_int::default()),
        Slot::ChecksumCoverage => LINUX.set_udplite_send_cscov(fd, int()?),
        Slot::Ipv4Options => LINUX.set_ip_options(fd, value),
        Slot::HopByHopOptions => LINUX.set_ipv6_hopopts(fd, value),
        Slot::RoutingDestinationOptions => LINUX.set_ipv6_rthdrdstopts(fd, value),
        Slot::RoutingHeader => LINUX.set_ipv6_rthdr(fd, value),
        Slot::DestinationOptions => LINUX.set_ipv6_dstopts(fd, value),
        Slot::Timestamping => {
            let (flags, bind_phc) = words(value)?;
            if flags & uapi::SOF_TIMESTAMPING_BIND_PHC as u32 == u32::MIN {
                // Without a bound clock the 4-byte flags are the whole value, as Linux reads a shorter one.
                nix(setsockopt(
                    &fd,
                    sockopt::Timestamping,
                    &TimestampingFlag::from_bits_retain(flags),
                ))
            } else {
                LINUX.set_so_timestamping(
                    fd,
                    &uapi::so_timestamping {
                        flags: flags.cast_signed(),
                        bind_phc: bind_phc.cast_signed(),
                    },
                )
            }
        }
        Slot::TransmitTime => {
            let (clockid, flags) = words(value)?;
            nix(setsockopt(
                &fd,
                sockopt::TxTime,
                &libc::sock_txtime {
                    clockid: clockid.cast_signed(),
                    flags,
                },
            ))
        }
    }
}

impl Default for Sticky {
    fn default() -> Self {
        Self::new()
    }
}
