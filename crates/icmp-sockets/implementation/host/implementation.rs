//! IcmpSockets on the ping sockets of the Linux machine it runs on; on other systems each socket is refused and counted.
#![cfg(unix)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod portable;

#[cfg(target_os = "linux")]
use core::cell::RefCell;
#[cfg(not(target_os = "linux"))]
use core::convert::Infallible;

use wcontainers_internet_protocol_linux_socket_options::Shortfalls;
#[cfg(target_os = "linux")]
use wcontainers_internet_protocol_linux_socket_options::readiness::{Watched, Watcher};

/// ICMP echo sockets of the machine this runs on, Linux's ping sockets, with a count of each refusal by its reason.
pub struct HostIcmpSockets {
    shortfalls: Shortfalls,
    #[cfg(target_os = "linux")]
    fields_not_given: linux::PacketCount,
    #[cfg(target_os = "linux")]
    drops_asked: linux::PacketCount,
    #[cfg(target_os = "linux")]
    watcher: Watcher,
    /// Tells where Linux's lookup sends an IPv6 ping from a bound address by a bound interface.
    #[cfg(target_os = "linux")]
    routes: wcontainers_route_resolver_host::HostRouteResolver,
}

/// A ping socket; on a system without them none can be made.
#[cfg_attr(not(target_os = "linux"), derive(PartialEq, Eq, PartialOrd, Ord))]
pub struct HostIcmpSocket {
    #[cfg(target_os = "linux")]
    watched: Watched,
    #[cfg(target_os = "linux")]
    ipv6: bool,
    #[cfg(target_os = "linux")]
    state: RefCell<linux::SocketState>,
    #[cfg(not(target_os = "linux"))]
    never: Infallible,
}
