//! The program the wcontainers native back end runs at the privileged level of its virtual machine.

#![no_std]

pub mod boot;
pub mod build_environment;
pub mod layout;
pub mod link;
pub mod syndrome;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub mod floor;
