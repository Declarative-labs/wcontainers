//! The socket options a socket keeps between calls, written only when a call wants another value.

use std::os::fd::BorrowedFd;

use libc::c_int;
use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;

use crate::datagram::addresses::as_bytes;
use crate::set_bytes;

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
pub const UDP_LITE_SEND_COVERAGE: u32 = 10;
/// `UDPLITE_RECV_CSCOV` of `linux/udp.h`.
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
        let (level, name, start) = slot.option();
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
        set_bytes(fd, level, name, wanted)?;
        self.values[slot as usize] = Value::Written(wanted.to_vec());
        Ok(())
    }

    pub fn ensure<T: Copy>(
        &mut self,
        fd: BorrowedFd<'_>,
        slot: Slot,
        wanted: &T,
    ) -> Result<(), HostError> {
        self.ensure_bytes(fd, slot, as_bytes(wanted))
    }

    pub fn ensure_int(
        &mut self,
        fd: BorrowedFd<'_>,
        slot: Slot,
        wanted: c_int,
    ) -> Result<(), HostError> {
        self.ensure(fd, slot, &wanted)
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

impl Default for Sticky {
    fn default() -> Self {
        Self::new()
    }
}
