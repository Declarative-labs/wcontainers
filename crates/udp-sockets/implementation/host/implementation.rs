//! UdpSockets on the UDP sockets of the machine it runs on.
//!
//! On Linux it serves every field and setting, elsewhere what socket2 covers.
#![cfg(unix)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod portable;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(not(target_os = "linux"))]
pub use portable::*;
