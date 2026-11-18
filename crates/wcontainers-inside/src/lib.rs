//! The program the wcontainers native back end runs at the privileged level of its virtual machine.

#![no_std]

pub mod boot;
pub mod build_environment;
/// ARM64's general registers, x0 to x30.
pub const GENERAL_REGISTERS: usize = 31;

/// Bytes of an ARM64 instruction.
pub const INSTRUCTION_BYTES: u64 = size_of::<u32>() as u64;

pub mod layout;
pub mod link;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub mod floor;
