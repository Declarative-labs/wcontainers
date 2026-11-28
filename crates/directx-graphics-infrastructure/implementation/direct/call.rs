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
use wcontainers_component_object_model_direct as component_object_model_direct;
use wcontainers_directx_graphics_infrastructure as directx_graphics_infrastructure;

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

/// An object of the C side's of the kind `Adapter`, held by one counted reference.
pub struct SystemAdapter<A> {
    object: ForeignObject<Adapter4Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemAdapter<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIAdapter4`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIAdapter4` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemAdapter<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIAdapter4"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<Adapter4Table>::from_owned(raw) }?;
            Some(SystemAdapter {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIAdapter4`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemAdapter<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4` names the interface `Adapter4Table` lays out.
            let object = unsafe {
                unknown.query::<Adapter4Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                )
            }?;
            Some(SystemAdapter {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemAdapter<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemAdapter<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIAdapter4, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemAdapter::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemAdapter<A> {
    fn clone(&self) -> SystemAdapter<A> {
        SystemAdapter {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemAdapter<A> {
    fn eq(&self, other: &SystemAdapter<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemAdapter<A> {}

impl<A> Hash for SystemAdapter<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Debug`, held by one counted reference.
pub struct SystemDebug<A> {
    object: ForeignObject<Debug1Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemDebug<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIDebug1`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIDebug1` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemDebug<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDebug1"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<Debug1Table>::from_owned(raw) }?;
            Some(SystemDebug {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIDebug1`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemDebug<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1` names the interface `Debug1Table` lays out.
            let object = unsafe {
                unknown.query::<Debug1Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                )
            }?;
            Some(SystemDebug {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemDebug<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemDebug<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDebug1, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemDebug::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemDebug<A> {
    fn clone(&self) -> SystemDebug<A> {
        SystemDebug {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemDebug<A> {
    fn eq(&self, other: &SystemDebug<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemDebug<A> {}

impl<A> Hash for SystemDebug<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `DecodeSwapChain`, held by one counted reference.
pub struct SystemDecodeSwapChain<A> {
    object: ForeignObject<DecodeSwapChainTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemDecodeSwapChain<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIDecodeSwapChain`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIDecodeSwapChain` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemDecodeSwapChain<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDecodeSwapChain"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<DecodeSwapChainTable>::from_owned(raw) }?;
            Some(SystemDecodeSwapChain {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIDecodeSwapChain`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemDecodeSwapChain<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN` names the interface `DecodeSwapChainTable` lays out.
            let object = unsafe {
                unknown.query::<DecodeSwapChainTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                )
            }?;
            Some(SystemDecodeSwapChain {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemDecodeSwapChain<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemDecodeSwapChain<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDecodeSwapChain, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemDecodeSwapChain::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemDecodeSwapChain<A> {
    fn clone(&self) -> SystemDecodeSwapChain<A> {
        SystemDecodeSwapChain {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemDecodeSwapChain<A> {
    fn eq(&self, other: &SystemDecodeSwapChain<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemDecodeSwapChain<A> {}

impl<A> Hash for SystemDecodeSwapChain<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Device`, held by one counted reference.
pub struct SystemDevice<A> {
    object: ForeignObject<Device4Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemDevice<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIDevice4`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIDevice4` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemDevice<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDevice4"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<Device4Table>::from_owned(raw) }?;
            Some(SystemDevice {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIDevice4`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemDevice<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4` names the interface `Device4Table` lays out.
            let object = unsafe {
                unknown.query::<Device4Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                )
            }?;
            Some(SystemDevice {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemDevice<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemDevice<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDevice4, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemDevice::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemDevice<A> {
    fn clone(&self) -> SystemDevice<A> {
        SystemDevice {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemDevice<A> {
    fn eq(&self, other: &SystemDevice<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemDevice<A> {}

impl<A> Hash for SystemDevice<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `DisplayControl`, held by one counted reference.
pub struct SystemDisplayControl<A> {
    object: ForeignObject<DisplayControlTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemDisplayControl<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIDisplayControl`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIDisplayControl` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemDisplayControl<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDisplayControl"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<DisplayControlTable>::from_owned(raw) }?;
            Some(SystemDisplayControl {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIDisplayControl`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemDisplayControl<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL` names the interface `DisplayControlTable` lays out.
            let object = unsafe {
                unknown.query::<DisplayControlTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                )
            }?;
            Some(SystemDisplayControl {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemDisplayControl<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemDisplayControl<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIDisplayControl, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemDisplayControl::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemDisplayControl<A> {
    fn clone(&self) -> SystemDisplayControl<A> {
        SystemDisplayControl {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemDisplayControl<A> {
    fn eq(&self, other: &SystemDisplayControl<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemDisplayControl<A> {}

impl<A> Hash for SystemDisplayControl<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Factory`, held by one counted reference.
pub struct SystemFactory<A> {
    object: ForeignObject<Factory7Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemFactory<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIFactory7`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIFactory7` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemFactory<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIFactory7"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<Factory7Table>::from_owned(raw) }?;
            Some(SystemFactory {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIFactory7`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemFactory<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7` names the interface `Factory7Table` lays out.
            let object = unsafe {
                unknown.query::<Factory7Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                )
            }?;
            Some(SystemFactory {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemFactory<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemFactory<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIFactory7, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemFactory::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemFactory<A> {
    fn clone(&self) -> SystemFactory<A> {
        SystemFactory {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemFactory<A> {
    fn eq(&self, other: &SystemFactory<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemFactory<A> {}

impl<A> Hash for SystemFactory<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `FactoryMedia`, held by one counted reference.
pub struct SystemFactoryMedia<A> {
    object: ForeignObject<FactoryMediaTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemFactoryMedia<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIFactoryMedia`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIFactoryMedia` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemFactoryMedia<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIFactoryMedia"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<FactoryMediaTable>::from_owned(raw) }?;
            Some(SystemFactoryMedia {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIFactoryMedia`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemFactoryMedia<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA` names the interface `FactoryMediaTable` lays out.
            let object = unsafe {
                unknown.query::<FactoryMediaTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                )
            }?;
            Some(SystemFactoryMedia {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemFactoryMedia<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemFactoryMedia<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIFactoryMedia, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemFactoryMedia::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemFactoryMedia<A> {
    fn clone(&self) -> SystemFactoryMedia<A> {
        SystemFactoryMedia {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemFactoryMedia<A> {
    fn eq(&self, other: &SystemFactoryMedia<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemFactoryMedia<A> {}

impl<A> Hash for SystemFactoryMedia<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `InfoQueue`, held by one counted reference.
pub struct SystemInfoQueue<A> {
    object: ForeignObject<InfoQueueTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemInfoQueue<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIInfoQueue`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIInfoQueue` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemInfoQueue<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIInfoQueue"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<InfoQueueTable>::from_owned(raw) }?;
            Some(SystemInfoQueue {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIInfoQueue`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemInfoQueue<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE` names the interface `InfoQueueTable` lays out.
            let object = unsafe {
                unknown.query::<InfoQueueTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                )
            }?;
            Some(SystemInfoQueue {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemInfoQueue<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemInfoQueue<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIInfoQueue, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemInfoQueue::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemInfoQueue<A> {
    fn clone(&self) -> SystemInfoQueue<A> {
        SystemInfoQueue {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemInfoQueue<A> {
    fn eq(&self, other: &SystemInfoQueue<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemInfoQueue<A> {}

impl<A> Hash for SystemInfoQueue<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `KeyedMutex`, held by one counted reference.
pub struct SystemKeyedMutex<A> {
    object: ForeignObject<KeyedMutexTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemKeyedMutex<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIKeyedMutex`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIKeyedMutex` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemKeyedMutex<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIKeyedMutex"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<KeyedMutexTable>::from_owned(raw) }?;
            Some(SystemKeyedMutex {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIKeyedMutex`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemKeyedMutex<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX` names the interface `KeyedMutexTable` lays out.
            let object = unsafe {
                unknown.query::<KeyedMutexTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                )
            }?;
            Some(SystemKeyedMutex {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemKeyedMutex<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemKeyedMutex<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIKeyedMutex, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemKeyedMutex::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemKeyedMutex<A> {
    fn clone(&self) -> SystemKeyedMutex<A> {
        SystemKeyedMutex {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemKeyedMutex<A> {
    fn eq(&self, other: &SystemKeyedMutex<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemKeyedMutex<A> {}

impl<A> Hash for SystemKeyedMutex<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Output`, held by one counted reference.
pub struct SystemOutput<A> {
    object: ForeignObject<Output6Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemOutput<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIOutput6`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIOutput6` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemOutput<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIOutput6"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<Output6Table>::from_owned(raw) }?;
            Some(SystemOutput {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIOutput6`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemOutput<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6` names the interface `Output6Table` lays out.
            let object = unsafe {
                unknown.query::<Output6Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                )
            }?;
            Some(SystemOutput {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemOutput<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemOutput<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIOutput6, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemOutput::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemOutput<A> {
    fn clone(&self) -> SystemOutput<A> {
        SystemOutput {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemOutput<A> {
    fn eq(&self, other: &SystemOutput<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemOutput<A> {}

impl<A> Hash for SystemOutput<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `OutputDuplication`, held by one counted reference.
pub struct SystemOutputDuplication<A> {
    object: ForeignObject<OutputDuplicationTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemOutputDuplication<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIOutputDuplication`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIOutputDuplication` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemOutputDuplication<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIOutputDuplication"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<OutputDuplicationTable>::from_owned(raw) }?;
            Some(SystemOutputDuplication {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIOutputDuplication`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemOutputDuplication<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION` names the interface `OutputDuplicationTable` lays out.
            let object = unsafe {
                unknown.query::<OutputDuplicationTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                )
            }?;
            Some(SystemOutputDuplication {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemOutputDuplication<A> {
    const INTERFACE: Guid =
        directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemOutputDuplication<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIOutputDuplication, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemOutputDuplication::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemOutputDuplication<A> {
    fn clone(&self) -> SystemOutputDuplication<A> {
        SystemOutputDuplication {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemOutputDuplication<A> {
    fn eq(&self, other: &SystemOutputDuplication<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemOutputDuplication<A> {}

impl<A> Hash for SystemOutputDuplication<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Resource`, held by one counted reference.
pub struct SystemResource<A> {
    object: ForeignObject<Resource1Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemResource<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGIResource1`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGIResource1` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemResource<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIResource1"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<Resource1Table>::from_owned(raw) }?;
            Some(SystemResource {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGIResource1`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemResource<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1` names the interface `Resource1Table` lays out.
            let object = unsafe {
                unknown.query::<Resource1Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                )
            }?;
            Some(SystemResource {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemResource<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemResource<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGIResource1, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemResource::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemResource<A> {
    fn clone(&self) -> SystemResource<A> {
        SystemResource {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemResource<A> {
    fn eq(&self, other: &SystemResource<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemResource<A> {}

impl<A> Hash for SystemResource<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Surface`, held by one counted reference.
pub struct SystemSurface<A> {
    object: ForeignObject<Surface2Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemSurface<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGISurface2`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGISurface2` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemSurface<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGISurface2"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<Surface2Table>::from_owned(raw) }?;
            Some(SystemSurface {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGISurface2`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemSurface<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2` names the interface `Surface2Table` lays out.
            let object = unsafe {
                unknown.query::<Surface2Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                )
            }?;
            Some(SystemSurface {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemSurface<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemSurface<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGISurface2, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemSurface::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemSurface<A> {
    fn clone(&self) -> SystemSurface<A> {
        SystemSurface {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemSurface<A> {
    fn eq(&self, other: &SystemSurface<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemSurface<A> {}

impl<A> Hash for SystemSurface<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `SwapChain`, held by one counted reference.
pub struct SystemSwapChain<A> {
    object: ForeignObject<SwapChain4Table>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemSwapChain<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGISwapChain4`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGISwapChain4` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemSwapChain<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGISwapChain4"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<SwapChain4Table>::from_owned(raw) }?;
            Some(SystemSwapChain {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGISwapChain4`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemSwapChain<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4` names the interface `SwapChain4Table` lays out.
            let object = unsafe {
                unknown.query::<SwapChain4Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                )
            }?;
            Some(SystemSwapChain {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemSwapChain<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemSwapChain<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGISwapChain4, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemSwapChain::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemSwapChain<A> {
    fn clone(&self) -> SystemSwapChain<A> {
        SystemSwapChain {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemSwapChain<A> {
    fn eq(&self, other: &SystemSwapChain<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemSwapChain<A> {}

impl<A> Hash for SystemSwapChain<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `SwapChainMedia`, held by one counted reference.
pub struct SystemSwapChainMedia<A> {
    object: ForeignObject<SwapChainMediaTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemSwapChainMedia<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGISwapChainMedia`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGISwapChainMedia` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemSwapChainMedia<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGISwapChainMedia"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<SwapChainMediaTable>::from_owned(raw) }?;
            Some(SystemSwapChainMedia {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGISwapChainMedia`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemSwapChainMedia<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA` names the interface `SwapChainMediaTable` lays out.
            let object = unsafe {
                unknown.query::<SwapChainMediaTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                )
            }?;
            Some(SystemSwapChainMedia {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemSwapChainMedia<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemSwapChainMedia<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGISwapChainMedia, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemSwapChainMedia::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemSwapChainMedia<A> {
    fn clone(&self) -> SystemSwapChainMedia<A> {
        SystemSwapChainMedia {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemSwapChainMedia<A> {
    fn eq(&self, other: &SystemSwapChainMedia<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemSwapChainMedia<A> {}

impl<A> Hash for SystemSwapChainMedia<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `GraphicsAnalysis`, held by one counted reference.
pub struct SystemGraphicsAnalysis<A> {
    object: ForeignObject<GraphicsAnalysisTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemGraphicsAnalysis<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `IDXGraphicsAnalysis`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `IDXGraphicsAnalysis` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemGraphicsAnalysis<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGraphicsAnalysis"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<GraphicsAnalysisTable>::from_owned(raw) }?;
            Some(SystemGraphicsAnalysis {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `IDXGraphicsAnalysis`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemGraphicsAnalysis<A>> {
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
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS` names the interface `GraphicsAnalysisTable` lays out.
            let object = unsafe {
                unknown.query::<GraphicsAnalysisTable>(
                    &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                )
            }?;
            Some(SystemGraphicsAnalysis {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemGraphicsAnalysis<A> {
    const INTERFACE: Guid = directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemGraphicsAnalysis<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of IDXGraphicsAnalysis, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemGraphicsAnalysis::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemGraphicsAnalysis<A> {
    fn clone(&self) -> SystemGraphicsAnalysis<A> {
        SystemGraphicsAnalysis {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemGraphicsAnalysis<A> {
    fn eq(&self, other: &SystemGraphicsAnalysis<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemGraphicsAnalysis<A> {}

impl<A> Hash for SystemGraphicsAnalysis<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// Takes over a reference to an object of the interface `riid` names: as the kind of this API that answers it (each
/// candidate asked by QueryInterface), else as a foreign object; none for null.
///
/// # Safety
/// `raw` is null or a live COM object of the interface `riid` names, whose reference the caller gives up, and which may be
/// called from any thread as `Interface` states (a context only under the system's multithread protection).
#[expect(
    unsafe_code,
    reason = "the promise that a pointer is a live COM object"
)]
pub unsafe fn unknown_object_of<A: System>(
    riid: &Guid,
    raw: Raw,
) -> Option<directx_graphics_infrastructure::UnknownObject<A>> {
    #[deny(unsafe_code)]
    let body = move || {
        #[expect(
            unsafe_code,
            reason = "the promise of the caller: a live COM object, which is an IUnknown"
        )]
        // SAFETY: the caller's promise: any COM object is an IUnknown; the reference it gave up is released when this drops.
        let unknown = unsafe { ForeignObject::<UnknownTable>::from_owned(raw) }?;
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER1,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER2,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER3,
            directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4` names the interface `Adapter4Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<Adapter4Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_ADAPTER4,
                )
            } {
                let object = SystemAdapter {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::Adapter(
                    object,
                ));
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1` names the interface `Debug1Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<Debug1Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DEBUG1,
                )
            } {
                let object = SystemDebug {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::Debug(
                    object,
                ));
            }
        }
        if [directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN].contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN` names the interface `DecodeSwapChainTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<DecodeSwapChainTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DECODE_SWAP_CHAIN,
                )
            } {
                let object = SystemDecodeSwapChain {
                    object,
                    api: PhantomData,
                };
                return Some(
                    directx_graphics_infrastructure::UnknownObject::DecodeSwapChain(object),
                );
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE1,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE2,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE3,
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4` names the interface `Device4Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<Device4Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE4,
                )
            } {
                let object = SystemDevice {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::Device(
                    object,
                ));
            }
        }
        if [directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL].contains(riid) {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL` names the interface `DisplayControlTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<DisplayControlTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_DISPLAY_CONTROL,
                )
            } {
                let object = SystemDisplayControl {
                    object,
                    api: PhantomData,
                };
                return Some(
                    directx_graphics_infrastructure::UnknownObject::DisplayControl(object),
                );
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY2,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY3,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY4,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY5,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY6,
            directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7` names the interface `Factory7Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<Factory7Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY7,
                )
            } {
                let object = SystemFactory {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::Factory(
                    object,
                ));
            }
        }
        if [directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA].contains(riid) {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA` names the interface `FactoryMediaTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<FactoryMediaTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY_MEDIA,
                )
            } {
                let object = SystemFactoryMedia {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::FactoryMedia(object));
            }
        }
        if [directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE].contains(riid) {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE` names the interface `InfoQueueTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<InfoQueueTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_INFO_QUEUE,
                )
            } {
                let object = SystemInfoQueue {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::InfoQueue(
                    object,
                ));
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX` names the interface `KeyedMutexTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<KeyedMutexTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_KEYED_MUTEX,
                )
            } {
                let object = SystemKeyedMutex {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::KeyedMutex(
                    object,
                ));
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT1,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT2,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT3,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT4,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT5,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6` names the interface `Output6Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<Output6Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT6,
                )
            } {
                let object = SystemOutput {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::Output(
                    object,
                ));
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION` names the interface `OutputDuplicationTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<OutputDuplicationTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_OUTPUT_DUPLICATION,
                )
            } {
                let object = SystemOutputDuplication {
                    object,
                    api: PhantomData,
                };
                return Some(
                    directx_graphics_infrastructure::UnknownObject::OutputDuplication(object),
                );
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE,
            directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1` names the interface `Resource1Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<Resource1Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_RESOURCE1,
                )
            } {
                let object = SystemResource {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::Resource(
                    object,
                ));
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE,
            directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE1,
            directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2` names the interface `Surface2Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<Surface2Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_SURFACE2,
                )
            } {
                let object = SystemSurface {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::Surface(
                    object,
                ));
            }
        }
        if [
            directx_graphics_infrastructure::constants::IID_IDXGI_DEVICE_SUB_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_OBJECT,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN1,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN2,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN3,
            directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4` names the interface `SwapChain4Table` lays out.
            if let Some(object) = unsafe {
                unknown.query::<SwapChain4Table>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN4,
                )
            } {
                let object = SystemSwapChain {
                    object,
                    api: PhantomData,
                };
                return Some(directx_graphics_infrastructure::UnknownObject::SwapChain(
                    object,
                ));
            }
        }
        if [directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA].contains(riid) {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA` names the interface `SwapChainMediaTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<SwapChainMediaTable>(
                    &directx_graphics_infrastructure::constants::IID_IDXGI_SWAP_CHAIN_MEDIA,
                )
            } {
                let object = SystemSwapChainMedia {
                    object,
                    api: PhantomData,
                };
                return Some(
                    directx_graphics_infrastructure::UnknownObject::SwapChainMedia(object),
                );
            }
        }
        if [directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS].contains(riid) {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS` names the interface `GraphicsAnalysisTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<GraphicsAnalysisTable>(
                    &directx_graphics_infrastructure::constants::IID_IDX_GRAPHICS_ANALYSIS,
                )
            } {
                let object = SystemGraphicsAnalysis {
                    object,
                    api: PhantomData,
                };
                return Some(
                    directx_graphics_infrastructure::UnknownObject::GraphicsAnalysis(object),
                );
            }
        }
        #[expect(
            unsafe_code,
            reason = "the promise of the caller: a live COM object, which is an IUnknown"
        )]
        // SAFETY: the caller's promise; a reference of our own is taken.
        let foreign = unsafe { ForeignUnknown::from_borrowed(unknown.raw()) }?;
        Some(directx_graphics_infrastructure::UnknownObject::Unknown(
            wcontainers_component_object_model::structures::Foreign::new(
                unknown.raw().addr(),
                std::sync::Arc::new(foreign),
            ),
        ))
    };
    body()
}

/// An API value whose objects are the C side's: every kind is this crate's object type.
pub trait System:
    directx_graphics_infrastructure::DirectXGraphicsInfrastructure<
        Adapter = SystemAdapter<Self>,
        Debug = SystemDebug<Self>,
        DecodeSwapChain = SystemDecodeSwapChain<Self>,
        Device = SystemDevice<Self>,
        DisplayControl = SystemDisplayControl<Self>,
        Factory = SystemFactory<Self>,
        FactoryMedia = SystemFactoryMedia<Self>,
        InfoQueue = SystemInfoQueue<Self>,
        KeyedMutex = SystemKeyedMutex<Self>,
        Output = SystemOutput<Self>,
        OutputDuplication = SystemOutputDuplication<Self>,
        Resource = SystemResource<Self>,
        Surface = SystemSurface<Self>,
        SwapChain = SystemSwapChain<Self>,
        SwapChainMedia = SystemSwapChainMedia<Self>,
        GraphicsAnalysis = SystemGraphicsAnalysis<Self>,
        Buffer: Adopt,
    >
{
}

impl<A: System> directx_graphics_infrastructure::Adapter4 for SystemAdapter<A> {
    fn get_desc3(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc3,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc3Layout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGIAdapter4::GetDesc3, called out")]
        // SAFETY: a live object whose table holds `IDXGIAdapter4::GetDesc3` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().adapter4_get_desc3)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_adapter_desc3(&desc.0);
        Ok(desc)
    }
}

impl<A: System> directx_graphics_infrastructure::Adapter3 for SystemAdapter<A> {
    fn register_hardware_content_protection_teardown_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut pdw_cookie = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIAdapter3::RegisterHardwareContentProtectionTeardownStatusEvent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .adapter3_register_hardware_content_protection_teardown_status_event)(
                self.object.raw(),
                h_event.0,
                pdw_cookie.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pdw_cookie = get_u32(&pdw_cookie.0, usize::MIN);
        Ok(pdw_cookie)
    }
    fn unregister_hardware_content_protection_teardown_status(&self, dw_cookie: u32) {
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIAdapter3::UnregisterHardwareContentProtectionTeardownStatus` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .adapter3_unregister_hardware_content_protection_teardown_status)(
                self.object.raw(),
                dw_cookie,
            )
        };
        let () = returned;
    }
    fn query_video_memory_info(
        &self,
        node_index: u32,
        memory_segment_group: directx_graphics_infrastructure::enumerations::MemorySegmentGroup,
    ) -> Result<
        directx_graphics_infrastructure::structures::QueryVideoMemoryInfo,
        component_object_model::structures::HResult,
    > {
        let mut video_memory_info = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiDxgiQueryVideoMemoryInfoLayout>() },
        >::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::QueryVideoMemoryInfo, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIAdapter3::QueryVideoMemoryInfo` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().adapter3_query_video_memory_info)(
                self.object.raw(),
                node_index,
                memory_segment_group.0,
                video_memory_info.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let video_memory_info =
            decode_windows_win32_graphics_dxgi_dxgi_query_video_memory_info(&video_memory_info.0);
        Ok(video_memory_info)
    }
    fn set_video_memory_reservation(
        &self,
        node_index: u32,
        memory_segment_group: directx_graphics_infrastructure::enumerations::MemorySegmentGroup,
        reservation: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::SetVideoMemoryReservation, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIAdapter3::SetVideoMemoryReservation` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .adapter3_set_video_memory_reservation)(
                self.object.raw(),
                node_index,
                memory_segment_group.0,
                reservation,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn register_video_memory_budget_change_notification_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut pdw_cookie = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIAdapter3::RegisterVideoMemoryBudgetChangeNotificationEvent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .adapter3_register_video_memory_budget_change_notification_event)(
                self.object.raw(),
                h_event.0,
                pdw_cookie.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pdw_cookie = get_u32(&pdw_cookie.0, usize::MIN);
        Ok(pdw_cookie)
    }
    fn unregister_video_memory_budget_change_notification(&self, dw_cookie: u32) {
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIAdapter3::UnregisterVideoMemoryBudgetChangeNotification` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .adapter3_unregister_video_memory_budget_change_notification)(
                self.object.raw(),
                dw_cookie,
            )
        };
        let () = returned;
    }
}

impl<A: System> directx_graphics_infrastructure::Adapter2 for SystemAdapter<A> {
    fn get_desc2(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc2,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc2Layout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGIAdapter2::GetDesc2, called out")]
        // SAFETY: a live object whose table holds `IDXGIAdapter2::GetDesc2` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().adapter2_get_desc2)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_adapter_desc2(&desc.0);
        Ok(desc)
    }
}

impl<A: System> directx_graphics_infrastructure::Adapter1 for SystemAdapter<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc1,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDesc1Layout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGIAdapter1::GetDesc1, called out")]
        // SAFETY: a live object whose table holds `IDXGIAdapter1::GetDesc1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().adapter1_get_desc1)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_adapter_desc1(&desc.0);
        Ok(desc)
    }
}

impl<A: System> directx_graphics_infrastructure::Adapter for SystemAdapter<A> {
    fn enum_outputs(
        &self,
        output: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        component_object_model::structures::HResult,
    > {
        let mut output_: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIAdapter::EnumOutputs, called out")]
        // SAFETY: a live object whose table holds `IDXGIAdapter::EnumOutputs` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().adapter_enum_outputs)(
                self.object.raw(),
                output,
                &raw mut output_,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter::EnumOutputs: ppOutput, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppOutput` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIOutput`.
        let Some(output_) = (unsafe { SystemOutput::<A>::from_owned_older(output_) }) else {
            return Err(E_FAIL);
        };
        Ok(output_)
    }
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::AdapterDesc,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiAdapterDescLayout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGIAdapter::GetDesc, called out")]
        // SAFETY: a live object whose table holds `IDXGIAdapter::GetDesc` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().adapter_get_desc)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_adapter_desc(&desc.0);
        Ok(desc)
    }
    fn check_interface_support(
        &self,
        interface_name: &component_object_model::structures::Guid,
    ) -> Result<i64, component_object_model::structures::HResult> {
        let mut interface_name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(interface_name, &mut interface_name_bytes.0);
        let mut umd_version = Aligned::<8>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIAdapter::CheckInterfaceSupport, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIAdapter::CheckInterfaceSupport` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().adapter_check_interface_support)(
                self.object.raw(),
                interface_name_bytes.0.as_ptr(),
                umd_version.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let umd_version = get_i64(&umd_version.0, usize::MIN);
        Ok(umd_version)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemAdapter<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::Debug1 for SystemDebug<A> {
    fn enable_leak_tracking_for_thread(&self) {
        #[expect(
            unsafe_code,
            reason = "IDXGIDebug1::EnableLeakTrackingForThread, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDebug1::EnableLeakTrackingForThread` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .debug1_enable_leak_tracking_for_thread)(self.object.raw())
        };
        let () = returned;
    }
    fn disable_leak_tracking_for_thread(&self) {
        #[expect(
            unsafe_code,
            reason = "IDXGIDebug1::DisableLeakTrackingForThread, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDebug1::DisableLeakTrackingForThread` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .debug1_disable_leak_tracking_for_thread)(self.object.raw())
        };
        let () = returned;
    }
    fn is_leak_tracking_enabled_for_thread(&self) -> bool {
        #[expect(
            unsafe_code,
            reason = "IDXGIDebug1::IsLeakTrackingEnabledForThread, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDebug1::IsLeakTrackingEnabledForThread` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .debug1_is_leak_tracking_enabled_for_thread)(self.object.raw())
        };
        returned != 0
    }
}

impl<A: System> directx_graphics_infrastructure::Debug for SystemDebug<A> {
    fn report_live_objects(
        &self,
        apiid: component_object_model::structures::Guid,
        flags: directx_graphics_infrastructure::enumerations::DebugRloFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIDebug::ReportLiveObjects, called out")]
        // SAFETY: a live object whose table holds `IDXGIDebug::ReportLiveObjects` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().debug_report_live_objects)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&apiid),
                flags.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> directx_graphics_infrastructure::DecodeSwapChain for SystemDecodeSwapChain<A> {
    fn present_buffer(
        &self,
        buffer_to_present: u32,
        sync_interval: u32,
        flags: directx_graphics_infrastructure::enumerations::Present,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::PresentBuffer, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::PresentBuffer` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_present_buffer)(
                self.object.raw(),
                buffer_to_present,
                sync_interval,
                flags.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_source_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut rect_bytes =
            Aligned::<{ size_of::<WindowsWin32FoundationRectLayout>() }>::default();
        encode_windows_win32_foundation_rect(rect, &mut rect_bytes.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::SetSourceRect, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::SetSourceRect` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_set_source_rect)(
                self.object.raw(),
                rect_bytes.0.as_ptr(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_target_rect(
        &self,
        rect: &component_object_model::structures::Rect,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut rect_bytes =
            Aligned::<{ size_of::<WindowsWin32FoundationRectLayout>() }>::default();
        encode_windows_win32_foundation_rect(rect, &mut rect_bytes.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::SetTargetRect, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::SetTargetRect` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_set_target_rect)(
                self.object.raw(),
                rect_bytes.0.as_ptr(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_dest_size(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIDecodeSwapChain::SetDestSize, called out")]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::SetDestSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_set_dest_size)(
                self.object.raw(),
                width,
                height,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_source_rect(
        &self,
    ) -> Result<component_object_model::structures::Rect, component_object_model::structures::HResult>
    {
        let mut rect = Aligned::<{ size_of::<WindowsWin32FoundationRectLayout>() }>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::GetSourceRect, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::GetSourceRect` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_get_source_rect)(
                self.object.raw(),
                rect.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let rect = decode_windows_win32_foundation_rect(&rect.0);
        Ok(rect)
    }
    fn get_target_rect(
        &self,
    ) -> Result<component_object_model::structures::Rect, component_object_model::structures::HResult>
    {
        let mut rect = Aligned::<{ size_of::<WindowsWin32FoundationRectLayout>() }>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::GetTargetRect, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::GetTargetRect` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_get_target_rect)(
                self.object.raw(),
                rect.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let rect = decode_windows_win32_foundation_rect(&rect.0);
        Ok(rect)
    }
    fn get_dest_size(&self) -> Result<(u32, u32), component_object_model::structures::HResult> {
        let mut width = Aligned::<4>::default();
        let mut height = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGIDecodeSwapChain::GetDestSize, called out")]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::GetDestSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_get_dest_size)(
                self.object.raw(),
                width.0.as_mut_ptr(),
                height.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let width = get_u32(&width.0, usize::MIN);
        let height = get_u32(&height.0, usize::MIN);
        Ok((width, height))
    }
    fn set_color_space(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::SetColorSpace, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::SetColorSpace` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_set_color_space)(
                self.object.raw(),
                color_space.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_color_space(
        &self,
    ) -> directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags {
        #[expect(
            unsafe_code,
            reason = "IDXGIDecodeSwapChain::GetColorSpace, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDecodeSwapChain::GetColorSpace` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().decode_swap_chain_get_color_space)(self.object.raw())
        };
        directx_graphics_infrastructure::enumerations::MultiplaneOverlayYcbcrFlags(returned)
    }
}

impl<A: System> directx_graphics_infrastructure::Device4 for SystemDevice<A> {
    fn offer_resources1(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
        priority: directx_graphics_infrastructure::enumerations::OfferResourcePriority,
        flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = resources;
        let _ = priority;
        let _ = flags;
        // Pinned (src/api.rs): never called by the calling glue: as OfferResources.
        Err(E_NOTIMPL)
    }
    fn reclaim_resources1(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<
        Vec<directx_graphics_infrastructure::enumerations::ReclaimResourceResults>,
        component_object_model::structures::HResult,
    > {
        let _ = resources;
        // Pinned (src/api.rs): never called by the calling glue: as ReclaimResources.
        Err(E_NOTIMPL)
    }
}

impl<A: System> directx_graphics_infrastructure::Device3 for SystemDevice<A> {
    fn trim(&self) {
        #[expect(unsafe_code, reason = "IDXGIDevice3::Trim, called out")]
        // SAFETY: a live object whose table holds `IDXGIDevice3::Trim` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe { (self.object.entry_points().device3_trim)(self.object.raw()) };
        let () = returned;
    }
}

impl<A: System> directx_graphics_infrastructure::Device2 for SystemDevice<A> {
    fn offer_resources(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
        priority: directx_graphics_infrastructure::enumerations::OfferResourcePriority,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = resources;
        let _ = priority;
        // Pinned (src/api.rs): never called by the calling glue: an offered resource's memory may be discarded, under memory a view may hold.
        Err(E_NOTIMPL)
    }
    fn reclaim_resources(
        &self,
        resources: &[Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource>],
    ) -> Result<Vec<bool>, component_object_model::structures::HResult> {
        let _ = resources;
        // Pinned (src/api.rs): never called by the calling glue: pairs with OfferResources, which is never called.
        Err(E_NOTIMPL)
    }
    fn enqueue_set_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIDevice2::EnqueueSetEvent, called out")]
        // SAFETY: a live object whose table holds `IDXGIDevice2::EnqueueSetEvent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device2_enqueue_set_event)(self.object.raw(), h_event.0)
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> directx_graphics_infrastructure::Device1 for SystemDevice<A> {
    fn set_maximum_frame_latency(
        &self,
        max_latency: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIDevice1::SetMaximumFrameLatency, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDevice1::SetMaximumFrameLatency` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device1_set_maximum_frame_latency)(
                self.object.raw(),
                max_latency,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_maximum_frame_latency(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut max_latency = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIDevice1::GetMaximumFrameLatency, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDevice1::GetMaximumFrameLatency` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device1_get_maximum_frame_latency)(
                self.object.raw(),
                max_latency.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let max_latency = get_u32(&max_latency.0, usize::MIN);
        Ok(max_latency)
    }
}

impl<A: System> directx_graphics_infrastructure::Device for SystemDevice<A> {
    fn get_adapter(
        &self,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let mut adapter: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIDevice::GetAdapter, called out")]
        // SAFETY: a live object whose table holds `IDXGIDevice::GetAdapter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_get_adapter)(self.object.raw(), &raw mut adapter)
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIDevice::GetAdapter: pAdapter, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `pAdapter` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIAdapter`.
        let Some(adapter) = (unsafe { SystemAdapter::<A>::from_owned_older(adapter) }) else {
            return Err(E_FAIL);
        };
        Ok(adapter)
    }
    fn create_surface(&self, desc: &directx_graphics_infrastructure::structures::SurfaceDesc, num_surfaces: u32, usage: directx_graphics_infrastructure::enumerations::Usage, shared_resource: Option<&directx_graphics_infrastructure::structures::SharedResource>) -> Result<Vec<Option<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface>>, component_object_model::structures::HResult>{
        let mut desc_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_surface_desc(desc, &mut desc_bytes.0);
        let shared_resource_bytes = shared_resource.map(|value| {
            let mut bytes = Aligned::<
                { size_of::<WindowsWin32GraphicsDxgiDxgiSharedResourceLayout>() },
            >::default();
            encode_windows_win32_graphics_dxgi_dxgi_shared_resource(value, &mut bytes.0);
            bytes
        });
        let Some(surface_count) = usize::try_from(num_surfaces)
            .ok()
            .filter(|count| *count <= 1 << 16)
        else {
            return Err(E_INVALIDARG);
        };
        let mut surface: Vec<Raw> = vec![null_mut(); surface_count];
        #[expect(unsafe_code, reason = "IDXGIDevice::CreateSurface, called out")]
        // SAFETY: a live object whose table holds `IDXGIDevice::CreateSurface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_surface)(
                self.object.raw(),
                desc_bytes.0.as_ptr(),
                num_surfaces,
                usage.0,
                shared_resource_bytes
                    .as_ref()
                    .map_or(null(), |bytes| bytes.0.as_ptr()),
                surface.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let mut surface_adopted = Vec::new();
        for raw in surface {
            #[expect(
                unsafe_code,
                reason = "IDXGIDevice::CreateSurface: ppSurface, an object the metadata's Out hands over"
            )]
            // SAFETY: the metadata's `ppSurface` is `Out` with `num_surfaces` its count: after the call, each element null or one reference to an object of `IDXGISurface`.
            surface_adopted.push(unsafe { SystemSurface::<A>::from_owned_older(raw) });
        }
        let surface = surface_adopted;
        Ok(surface)
    }
    fn query_resource_residency(
        &self,
        resources: &[Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>],
    ) -> Result<
        Vec<directx_graphics_infrastructure::enumerations::Residency>,
        component_object_model::structures::HResult,
    > {
        let _ = resources;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn set_gpu_thread_priority(
        &self,
        priority: i32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIDevice::SetGPUThreadPriority, called out")]
        // SAFETY: a live object whose table holds `IDXGIDevice::SetGPUThreadPriority` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_set_gpu_thread_priority)(self.object.raw(), priority)
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_gpu_thread_priority(&self) -> Result<i32, component_object_model::structures::HResult> {
        let mut priority = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGIDevice::GetGPUThreadPriority, called out")]
        // SAFETY: a live object whose table holds `IDXGIDevice::GetGPUThreadPriority` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_get_gpu_thread_priority)(
                self.object.raw(),
                priority.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let priority = get_i32(&priority.0, usize::MIN);
        Ok(priority)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemDevice<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::DisplayControl for SystemDisplayControl<A> {
    fn is_stereo_enabled(&self) -> bool {
        #[expect(
            unsafe_code,
            reason = "IDXGIDisplayControl::IsStereoEnabled, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDisplayControl::IsStereoEnabled` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().display_control_is_stereo_enabled)(self.object.raw())
        };
        returned != 0
    }
    fn set_stereo_enabled(&self, enabled: bool) {
        #[expect(
            unsafe_code,
            reason = "IDXGIDisplayControl::SetStereoEnabled, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIDisplayControl::SetStereoEnabled` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .display_control_set_stereo_enabled)(
                self.object.raw(), i32::from(enabled)
            )
        };
        let () = returned;
    }
}

impl<A: System> directx_graphics_infrastructure::Factory7 for SystemFactory<A> {
    fn register_adapters_changed_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut pdw_cookie = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory7::RegisterAdaptersChangedEvent, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory7::RegisterAdaptersChangedEvent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory7_register_adapters_changed_event)(
                self.object.raw(),
                h_event.0,
                pdw_cookie.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pdw_cookie = get_u32(&pdw_cookie.0, usize::MIN);
        Ok(pdw_cookie)
    }
    fn unregister_adapters_changed_event(
        &self,
        dw_cookie: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory7::UnregisterAdaptersChangedEvent, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory7::UnregisterAdaptersChangedEvent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory7_unregister_adapters_changed_event)(
                self.object.raw(), dw_cookie
            )
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> directx_graphics_infrastructure::Factory6 for SystemFactory<A> {
    fn enum_adapter_by_gpu_preference(
        &self,
        adapter: u32,
        gpu_preference: directx_graphics_infrastructure::enumerations::GpuPreference,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut ppv_adapter: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory6::EnumAdapterByGpuPreference, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory6::EnumAdapterByGpuPreference` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory6_enum_adapter_by_gpu_preference)(
                self.object.raw(),
                adapter,
                gpu_preference.0,
                riid_bytes.0.as_ptr(),
                &raw mut ppv_adapter,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory6::EnumAdapterByGpuPreference: ppvAdapter, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppvAdapter` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(ppv_adapter) = (unsafe { unknown_object_of::<A>(riid, ppv_adapter) }) else {
            return Err(E_FAIL);
        };
        Ok(ppv_adapter)
    }
}

impl<A: System> directx_graphics_infrastructure::Factory5 for SystemFactory<A> {
    fn check_feature_support(
        &self,
        feature: directx_graphics_infrastructure::enumerations::Feature,
        feature_support_data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let Ok(feature_support_data_length) = usize::try_from(feature_support_data.len()) else {
            return Err(E_INVALIDARG);
        };
        let mut feature_support_data_buffer = Vec::new();
        if feature_support_data_buffer
            .try_reserve_exact(feature_support_data_length)
            .is_err()
        {
            return Err(E_OUTOFMEMORY);
        }
        feature_support_data_buffer.resize(feature_support_data_length, u8::MIN);
        let Ok(feature_support_data_size) = u32::try_from(feature_support_data_length) else {
            return Err(E_INVALIDARG);
        };
        #[expect(unsafe_code, reason = "IDXGIFactory5::CheckFeatureSupport, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory5::CheckFeatureSupport` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory5_check_feature_support)(
                self.object.raw(),
                feature.0,
                feature_support_data_buffer.as_mut_ptr(),
                feature_support_data_size,
            )
        };
        succeeded(returned)?;
        feature_support_data.write(&feature_support_data_buffer);
        Ok(())
    }
}

impl<A: System> directx_graphics_infrastructure::Factory4 for SystemFactory<A> {
    fn enum_adapter_by_luid(
        &self,
        adapter_luid: component_object_model::structures::Luid,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let _ = adapter_luid;
        let _ = riid;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn enum_warp_adapter(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut ppv_adapter: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIFactory4::EnumWarpAdapter, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory4::EnumWarpAdapter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory4_enum_warp_adapter)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut ppv_adapter,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory4::EnumWarpAdapter: ppvAdapter, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppvAdapter` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(ppv_adapter) = (unsafe { unknown_object_of::<A>(riid, ppv_adapter) }) else {
            return Err(E_FAIL);
        };
        Ok(ppv_adapter)
    }
}

impl<A: System> directx_graphics_infrastructure::Factory3 for SystemFactory<A> {
    fn get_creation_flags(
        &self,
    ) -> directx_graphics_infrastructure::enumerations::CreateFactoryFlags {
        #[expect(unsafe_code, reason = "IDXGIFactory3::GetCreationFlags, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory3::GetCreationFlags` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().factory3_get_creation_flags)(self.object.raw()) };
        directx_graphics_infrastructure::enumerations::CreateFactoryFlags(returned)
    }
}

impl<A: System> directx_graphics_infrastructure::Factory2 for SystemFactory<A> {
    fn is_windowed_stereo_enabled(&self) -> bool {
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::IsWindowedStereoEnabled, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::IsWindowedStereoEnabled` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_is_windowed_stereo_enabled)(self.object.raw())
        };
        returned != 0
    }
    fn create_swap_chain_for_hwnd(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        h_wnd: component_object_model::structures::Hwnd,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        fullscreen_desc: Option<
            &directx_graphics_infrastructure::structures::SwapChainFullscreenDesc,
        >,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut desc_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(desc, &mut desc_bytes.0);
        let fullscreen_desc_bytes = fullscreen_desc.map(|value| {
            let mut bytes = Aligned::<
                { size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout>() },
            >::default();
            encode_windows_win32_graphics_dxgi_dxgi_swap_chain_fullscreen_desc(value, &mut bytes.0);
            bytes
        });
        let mut swap_chain: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForHwnd, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::CreateSwapChainForHwnd` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_create_swap_chain_for_hwnd)(
                self.object.raw(),
                device_raw,
                h_wnd.0,
                desc_bytes.0.as_ptr(),
                fullscreen_desc_bytes
                    .as_ref()
                    .map_or(null(), |bytes| bytes.0.as_ptr()),
                restrict_to_output.map_or(null_mut(), |held| held.raw()),
                &raw mut swap_chain,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForHwnd: ppSwapChain, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGISwapChain1`.
        let Some(swap_chain) = (unsafe { SystemSwapChain::<A>::from_owned_older(swap_chain) })
        else {
            return Err(E_FAIL);
        };
        Ok(swap_chain)
    }
    fn create_swap_chain_for_core_window(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        window: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let window_raw: Raw = match window {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut desc_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(desc, &mut desc_bytes.0);
        let mut swap_chain: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForCoreWindow, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::CreateSwapChainForCoreWindow` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_create_swap_chain_for_core_window)(
                self.object.raw(),
                device_raw,
                window_raw,
                desc_bytes.0.as_ptr(),
                restrict_to_output.map_or(null_mut(), |held| held.raw()),
                &raw mut swap_chain,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForCoreWindow: ppSwapChain, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGISwapChain1`.
        let Some(swap_chain) = (unsafe { SystemSwapChain::<A>::from_owned_older(swap_chain) })
        else {
            return Err(E_FAIL);
        };
        Ok(swap_chain)
    }
    fn get_shared_resource_adapter_luid(
        &self,
        h_resource: component_object_model::structures::Handle,
    ) -> Result<component_object_model::structures::Luid, component_object_model::structures::HResult>
    {
        let mut luid = Aligned::<{ size_of::<WindowsWin32FoundationLuidLayout>() }>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::GetSharedResourceAdapterLuid, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::GetSharedResourceAdapterLuid` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_get_shared_resource_adapter_luid)(
                self.object.raw(),
                h_resource.0,
                luid.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let luid = decode_windows_win32_foundation_luid(&luid.0);
        Ok(luid)
    }
    fn register_stereo_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut pdw_cookie = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterStereoStatusWindow, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::RegisterStereoStatusWindow` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_register_stereo_status_window)(
                self.object.raw(),
                window_handle.0,
                w_msg,
                pdw_cookie.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pdw_cookie = get_u32(&pdw_cookie.0, usize::MIN);
        Ok(pdw_cookie)
    }
    fn register_stereo_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut pdw_cookie = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterStereoStatusEvent, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::RegisterStereoStatusEvent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_register_stereo_status_event)(
                self.object.raw(),
                h_event.0,
                pdw_cookie.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pdw_cookie = get_u32(&pdw_cookie.0, usize::MIN);
        Ok(pdw_cookie)
    }
    fn unregister_stereo_status(&self, dw_cookie: u32) {
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::UnregisterStereoStatus, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::UnregisterStereoStatus` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory2_unregister_stereo_status)(
                self.object.raw(),
                dw_cookie,
            )
        };
        let () = returned;
    }
    fn register_occlusion_status_window(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        w_msg: u32,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut pdw_cookie = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterOcclusionStatusWindow, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::RegisterOcclusionStatusWindow` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_register_occlusion_status_window)(
                self.object.raw(),
                window_handle.0,
                w_msg,
                pdw_cookie.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pdw_cookie = get_u32(&pdw_cookie.0, usize::MIN);
        Ok(pdw_cookie)
    }
    fn register_occlusion_status_event(
        &self,
        h_event: component_object_model::structures::Handle,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut pdw_cookie = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::RegisterOcclusionStatusEvent, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::RegisterOcclusionStatusEvent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_register_occlusion_status_event)(
                self.object.raw(),
                h_event.0,
                pdw_cookie.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pdw_cookie = get_u32(&pdw_cookie.0, usize::MIN);
        Ok(pdw_cookie)
    }
    fn unregister_occlusion_status(&self, dw_cookie: u32) {
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::UnregisterOcclusionStatus, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::UnregisterOcclusionStatus` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_unregister_occlusion_status)(self.object.raw(), dw_cookie)
        };
        let () = returned;
    }
    fn create_swap_chain_for_composition(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut desc_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(desc, &mut desc_bytes.0);
        let mut swap_chain: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForComposition, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory2::CreateSwapChainForComposition` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory2_create_swap_chain_for_composition)(
                self.object.raw(),
                device_raw,
                desc_bytes.0.as_ptr(),
                restrict_to_output.map_or(null_mut(), |held| held.raw()),
                &raw mut swap_chain,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory2::CreateSwapChainForComposition: ppSwapChain, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGISwapChain1`.
        let Some(swap_chain) = (unsafe { SystemSwapChain::<A>::from_owned_older(swap_chain) })
        else {
            return Err(E_FAIL);
        };
        Ok(swap_chain)
    }
}

impl<A: System> directx_graphics_infrastructure::Factory1 for SystemFactory<A> {
    fn enum_adapters1(
        &self,
        adapter: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let mut adapter_: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIFactory1::EnumAdapters1, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory1::EnumAdapters1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory1_enum_adapters1)(
                self.object.raw(),
                adapter,
                &raw mut adapter_,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory1::EnumAdapters1: ppAdapter, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppAdapter` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIAdapter1`.
        let Some(adapter_) = (unsafe { SystemAdapter::<A>::from_owned_older(adapter_) }) else {
            return Err(E_FAIL);
        };
        Ok(adapter_)
    }
    fn is_current(&self) -> bool {
        #[expect(unsafe_code, reason = "IDXGIFactory1::IsCurrent, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory1::IsCurrent` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().factory1_is_current)(self.object.raw()) };
        returned != 0
    }
}

impl<A: System> directx_graphics_infrastructure::Factory for SystemFactory<A> {
    fn enum_adapters(
        &self,
        adapter: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let mut adapter_: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIFactory::EnumAdapters, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory::EnumAdapters` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory_enum_adapters)(
                self.object.raw(),
                adapter,
                &raw mut adapter_,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory::EnumAdapters: ppAdapter, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppAdapter` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIAdapter`.
        let Some(adapter_) = (unsafe { SystemAdapter::<A>::from_owned_older(adapter_) }) else {
            return Err(E_FAIL);
        };
        Ok(adapter_)
    }
    fn make_window_association(
        &self,
        window_handle: component_object_model::structures::Hwnd,
        flags: directx_graphics_infrastructure::enumerations::MwaFlags,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory::MakeWindowAssociation, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactory::MakeWindowAssociation` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory_make_window_association)(
                self.object.raw(),
                window_handle.0,
                flags.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_window_association(
        &self,
    ) -> Result<component_object_model::structures::Hwnd, component_object_model::structures::HResult>
    {
        let mut window_handle = Aligned::<8>::default();
        #[expect(unsafe_code, reason = "IDXGIFactory::GetWindowAssociation, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory::GetWindowAssociation` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory_get_window_association)(
                self.object.raw(),
                window_handle.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let window_handle =
            component_object_model::structures::Hwnd(get_usize(&window_handle.0, usize::MIN));
        Ok(window_handle)
    }
    fn create_swap_chain(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut desc_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(desc, &mut desc_bytes.0);
        let mut swap_chain: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIFactory::CreateSwapChain, called out")]
        // SAFETY: a live object whose table holds `IDXGIFactory::CreateSwapChain` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().factory_create_swap_chain)(
                self.object.raw(),
                device_raw,
                desc_bytes.0.as_ptr(),
                &raw mut swap_chain,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactory::CreateSwapChain: ppSwapChain, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGISwapChain`.
        let Some(swap_chain) = (unsafe { SystemSwapChain::<A>::from_owned_older(swap_chain) })
        else {
            return Err(E_FAIL);
        };
        Ok(swap_chain)
    }
    fn create_software_adapter(
        &self,
        module: component_object_model::structures::Hmodule,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Adapter,
        component_object_model::structures::HResult,
    > {
        let _ = module;
        // Refused by its kinds (src/kinds.rs), the system never called: Module: HMODULE: the address of a loaded module, whose code the system would find and run.
        Err(E_NOTIMPL)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemFactory<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::FactoryMedia for SystemFactoryMedia<A> {
    type Api = A;

    fn create_swap_chain_for_composition_surface_handle(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        h_surface: component_object_model::structures::Handle,
        desc: &directx_graphics_infrastructure::structures::SwapChainDesc1,
        restrict_to_output: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::SwapChain,
        component_object_model::structures::HResult,
    > {
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut desc_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(desc, &mut desc_bytes.0);
        let mut swap_chain: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory_media_create_swap_chain_for_composition_surface_handle)(
                self.object.raw(),
                device_raw,
                h_surface.0,
                desc_bytes.0.as_ptr(),
                restrict_to_output.map_or(null_mut(), |held| held.raw()),
                &raw mut swap_chain,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactoryMedia::CreateSwapChainForCompositionSurfaceHandle: ppSwapChain, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGISwapChain1`.
        let Some(swap_chain) = (unsafe { SystemSwapChain::<A>::from_owned_older(swap_chain) })
        else {
            return Err(E_FAIL);
        };
        Ok(swap_chain)
    }
    fn create_decode_swap_chain_for_composition_surface_handle(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>, h_surface: component_object_model::structures::Handle, desc: &directx_graphics_infrastructure::structures::DecodeSwapChainDesc, yuv_decode_buffers: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource, restrict_to_output: Option<&<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output>) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::DecodeSwapChain, component_object_model::structures::HResult>{
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut desc_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiDxgiDecodeSwapChainDescLayout>() },
        >::default();
        encode_windows_win32_graphics_dxgi_dxgi_decode_swap_chain_desc(desc, &mut desc_bytes.0);
        let mut swap_chain: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .factory_media_create_decode_swap_chain_for_composition_surface_handle)(
                self.object.raw(),
                device_raw,
                h_surface.0,
                desc_bytes.0.as_ptr(),
                {
                    let held = yuv_decode_buffers;
                    held.raw()
                },
                restrict_to_output.map_or(null_mut(), |held| held.raw()),
                &raw mut swap_chain,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIFactoryMedia::CreateDecodeSwapChainForCompositionSurfaceHandle: ppSwapChain, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSwapChain` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIDecodeSwapChain`.
        let Some(swap_chain) = (unsafe { SystemDecodeSwapChain::<A>::from_owned(swap_chain) })
        else {
            return Err(E_FAIL);
        };
        Ok(swap_chain)
    }
}

impl<A: System> directx_graphics_infrastructure::InfoQueue for SystemInfoQueue<A> {
    fn set_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
        message_count_limit: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::SetMessageCountLimit, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::SetMessageCountLimit` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_set_message_count_limit)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                message_count_limit,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn clear_stored_messages(&self, producer: component_object_model::structures::Guid) {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::ClearStoredMessages, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::ClearStoredMessages` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_clear_stored_messages)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        let () = returned;
    }
    fn get_message(
        &self,
        producer: component_object_model::structures::Guid,
        message_index: u64,
        message: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = producer;
        let _ = message_index;
        let _ = message;
        // Refused by its kinds (src/kinds.rs), the system never called: pMessage: DXGI_INFO_QUEUE_MESSAGE out.pDescription: an address the system writes into a structure, valid until a later call the metadata does not name.
        Err(E_NOTIMPL)
    }
    fn get_num_stored_messages_allowed_by_retrieval_filters(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetNumStoredMessagesAllowedByRetrievalFilters` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_num_stored_messages_allowed_by_retrieval_filters)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn get_num_stored_messages(&self, producer: component_object_model::structures::Guid) -> u64 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetNumStoredMessages, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetNumStoredMessages` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_num_stored_messages)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn get_num_messages_discarded_by_message_count_limit(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetNumMessagesDiscardedByMessageCountLimit` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_num_messages_discarded_by_message_count_limit)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn get_message_count_limit(&self, producer: component_object_model::structures::Guid) -> u64 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetMessageCountLimit, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetMessageCountLimit` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_message_count_limit)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn get_num_messages_allowed_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetNumMessagesAllowedByStorageFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_num_messages_allowed_by_storage_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn get_num_messages_denied_by_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u64 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetNumMessagesDeniedByStorageFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_num_messages_denied_by_storage_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn add_storage_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = producer;
        let _ = filter;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = producer;
        let _ = filter;
        // Refused by its kinds (src/kinds.rs), the system never called: pFilter: DXGI_INFO_QUEUE_FILTER out.AllowList: DXGI_INFO_QUEUE_FILTER_DESC out.pCategoryList: an address the system writes into a structure, valid until a later call the metadata does not name; pFilter: DXGI_INFO_QUEUE_FILTER out.AllowList: DXGI_INFO_QUEUE_FILTER_DESC out.pSeverityList: an address the system writes into a structure, valid until a later call the metadata does not name; pFilter: DXGI_INFO_QUEUE_FILTER out.AllowList: DXGI_INFO_QUEUE_FILTER_DESC out.pIDList: an address the system writes into a structure, valid until a later call the metadata does not name.
        Err(E_NOTIMPL)
    }
    fn clear_storage_filter(&self, producer: component_object_model::structures::Guid) {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::ClearStorageFilter, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::ClearStorageFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_clear_storage_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        let () = returned;
    }
    fn push_empty_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::PushEmptyStorageFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PushEmptyStorageFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_push_empty_storage_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn push_deny_all_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::PushDenyAllStorageFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PushDenyAllStorageFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_push_deny_all_storage_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn push_copy_of_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::PushCopyOfStorageFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PushCopyOfStorageFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_push_copy_of_storage_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn push_storage_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = producer;
        let _ = filter;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn pop_storage_filter(&self, producer: component_object_model::structures::Guid) {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::PopStorageFilter, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PopStorageFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_pop_storage_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        let () = returned;
    }
    fn get_storage_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetStorageFilterStackSize, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetStorageFilterStackSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_storage_filter_stack_size)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn add_retrieval_filter_entries(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = producer;
        let _ = filter;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = producer;
        let _ = filter;
        // Refused by its kinds (src/kinds.rs), the system never called: pFilter: DXGI_INFO_QUEUE_FILTER out.AllowList: DXGI_INFO_QUEUE_FILTER_DESC out.pCategoryList: an address the system writes into a structure, valid until a later call the metadata does not name; pFilter: DXGI_INFO_QUEUE_FILTER out.AllowList: DXGI_INFO_QUEUE_FILTER_DESC out.pSeverityList: an address the system writes into a structure, valid until a later call the metadata does not name; pFilter: DXGI_INFO_QUEUE_FILTER out.AllowList: DXGI_INFO_QUEUE_FILTER_DESC out.pIDList: an address the system writes into a structure, valid until a later call the metadata does not name.
        Err(E_NOTIMPL)
    }
    fn clear_retrieval_filter(&self, producer: component_object_model::structures::Guid) {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::ClearRetrievalFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::ClearRetrievalFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_clear_retrieval_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        let () = returned;
    }
    fn push_empty_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::PushEmptyRetrievalFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PushEmptyRetrievalFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_push_empty_retrieval_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn push_deny_all_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::PushDenyAllRetrievalFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PushDenyAllRetrievalFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_push_deny_all_retrieval_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn push_copy_of_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::PushCopyOfRetrievalFilter, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PushCopyOfRetrievalFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_push_copy_of_retrieval_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn push_retrieval_filter(
        &self,
        producer: component_object_model::structures::Guid,
        filter: &directx_graphics_infrastructure::structures::InfoQueueFilter,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = producer;
        let _ = filter;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn pop_retrieval_filter(&self, producer: component_object_model::structures::Guid) {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::PopRetrievalFilter, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::PopRetrievalFilter` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_pop_retrieval_filter)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        let () = returned;
    }
    fn get_retrieval_filter_stack_size(
        &self,
        producer: component_object_model::structures::Guid,
    ) -> u32 {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::GetRetrievalFilterStackSize, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetRetrievalFilterStackSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_get_retrieval_filter_stack_size)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned
    }
    fn add_message(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        id: i32,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::AddMessage, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::AddMessage` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_add_message)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                category.0,
                severity.0,
                id,
                description.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn add_application_message(
        &self,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        description: &std::ffi::CStr,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIInfoQueue::AddApplicationMessage, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::AddApplicationMessage` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .info_queue_add_application_message)(
                self.object.raw(),
                severity.0,
                description.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        // Pinned (src/api.rs): a break raises a breakpoint exception inside the call, through our frames.
        if b_enable {
            return Err(E_INVALIDARG);
        }
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::SetBreakOnCategory, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::SetBreakOnCategory` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_set_break_on_category)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                category.0,
                0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        // Pinned (src/api.rs): a break raises a breakpoint exception inside the call, through our frames.
        if b_enable {
            return Err(E_INVALIDARG);
        }
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::SetBreakOnSeverity, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::SetBreakOnSeverity` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_set_break_on_severity)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                severity.0,
                0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_break_on_id(
        &self,
        producer: component_object_model::structures::Guid,
        id: i32,
        b_enable: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        // Pinned (src/api.rs): a break raises a breakpoint exception inside the call, through our frames.
        if b_enable {
            return Err(E_INVALIDARG);
        }
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::SetBreakOnID, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::SetBreakOnID` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_set_break_on_id)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                id,
                0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_break_on_category(
        &self,
        producer: component_object_model::structures::Guid,
        category: directx_graphics_infrastructure::enumerations::InfoQueueMessageCategory,
    ) -> bool {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::GetBreakOnCategory, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetBreakOnCategory` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_get_break_on_category)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                category.0,
            )
        };
        returned != 0
    }
    fn get_break_on_severity(
        &self,
        producer: component_object_model::structures::Guid,
        severity: directx_graphics_infrastructure::enumerations::InfoQueueMessageSeverity,
    ) -> bool {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::GetBreakOnSeverity, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetBreakOnSeverity` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_get_break_on_severity)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                severity.0,
            )
        };
        returned != 0
    }
    fn get_break_on_id(&self, producer: component_object_model::structures::Guid, id: i32) -> bool {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::GetBreakOnID, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetBreakOnID` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_get_break_on_id)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                id,
            )
        };
        returned != 0
    }
    fn set_mute_debug_output(
        &self,
        producer: component_object_model::structures::Guid,
        b_mute: bool,
    ) {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::SetMuteDebugOutput, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::SetMuteDebugOutput` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_set_mute_debug_output)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
                i32::from(b_mute),
            )
        };
        let () = returned;
    }
    fn get_mute_debug_output(&self, producer: component_object_model::structures::Guid) -> bool {
        #[expect(unsafe_code, reason = "IDXGIInfoQueue::GetMuteDebugOutput, called out")]
        // SAFETY: a live object whose table holds `IDXGIInfoQueue::GetMuteDebugOutput` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().info_queue_get_mute_debug_output)(
                self.object.raw(),
                component_object_model_direct::GuidAbi::of(&producer),
            )
        };
        returned != 0
    }
}

impl<A: System> directx_graphics_infrastructure::KeyedMutex for SystemKeyedMutex<A> {
    fn acquire_sync(
        &self,
        key: u64,
        dw_milliseconds: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIKeyedMutex::AcquireSync, called out")]
        // SAFETY: a live object whose table holds `IDXGIKeyedMutex::AcquireSync` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().keyed_mutex_acquire_sync)(
                self.object.raw(),
                key,
                dw_milliseconds,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn release_sync(&self, key: u64) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIKeyedMutex::ReleaseSync, called out")]
        // SAFETY: a live object whose table holds `IDXGIKeyedMutex::ReleaseSync` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().keyed_mutex_release_sync)(self.object.raw(), key)
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> directx_graphics_infrastructure::DeviceSubObject for SystemKeyedMutex<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut device: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIDeviceSubObject::GetDevice, called out")]
        // SAFETY: a live object whose table holds `IDXGIDeviceSubObject::GetDevice` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_sub_object_get_device)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut device,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIDeviceSubObject::GetDevice: ppDevice, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppDevice` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(device) = (unsafe { unknown_object_of::<A>(riid, device) }) else {
            return Err(E_FAIL);
        };
        Ok(device)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemKeyedMutex<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::Output6 for SystemOutput<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::OutputDesc1,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiOutputDesc1Layout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGIOutput6::GetDesc1, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput6::GetDesc1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output6_get_desc1)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_output_desc1(&desc.0);
        Ok(desc)
    }
    fn check_hardware_composition_support(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut flags = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput6::CheckHardwareCompositionSupport, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutput6::CheckHardwareCompositionSupport` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output6_check_hardware_composition_support)(
                self.object.raw(),
                flags.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let flags = get_u32(&flags.0, usize::MIN);
        Ok(flags)
    }
}

impl<A: System> directx_graphics_infrastructure::Output5 for SystemOutput<A> {
    fn duplicate_output1(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>, flags: u32, supported_formats: &[directx_graphics_infrastructure::enumerations::Format]) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication, component_object_model::structures::HResult>{
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let Ok(supported_formats_count) = u32::try_from(supported_formats.len()) else {
            return Err(E_INVALIDARG);
        };
        let supported_formats_c: Vec<i32> = supported_formats.iter().map(|value| value.0).collect();
        let mut output_duplication: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIOutput5::DuplicateOutput1, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput5::DuplicateOutput1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output5_duplicate_output1)(
                self.object.raw(),
                device_raw,
                flags,
                supported_formats_count,
                supported_formats_c.as_ptr().cast::<u8>(),
                &raw mut output_duplication,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput5::DuplicateOutput1: ppOutputDuplication, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppOutputDuplication` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIOutputDuplication`.
        let Some(output_duplication) =
            (unsafe { SystemOutputDuplication::<A>::from_owned(output_duplication) })
        else {
            return Err(E_FAIL);
        };
        Ok(output_duplication)
    }
}

impl<A: System> directx_graphics_infrastructure::Output4 for SystemOutput<A> {
    fn check_overlay_color_space_support(
        &self,
        format: directx_graphics_infrastructure::enumerations::Format,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
        concerned_device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let concerned_device_raw: Raw = match concerned_device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut flags = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput4::CheckOverlayColorSpaceSupport, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutput4::CheckOverlayColorSpaceSupport` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output4_check_overlay_color_space_support)(
                self.object.raw(),
                format.0,
                color_space.0,
                concerned_device_raw,
                flags.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let flags = get_u32(&flags.0, usize::MIN);
        Ok(flags)
    }
}

impl<A: System> directx_graphics_infrastructure::Output3 for SystemOutput<A> {
    fn check_overlay_support(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        concerned_device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let concerned_device_raw: Raw = match concerned_device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut flags = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGIOutput3::CheckOverlaySupport, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput3::CheckOverlaySupport` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output3_check_overlay_support)(
                self.object.raw(),
                enum_format.0,
                concerned_device_raw,
                flags.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let flags = get_u32(&flags.0, usize::MIN);
        Ok(flags)
    }
}

impl<A: System> directx_graphics_infrastructure::Output2 for SystemOutput<A> {
    fn supports_overlays(&self) -> bool {
        #[expect(unsafe_code, reason = "IDXGIOutput2::SupportsOverlays, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput2::SupportsOverlays` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().output2_supports_overlays)(self.object.raw()) };
        returned != 0
    }
}

impl<A: System> directx_graphics_infrastructure::Output1 for SystemOutput<A> {
    fn get_display_mode_list1(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        flags: directx_graphics_infrastructure::enumerations::EnumModes,
    ) -> Result<
        Vec<directx_graphics_infrastructure::structures::ModeDesc1>,
        component_object_model::structures::HResult,
    > {
        let mut num_modes_cell: u32 = 0;
        let mut desc_words: Vec<u64> = Vec::new();
        #[expect(unsafe_code, reason = "IDXGIOutput1::GetDisplayModeList1, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput1::GetDisplayModeList1` where the metadata orders it; every pointer passed is ours for the call.
        let first = unsafe {
            (self.object.entry_points().output1_get_display_mode_list1)(
                self.object.raw(),
                enum_format.0,
                flags.0,
                (&raw mut num_modes_cell).cast::<u8>(),
                if desc_words.is_empty() {
                    null_mut()
                } else {
                    desc_words.as_mut_ptr().cast::<u8>()
                },
            )
        };
        succeeded(first)?;
        let Some(desc_room) = usize::try_from(num_modes_cell)
            .ok()
            .filter(|count| *count <= 1 << 16)
        else {
            return Err(E_INVALIDARG);
        };
        desc_words = vec![0_u64; (desc_room * 32).div_ceil(8).max(1)];
        let Ok(room) = u32::try_from(desc_room) else {
            return Err(E_INVALIDARG);
        };
        num_modes_cell = room;
        #[expect(unsafe_code, reason = "IDXGIOutput1::GetDisplayModeList1, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput1::GetDisplayModeList1` where the metadata orders it; every pointer passed is ours for the call, the array the room its count says.
        let returned = unsafe {
            (self.object.entry_points().output1_get_display_mode_list1)(
                self.object.raw(),
                enum_format.0,
                flags.0,
                (&raw mut num_modes_cell).cast::<u8>(),
                if desc_words.is_empty() {
                    null_mut()
                } else {
                    desc_words.as_mut_ptr().cast::<u8>()
                },
            )
        };
        succeeded(returned)?;
        let desc_number = usize::try_from(num_modes_cell)
            .unwrap_or(usize::MAX)
            .min(desc_room);
        let desc_bytes: Vec<u8> = desc_words
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        let desc: Vec<_> = desc_bytes
            .get(..desc_number * 32)
            .unwrap_or_default()
            .as_chunks::<32>()
            .0
            .iter()
            .map(|element| decode_windows_win32_graphics_dxgi_dxgi_mode_desc1(element))
            .collect();
        Ok(desc)
    }
    fn find_closest_matching_mode1(
        &self,
        mode_to_match: &directx_graphics_infrastructure::structures::ModeDesc1,
        concerned_device: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<
        directx_graphics_infrastructure::structures::ModeDesc1,
        component_object_model::structures::HResult,
    > {
        let mut mode_to_match_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiModeDesc1Layout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_mode_desc1(
            mode_to_match,
            &mut mode_to_match_bytes.0,
        );
        let mut closest_match =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiModeDesc1Layout>() }>::default();
        let concerned_device_raw: Raw = match concerned_device {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput1::FindClosestMatchingMode1, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutput1::FindClosestMatchingMode1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output1_find_closest_matching_mode1)(
                self.object.raw(),
                mode_to_match_bytes.0.as_ptr(),
                closest_match.0.as_mut_ptr(),
                concerned_device_raw,
            )
        };
        succeeded(returned)?;
        let closest_match = decode_windows_win32_graphics_dxgi_dxgi_mode_desc1(&closest_match.0);
        Ok(closest_match)
    }
    fn get_display_surface_data1(
        &self,
        destination: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput1::GetDisplaySurfaceData1, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutput1::GetDisplaySurfaceData1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output1_get_display_surface_data1)(self.object.raw(), {
                let held = destination;
                held.raw()
            })
        };
        succeeded(returned)?;
        Ok(())
    }
    fn duplicate_output(&self, device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>) -> Result<<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::OutputDuplication, component_object_model::structures::HResult>{
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        let mut output_duplication: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIOutput1::DuplicateOutput, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput1::DuplicateOutput` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output1_duplicate_output)(
                self.object.raw(),
                device_raw,
                &raw mut output_duplication,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput1::DuplicateOutput: ppOutputDuplication, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppOutputDuplication` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIOutputDuplication`.
        let Some(output_duplication) =
            (unsafe { SystemOutputDuplication::<A>::from_owned(output_duplication) })
        else {
            return Err(E_FAIL);
        };
        Ok(output_duplication)
    }
}

impl<A: System> directx_graphics_infrastructure::Output for SystemOutput<A> {
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::OutputDesc,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiOutputDescLayout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGIOutput::GetDesc, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::GetDesc` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_get_desc)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_output_desc(&desc.0);
        Ok(desc)
    }
    fn get_display_mode_list(
        &self,
        enum_format: directx_graphics_infrastructure::enumerations::Format,
        flags: directx_graphics_infrastructure::enumerations::EnumModes,
    ) -> Result<
        Vec<directx_graphics_infrastructure::structures::ModeDesc>,
        component_object_model::structures::HResult,
    > {
        let mut num_modes_cell: u32 = 0;
        let mut desc_words: Vec<u64> = Vec::new();
        #[expect(unsafe_code, reason = "IDXGIOutput::GetDisplayModeList, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::GetDisplayModeList` where the metadata orders it; every pointer passed is ours for the call.
        let first = unsafe {
            (self.object.entry_points().output_get_display_mode_list)(
                self.object.raw(),
                enum_format.0,
                flags.0,
                (&raw mut num_modes_cell).cast::<u8>(),
                if desc_words.is_empty() {
                    null_mut()
                } else {
                    desc_words.as_mut_ptr().cast::<u8>()
                },
            )
        };
        succeeded(first)?;
        let Some(desc_room) = usize::try_from(num_modes_cell)
            .ok()
            .filter(|count| *count <= 1 << 16)
        else {
            return Err(E_INVALIDARG);
        };
        desc_words = vec![0_u64; (desc_room * 28).div_ceil(8).max(1)];
        let Ok(room) = u32::try_from(desc_room) else {
            return Err(E_INVALIDARG);
        };
        num_modes_cell = room;
        #[expect(unsafe_code, reason = "IDXGIOutput::GetDisplayModeList, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::GetDisplayModeList` where the metadata orders it; every pointer passed is ours for the call, the array the room its count says.
        let returned = unsafe {
            (self.object.entry_points().output_get_display_mode_list)(
                self.object.raw(),
                enum_format.0,
                flags.0,
                (&raw mut num_modes_cell).cast::<u8>(),
                if desc_words.is_empty() {
                    null_mut()
                } else {
                    desc_words.as_mut_ptr().cast::<u8>()
                },
            )
        };
        succeeded(returned)?;
        let desc_number = usize::try_from(num_modes_cell)
            .unwrap_or(usize::MAX)
            .min(desc_room);
        let desc_bytes: Vec<u8> = desc_words
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        let desc: Vec<_> = desc_bytes
            .get(..desc_number * 28)
            .unwrap_or_default()
            .as_chunks::<28>()
            .0
            .iter()
            .map(|element| decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(element))
            .collect();
        Ok(desc)
    }
    fn find_closest_matching_mode(
        &self,
        mode_to_match: &directx_graphics_infrastructure::structures::ModeDesc,
        concerned_device: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<
        directx_graphics_infrastructure::structures::ModeDesc,
        component_object_model::structures::HResult,
    > {
        let mut mode_to_match_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>() }>::default();
        encode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
            mode_to_match,
            &mut mode_to_match_bytes.0,
        );
        let mut closest_match =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>() }>::default();
        let concerned_device_raw: Raw = match concerned_device {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::FindClosestMatchingMode, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutput::FindClosestMatchingMode` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_find_closest_matching_mode)(
                self.object.raw(),
                mode_to_match_bytes.0.as_ptr(),
                closest_match.0.as_mut_ptr(),
                concerned_device_raw,
            )
        };
        succeeded(returned)?;
        let closest_match =
            decode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(&closest_match.0);
        Ok(closest_match)
    }
    fn wait_for_v_blank(&self) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIOutput::WaitForVBlank, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::WaitForVBlank` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().output_wait_for_v_blank)(self.object.raw()) };
        succeeded(returned)?;
        Ok(())
    }
    fn take_ownership(
        &self,
        device: directx_graphics_infrastructure::UnknownRef<'_, Self::Api>,
        exclusive: bool,
    ) -> Result<(), component_object_model::structures::HResult> {
        let device_raw: Raw = match device {
            directx_graphics_infrastructure::UnknownRef::Adapter(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Debug(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Device(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::DisplayControl(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Factory(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::FactoryMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::InfoQueue(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::KeyedMutex(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Output(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::OutputDuplication(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Resource(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Surface(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChain(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object) => object.raw(),
            directx_graphics_infrastructure::UnknownRef::Unknown(foreign) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
        };
        #[expect(unsafe_code, reason = "IDXGIOutput::TakeOwnership, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::TakeOwnership` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_take_ownership)(
                self.object.raw(),
                device_raw,
                i32::from(exclusive),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn release_ownership(&self) {
        #[expect(unsafe_code, reason = "IDXGIOutput::ReleaseOwnership, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::ReleaseOwnership` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().output_release_ownership)(self.object.raw()) };
        let () = returned;
    }
    fn get_gamma_control_capabilities(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::GammaControlCapabilities,
        component_object_model::structures::HResult,
    > {
        let mut gamma_caps = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlCapabilitiesLayout>() },
        >::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIOutput::GetGammaControlCapabilities, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutput::GetGammaControlCapabilities` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output_get_gamma_control_capabilities)(
                self.object.raw(),
                gamma_caps.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let gamma_caps = decode_windows_win32_graphics_dxgi_common_dxgi_gamma_control_capabilities(
            &gamma_caps.0,
        );
        Ok(gamma_caps)
    }
    fn set_gamma_control(
        &self,
        array: &directx_graphics_infrastructure::structures::GammaControl,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut array_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout>() },
        >::default();
        encode_windows_win32_graphics_dxgi_common_dxgi_gamma_control(array, &mut array_bytes.0);
        #[expect(unsafe_code, reason = "IDXGIOutput::SetGammaControl, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::SetGammaControl` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_set_gamma_control)(
                self.object.raw(),
                array_bytes.0.as_ptr(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_gamma_control(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::GammaControl,
        component_object_model::structures::HResult,
    > {
        let mut array = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiCommonDxgiGammaControlLayout>() },
        >::default();
        #[expect(unsafe_code, reason = "IDXGIOutput::GetGammaControl, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::GetGammaControl` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_get_gamma_control)(
                self.object.raw(),
                array.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let array = decode_windows_win32_graphics_dxgi_common_dxgi_gamma_control(&array.0);
        Ok(array)
    }
    fn set_display_surface(
        &self,
        scanout_surface: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIOutput::SetDisplaySurface, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::SetDisplaySurface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_set_display_surface)(self.object.raw(), {
                let held = scanout_surface;
                held.raw()
            })
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_display_surface_data(
        &self,
        destination: &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIOutput::GetDisplaySurfaceData, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::GetDisplaySurfaceData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_get_display_surface_data)(self.object.raw(), {
                let held = destination;
                held.raw()
            })
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_frame_statistics(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatistics,
        component_object_model::structures::HResult,
    > {
        let mut stats =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout>() }>::default(
            );
        #[expect(unsafe_code, reason = "IDXGIOutput::GetFrameStatistics, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutput::GetFrameStatistics` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_get_frame_statistics)(
                self.object.raw(),
                stats.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let stats = decode_windows_win32_graphics_dxgi_dxgi_frame_statistics(&stats.0);
        Ok(stats)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemOutput<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::OutputDuplication for SystemOutputDuplication<A> {
    fn get_desc(&self) -> directx_graphics_infrastructure::structures::OutduplDesc {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiOutduplDescLayout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGIOutputDuplication::GetDesc, called out")]
        // SAFETY: a live object whose table holds `IDXGIOutputDuplication::GetDesc` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_duplication_get_desc)(
                self.object.raw(),
                desc.0.as_mut_ptr(),
            )
        };
        let desc = decode_windows_win32_graphics_dxgi_dxgi_outdupl_desc(&desc.0);
        let () = returned;
        desc
    }
    fn acquire_next_frame(
        &self,
        timeout_in_milliseconds: u32,
    ) -> Result<
        (
            directx_graphics_infrastructure::structures::OutduplFrameInfo,
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Resource,
        ),
        component_object_model::structures::HResult,
    > {
        let mut frame_info = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiDxgiOutduplFrameInfoLayout>() },
        >::default();
        let mut desktop_resource: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::AcquireNextFrame, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutputDuplication::AcquireNextFrame` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output_duplication_acquire_next_frame)(
                self.object.raw(),
                timeout_in_milliseconds,
                frame_info.0.as_mut_ptr(),
                &raw mut desktop_resource,
            )
        };
        succeeded(returned)?;
        let frame_info = decode_windows_win32_graphics_dxgi_dxgi_outdupl_frame_info(&frame_info.0);
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::AcquireNextFrame: ppDesktopResource, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppDesktopResource` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIResource`.
        let Some(desktop_resource) =
            (unsafe { SystemResource::<A>::from_owned_older(desktop_resource) })
        else {
            return Err(E_FAIL);
        };
        Ok((frame_info, desktop_resource))
    }
    fn get_frame_dirty_rects(
        &self,
        dirty_rects_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let Ok(dirty_rects_buffer_length) = usize::try_from(dirty_rects_buffer.len()) else {
            return Err(E_INVALIDARG);
        };
        let mut dirty_rects_buffer_buffer = Vec::new();
        if dirty_rects_buffer_buffer
            .try_reserve_exact(dirty_rects_buffer_length)
            .is_err()
        {
            return Err(E_OUTOFMEMORY);
        }
        dirty_rects_buffer_buffer.resize(dirty_rects_buffer_length, u8::MIN);
        let Ok(dirty_rects_buffer_size) = u32::try_from(dirty_rects_buffer_length) else {
            return Err(E_INVALIDARG);
        };
        let mut dirty_rects_buffer_size_required = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetFrameDirtyRects, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutputDuplication::GetFrameDirtyRects` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output_duplication_get_frame_dirty_rects)(
                self.object.raw(),
                dirty_rects_buffer_size,
                dirty_rects_buffer_buffer.as_mut_ptr(),
                dirty_rects_buffer_size_required.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let dirty_rects_buffer_size_required =
            get_u32(&dirty_rects_buffer_size_required.0, usize::MIN);
        dirty_rects_buffer.write(&dirty_rects_buffer_buffer);
        Ok(dirty_rects_buffer_size_required)
    }
    fn get_frame_move_rects(
        &self,
        move_rect_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let Ok(move_rect_buffer_length) = usize::try_from(move_rect_buffer.len()) else {
            return Err(E_INVALIDARG);
        };
        let mut move_rect_buffer_buffer = Vec::new();
        if move_rect_buffer_buffer
            .try_reserve_exact(move_rect_buffer_length)
            .is_err()
        {
            return Err(E_OUTOFMEMORY);
        }
        move_rect_buffer_buffer.resize(move_rect_buffer_length, u8::MIN);
        let Ok(move_rects_buffer_size) = u32::try_from(move_rect_buffer_length) else {
            return Err(E_INVALIDARG);
        };
        let mut move_rects_buffer_size_required = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetFrameMoveRects, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutputDuplication::GetFrameMoveRects` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output_duplication_get_frame_move_rects)(
                self.object.raw(),
                move_rects_buffer_size,
                move_rect_buffer_buffer.as_mut_ptr(),
                move_rects_buffer_size_required.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let move_rects_buffer_size_required =
            get_u32(&move_rects_buffer_size_required.0, usize::MIN);
        move_rect_buffer.write(&move_rect_buffer_buffer);
        Ok(move_rects_buffer_size_required)
    }
    fn get_frame_pointer_shape(
        &self,
        pointer_shape_buffer: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<
        (
            u32,
            directx_graphics_infrastructure::structures::OutduplPointerShapeInfo,
        ),
        component_object_model::structures::HResult,
    > {
        let Ok(pointer_shape_buffer_length) = usize::try_from(pointer_shape_buffer.len()) else {
            return Err(E_INVALIDARG);
        };
        let mut pointer_shape_buffer_buffer = Vec::new();
        if pointer_shape_buffer_buffer
            .try_reserve_exact(pointer_shape_buffer_length)
            .is_err()
        {
            return Err(E_OUTOFMEMORY);
        }
        pointer_shape_buffer_buffer.resize(pointer_shape_buffer_length, u8::MIN);
        let Ok(pointer_shape_buffer_size) = u32::try_from(pointer_shape_buffer_length) else {
            return Err(E_INVALIDARG);
        };
        let mut pointer_shape_buffer_size_required = Aligned::<4>::default();
        let mut pointer_shape_info = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiDxgiOutduplPointerShapeInfoLayout>() },
        >::default();
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::GetFramePointerShape, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutputDuplication::GetFramePointerShape` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .output_duplication_get_frame_pointer_shape)(
                self.object.raw(),
                pointer_shape_buffer_size,
                pointer_shape_buffer_buffer.as_mut_ptr(),
                pointer_shape_buffer_size_required.0.as_mut_ptr(),
                pointer_shape_info.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let pointer_shape_buffer_size_required =
            get_u32(&pointer_shape_buffer_size_required.0, usize::MIN);
        let pointer_shape_info = decode_windows_win32_graphics_dxgi_dxgi_outdupl_pointer_shape_info(
            &pointer_shape_info.0,
        );
        pointer_shape_buffer.write(&pointer_shape_buffer_buffer);
        Ok((pointer_shape_buffer_size_required, pointer_shape_info))
    }
    fn map_desktop_surface(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::MappedRect,
        component_object_model::structures::HResult,
    > {
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn un_map_desktop_surface(&self) -> Result<(), component_object_model::structures::HResult> {
        // Pinned (src/api.rs): never called by the calling glue: pairs with MapDesktopSurface, which is never called.
        Err(E_NOTIMPL)
    }
    fn release_frame(&self) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGIOutputDuplication::ReleaseFrame, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIOutputDuplication::ReleaseFrame` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().output_duplication_release_frame)(self.object.raw())
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemOutputDuplication<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::Resource1 for SystemResource<A> {
    fn create_subresource_surface(
        &self,
        index: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Surface,
        component_object_model::structures::HResult,
    > {
        let mut surface: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGIResource1::CreateSubresourceSurface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIResource1::CreateSubresourceSurface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .resource1_create_subresource_surface)(
                self.object.raw(), index, &raw mut surface
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIResource1::CreateSubresourceSurface: ppSurface, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSurface` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGISurface2`.
        let Some(surface) = (unsafe { SystemSurface::<A>::from_owned(surface) }) else {
            return Err(E_FAIL);
        };
        Ok(surface)
    }
    fn create_shared_handle(
        &self,
        attributes: Option<&component_object_model::structures::SecurityAttributes>,
        dw_access: u32,
        lp_name: Option<&[u16]>,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        let _ = attributes;
        let _ = dw_access;
        let _ = lp_name;
        // Refused by its kinds (src/kinds.rs), the system never called: pAttributes: SECURITY_ATTRIBUTES.lpSecurityDescriptor: a raw pointer with no size, which the system reads or writes as it likes.
        Err(E_NOTIMPL)
    }
}

impl<A: System> directx_graphics_infrastructure::Resource for SystemResource<A> {
    fn get_shared_handle(
        &self,
    ) -> Result<
        component_object_model::structures::Handle,
        component_object_model::structures::HResult,
    > {
        let mut shared_handle = Aligned::<8>::default();
        #[expect(unsafe_code, reason = "IDXGIResource::GetSharedHandle, called out")]
        // SAFETY: a live object whose table holds `IDXGIResource::GetSharedHandle` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().resource_get_shared_handle)(
                self.object.raw(),
                shared_handle.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let shared_handle =
            component_object_model::structures::Handle(get_usize(&shared_handle.0, usize::MIN));
        Ok(shared_handle)
    }
    fn get_usage(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::Usage,
        component_object_model::structures::HResult,
    > {
        let mut usage = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGIResource::GetUsage, called out")]
        // SAFETY: a live object whose table holds `IDXGIResource::GetUsage` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().resource_get_usage)(self.object.raw(), usage.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let usage =
            directx_graphics_infrastructure::enumerations::Usage(get_u32(&usage.0, usize::MIN));
        Ok(usage)
    }
    fn set_eviction_priority(
        &self,
        eviction_priority: directx_graphics_infrastructure::enumerations::ResourcePriority,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGIResource::SetEvictionPriority, called out")]
        // SAFETY: a live object whose table holds `IDXGIResource::SetEvictionPriority` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().resource_set_eviction_priority)(
                self.object.raw(),
                eviction_priority.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_eviction_priority(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ResourcePriority,
        component_object_model::structures::HResult,
    > {
        let mut eviction_priority = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGIResource::GetEvictionPriority, called out")]
        // SAFETY: a live object whose table holds `IDXGIResource::GetEvictionPriority` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().resource_get_eviction_priority)(
                self.object.raw(),
                eviction_priority.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let eviction_priority = directx_graphics_infrastructure::enumerations::ResourcePriority(
            get_u32(&eviction_priority.0, usize::MIN),
        );
        Ok(eviction_priority)
    }
}

impl<A: System> directx_graphics_infrastructure::DeviceSubObject for SystemResource<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut device: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIDeviceSubObject::GetDevice, called out")]
        // SAFETY: a live object whose table holds `IDXGIDeviceSubObject::GetDevice` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_sub_object_get_device)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut device,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIDeviceSubObject::GetDevice: ppDevice, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppDevice` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(device) = (unsafe { unknown_object_of::<A>(riid, device) }) else {
            return Err(E_FAIL);
        };
        Ok(device)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemResource<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::Surface2 for SystemSurface<A> {
    fn get_resource(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        (
            directx_graphics_infrastructure::UnknownObject<Self::Api>,
            u32,
        ),
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent_resource: Raw = null_mut();
        let mut subresource_index = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGISurface2::GetResource, called out")]
        // SAFETY: a live object whose table holds `IDXGISurface2::GetResource` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().surface2_get_resource)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent_resource,
                subresource_index.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGISurface2::GetResource: ppParentResource, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParentResource` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent_resource) = (unsafe { unknown_object_of::<A>(riid, parent_resource) })
        else {
            return Err(E_FAIL);
        };
        let subresource_index = get_u32(&subresource_index.0, usize::MIN);
        Ok((parent_resource, subresource_index))
    }
}

impl<A: System> directx_graphics_infrastructure::Surface1 for SystemSurface<A> {
    fn get_dc(
        &self,
        discard: bool,
    ) -> Result<component_object_model::structures::Hdc, component_object_model::structures::HResult>
    {
        let _ = discard;
        // Pinned (src/api.rs): never called by the calling glue: locks a surface for GDI, beside memory a view may hold.
        Err(E_NOTIMPL)
    }
    fn release_dc(
        &self,
        dirty_rect: Option<&component_object_model::structures::Rect>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = dirty_rect;
        // Pinned (src/api.rs): never called by the calling glue: pairs with GetDC, which is never called.
        Err(E_NOTIMPL)
    }
}

impl<A: System> directx_graphics_infrastructure::Surface for SystemSurface<A> {
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SurfaceDesc,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSurfaceDescLayout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGISurface::GetDesc, called out")]
        // SAFETY: a live object whose table holds `IDXGISurface::GetDesc` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().surface_get_desc)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_surface_desc(&desc.0);
        Ok(desc)
    }
    fn map(
        &self,
        map_flags: directx_graphics_infrastructure::enumerations::MapFlags,
    ) -> Result<
        directx_graphics_infrastructure::structures::MappedRect,
        component_object_model::structures::HResult,
    > {
        let _ = map_flags;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn unmap(&self) -> Result<(), component_object_model::structures::HResult> {
        // Pinned (src/api.rs): never called by the calling glue: could unmap a subresource the context lent, behind its view.
        Err(E_NOTIMPL)
    }
}

impl<A: System> directx_graphics_infrastructure::DeviceSubObject for SystemSurface<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut device: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIDeviceSubObject::GetDevice, called out")]
        // SAFETY: a live object whose table holds `IDXGIDeviceSubObject::GetDevice` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_sub_object_get_device)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut device,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIDeviceSubObject::GetDevice: ppDevice, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppDevice` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(device) = (unsafe { unknown_object_of::<A>(riid, device) }) else {
            return Err(E_FAIL);
        };
        Ok(device)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemSurface<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::SwapChain4 for SystemSwapChain<A> {
    fn set_hdr_meta_data(
        &self,
        type_: directx_graphics_infrastructure::enumerations::HdrMetadataType,
        meta_data: Option<&[u8]>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let meta_data_given = meta_data.is_some();
        let meta_data: &[u8] = meta_data.unwrap_or_default();
        let Ok(size) = u32::try_from(meta_data.len()) else {
            return Err(E_INVALIDARG);
        };
        let meta_data_c: Vec<u8> = meta_data.to_vec();
        #[expect(unsafe_code, reason = "IDXGISwapChain4::SetHDRMetaData, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain4::SetHDRMetaData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain4_set_hdr_meta_data)(
                self.object.raw(),
                type_.0,
                size,
                if meta_data_given {
                    meta_data_c.as_ptr()
                } else {
                    null()
                },
            )
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> directx_graphics_infrastructure::SwapChain3 for SystemSwapChain<A> {
    fn get_current_back_buffer_index(&self) -> u32 {
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain3::GetCurrentBackBufferIndex, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain3::GetCurrentBackBufferIndex` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain3_get_current_back_buffer_index)(self.object.raw())
        };
        returned
    }
    fn check_color_space_support(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut color_space_support = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain3::CheckColorSpaceSupport, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain3::CheckColorSpaceSupport` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain3_check_color_space_support)(
                self.object.raw(),
                color_space.0,
                color_space_support.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let color_space_support = get_u32(&color_space_support.0, usize::MIN);
        Ok(color_space_support)
    }
    fn set_color_space1(
        &self,
        color_space: directx_graphics_infrastructure::enumerations::ColorSpaceType,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGISwapChain3::SetColorSpace1, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain3::SetColorSpace1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain3_set_color_space1)(
                self.object.raw(),
                color_space.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn resize_buffers1(
        &self,
        width: u32,
        height: u32,
        format: directx_graphics_infrastructure::enumerations::Format,
        swap_chain_flags: u32,
        creation_node_mask: &[u32],
        present_queue: &[Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>],
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = width;
        let _ = height;
        let _ = format;
        let _ = swap_chain_flags;
        let _ = creation_node_mask;
        let _ = present_queue;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
}

impl<A: System> directx_graphics_infrastructure::SwapChain2 for SystemSwapChain<A> {
    fn set_source_size(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGISwapChain2::SetSourceSize, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain2::SetSourceSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain2_set_source_size)(
                self.object.raw(),
                width,
                height,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_source_size(&self) -> Result<(u32, u32), component_object_model::structures::HResult> {
        let mut width = Aligned::<4>::default();
        let mut height = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGISwapChain2::GetSourceSize, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain2::GetSourceSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain2_get_source_size)(
                self.object.raw(),
                width.0.as_mut_ptr(),
                height.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let width = get_u32(&width.0, usize::MIN);
        let height = get_u32(&height.0, usize::MIN);
        Ok((width, height))
    }
    fn set_maximum_frame_latency(
        &self,
        max_latency: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::SetMaximumFrameLatency, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain2::SetMaximumFrameLatency` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain2_set_maximum_frame_latency)(self.object.raw(), max_latency)
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_maximum_frame_latency(
        &self,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let mut max_latency = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::GetMaximumFrameLatency, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain2::GetMaximumFrameLatency` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain2_get_maximum_frame_latency)(
                self.object.raw(),
                max_latency.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let max_latency = get_u32(&max_latency.0, usize::MIN);
        Ok(max_latency)
    }
    fn get_frame_latency_waitable_object(&self) -> component_object_model::structures::Handle {
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::GetFrameLatencyWaitableObject, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain2::GetFrameLatencyWaitableObject` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain2_get_frame_latency_waitable_object)(self.object.raw())
        };
        component_object_model::structures::Handle(returned)
    }
    fn set_matrix_transform(
        &self,
        matrix: &directx_graphics_infrastructure::structures::Matrix3x2F,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut matrix_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_matrix_3_x2_f(matrix, &mut matrix_bytes.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::SetMatrixTransform, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain2::SetMatrixTransform` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain2_set_matrix_transform)(
                self.object.raw(),
                matrix_bytes.0.as_ptr(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_matrix_transform(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::Matrix3x2F,
        component_object_model::structures::HResult,
    > {
        let mut matrix =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiMatrix3X2FLayout>() }>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain2::GetMatrixTransform, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain2::GetMatrixTransform` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain2_get_matrix_transform)(
                self.object.raw(),
                matrix.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let matrix = decode_windows_win32_graphics_dxgi_dxgi_matrix_3_x2_f(&matrix.0);
        Ok(matrix)
    }
}

impl<A: System> directx_graphics_infrastructure::SwapChain1 for SystemSwapChain<A> {
    fn get_desc1(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainDesc1,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDesc1Layout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGISwapChain1::GetDesc1, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::GetDesc1` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_get_desc1)(
                self.object.raw(),
                desc.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc1(&desc.0);
        Ok(desc)
    }
    fn get_fullscreen_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainFullscreenDesc,
        component_object_model::structures::HResult,
    > {
        let mut desc = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainFullscreenDescLayout>() },
        >::default();
        #[expect(unsafe_code, reason = "IDXGISwapChain1::GetFullscreenDesc, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::GetFullscreenDesc` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_get_fullscreen_desc)(
                self.object.raw(),
                desc.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_swap_chain_fullscreen_desc(&desc.0);
        Ok(desc)
    }
    fn get_hwnd(
        &self,
    ) -> Result<component_object_model::structures::Hwnd, component_object_model::structures::HResult>
    {
        let mut hwnd = Aligned::<8>::default();
        #[expect(unsafe_code, reason = "IDXGISwapChain1::GetHwnd, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::GetHwnd` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_get_hwnd)(
                self.object.raw(),
                hwnd.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let hwnd = component_object_model::structures::Hwnd(get_usize(&hwnd.0, usize::MIN));
        Ok(hwnd)
    }
    fn get_core_window(
        &self,
        refiid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut refiid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(refiid, &mut refiid_bytes.0);
        let mut unk: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGISwapChain1::GetCoreWindow, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::GetCoreWindow` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_get_core_window)(
                self.object.raw(),
                refiid_bytes.0.as_ptr(),
                &raw mut unk,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetCoreWindow: ppUnk, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppUnk` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `refiid` names.
        let Some(unk) = (unsafe { unknown_object_of::<A>(refiid, unk) }) else {
            return Err(E_FAIL);
        };
        Ok(unk)
    }
    fn present1(
        &self,
        sync_interval: u32,
        present_flags: directx_graphics_infrastructure::enumerations::Present,
        present_parameters: &directx_graphics_infrastructure::structures::PresentParameters,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = sync_interval;
        let _ = present_flags;
        let _ = present_parameters;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn is_temporary_mono_supported(&self) -> bool {
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::IsTemporaryMonoSupported, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::IsTemporaryMonoSupported` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain1_is_temporary_mono_supported)(self.object.raw())
        };
        returned != 0
    }
    fn get_restrict_to_output(
        &self,
    ) -> Result<
        Option<
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
        component_object_model::structures::HResult,
    > {
        let mut restrict_to_output: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetRestrictToOutput, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::GetRestrictToOutput` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain1_get_restrict_to_output)(
                self.object.raw(), &raw mut restrict_to_output
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetRestrictToOutput: ppRestrictToOutput, an object the metadata's Out hands over"
        )]
        // SAFETY: the metadata's `ppRestrictToOutput` is `Out`: after a success, null or one reference to an object of `IDXGIOutput`.
        let restrict_to_output = unsafe { SystemOutput::<A>::from_owned_older(restrict_to_output) };
        Ok(restrict_to_output)
    }
    fn set_background_color(
        &self,
        color: &directx_graphics_infrastructure::structures::Rgba,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut color_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiRgbaLayout>() }>::default();
        encode_windows_win32_graphics_dxgi_dxgi_rgba(color, &mut color_bytes.0);
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::SetBackgroundColor, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::SetBackgroundColor` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_set_background_color)(
                self.object.raw(),
                color_bytes.0.as_ptr(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_background_color(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::Rgba,
        component_object_model::structures::HResult,
    > {
        let mut color =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiRgbaLayout>() }>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain1::GetBackgroundColor, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::GetBackgroundColor` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_get_background_color)(
                self.object.raw(),
                color.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let color = decode_windows_win32_graphics_dxgi_dxgi_rgba(&color.0);
        Ok(color)
    }
    fn set_rotation(
        &self,
        rotation: directx_graphics_infrastructure::enumerations::ModeRotation,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGISwapChain1::SetRotation, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::SetRotation` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_set_rotation)(self.object.raw(), rotation.0)
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_rotation(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::enumerations::ModeRotation,
        component_object_model::structures::HResult,
    > {
        let mut rotation = Aligned::<4>::default();
        #[expect(unsafe_code, reason = "IDXGISwapChain1::GetRotation, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain1::GetRotation` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain1_get_rotation)(
                self.object.raw(),
                rotation.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let rotation = directx_graphics_infrastructure::enumerations::ModeRotation(get_i32(
            &rotation.0,
            usize::MIN,
        ));
        Ok(rotation)
    }
}

impl<A: System> directx_graphics_infrastructure::SwapChain for SystemSwapChain<A> {
    fn present(
        &self,
        sync_interval: u32,
        flags: directx_graphics_infrastructure::enumerations::Present,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGISwapChain::Present, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::Present` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_present)(
                self.object.raw(),
                sync_interval,
                flags.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_buffer(
        &self,
        buffer: u32,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer,
        component_object_model::structures::HResult,
    > {
        let riid = guid_bytes(&<<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer as Adopt>::INTERFACE);
        let mut surface: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGISwapChain::GetBuffer, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::GetBuffer` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_get_buffer)(
                self.object.raw(),
                buffer,
                riid.as_ptr(),
                &raw mut surface,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetBuffer: ppSurface, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppSurface` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` named, which is `Adopt::INTERFACE`.
        let Some(surface) = (unsafe {
            <<A as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Buffer as Adopt>::adopt(surface)
        }) else {
            return Err(E_FAIL);
        };
        Ok(surface)
    }
    fn set_fullscreen_state(
        &self,
        fullscreen: bool,
        target: Option<
            &<Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        >,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGISwapChain::SetFullscreenState, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::SetFullscreenState` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_set_fullscreen_state)(
                self.object.raw(),
                i32::from(fullscreen),
                target.map_or(null_mut(), |held| held.raw()),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_fullscreen_state(
        &self,
    ) -> Result<
        (
            bool,
            <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        ),
        component_object_model::structures::HResult,
    > {
        let mut fullscreen = Aligned::<4>::default();
        let mut target: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGISwapChain::GetFullscreenState, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::GetFullscreenState` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_get_fullscreen_state)(
                self.object.raw(),
                fullscreen.0.as_mut_ptr(),
                &raw mut target,
            )
        };
        succeeded(returned)?;
        let fullscreen = get_i32(&fullscreen.0, usize::MIN) != 0;
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetFullscreenState: ppTarget, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppTarget` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIOutput`.
        let Some(target) = (unsafe { SystemOutput::<A>::from_owned_older(target) }) else {
            return Err(E_FAIL);
        };
        Ok((fullscreen, target))
    }
    fn get_desc(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::SwapChainDesc,
        component_object_model::structures::HResult,
    > {
        let mut desc =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiSwapChainDescLayout>() }>::default();
        #[expect(unsafe_code, reason = "IDXGISwapChain::GetDesc, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::GetDesc` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_get_desc)(self.object.raw(), desc.0.as_mut_ptr())
        };
        succeeded(returned)?;
        let desc = decode_windows_win32_graphics_dxgi_dxgi_swap_chain_desc(&desc.0);
        Ok(desc)
    }
    fn resize_buffers(
        &self,
        buffer_count: u32,
        width: u32,
        height: u32,
        new_format: directx_graphics_infrastructure::enumerations::Format,
        swap_chain_flags: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "IDXGISwapChain::ResizeBuffers, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::ResizeBuffers` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_resize_buffers)(
                self.object.raw(),
                buffer_count,
                width,
                height,
                new_format.0,
                swap_chain_flags,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn resize_target(
        &self,
        new_target_parameters: &directx_graphics_infrastructure::structures::ModeDesc,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut new_target_parameters_bytes =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiCommonDxgiModeDescLayout>() }>::default();
        encode_windows_win32_graphics_dxgi_common_dxgi_mode_desc(
            new_target_parameters,
            &mut new_target_parameters_bytes.0,
        );
        #[expect(unsafe_code, reason = "IDXGISwapChain::ResizeTarget, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::ResizeTarget` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_resize_target)(
                self.object.raw(),
                new_target_parameters_bytes.0.as_ptr(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_containing_output(
        &self,
    ) -> Result<
        <Self::Api as directx_graphics_infrastructure::DirectXGraphicsInfrastructure>::Output,
        component_object_model::structures::HResult,
    > {
        let mut output: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetContainingOutput, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain::GetContainingOutput` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_get_containing_output)(
                self.object.raw(),
                &raw mut output,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetContainingOutput: ppOutput, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppOutput` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of `IDXGIOutput`.
        let Some(output) = (unsafe { SystemOutput::<A>::from_owned_older(output) }) else {
            return Err(E_FAIL);
        };
        Ok(output)
    }
    fn get_frame_statistics(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatistics,
        component_object_model::structures::HResult,
    > {
        let mut stats =
            Aligned::<{ size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsLayout>() }>::default(
            );
        #[expect(unsafe_code, reason = "IDXGISwapChain::GetFrameStatistics, called out")]
        // SAFETY: a live object whose table holds `IDXGISwapChain::GetFrameStatistics` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_get_frame_statistics)(
                self.object.raw(),
                stats.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let stats = decode_windows_win32_graphics_dxgi_dxgi_frame_statistics(&stats.0);
        Ok(stats)
    }
    fn get_last_present_count(&self) -> Result<u32, component_object_model::structures::HResult> {
        let mut last_present_count = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChain::GetLastPresentCount, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChain::GetLastPresentCount` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().swap_chain_get_last_present_count)(
                self.object.raw(),
                last_present_count.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let last_present_count = get_u32(&last_present_count.0, usize::MIN);
        Ok(last_present_count)
    }
}

impl<A: System> directx_graphics_infrastructure::DeviceSubObject for SystemSwapChain<A> {
    fn get_device(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut device: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIDeviceSubObject::GetDevice, called out")]
        // SAFETY: a live object whose table holds `IDXGIDeviceSubObject::GetDevice` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_sub_object_get_device)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut device,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIDeviceSubObject::GetDevice: ppDevice, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppDevice` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(device) = (unsafe { unknown_object_of::<A>(riid, device) }) else {
            return Err(E_FAIL);
        };
        Ok(device)
    }
}

impl<A: System> directx_graphics_infrastructure::Object for SystemSwapChain<A> {
    type Api = A;

    fn set_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: &[u8],
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let Ok(data_size) = u32::try_from(data.len()) else {
            return Err(E_INVALIDARG);
        };
        let data_c: Vec<u8> = data.to_vec();
        #[expect(unsafe_code, reason = "IDXGIObject::SetPrivateData, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                data_size,
                data_c.as_ptr().cast::<u8>(),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn set_private_data_interface(
        &self,
        name: &component_object_model::structures::Guid,
        unknown: Option<directx_graphics_infrastructure::UnknownRef<'_, Self::Api>>,
    ) -> Result<(), component_object_model::structures::HResult> {
        let mut name_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(name, &mut name_bytes.0);
        let unknown_raw: Raw = match unknown {
            Some(directx_graphics_infrastructure::UnknownRef::Adapter(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Debug(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DecodeSwapChain(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Device(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::DisplayControl(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Factory(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::FactoryMedia(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::InfoQueue(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::KeyedMutex(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Output(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::OutputDuplication(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Resource(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::Surface(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChain(object)) => object.raw(),
            Some(directx_graphics_infrastructure::UnknownRef::SwapChainMedia(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::GraphicsAnalysis(object)) => {
                object.raw()
            }
            Some(directx_graphics_infrastructure::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::SetPrivateDataInterface, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGIObject::SetPrivateDataInterface` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_set_private_data_interface)(
                self.object.raw(),
                name_bytes.0.as_ptr(),
                unknown_raw,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn get_private_data(
        &self,
        name: &component_object_model::structures::Guid,
        data: Option<&impl wcontainers_volatile_bytes::VolatileBytes>,
    ) -> Result<u32, component_object_model::structures::HResult> {
        let _ = name;
        let _ = data;
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Err(E_NOTIMPL)
    }
    fn get_parent(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self::Api>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut parent: Raw = null_mut();
        #[expect(unsafe_code, reason = "IDXGIObject::GetParent, called out")]
        // SAFETY: a live object whose table holds `IDXGIObject::GetParent` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().object_get_parent)(
                self.object.raw(),
                riid_bytes.0.as_ptr(),
                &raw mut parent,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "IDXGIObject::GetParent: ppParent, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppParent` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(parent) = (unsafe { unknown_object_of::<A>(riid, parent) }) else {
            return Err(E_FAIL);
        };
        Ok(parent)
    }
}

impl<A: System> directx_graphics_infrastructure::SwapChainMedia for SystemSwapChainMedia<A> {
    fn get_frame_statistics_media(
        &self,
    ) -> Result<
        directx_graphics_infrastructure::structures::FrameStatisticsMedia,
        component_object_model::structures::HResult,
    > {
        let mut stats = Aligned::<
            { size_of::<WindowsWin32GraphicsDxgiDxgiFrameStatisticsMediaLayout>() },
        >::default();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChainMedia::GetFrameStatisticsMedia, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChainMedia::GetFrameStatisticsMedia` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain_media_get_frame_statistics_media)(
                self.object.raw(),
                stats.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let stats = decode_windows_win32_graphics_dxgi_dxgi_frame_statistics_media(&stats.0);
        Ok(stats)
    }
    fn set_present_duration(
        &self,
        duration: u32,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChainMedia::SetPresentDuration, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChainMedia::SetPresentDuration` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain_media_set_present_duration)(self.object.raw(), duration)
        };
        succeeded(returned)?;
        Ok(())
    }
    fn check_present_duration_support(
        &self,
        desired_present_duration: u32,
    ) -> Result<(u32, u32), component_object_model::structures::HResult> {
        let mut closest_smaller_present_duration = Aligned::<4>::default();
        let mut closest_larger_present_duration = Aligned::<4>::default();
        #[expect(
            unsafe_code,
            reason = "IDXGISwapChainMedia::CheckPresentDurationSupport, called out"
        )]
        // SAFETY: a live object whose table holds `IDXGISwapChainMedia::CheckPresentDurationSupport` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .swap_chain_media_check_present_duration_support)(
                self.object.raw(),
                desired_present_duration,
                closest_smaller_present_duration.0.as_mut_ptr(),
                closest_larger_present_duration.0.as_mut_ptr(),
            )
        };
        succeeded(returned)?;
        let closest_smaller_present_duration =
            get_u32(&closest_smaller_present_duration.0, usize::MIN);
        let closest_larger_present_duration =
            get_u32(&closest_larger_present_duration.0, usize::MIN);
        Ok((
            closest_smaller_present_duration,
            closest_larger_present_duration,
        ))
    }
}

impl<A: System> directx_graphics_infrastructure::GraphicsAnalysis for SystemGraphicsAnalysis<A> {
    fn begin_capture(&self) {
        #[expect(unsafe_code, reason = "IDXGraphicsAnalysis::BeginCapture, called out")]
        // SAFETY: a live object whose table holds `IDXGraphicsAnalysis::BeginCapture` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().graphics_analysis_begin_capture)(self.object.raw())
        };
        let () = returned;
    }
    fn end_capture(&self) {
        #[expect(unsafe_code, reason = "IDXGraphicsAnalysis::EndCapture, called out")]
        // SAFETY: a live object whose table holds `IDXGraphicsAnalysis::EndCapture` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().graphics_analysis_end_capture)(self.object.raw())
        };
        let () = returned;
    }
}

/// The C side's library, as an implementation of the API's top-level trait and of those it extends.
pub struct SystemDirectXGraphicsInfrastructure {
    library: Library,
}

impl SystemDirectXGraphicsInfrastructure {
    /// Returns the API over the C side's library.
    ///
    /// # Safety
    /// Each entry is the library's own export of that name, which takes the parameters its metadata gives.
    #[expect(
        unsafe_code,
        reason = "the promise that each entry is the library's export"
    )]
    pub unsafe fn new(library: Library) -> SystemDirectXGraphicsInfrastructure {
        SystemDirectXGraphicsInfrastructure { library }
    }
}

impl directx_graphics_infrastructure::DirectXGraphicsInfrastructure
    for SystemDirectXGraphicsInfrastructure
{
    type Adapter = SystemAdapter<Self>;
    type Debug = SystemDebug<Self>;
    type DecodeSwapChain = SystemDecodeSwapChain<Self>;
    type Device = SystemDevice<Self>;
    type DisplayControl = SystemDisplayControl<Self>;
    type Factory = SystemFactory<Self>;
    type FactoryMedia = SystemFactoryMedia<Self>;
    type InfoQueue = SystemInfoQueue<Self>;
    type KeyedMutex = SystemKeyedMutex<Self>;
    type Output = SystemOutput<Self>;
    type OutputDuplication = SystemOutputDuplication<Self>;
    type Resource = SystemResource<Self>;
    type Surface = SystemSurface<Self>;
    type SwapChain = SystemSwapChain<Self>;
    type SwapChainMedia = SystemSwapChainMedia<Self>;
    type GraphicsAnalysis = SystemGraphicsAnalysis<Self>;
    type Buffer = SystemSurface<Self>;

    fn create_dxgi_factory(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut factory: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory, the library's export, called out"
        )]
        // SAFETY: the library's export `CreateDXGIFactory`, as `new`'s caller promised; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.library.create_dxgi_factory)(riid_bytes.0.as_ptr(), &raw mut factory) };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory: ppFactory, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppFactory` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(factory) = (unsafe { unknown_object_of::<Self>(riid, factory) }) else {
            return Err(E_FAIL);
        };
        Ok(factory)
    }
    fn create_dxgi_factory1(
        &self,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut factory: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory1, the library's export, called out"
        )]
        // SAFETY: the library's export `CreateDXGIFactory1`, as `new`'s caller promised; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.library.create_dxgi_factory1)(riid_bytes.0.as_ptr(), &raw mut factory) };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory1: ppFactory, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppFactory` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(factory) = (unsafe { unknown_object_of::<Self>(riid, factory) }) else {
            return Err(E_FAIL);
        };
        Ok(factory)
    }
    fn create_dxgi_factory2(
        &self,
        flags: directx_graphics_infrastructure::enumerations::CreateFactoryFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        // DXGI_CREATE_FACTORY_FLAGS's judgment (src/api.rs, checked by src/kinds.rs).
        if flags.0 & 0x1_u32 != 0 {
            return Err(E_INVALIDARG);
        }
        if flags.0 & !0x1_u32 != 0 {
            return Err(E_INVALIDARG);
        }
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut factory: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory2, the library's export, called out"
        )]
        // SAFETY: the library's export `CreateDXGIFactory2`, as `new`'s caller promised; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.library.create_dxgi_factory2)(flags.0, riid_bytes.0.as_ptr(), &raw mut factory)
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "CreateDXGIFactory2: ppFactory, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppFactory` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(factory) = (unsafe { unknown_object_of::<Self>(riid, factory) }) else {
            return Err(E_FAIL);
        };
        Ok(factory)
    }
    fn dxgi_declare_adapter_removal_support(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "DXGIDeclareAdapterRemovalSupport, the library's export, called out"
        )]
        // SAFETY: the library's export `DXGIDeclareAdapterRemovalSupport`, as `new`'s caller promised; every pointer passed is ours for the call.
        let returned = unsafe { (self.library.dxgi_declare_adapter_removal_support)() };
        succeeded(returned)?;
        Ok(())
    }
    fn dxgi_disable_v_blank_virtualization(
        &self,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(
            unsafe_code,
            reason = "DXGIDisableVBlankVirtualization, the library's export, called out"
        )]
        // SAFETY: the library's export `DXGIDisableVBlankVirtualization`, as `new`'s caller promised; every pointer passed is ours for the call.
        let returned = unsafe { (self.library.dxgi_disable_v_blank_virtualization)() };
        succeeded(returned)?;
        Ok(())
    }
    fn dxgi_get_debug_interface1(
        &self,
        flags: u32,
        riid: &component_object_model::structures::Guid,
    ) -> Result<
        directx_graphics_infrastructure::UnknownObject<Self>,
        component_object_model::structures::HResult,
    > {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut debug: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "DXGIGetDebugInterface1, the library's export, called out"
        )]
        // SAFETY: the library's export `DXGIGetDebugInterface1`, as `new`'s caller promised; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.library.dxgi_get_debug_interface1)(flags, riid_bytes.0.as_ptr(), &raw mut debug)
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "DXGIGetDebugInterface1: pDebug, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `pDebug` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(debug) = (unsafe { unknown_object_of::<Self>(riid, debug) }) else {
            return Err(E_FAIL);
        };
        Ok(debug)
    }
}

impl System for SystemDirectXGraphicsInfrastructure {}
