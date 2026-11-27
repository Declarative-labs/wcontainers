//! IUnknown in both directions: its table and the three entries every served object shares, and `ForeignUnknown`,
//! an `IUnknown*` of the C side's held by one counted reference.

use core::ffi::c_void;

use wcontainers_component_object_model::constants::{E_FAIL, E_NOINTERFACE, E_POINTER, S_OK};

use crate::foreign::ForeignObject;
use crate::key::GlueKey;
use crate::objects::{self, Counted, guarded};
use crate::pointers::{read_bytes, write_bytes};
use crate::{GUID_SIZE, guid_from};

/// This crate's own key to its bookkeeping.
const KEY: GlueKey = GlueKey::OWN;

/// A COM interface pointer, or any other pointer the C side passes.
pub type Raw = *mut c_void;

/// IUnknown's table, which begins every COM table.
#[repr(C)]
pub struct UnknownTable {
    pub query_interface: unsafe extern "system" fn(Raw, *const u8, *mut Raw) -> i32,
    pub add_ref: unsafe extern "system" fn(Raw) -> u32,
    pub release: unsafe extern "system" fn(Raw) -> u32,
}

/// Returns IUnknown's part of the table of an object of type `V`.
pub const fn unknown_table<V: Send + Sync + 'static>() -> UnknownTable {
    UnknownTable { query_interface: query_interface::<V>, add_ref: add_ref::<V>, release: release::<V> }
}

#[expect(unsafe_code, reason = "IUnknown::QueryInterface, which the C side calls")]
unsafe extern "system" fn query_interface<V: Send + Sync + 'static>(this: Raw, interface: *const u8, object: *mut Raw) -> i32 {
    #[deny(unsafe_code)]
    let body = move || {
        let answer: Option<Counted> = guarded(None, move || {
            let mut bytes = [0_u8; GUID_SIZE];
            #[expect(unsafe_code, reason = "IUnknown::QueryInterface's riid")]
            // SAFETY: the metadata's `riid` is `In` and `Const`: one GUID, readable for the call.
            let read = unsafe { read_bytes(interface, &mut bytes) };
            read.then_some(())?;
            objects::query::<V>(KEY, this.addr(), &guid_from(&bytes))
        });
        let found = answer.is_some();
        if object.is_null() {
            return E_POINTER.0;
        }
        let address = answer.map_or(0, |answer| answer.into_address().get());
        #[expect(unsafe_code, reason = "IUnknown::QueryInterface's ppvObject")]
        // SAFETY: the metadata's `ppvObject` is `Out` and `ComOutPtr`: one pointer to write for the call.
        let _ = unsafe { write_bytes(object.cast::<u8>(), &address.to_ne_bytes()) };
        if found { S_OK.0 } else { E_NOINTERFACE.0 }
    };
    guarded(E_FAIL.0, body)
}

extern "system" fn add_ref<V: Send + Sync + 'static>(this: Raw) -> u32 {
    guarded(0, || objects::add_reference::<V>(KEY, this.addr()))
}

extern "system" fn release<V: Send + Sync + 'static>(this: Raw) -> u32 {
    guarded(0, || objects::release::<V>(KEY, this.addr()))
}

/// Any COM object of the C side's, as an `IUnknown*`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ForeignUnknown(ForeignObject<UnknownTable>);

impl ForeignUnknown {
    /// Takes a new reference to the C side's object, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live COM object, for the call, which may be released on any thread.
    #[expect(unsafe_code, reason = "an IUnknown* the C side passes in")]
    pub unsafe fn from_borrowed(raw: Raw) -> Option<ForeignUnknown> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(unsafe_code, reason = "an IUnknown* the C side passes in")]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<UnknownTable>::from_borrowed(raw) };
            object.map(ForeignUnknown)
        };
        body()
    }

    /// Returns the pointer, which stays live while this lives.
    pub fn raw(&self) -> Raw {
        self.0.raw()
    }
}
