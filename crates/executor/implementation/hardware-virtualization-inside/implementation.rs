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

// The inside keeps the guest's vector registers in the hardware, and native.rs calls host routines that overwrite
// v8 to v15, which a call must keep: so no Rust code of the inside may use a vector register, which a target with
// NEON lets the compiler do anywhere.
#[cfg(all(target_arch = "aarch64", target_os = "none", target_feature = "neon"))]
compile_error!(
    "the inside is built only for aarch64-unknown-none-softfloat: it may use no vector register, since those hold the guest's and native.rs's host routines overwrite v8 to v15"
);

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub mod floor;
#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub mod native;
