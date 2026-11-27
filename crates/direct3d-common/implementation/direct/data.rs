//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! Every structure this crate passes, as the bytes C lays it out in.

#![allow(
    dead_code,
    unused_imports,
    reason = "each structure's both forms are written; a method may use one"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_component_object_model_direct as component_object_model_direct;

use core::mem::offset_of;

macro_rules! primitive_bytes {
    ($($get:ident $put:ident $type:ty;)*) => {$(
        /// Reads the value at this offset, as C lays it out (little-endian).
        pub fn $get(bytes: &[u8], at: usize) -> $type {
            let mut value = [u8::MIN; size_of::<$type>()];
            value.copy_from_slice(&bytes[at..at + size_of::<$type>()]);
            <$type>::from_le_bytes(value)
        }

        /// Writes the value at this offset, as C lays it out (little-endian).
        pub fn $put(bytes: &mut [u8], at: usize, value: $type) {
            bytes[at..at + size_of::<$type>()].copy_from_slice(&value.to_le_bytes());
        }
    )*};
}

primitive_bytes! {
    get_u8 put_u8 u8;
    get_i8 put_i8 i8;
    get_u16 put_u16 u16;
    get_i16 put_i16 i16;
    get_u32 put_u32 u32;
    get_i32 put_i32 i32;
    get_u64 put_u64 u64;
    get_i64 put_i64 i64;
    get_f32 put_f32 f32;
    get_f64 put_f64 f64;
    get_usize put_usize usize;
    get_isize put_isize isize;
}

/// Reads a GUID at this offset.
pub(crate) fn get_guid(
    bytes: &[u8],
    at: usize,
) -> wcontainers_component_object_model::structures::Guid {
    let mut value = [u8::MIN; wcontainers_component_object_model_direct::GUID_SIZE];
    value.copy_from_slice(&bytes[at..at + wcontainers_component_object_model_direct::GUID_SIZE]);
    wcontainers_component_object_model_direct::guid_from(&value)
}

/// Writes a GUID at this offset.
pub(crate) fn put_guid(
    bytes: &mut [u8],
    at: usize,
    value: &wcontainers_component_object_model::structures::Guid,
) {
    let value = wcontainers_component_object_model_direct::guid_bytes(value);
    bytes[at..at + value.len()].copy_from_slice(&value);
}

/// Reads a GUID from its 16 bytes.
pub(crate) fn decode_guid(bytes: &[u8]) -> wcontainers_component_object_model::structures::Guid {
    get_guid(bytes, usize::MIN)
}

/// Writes a GUID as its 16 bytes.
pub(crate) fn encode_guid(
    value: &wcontainers_component_object_model::structures::Guid,
    bytes: &mut [u8],
) {
    put_guid(bytes, usize::MIN, value);
}

/// A GUID as C lays it out: its 16 bytes, aligned as its first field (a 32-bit word).
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct GuidLayout(
    [u32; wcontainers_component_object_model_direct::GUID_SIZE / size_of::<u32>()],
);
