//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The part of Direct3D that versions 10, 11 and 12 share, from d3dcommon.h.

use wcontainers_component_object_model::structures::HResult;

pub mod constants;
pub mod enumerations;
pub mod structures;

/// The part of Direct3D that versions 10, 11 and 12 share, from d3dcommon.h.
pub trait Direct3DCommon: Sized + Send + Sync + 'static {
    type Blob: Blob;
    type DestructionNotifier: DestructionNotifier;
}

pub trait Blob: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    fn get_buffer_pointer(&self) -> u8;
    fn get_buffer_size(&self) -> usize;
}

#[allow(
    reviewable::explicit_shared_error::HResult,
    reason = "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)"
)]
/// The header's `ID3DDestructionNotifier`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/nn-d3dcommon-id3ddestructionotifier).
pub trait DestructionNotifier: Clone + Eq + core::hash::Hash + Send + Sync + 'static {
    /// The header's `RegisterDestructionCallback`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/nf-d3dcommon-id3ddestructionotifier-registerdestructioncallback).
    fn register_destruction_callback(
        &self,
        callback_fn: usize,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, HResult>;
    /// The header's `UnregisterDestructionCallback`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/d3dcommon/nf-d3dcommon-id3ddestructionotifier-unregisterdestructioncallback).
    fn unregister_destruction_callback(&self, callback_id: u32) -> Result<(), HResult>;
}
