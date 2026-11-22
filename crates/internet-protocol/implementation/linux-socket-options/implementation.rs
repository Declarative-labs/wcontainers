//! The shared settings of crates/internet-protocol as host socket options, for the host socket implementations.
//!
//! On Linux all of them, elsewhere what socket2 covers.
#![cfg(unix)]

#[cfg(target_os = "linux")]
pub mod datagram;
pub mod errno;
mod host;
#[cfg(target_os = "linux")]
pub mod icmp_error;
#[cfg(target_os = "linux")]
mod linux;
pub mod readiness;

pub use host::*;
#[cfg(target_os = "linux")]
pub use linux::*;
