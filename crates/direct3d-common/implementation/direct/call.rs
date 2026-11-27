//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The calling glue: the C side's objects as an implementation of the traits.

#![allow(
    unused_variables,
    unused_imports,
    non_snake_case,
    clippy::let_unit_value,
    clippy::unit_arg,
    clippy::needless_return,
    reason = "generated: every method has the same shape"
)]

use wcontainers_component_object_model as component_object_model;
use wcontainers_direct3d_common as direct3d_common;

use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::ptr::{null, null_mut};

use wcontainers_component_object_model::constants::{
    E_FAIL, E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL, E_OUTOFMEMORY,
};
use wcontainers_component_object_model::structures::{Guid, HResult};
use wcontainers_component_object_model_direct::{
    Adopt, Aligned, ForeignLent, ForeignObject, ForeignUnknown, Raw, UnknownTable, guid_bytes,
    succeeded,
};
use wcontainers_volatile_bytes::VolatileBytes;

use crate::abi::*;
use crate::data::*;
use crate::serve::*;

/// An object of the C side's of the kind `Blob`, held by one counted reference.
pub struct SystemBlob<A> {
    object: ForeignObject<BlobTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemBlob<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3DBlob`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3DBlob` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemBlob<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3DBlob"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<BlobTable>::from_owned(raw) }?;
            Some(SystemBlob {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3DBlob`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemBlob<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live COM object, which is an IUnknown"
            )]
            // SAFETY: the caller's promise: any COM object is an IUnknown.
            let unknown = unsafe { ForeignObject::<UnknownTable>::from_owned(raw) }?;
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_common::constants::IID_ID3D_BLOB` names the interface `BlobTable` lays out.
            let object =
                unsafe { unknown.query::<BlobTable>(&direct3d_common::constants::IID_ID3D_BLOB) }?;
            Some(SystemBlob {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemBlob<A> {
    const INTERFACE: Guid = direct3d_common::constants::IID_ID3D_BLOB;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemBlob<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3DBlob, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemBlob::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemBlob<A> {
    fn clone(&self) -> SystemBlob<A> {
        SystemBlob {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemBlob<A> {
    fn eq(&self, other: &SystemBlob<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemBlob<A> {}

impl<A> Hash for SystemBlob<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `DestructionNotifier`, held by one counted reference.
pub struct SystemDestructionNotifier<A> {
    object: ForeignObject<DestructionNotifierTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemDestructionNotifier<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3DDestructionNotifier`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3DDestructionNotifier` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemDestructionNotifier<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3DDestructionNotifier"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<DestructionNotifierTable>::from_owned(raw) }?;
            Some(SystemDestructionNotifier {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3DDestructionNotifier`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemDestructionNotifier<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live COM object, which is an IUnknown"
            )]
            // SAFETY: the caller's promise: any COM object is an IUnknown.
            let unknown = unsafe { ForeignObject::<UnknownTable>::from_owned(raw) }?;
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_common::constants::IID_ID3D_DESTRUCTION_NOTIFIER` names the interface `DestructionNotifierTable` lays out.
            let object = unsafe {
                unknown.query::<DestructionNotifierTable>(
                    &direct3d_common::constants::IID_ID3D_DESTRUCTION_NOTIFIER,
                )
            }?;
            Some(SystemDestructionNotifier {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemDestructionNotifier<A> {
    const INTERFACE: Guid = direct3d_common::constants::IID_ID3D_DESTRUCTION_NOTIFIER;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemDestructionNotifier<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3DDestructionNotifier, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemDestructionNotifier::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemDestructionNotifier<A> {
    fn clone(&self) -> SystemDestructionNotifier<A> {
        SystemDestructionNotifier {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemDestructionNotifier<A> {
    fn eq(&self, other: &SystemDestructionNotifier<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemDestructionNotifier<A> {}

impl<A> Hash for SystemDestructionNotifier<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An API value whose objects are the C side's: every kind is this crate's object type.
pub trait System:
    direct3d_common::Direct3DCommon<
        Blob = SystemBlob<Self>,
        DestructionNotifier = SystemDestructionNotifier<Self>,
    >
{
}

impl<A: System> direct3d_common::Blob for SystemBlob<A> {
    fn get_buffer_pointer(&self) -> u8 {
        // Refused by its kinds (src/kinds.rs), the system never called: return: an address the system returns, valid until a later call the metadata does not name.
        Default::default()
    }
    fn get_buffer_size(&self) -> usize {
        #[expect(unsafe_code, reason = "ID3DBlob::GetBufferSize, called out")]
        // SAFETY: a live object whose table holds `ID3DBlob::GetBufferSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().blob_get_buffer_size)(self.object.raw()) };
        returned
    }
}

impl<A: System> direct3d_common::DestructionNotifier for SystemDestructionNotifier<A> {
    fn register_destruction_callback(
        &self,
        callback_fn: usize,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = callback_fn;
        let _ = data;
        // Refused by its kinds (src/kinds.rs), the system never called: callbackFn: the function pointer PFN_DESTRUCTION_CALLBACK, whose code the system would run; pData: a raw pointer with no size, which the system reads or writes as it likes.
        Err(E_NOTIMPL)
    }
    fn unregister_destruction_callback(
        &self,
        callback_id: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "ID3DDestructionNotifier::UnregisterDestructionCallback, called out"
        )]
        // SAFETY: a live object whose table holds `ID3DDestructionNotifier::UnregisterDestructionCallback` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .destruction_notifier_unregister_destruction_callback)(
                self.object.raw(),
                callback_id,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
}

/// The C side's library, as an implementation of the API's top-level trait and of those it extends.
pub struct SystemDirect3DCommon;

impl direct3d_common::Direct3DCommon for SystemDirect3DCommon {
    type Blob = SystemBlob<Self>;
    type DestructionNotifier = SystemDestructionNotifier<Self>;
}

impl System for SystemDirect3DCommon {}
