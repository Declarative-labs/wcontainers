//! The floor's cases: a test program run by the monitor's floor test, never shipped. Each case sets up pages,
//! runs a hand-written program at the guest's level and checks what came back, reporting through the link.

#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg(target_os = "none")]
mod cases;
#[cfg(target_os = "none")]
mod protocol;

/// Built for the host this program is empty: it only runs inside the virtual machine.
#[cfg(not(target_os = "none"))]
fn main() {}
