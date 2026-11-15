//! Linux's ARM64 constants and structure layouts, copied from linux-raw-sys by `generate-linux` in
//! wcontainers-kernel-test so that they hold on every target the kernel is built for.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    dead_code,
    unused_unsafe,
    unsafe_op_in_unsafe_fn,
    clippy::all
)]

/// C's types as ARM64 Linux has them (LP64, unsigned `char`).
pub mod ctypes {
    pub type c_char = u8;
    pub type c_schar = i8;
    pub type c_uchar = u8;
    pub type c_short = i16;
    pub type c_ushort = u16;
    pub type c_int = i32;
    pub type c_uint = u32;
    pub type c_long = i64;
    pub type c_ulong = u64;
    pub type c_longlong = i64;
    pub type c_ulonglong = u64;
    pub type c_float = f32;
    pub type c_double = f64;
    pub use core::ffi::c_void;
}

pub mod auxvec;
pub mod errno;
pub mod general;
pub mod ioctl;
pub mod net;
pub mod prctl;
pub mod system;
