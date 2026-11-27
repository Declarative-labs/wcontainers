//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The serving glue: raw C calls in, any implementation of the traits out.

#![allow(
    unused_variables,
    unused_imports,
    non_snake_case,
    clippy::let_unit_value,
    clippy::unit_arg,
    reason = "generated: every method has the same shape"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_component_object_model_direct as component_object_model_direct;
use wcontainers_direct3d_common as direct3d_common;

use core::marker::PhantomData;
use std::any::Any;
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_component_object_model::constants::{
    E_FAIL, E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER,
};
use wcontainers_component_object_model::structures::Foreign;
use wcontainers_component_object_model::structures::Guid;
use wcontainers_component_object_model_direct::{
    ForeignUnknown, Found, IID_IUNKNOWN, Interfaces, Lendings, MOST_TEXT_UNITS, Raw, Recorded,
    Shadow, Table, counted, guarded, made_by_of, object, query, read_bytes, read_only_at,
    read_text, record, record_related, relate, relate_at, related, related_at, related_of, serve,
    status, unknown_table, writable_at, write_bytes,
};

use crate::KEY;
use crate::abi::*;
use crate::data::*;

/// The tables of this API's interfaces, for objects of type `V`.
pub struct Tables<V>(PhantomData<V>);

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_common::Blob> Tables<V> {
    /// `ID3DBlob`'s table for objects of type `V`.
    pub(crate) const BLOB: &'static BlobTable = &BlobTable {
        unknown: unknown_table::<V>(),
        blob_get_buffer_pointer: blob_get_buffer_pointer_refused,
        blob_get_buffer_size: blob_get_buffer_size::<V>,
    };
}

#[allow(
    dead_code,
    reason = "every interface's table is written; a kind's own uses its newest, a face another API's"
)]
impl<V: direct3d_common::DestructionNotifier> Tables<V> {
    /// `ID3DDestructionNotifier`'s table for objects of type `V`.
    pub const DESTRUCTION_NOTIFIER: &'static DestructionNotifierTable = &DestructionNotifierTable {
        unknown: unknown_table::<V>(),
        destruction_notifier_register_destruction_callback:
            destruction_notifier_register_destruction_callback_refused,
        destruction_notifier_unregister_destruction_callback:
            destruction_notifier_unregister_destruction_callback::<V>,
    };
}

/// The tables an object of the kind `Blob` answers, by interface; IUnknown's is the first.
pub fn blob_interfaces<V: direct3d_common::Blob>(interface: &Guid) -> Option<Table> {
    if *interface == IID_IUNKNOWN || [direct3d_common::constants::IID_ID3D_BLOB].contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `BlobTable` lays out `ID3DBlob`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::BLOB) });
    }
    None
}

/// The tables an object of the kind `DestructionNotifier` answers, by interface; IUnknown's is the first.
pub fn destruction_notifier_interfaces<V: direct3d_common::DestructionNotifier>(
    interface: &Guid,
) -> Option<Table> {
    if *interface == IID_IUNKNOWN
        || [direct3d_common::constants::IID_ID3D_DESTRUCTION_NOTIFIER].contains(interface)
    {
        #[expect(unsafe_code, reason = "a table of entry points the C side calls")]
        // SAFETY: `DestructionNotifierTable` lays out `ID3DDestructionNotifier`, and each of its entries checks the type of the object it finds.
        return Some(unsafe { Table::new(Tables::<V>::DESTRUCTION_NOTIFIER) });
    }
    None
}

/// `ID3DBlob::GetBufferSize` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3DBlob::GetBufferSize, which the C side calls"
)]
pub(crate) unsafe extern "system" fn blob_get_buffer_size<V: direct3d_common::Blob>(
    this: Raw,
) -> usize {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(None, move || {
            let found = object::<V>(KEY, this.addr())?;
            let returned = found.value().get_buffer_size();
            drop(found);
            Some((returned,))
        });
        answer.map_or(Default::default(), |written| written.0)
    };
    guarded(Default::default(), body)
}

/// `ID3DBlob::GetBufferPointer` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers null.
pub(crate) extern "system" fn blob_get_buffer_pointer_refused(_this: Raw) -> *mut u8 {
    core::ptr::null_mut()
}

/// `ID3DDestructionNotifier::UnregisterDestructionCallback` as the C side calls it.
///
/// # Safety
/// Every pointer is what the metadata describes for this entry: a pointer of ours or of the C side's, or null.
#[expect(
    unsafe_code,
    reason = "ID3DDestructionNotifier::UnregisterDestructionCallback, which the C side calls"
)]
pub unsafe extern "system" fn destruction_notifier_unregister_destruction_callback<
    V: direct3d_common::DestructionNotifier,
>(
    this: Raw,
    callback_id: u32,
) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer = guarded(Err(E_FAIL), move || {
            let found = object::<V>(KEY, this.addr()).ok_or(E_POINTER)?;
            let callback_id = callback_id;
            let () = found.value().unregister_destruction_callback(callback_id)?;
            drop(found);
            Ok(())
        });
        wcontainers_component_object_model_direct::status(&answer)
    };
    guarded(E_FAIL.0, body)
}

/// `ID3DDestructionNotifier::RegisterDestructionCallback` as the C side calls it, refused by its form: nothing is read, the method is not called, and it
/// answers E_NOTIMPL.
///
/// # Safety
/// Every pointer it writes through is what the metadata describes for this entry: writable for the call, or null.
#[expect(
    unsafe_code,
    reason = "ID3DDestructionNotifier::RegisterDestructionCallback, which the C side calls"
)]
pub(crate) unsafe extern "system" fn destruction_notifier_register_destruction_callback_refused(
    _this: Raw,
    _p0: usize,
    _p1: *mut u8,
    p2: *mut u8,
) -> i32 {
    #[expect(
        unsafe_code,
        reason = "ID3DDestructionNotifier::RegisterDestructionCallback: pCallbackID, as the header declares it for the C side"
    )]
    // SAFETY: the metadata's `pCallbackID` is `Out`: one value of this size to write, or null, for the call.
    let _ = unsafe { write_bytes(p2, &[u8::MIN; 4]) };
    wcontainers_component_object_model::constants::E_NOTIMPL.0
}
