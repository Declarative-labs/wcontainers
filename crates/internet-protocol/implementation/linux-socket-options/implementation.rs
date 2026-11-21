//! The shared settings of crates/internet-protocol as host socket options, for the host implementations of the
//! socket interfaces: on Linux all of them, elsewhere what socket2 covers.
#![cfg(unix)]

#[cfg(target_os = "linux")]
pub mod datagram;
mod host;
#[cfg(target_os = "linux")]
pub mod icmp_error;
#[cfg(target_os = "linux")]
mod linux;
pub mod readiness;

pub use host::*;
#[cfg(target_os = "linux")]
pub use linux::*;
