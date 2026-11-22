//! RawSockets on the raw IP sockets of the Linux machine it runs on; on other systems each socket is refused and counted.
#![cfg(unix)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod portable;

#[cfg(target_os = "linux")]
use core::cell::RefCell;
#[cfg(not(target_os = "linux"))]
use core::convert::Infallible;

#[cfg(target_os = "linux")]
use etherparse::IpNumber;
use wcontainers_internet_protocol_linux_socket_options::Shortfalls;
#[cfg(target_os = "linux")]
use wcontainers_internet_protocol_linux_socket_options::readiness::{Watched, Watcher};

/// Raw IP sockets of the machine this runs on, with a count of each refusal by its reason.
pub struct HostRawSockets {
    shortfalls: Shortfalls,
    #[cfg(target_os = "linux")]
    watcher: Watcher,
}

/// A raw IP socket; on a system without Linux's none can be made.
#[cfg_attr(not(target_os = "linux"), derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct HostRawSocket {
    #[cfg(target_os = "linux")]
    watched: Watched,
    #[cfg(target_os = "linux")]
    ipv6: bool,
    #[cfg(target_os = "linux")]
    protocol: IpNumber,
    #[cfg(target_os = "linux")]
    state: RefCell<linux::SocketState>,
    #[cfg(not(target_os = "linux"))]
    never: Infallible,
}
