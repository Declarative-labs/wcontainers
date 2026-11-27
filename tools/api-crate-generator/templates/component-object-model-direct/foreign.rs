//! A COM object of the C side's, held by one counted reference: `Clone` is `AddRef`, `Drop` is `Release`. Its type
//! names the table it is called through, so the promise "this object implements that interface" is made once, when
//! the object is adopted, and every call out after it is one `unsafe` operation: the call itself.

use core::ffi::c_void;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ptr::{NonNull, null_mut};

use wcontainers_component_object_model::structures::Guid;

use crate::guid_bytes;
use crate::unknown::{Raw, UnknownTable};

/// A table of entry points of one COM interface.
///
/// # Safety
/// `Self` is `#[repr(C)]` and lays out the interface's table exactly as its header declares, beginning with IUnknown's
/// three entries, which `unknown` returns; and the header's documentation lets every object of the interface be
/// called and released from any thread (free threaded), which `ForeignObject`'s `Send` and `Sync` rest on. An
/// interface free threaded only under a condition (the immediate context, under multithread protection) states that
/// condition where it is made true.
#[expect(unsafe_code, reason = "the promise a table's layout and its objects' threads make")]
pub unsafe trait Interface: 'static {
    fn unknown(&self) -> &UnknownTable;
}

// SAFETY: `UnknownTable` lays out unknwn.h's IUnknown; an `IUnknown*` handed to Direct3D (an adapter, private data)
// must allow its release on any thread, since Direct3D releases it when its holder dies, on the thread that drops
// the holder's last reference.
#[expect(unsafe_code, reason = "IUnknown's table")]
unsafe impl Interface for UnknownTable {
    fn unknown(&self) -> &UnknownTable {
        self
    }
}

/// A COM object of the C side's, called through the table `T`, of which we hold one reference.
pub struct ForeignObject<T: Interface> {
    raw: NonNull<c_void>,
    interface: PhantomData<fn() -> T>,
}

// SAFETY: `Interface`'s promise: every object of `T` may be called and released from any thread.
#[expect(unsafe_code, reason = "a foreign object free threaded by its interface's promise")]
unsafe impl<T: Interface> Send for ForeignObject<T> {}

// SAFETY: as for `Send`; `&ForeignObject` only calls the object, which is free threaded.
#[expect(unsafe_code, reason = "a foreign object free threaded by its interface's promise")]
unsafe impl<T: Interface> Sync for ForeignObject<T> {}

impl<T: Interface> ForeignObject<T> {
    /// Takes over a reference the caller holds, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live COM object implementing `T`'s interface (or one derived from it), one of whose
    /// references the caller gives up.
    #[expect(unsafe_code, reason = "the one promise that a pointer is a live COM object of an interface")]
    pub unsafe fn from_owned(raw: Raw) -> Option<ForeignObject<T>> {
        NonNull::new(raw).map(|raw| ForeignObject { raw, interface: PhantomData })
    }

    /// Takes a new reference to a live COM object, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live COM object implementing `T`'s interface, for the call.
    #[expect(unsafe_code, reason = "the one promise that a pointer is a live COM object of an interface")]
    pub unsafe fn from_borrowed(raw: Raw) -> Option<ForeignObject<T>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(unsafe_code, reason = "the promise of from_borrowed's caller: a live COM object of T's interface")]
            // SAFETY: the caller's promise; the borrowed reference is not given back, as it was never ours.
            let borrowed = ManuallyDrop::new(unsafe { ForeignObject::<T>::from_owned(raw) }?);
            Some(ForeignObject::clone(&borrowed))
        };
        body()
    }

    pub fn raw(&self) -> Raw {
        self.raw.as_ptr()
    }

    /// Returns the object's table of entry points.
    pub fn entry_points(&self) -> &T {
        let first = self.raw.as_ptr().cast::<*const T>();
        #[expect(unsafe_code, reason = "the first word of a live COM object")]
        // SAFETY: the object is live while we hold our reference, and a COM object's first word points to its table.
        let table = unsafe { first.read() };
        #[expect(unsafe_code, reason = "a live COM object's table")]
        // SAFETY: the table lives as long as the object, and it is `T`'s by the promise of `from_owned`.
        let table = unsafe { &*table };
        table
    }

    /// Asks the object for another of its interfaces, by `QueryInterface`.
    ///
    /// # Safety
    /// `interface` names the interface `U` lays out.
    #[expect(unsafe_code, reason = "an interface id and the table it names")]
    pub unsafe fn query<U: Interface>(&self, interface: &Guid) -> Option<ForeignObject<U>> {
        #[deny(unsafe_code)]
        let body = move || {
            let mut answer = null_mut();
            let interface = guid_bytes(interface);
            #[expect(unsafe_code, reason = "IUnknown::QueryInterface, called out")]
            // SAFETY: a live object; the id's 16 bytes and the out-parameter are ours for the call.
            let status = unsafe { (self.entry_points().unknown().query_interface)(self.raw(), interface.as_ptr(), &mut answer) };
            if status < 0 {
                return None;
            }
            #[expect(unsafe_code, reason = "the answer of QueryInterface")]
            // SAFETY: on success the answer is a live object of the interface asked for, with one reference; the
            // caller's promise names its table.
            unsafe { ForeignObject::<U>::from_owned(answer) }
        };
        body()
    }
}

/// Equal to another holding the same interface pointer: COM's identity, for objects reached by one interface.
impl<T: Interface> PartialEq for ForeignObject<T> {
    fn eq(&self, other: &ForeignObject<T>) -> bool {
        self.raw == other.raw
    }
}

impl<T: Interface> Eq for ForeignObject<T> {}

impl<T: Interface> Hash for ForeignObject<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T: Interface> Clone for ForeignObject<T> {
    fn clone(&self) -> ForeignObject<T> {
        #[expect(unsafe_code, reason = "IUnknown::AddRef, called out")]
        // SAFETY: the object is live while we hold our reference.
        unsafe {
            (self.entry_points().unknown().add_ref)(self.raw());
        }
        ForeignObject { raw: self.raw, interface: PhantomData }
    }
}

impl<T: Interface> Drop for ForeignObject<T> {
    fn drop(&mut self) {
        #[expect(unsafe_code, reason = "IUnknown::Release, called out")]
        // SAFETY: as in `clone`; this gives back the one reference we held.
        unsafe {
            (self.entry_points().unknown().release)(self.raw());
        }
    }
}
