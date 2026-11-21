//! The program the wcontainers native back end runs at the privileged level of its virtual machine.

#![no_std]

extern crate alloc;

pub mod boot;
pub mod build_environment;
pub mod container;
pub mod host;
pub mod layout;
pub mod link;
pub mod reason;
pub mod routines;
pub mod stop;
pub mod syndrome;
pub mod translation;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub mod floor;
#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub mod native;
