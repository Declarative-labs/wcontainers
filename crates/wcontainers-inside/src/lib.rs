//! The program the wcontainers native back end runs at the privileged level of its virtual machine.

#![no_std]

pub mod boot;
/// ARM64's general registers, x0 to x30.
pub const GENERAL_REGISTERS: usize = 31;

pub mod layout;
pub mod link;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub mod floor;
