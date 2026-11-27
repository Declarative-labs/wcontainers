//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The C tables of this API's interfaces, and its library's exports.

use wcontainers_component_object_model_direct::{Interface, Raw, UnknownTable};

/// `ID3DBlob`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub(crate) struct BlobTable {
    pub(crate) unknown: UnknownTable,
    pub(crate) blob_get_buffer_pointer: unsafe extern "system" fn(Raw) -> *mut u8,
    pub(crate) blob_get_buffer_size: unsafe extern "system" fn(Raw) -> usize,
}

// SAFETY: `BlobTable` lays out `ID3DBlob` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3DBlob"
)]
unsafe impl Interface for BlobTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}

/// `ID3DDestructionNotifier`'s table of entry points, in the metadata's order; every entry is a function (none is null).
#[repr(C)]
pub struct DestructionNotifierTable {
    pub unknown: UnknownTable,
    pub destruction_notifier_register_destruction_callback:
        unsafe extern "system" fn(Raw, usize, *mut u8, *mut u8) -> i32,
    pub destruction_notifier_unregister_destruction_callback:
        unsafe extern "system" fn(Raw, u32) -> i32,
}

// SAFETY: `DestructionNotifierTable` lays out `ID3DDestructionNotifier` as the metadata orders its methods, after IUnknown's three; its objects
// may be called and released from any thread (COM's free threading), except where a kind's glue says otherwise.
#[expect(
    unsafe_code,
    reason = "the layout and threads of the table of ID3DDestructionNotifier"
)]
unsafe impl Interface for DestructionNotifierTable {
    fn unknown(&self) -> &UnknownTable {
        &self.unknown
    }
}
