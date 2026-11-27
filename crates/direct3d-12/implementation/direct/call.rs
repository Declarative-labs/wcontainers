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
use wcontainers_direct3d_12 as direct3d_12;
use wcontainers_direct3d_common as direct3d_common;
use wcontainers_direct3d_common_direct as direct3d_common_direct;

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

/// An object of the C side's of the kind `CommandAllocator`, held by one counted reference.
pub struct SystemCommandAllocator<A> {
    object: ForeignObject<CommandAllocatorTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemCommandAllocator<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12CommandAllocator`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12CommandAllocator` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemCommandAllocator<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12CommandAllocator"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<CommandAllocatorTable>::from_owned(raw) }?;
            Some(SystemCommandAllocator {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12CommandAllocator`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemCommandAllocator<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR` names the interface `CommandAllocatorTable` lays out.
            let object = unsafe {
                unknown.query::<CommandAllocatorTable>(
                    &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                )
            }?;
            Some(SystemCommandAllocator {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemCommandAllocator<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemCommandAllocator<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12CommandAllocator, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemCommandAllocator::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemCommandAllocator<A> {
    fn clone(&self) -> SystemCommandAllocator<A> {
        SystemCommandAllocator {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemCommandAllocator<A> {
    fn eq(&self, other: &SystemCommandAllocator<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemCommandAllocator<A> {}

impl<A> Hash for SystemCommandAllocator<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `CommandQueue`, held by one counted reference.
pub struct SystemCommandQueue<A> {
    object: ForeignObject<CommandQueueTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemCommandQueue<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12CommandQueue`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12CommandQueue` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemCommandQueue<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12CommandQueue"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<CommandQueueTable>::from_owned(raw) }?;
            Some(SystemCommandQueue {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12CommandQueue`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemCommandQueue<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE` names the interface `CommandQueueTable` lays out.
            let object = unsafe {
                unknown
                    .query::<CommandQueueTable>(&direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE)
            }?;
            Some(SystemCommandQueue {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemCommandQueue<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemCommandQueue<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12CommandQueue, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemCommandQueue::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemCommandQueue<A> {
    fn clone(&self) -> SystemCommandQueue<A> {
        SystemCommandQueue {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemCommandQueue<A> {
    fn eq(&self, other: &SystemCommandQueue<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemCommandQueue<A> {}

impl<A> Hash for SystemCommandQueue<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `DescriptorHeap`, held by one counted reference.
pub struct SystemDescriptorHeap<A> {
    object: ForeignObject<DescriptorHeapTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemDescriptorHeap<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12DescriptorHeap`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12DescriptorHeap` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemDescriptorHeap<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12DescriptorHeap"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<DescriptorHeapTable>::from_owned(raw) }?;
            Some(SystemDescriptorHeap {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12DescriptorHeap`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemDescriptorHeap<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP` names the interface `DescriptorHeapTable` lays out.
            let object = unsafe {
                unknown.query::<DescriptorHeapTable>(
                    &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                )
            }?;
            Some(SystemDescriptorHeap {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemDescriptorHeap<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemDescriptorHeap<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12DescriptorHeap, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemDescriptorHeap::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemDescriptorHeap<A> {
    fn clone(&self) -> SystemDescriptorHeap<A> {
        SystemDescriptorHeap {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemDescriptorHeap<A> {
    fn eq(&self, other: &SystemDescriptorHeap<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemDescriptorHeap<A> {}

impl<A> Hash for SystemDescriptorHeap<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Device`, held by one counted reference.
pub struct SystemDevice<A> {
    object: ForeignObject<DeviceTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemDevice<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12Device`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12Device` whose reference the caller gives up, and which may be called from any
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
                reason = "the promise of the caller: a live object of ID3D12Device"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<DeviceTable>::from_owned(raw) }?;
            Some(SystemDevice {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12Device`.
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_DEVICE` names the interface `DeviceTable` lays out.
            let object = unsafe {
                unknown.query::<DeviceTable>(&direct3d_12::constants::IID_ID3D12_DEVICE)
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
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_DEVICE;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemDevice<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12Device, the newest interface of the kind"
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

/// An object of the C side's of the kind `Fence`, held by one counted reference.
pub struct SystemFence<A> {
    object: ForeignObject<FenceTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemFence<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12Fence`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12Fence` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemFence<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12Fence"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<FenceTable>::from_owned(raw) }?;
            Some(SystemFence {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12Fence`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemFence<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_FENCE` names the interface `FenceTable` lays out.
            let object =
                unsafe { unknown.query::<FenceTable>(&direct3d_12::constants::IID_ID3D12_FENCE) }?;
            Some(SystemFence {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemFence<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_FENCE;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemFence<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12Fence, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemFence::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemFence<A> {
    fn clone(&self) -> SystemFence<A> {
        SystemFence {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemFence<A> {
    fn eq(&self, other: &SystemFence<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemFence<A> {}

impl<A> Hash for SystemFence<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `GraphicsCommandList`, held by one counted reference.
pub struct SystemGraphicsCommandList<A> {
    object: ForeignObject<GraphicsCommandListTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemGraphicsCommandList<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12GraphicsCommandList`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12GraphicsCommandList` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemGraphicsCommandList<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12GraphicsCommandList"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<GraphicsCommandListTable>::from_owned(raw) }?;
            Some(SystemGraphicsCommandList {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12GraphicsCommandList`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemGraphicsCommandList<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST` names the interface `GraphicsCommandListTable` lays out.
            let object = unsafe {
                unknown.query::<GraphicsCommandListTable>(
                    &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                )
            }?;
            Some(SystemGraphicsCommandList {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemGraphicsCommandList<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemGraphicsCommandList<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12GraphicsCommandList, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemGraphicsCommandList::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemGraphicsCommandList<A> {
    fn clone(&self) -> SystemGraphicsCommandList<A> {
        SystemGraphicsCommandList {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemGraphicsCommandList<A> {
    fn eq(&self, other: &SystemGraphicsCommandList<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemGraphicsCommandList<A> {}

impl<A> Hash for SystemGraphicsCommandList<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `PipelineState`, held by one counted reference.
pub struct SystemPipelineState<A> {
    object: ForeignObject<PipelineStateTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemPipelineState<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12PipelineState`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12PipelineState` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemPipelineState<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12PipelineState"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<PipelineStateTable>::from_owned(raw) }?;
            Some(SystemPipelineState {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12PipelineState`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemPipelineState<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_PIPELINE_STATE` names the interface `PipelineStateTable` lays out.
            let object = unsafe {
                unknown
                    .query::<PipelineStateTable>(&direct3d_12::constants::IID_ID3D12_PIPELINE_STATE)
            }?;
            Some(SystemPipelineState {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemPipelineState<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_PIPELINE_STATE;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemPipelineState<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12PipelineState, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemPipelineState::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemPipelineState<A> {
    fn clone(&self) -> SystemPipelineState<A> {
        SystemPipelineState {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemPipelineState<A> {
    fn eq(&self, other: &SystemPipelineState<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemPipelineState<A> {}

impl<A> Hash for SystemPipelineState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `QueryHeap`, held by one counted reference.
pub struct SystemQueryHeap<A> {
    object: ForeignObject<QueryHeapTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemQueryHeap<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12QueryHeap`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12QueryHeap` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemQueryHeap<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12QueryHeap"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<QueryHeapTable>::from_owned(raw) }?;
            Some(SystemQueryHeap {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12QueryHeap`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemQueryHeap<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_QUERY_HEAP` names the interface `QueryHeapTable` lays out.
            let object = unsafe {
                unknown.query::<QueryHeapTable>(&direct3d_12::constants::IID_ID3D12_QUERY_HEAP)
            }?;
            Some(SystemQueryHeap {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemQueryHeap<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_QUERY_HEAP;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemQueryHeap<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12QueryHeap, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemQueryHeap::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemQueryHeap<A> {
    fn clone(&self) -> SystemQueryHeap<A> {
        SystemQueryHeap {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemQueryHeap<A> {
    fn eq(&self, other: &SystemQueryHeap<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemQueryHeap<A> {}

impl<A> Hash for SystemQueryHeap<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.object.hash(state);
    }
}

/// An object of the C side's of the kind `Resource`, held by one counted reference.
pub struct SystemResource<A> {
    object: ForeignObject<ResourceTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemResource<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12Resource`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12Resource` whose reference the caller gives up, and which may be called from any
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
                reason = "the promise of the caller: a live object of ID3D12Resource"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<ResourceTable>::from_owned(raw) }?;
            Some(SystemResource {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12Resource`.
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_RESOURCE` names the interface `ResourceTable` lays out.
            let object = unsafe {
                unknown.query::<ResourceTable>(&direct3d_12::constants::IID_ID3D12_RESOURCE)
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
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_RESOURCE;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemResource<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12Resource, the newest interface of the kind"
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

/// An object of the C side's of the kind `RootSignature`, held by one counted reference.
pub struct SystemRootSignature<A> {
    object: ForeignObject<RootSignatureTable>,
    api: PhantomData<fn() -> A>,
}

impl<A> SystemRootSignature<A> {
    /// The object's pointer, live while this lives.
    pub fn raw(&self) -> Raw {
        self.object.raw()
    }

    /// Takes over a reference to an object of `ID3D12RootSignature`, or returns none for null.
    ///
    /// # Safety
    /// `raw` is null or a live object of `ID3D12RootSignature` whose reference the caller gives up, and which may be called from any
    /// thread as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    pub unsafe fn from_owned(raw: Raw) -> Option<SystemRootSignature<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12RootSignature"
            )]
            // SAFETY: the caller's promise.
            let object = unsafe { ForeignObject::<RootSignatureTable>::from_owned(raw) }?;
            Some(SystemRootSignature {
                object,
                api: PhantomData,
            })
        };
        body()
    }

    /// Takes over a reference to an object of an older interface of the kind, asking it for `ID3D12RootSignature`.
    ///
    /// # Safety
    /// `raw` is null or a live COM object whose reference the caller gives up, and which may be called from any thread
    /// as `Interface` states (a context only under the system's multithread protection, never a deferred one).
    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live COM object"
    )]
    pub unsafe fn from_owned_older(raw: Raw) -> Option<SystemRootSignature<A>> {
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
            // SAFETY: `direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE` names the interface `RootSignatureTable` lays out.
            let object = unsafe {
                unknown
                    .query::<RootSignatureTable>(&direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE)
            }?;
            Some(SystemRootSignature {
                object,
                api: PhantomData,
            })
        };
        body()
    }
}

impl<A> Adopt for SystemRootSignature<A> {
    const INTERFACE: Guid = direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE;

    #[expect(
        unsafe_code,
        reason = "the promise that a pointer is a live object of an interface"
    )]
    unsafe fn adopt(raw: Raw) -> Option<SystemRootSignature<A>> {
        #[deny(unsafe_code)]
        let body = move || {
            #[expect(
                unsafe_code,
                reason = "the promise of the caller: a live object of ID3D12RootSignature, the newest interface of the kind"
            )]
            // SAFETY: the caller's promise, for `INTERFACE`'s interface, the newest of the kind.
            unsafe {
                SystemRootSignature::from_owned(raw)
            }
        };
        body()
    }
}

impl<A> Clone for SystemRootSignature<A> {
    fn clone(&self) -> SystemRootSignature<A> {
        SystemRootSignature {
            object: self.object.clone(),
            api: PhantomData,
        }
    }
}

impl<A> PartialEq for SystemRootSignature<A> {
    fn eq(&self, other: &SystemRootSignature<A>) -> bool {
        self.object == other.object
    }
}

impl<A> Eq for SystemRootSignature<A> {}

impl<A> Hash for SystemRootSignature<A> {
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
pub(crate) unsafe fn unknown_object_of<A: System>(
    riid: &Guid,
    raw: Raw,
) -> Option<direct3d_12::UnknownObject<A>> {
    #[deny(unsafe_code)]
    let body = move || {
        #[expect(
            unsafe_code,
            reason = "the promise of the caller: a live COM object, which is an IUnknown"
        )]
        // SAFETY: the caller's promise: any COM object is an IUnknown; the reference it gave up is released when this drops.
        let unknown = unsafe { ForeignObject::<UnknownTable>::from_owned(raw) }?;
        if [
            direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR` names the interface `CommandAllocatorTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<CommandAllocatorTable>(
                    &direct3d_12::constants::IID_ID3D12_COMMAND_ALLOCATOR,
                )
            } {
                let object = SystemCommandAllocator {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::CommandAllocator(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE` names the interface `CommandQueueTable` lays out.
            if let Some(object) = unsafe {
                unknown
                    .query::<CommandQueueTable>(&direct3d_12::constants::IID_ID3D12_COMMAND_QUEUE)
            } {
                let object = SystemCommandQueue {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::CommandQueue(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP` names the interface `DescriptorHeapTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<DescriptorHeapTable>(
                    &direct3d_12::constants::IID_ID3D12_DESCRIPTOR_HEAP,
                )
            } {
                let object = SystemDescriptorHeap {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::DescriptorHeap(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_DEVICE,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_DEVICE` names the interface `DeviceTable` lays out.
            if let Some(object) =
                unsafe { unknown.query::<DeviceTable>(&direct3d_12::constants::IID_ID3D12_DEVICE) }
            {
                let object = SystemDevice {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::Device(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_FENCE,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_FENCE` names the interface `FenceTable` lays out.
            if let Some(object) =
                unsafe { unknown.query::<FenceTable>(&direct3d_12::constants::IID_ID3D12_FENCE) }
            {
                let object = SystemFence {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::Fence(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_COMMAND_LIST,
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
            direct3d_12::constants::IID_ID3D12_OBJECT,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST` names the interface `GraphicsCommandListTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<GraphicsCommandListTable>(
                    &direct3d_12::constants::IID_ID3D12_GRAPHICS_COMMAND_LIST,
                )
            } {
                let object = SystemGraphicsCommandList {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::GraphicsCommandList(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_PIPELINE_STATE,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_PIPELINE_STATE` names the interface `PipelineStateTable` lays out.
            if let Some(object) = unsafe {
                unknown
                    .query::<PipelineStateTable>(&direct3d_12::constants::IID_ID3D12_PIPELINE_STATE)
            } {
                let object = SystemPipelineState {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::PipelineState(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_QUERY_HEAP,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_QUERY_HEAP` names the interface `QueryHeapTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<QueryHeapTable>(&direct3d_12::constants::IID_ID3D12_QUERY_HEAP)
            } {
                let object = SystemQueryHeap {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::QueryHeap(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_PAGEABLE,
            direct3d_12::constants::IID_ID3D12_RESOURCE,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_RESOURCE` names the interface `ResourceTable` lays out.
            if let Some(object) = unsafe {
                unknown.query::<ResourceTable>(&direct3d_12::constants::IID_ID3D12_RESOURCE)
            } {
                let object = SystemResource {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::Resource(object));
            }
        }
        if [
            direct3d_12::constants::IID_ID3D12_DEVICE_CHILD,
            direct3d_12::constants::IID_ID3D12_OBJECT,
            direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE,
        ]
        .contains(riid)
        {
            #[expect(
                unsafe_code,
                reason = "QueryInterface for the newest interface of the kind"
            )]
            // SAFETY: `direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE` names the interface `RootSignatureTable` lays out.
            if let Some(object) = unsafe {
                unknown
                    .query::<RootSignatureTable>(&direct3d_12::constants::IID_ID3D12_ROOT_SIGNATURE)
            } {
                let object = SystemRootSignature {
                    object,
                    api: PhantomData,
                };
                return Some(direct3d_12::UnknownObject::RootSignature(object));
            }
        }
        #[expect(
            unsafe_code,
            reason = "the promise of the caller: a live COM object, which is an IUnknown"
        )]
        // SAFETY: the caller's promise; a reference of our own is taken.
        let foreign = unsafe { ForeignUnknown::from_borrowed(unknown.raw()) }?;
        Some(direct3d_12::UnknownObject::Unknown(
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
    direct3d_12::Direct3D12<
        CommandAllocator = SystemCommandAllocator<Self>,
        CommandQueue = SystemCommandQueue<Self>,
        DescriptorHeap = SystemDescriptorHeap<Self>,
        Device = SystemDevice<Self>,
        Fence = SystemFence<Self>,
        GraphicsCommandList = SystemGraphicsCommandList<Self>,
        PipelineState = SystemPipelineState<Self>,
        QueryHeap = SystemQueryHeap<Self>,
        Resource = SystemResource<Self>,
        RootSignature = SystemRootSignature<Self>,
    > + direct3d_common_direct::System
{
}

impl<A: System> direct3d_12::CommandAllocator for SystemCommandAllocator<A> {
    fn reset(&self) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "ID3D12CommandAllocator::Reset, called out")]
        // SAFETY: a live object whose table holds `ID3D12CommandAllocator::Reset` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().command_allocator_reset)(self.object.raw()) };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> direct3d_12::Pageable for SystemCommandAllocator<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemCommandAllocator<A> {}

impl<A: System> direct3d_12::Object for SystemCommandAllocator<A> {
    type Api = A;
}

impl<A: System> direct3d_12::CommandQueue for SystemCommandQueue<A> {
    fn execute_command_lists(
        &self,
        command_lists: &[Option<direct3d_12::CommandListRef<'_, Self::Api>>],
    ) {
        let _ = command_lists;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn signal(
        &self,
        fence: &<Self::Api as direct3d_12::Direct3D12>::Fence,
        value: u64,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "ID3D12CommandQueue::Signal, called out")]
        // SAFETY: a live object whose table holds `ID3D12CommandQueue::Signal` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().command_queue_signal)(
                self.object.raw(),
                {
                    let held = fence;
                    held.raw()
                },
                value,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> direct3d_12::Pageable for SystemCommandQueue<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemCommandQueue<A> {}

impl<A: System> direct3d_12::Object for SystemCommandQueue<A> {
    type Api = A;
}

impl<A: System> direct3d_12::DescriptorHeap for SystemDescriptorHeap<A> {
    fn get_desc(&self) -> direct3d_12::structures::DescriptorHeapDesc {
        // Refused by its form (the report's NOT GLUED): the system is not called.
        decode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
            &[u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout>()],
        )
    }
    fn get_cpu_descriptor_handle_for_heap_start(
        &self,
    ) -> direct3d_12::structures::CpuDescriptorHandle {
        // Refused by its form (the report's NOT GLUED): the system is not called.
        decode_windows_win32_graphics_direct_3d12_d_3d12_cpu_descriptor_handle(
            &[u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12CpuDescriptorHandleLayout>()],
        )
    }
    fn get_gpu_descriptor_handle_for_heap_start(
        &self,
    ) -> direct3d_12::structures::GpuDescriptorHandle {
        // Refused by its form (the report's NOT GLUED): the system is not called.
        decode_windows_win32_graphics_direct_3d12_d_3d12_gpu_descriptor_handle(
            &[u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12GpuDescriptorHandleLayout>()],
        )
    }
}

impl<A: System> direct3d_12::Pageable for SystemDescriptorHeap<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemDescriptorHeap<A> {}

impl<A: System> direct3d_12::Object for SystemDescriptorHeap<A> {
    type Api = A;
}

impl<A: System> direct3d_12::Device for SystemDevice<A> {
    fn create_command_queue(
        &self,
        desc: &direct3d_12::structures::CommandQueueDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let mut desc_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDirect3d12D3d12CommandQueueDescLayout>() },
        >::default();
        encode_windows_win32_graphics_direct_3d12_d_3d12_command_queue_desc(
            desc,
            &mut desc_bytes.0,
        );
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut command_queue: Raw = null_mut();
        #[expect(unsafe_code, reason = "ID3D12Device::CreateCommandQueue, called out")]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateCommandQueue` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_command_queue)(
                self.object.raw(),
                desc_bytes.0.as_ptr(),
                riid_bytes.0.as_ptr(),
                &raw mut command_queue,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommandQueue: ppCommandQueue, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppCommandQueue` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(command_queue) = (unsafe { unknown_object_of::<A>(riid, command_queue) }) else {
            return Err(E_FAIL);
        };
        Ok(command_queue)
    }
    fn create_command_allocator(
        &self,
        type_: direct3d_12::enumerations::CommandListType,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut command_allocator: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommandAllocator, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateCommandAllocator` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_command_allocator)(
                self.object.raw(),
                type_.0,
                riid_bytes.0.as_ptr(),
                &raw mut command_allocator,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommandAllocator: ppCommandAllocator, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppCommandAllocator` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(command_allocator) = (unsafe { unknown_object_of::<A>(riid, command_allocator) })
        else {
            return Err(E_FAIL);
        };
        Ok(command_allocator)
    }
    fn create_graphics_pipeline_state(
        &self,
        desc: &direct3d_12::structures::GraphicsPipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = desc;
        let _ = riid;
        // Refused by its kinds (src/kinds.rs), the system never called: pDesc: D3D12_GRAPHICS_PIPELINE_STATE_DESC.VS: D3D12_SHADER_BYTECODE.pShaderBytecode: a raw pointer with no size, which the system reads or writes as it likes; pDesc: D3D12_GRAPHICS_PIPELINE_STATE_DESC.VS: D3D12_SHADER_BYTECODE.BytecodeLength: an integer of pointer width, which the system may take as an address; pDesc: D3D12_GRAPHICS_PIPELINE_STATE_DESC.CachedPSO: D3D12_CACHED_PIPELINE_STATE.pCachedBlob: a raw pointer with no size, which the system reads or writes as it likes; pDesc: D3D12_GRAPHICS_PIPELINE_STATE_DESC.CachedPSO: D3D12_CACHED_PIPELINE_STATE.CachedBlobSizeInBytes: an integer of pointer width, which the system may take as an address.
        Err(E_NOTIMPL)
    }
    fn create_compute_pipeline_state(
        &self,
        desc: &direct3d_12::structures::ComputePipelineStateDesc<Self::Api>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let _ = desc;
        let _ = riid;
        // Refused by its kinds (src/kinds.rs), the system never called: pDesc: D3D12_COMPUTE_PIPELINE_STATE_DESC.CS: D3D12_SHADER_BYTECODE.pShaderBytecode: a raw pointer with no size, which the system reads or writes as it likes; pDesc: D3D12_COMPUTE_PIPELINE_STATE_DESC.CS: D3D12_SHADER_BYTECODE.BytecodeLength: an integer of pointer width, which the system may take as an address; pDesc: D3D12_COMPUTE_PIPELINE_STATE_DESC.CachedPSO: D3D12_CACHED_PIPELINE_STATE.pCachedBlob: a raw pointer with no size, which the system reads or writes as it likes; pDesc: D3D12_COMPUTE_PIPELINE_STATE_DESC.CachedPSO: D3D12_CACHED_PIPELINE_STATE.CachedBlobSizeInBytes: an integer of pointer width, which the system may take as an address.
        Err(E_NOTIMPL)
    }
    fn create_command_list(
        &self,
        node_mask: u32,
        type_: direct3d_12::enumerations::CommandListType,
        command_allocator: &<Self::Api as direct3d_12::Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as direct3d_12::Direct3D12>::PipelineState>,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut command_list: Raw = null_mut();
        #[expect(unsafe_code, reason = "ID3D12Device::CreateCommandList, called out")]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateCommandList` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_command_list)(
                self.object.raw(),
                node_mask,
                type_.0,
                {
                    let held = command_allocator;
                    held.raw()
                },
                initial_state.map_or(null_mut(), |held| held.raw()),
                riid_bytes.0.as_ptr(),
                &raw mut command_list,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommandList: ppCommandList, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppCommandList` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(command_list) = (unsafe { unknown_object_of::<A>(riid, command_list) }) else {
            return Err(E_FAIL);
        };
        Ok(command_list)
    }
    fn create_descriptor_heap(
        &self,
        descriptor_heap_desc: &direct3d_12::structures::DescriptorHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let mut descriptor_heap_desc_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDirect3d12D3d12DescriptorHeapDescLayout>() },
        >::default();
        encode_windows_win32_graphics_direct_3d12_d_3d12_descriptor_heap_desc(
            descriptor_heap_desc,
            &mut descriptor_heap_desc_bytes.0,
        );
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut ppv_heap: Raw = null_mut();
        #[expect(unsafe_code, reason = "ID3D12Device::CreateDescriptorHeap, called out")]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateDescriptorHeap` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_descriptor_heap)(
                self.object.raw(),
                descriptor_heap_desc_bytes.0.as_ptr(),
                riid_bytes.0.as_ptr(),
                &raw mut ppv_heap,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateDescriptorHeap: ppvHeap, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppvHeap` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(ppv_heap) = (unsafe { unknown_object_of::<A>(riid, ppv_heap) }) else {
            return Err(E_FAIL);
        };
        Ok(ppv_heap)
    }
    fn get_descriptor_handle_increment_size(
        &self,
        descriptor_heap_type: direct3d_12::enumerations::DescriptorHeapType,
    ) -> u32 {
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::GetDescriptorHandleIncrementSize, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12Device::GetDescriptorHandleIncrementSize` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .device_get_descriptor_handle_increment_size)(
                self.object.raw(),
                descriptor_heap_type.0,
            )
        };
        returned
    }
    fn create_root_signature(
        &self,
        node_mask: u32,
        blob_with_root_signature: &[u8],
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let blob_with_root_signature_c: Vec<u8> = blob_with_root_signature.to_vec();
        let blob_length_in_bytes: usize = blob_with_root_signature.len();
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut ppv_root_signature: Raw = null_mut();
        #[expect(unsafe_code, reason = "ID3D12Device::CreateRootSignature, called out")]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateRootSignature` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_root_signature)(
                self.object.raw(),
                node_mask,
                blob_with_root_signature_c.as_ptr().cast::<u8>(),
                blob_length_in_bytes,
                riid_bytes.0.as_ptr(),
                &raw mut ppv_root_signature,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateRootSignature: ppvRootSignature, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppvRootSignature` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(ppv_root_signature) =
            (unsafe { unknown_object_of::<A>(riid, ppv_root_signature) })
        else {
            return Err(E_FAIL);
        };
        Ok(ppv_root_signature)
    }
    fn create_shader_resource_view(
        &self,
        resource: Option<&<Self::Api as direct3d_12::Direct3D12>::Resource>,
        desc: Option<&direct3d_12::structures::ShaderResourceViewDesc>,
        dest_descriptor: direct3d_12::structures::CpuDescriptorHandle,
    ) {
        let _ = resource;
        let _ = desc;
        let _ = dest_descriptor;
        // Refused by its kinds (src/kinds.rs), the system never called: DestDescriptor: D3D12_CPU_DESCRIPTOR_HANDLE.ptr: an integer of pointer width, which the system may take as an address.
    }
    fn create_render_target_view(
        &self,
        resource: Option<&<Self::Api as direct3d_12::Direct3D12>::Resource>,
        desc: Option<&direct3d_12::structures::RenderTargetViewDesc>,
        dest_descriptor: direct3d_12::structures::CpuDescriptorHandle,
    ) {
        let _ = resource;
        let _ = desc;
        let _ = dest_descriptor;
        // Refused by its kinds (src/kinds.rs), the system never called: DestDescriptor: D3D12_CPU_DESCRIPTOR_HANDLE.ptr: an integer of pointer width, which the system may take as an address.
    }
    fn create_committed_resource(
        &self,
        heap_properties: &direct3d_12::structures::HeapProperties,
        heap_flags: direct3d_12::enumerations::HeapFlags,
        desc: &direct3d_12::structures::ResourceDesc,
        initial_resource_state: direct3d_12::enumerations::ResourceStates,
        optimized_clear_value: Option<&direct3d_12::structures::ClearValue>,
        riid_resource: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let mut heap_properties_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDirect3d12D3d12HeapPropertiesLayout>() },
        >::default();
        encode_windows_win32_graphics_direct_3d12_d_3d12_heap_properties(
            heap_properties,
            &mut heap_properties_bytes.0,
        );
        let mut desc_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout>() },
        >::default();
        encode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(desc, &mut desc_bytes.0);
        let optimized_clear_value_bytes = optimized_clear_value.map(|value| {
            let mut bytes = Aligned::<
                { size_of::<WindowsWin32GraphicsDirect3d12D3d12ClearValueLayout>() },
            >::default();
            encode_windows_win32_graphics_direct_3d12_d_3d12_clear_value(value, &mut bytes.0);
            bytes
        });
        let mut riid_resource_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid_resource, &mut riid_resource_bytes.0);
        let mut ppv_resource: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommittedResource, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateCommittedResource` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_committed_resource)(
                self.object.raw(),
                heap_properties_bytes.0.as_ptr(),
                heap_flags.0,
                desc_bytes.0.as_ptr(),
                initial_resource_state.0,
                optimized_clear_value_bytes
                    .as_ref()
                    .map_or(null(), |bytes| bytes.0.as_ptr()),
                riid_resource_bytes.0.as_ptr(),
                &raw mut ppv_resource,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateCommittedResource: ppvResource, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppvResource` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid_resource` names.
        let Some(ppv_resource) = (unsafe { unknown_object_of::<A>(riid_resource, ppv_resource) })
        else {
            return Err(E_FAIL);
        };
        Ok(ppv_resource)
    }
    fn create_fence(
        &self,
        initial_value: u64,
        flags: direct3d_12::enumerations::FenceFlags,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut fence: Raw = null_mut();
        #[expect(unsafe_code, reason = "ID3D12Device::CreateFence, called out")]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateFence` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_fence)(
                self.object.raw(),
                initial_value,
                flags.0,
                riid_bytes.0.as_ptr(),
                &raw mut fence,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateFence: ppFence, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppFence` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(fence) = (unsafe { unknown_object_of::<A>(riid, fence) }) else {
            return Err(E_FAIL);
        };
        Ok(fence)
    }
    fn get_copyable_footprints(
        &self,
        resource_desc: &direct3d_12::structures::ResourceDesc,
        first_subresource: u32,
        num_subresources: u32,
        base_offset: u64,
    ) -> (
        Vec<direct3d_12::structures::PlacedSubresourceFootprint>,
        Vec<u32>,
        Vec<u64>,
        u64,
    ) {
        let mut resource_desc_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout>() },
        >::default();
        encode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
            resource_desc,
            &mut resource_desc_bytes.0,
        );
        let Some(layouts_length) = usize::try_from(num_subresources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 32)
        else {
            return (Vec::new(), Vec::new(), Vec::new(), Default::default());
        };
        let mut layouts_words = vec![0_u64; layouts_length.div_ceil(8)];
        let Some(num_rows_length) = usize::try_from(num_subresources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 4)
        else {
            return (Vec::new(), Vec::new(), Vec::new(), Default::default());
        };
        let mut num_rows_words = vec![0_u64; num_rows_length.div_ceil(8)];
        let Some(row_size_in_bytes_length) = usize::try_from(num_subresources)
            .ok()
            .filter(|count| *count <= 1 << 16)
            .map(|count| count * 8)
        else {
            return (Vec::new(), Vec::new(), Vec::new(), Default::default());
        };
        let mut row_size_in_bytes_words = vec![0_u64; row_size_in_bytes_length.div_ceil(8)];
        let mut total_bytes = Aligned::<8>::default();
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::GetCopyableFootprints, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12Device::GetCopyableFootprints` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_get_copyable_footprints)(
                self.object.raw(),
                resource_desc_bytes.0.as_ptr(),
                first_subresource,
                num_subresources,
                base_offset,
                layouts_words.as_mut_ptr().cast::<u8>(),
                num_rows_words.as_mut_ptr().cast::<u8>(),
                row_size_in_bytes_words.as_mut_ptr().cast::<u8>(),
                total_bytes.0.as_mut_ptr(),
            )
        };
        let layouts_bytes: Vec<u8> = layouts_words
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        let layouts: Vec<_> = layouts_bytes
            .get(..layouts_length)
            .unwrap_or_default()
            .as_chunks::<32>()
            .0
            .iter()
            .map(|element| {
                decode_windows_win32_graphics_direct_3d12_d_3d12_placed_subresource_footprint(
                    element,
                )
            })
            .collect();
        let num_rows_bytes: Vec<u8> = num_rows_words
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        let num_rows: Vec<_> = num_rows_bytes
            .get(..num_rows_length)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|element| get_u32(element, usize::MIN))
            .collect();
        let row_size_in_bytes_bytes: Vec<u8> = row_size_in_bytes_words
            .iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        let row_size_in_bytes: Vec<_> = row_size_in_bytes_bytes
            .get(..row_size_in_bytes_length)
            .unwrap_or_default()
            .as_chunks::<8>()
            .0
            .iter()
            .map(|element| get_u64(element, usize::MIN))
            .collect();
        let total_bytes = get_u64(&total_bytes.0, usize::MIN);
        let () = returned;
        (layouts, num_rows, row_size_in_bytes, total_bytes)
    }
    fn create_query_heap(
        &self,
        desc: &direct3d_12::structures::QueryHeapDesc,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self::Api>, component_object_model::structures::HResult>
    {
        let mut desc_bytes = Aligned::<
            { size_of::<WindowsWin32GraphicsDirect3d12D3d12QueryHeapDescLayout>() },
        >::default();
        encode_windows_win32_graphics_direct_3d12_d_3d12_query_heap_desc(desc, &mut desc_bytes.0);
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut ppv_heap: Raw = null_mut();
        #[expect(unsafe_code, reason = "ID3D12Device::CreateQueryHeap, called out")]
        // SAFETY: a live object whose table holds `ID3D12Device::CreateQueryHeap` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().device_create_query_heap)(
                self.object.raw(),
                desc_bytes.0.as_ptr(),
                riid_bytes.0.as_ptr(),
                &raw mut ppv_heap,
            )
        };
        succeeded(returned)?;
        #[expect(
            unsafe_code,
            reason = "ID3D12Device::CreateQueryHeap: ppvHeap, Out and ComOutPtr"
        )]
        // SAFETY: the metadata's `ppvHeap` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(ppv_heap) = (unsafe { unknown_object_of::<A>(riid, ppv_heap) }) else {
            return Err(E_FAIL);
        };
        Ok(ppv_heap)
    }
}

impl<A: System> direct3d_12::Object for SystemDevice<A> {
    type Api = A;
}

impl<A: System> direct3d_12::Fence for SystemFence<A> {
    fn get_completed_value(&self) -> u64 {
        #[expect(unsafe_code, reason = "ID3D12Fence::GetCompletedValue, called out")]
        // SAFETY: a live object whose table holds `ID3D12Fence::GetCompletedValue` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().fence_get_completed_value)(self.object.raw()) };
        returned
    }
    fn set_event_on_completion(
        &self,
        value: u64,
        h_event: component_object_model::structures::Handle,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "ID3D12Fence::SetEventOnCompletion, called out")]
        // SAFETY: a live object whose table holds `ID3D12Fence::SetEventOnCompletion` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().fence_set_event_on_completion)(
                self.object.raw(),
                value,
                h_event.0,
            )
        };
        succeeded(returned)?;
        Ok(())
    }
}

impl<A: System> direct3d_12::Pageable for SystemFence<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemFence<A> {}

impl<A: System> direct3d_12::Object for SystemFence<A> {
    type Api = A;
}

impl<A: System> direct3d_12::GraphicsCommandList for SystemGraphicsCommandList<A> {
    fn close(&self) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "ID3D12GraphicsCommandList::Close, called out")]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::Close` where the metadata orders it; every pointer passed is ours for the call.
        let returned =
            unsafe { (self.object.entry_points().graphics_command_list_close)(self.object.raw()) };
        succeeded(returned)?;
        Ok(())
    }
    fn reset(
        &self,
        allocator: &<Self::Api as direct3d_12::Direct3D12>::CommandAllocator,
        initial_state: Option<&<Self::Api as direct3d_12::Direct3D12>::PipelineState>,
    ) -> Result<(), component_object_model::structures::HResult> {
        #[expect(unsafe_code, reason = "ID3D12GraphicsCommandList::Reset, called out")]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::Reset` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().graphics_command_list_reset)(
                self.object.raw(),
                {
                    let held = allocator;
                    held.raw()
                },
                initial_state.map_or(null_mut(), |held| held.raw()),
            )
        };
        succeeded(returned)?;
        Ok(())
    }
    fn draw_instanced(
        &self,
        vertex_count_per_instance: u32,
        instance_count: u32,
        start_vertex_location: u32,
        start_instance_location: u32,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::DrawInstanced, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::DrawInstanced` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_draw_instanced)(
                self.object.raw(),
                vertex_count_per_instance,
                instance_count,
                start_vertex_location,
                start_instance_location,
            )
        };
        let () = returned;
    }
    fn draw_indexed_instanced(
        &self,
        index_count_per_instance: u32,
        instance_count: u32,
        start_index_location: u32,
        base_vertex_location: i32,
        start_instance_location: u32,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::DrawIndexedInstanced, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::DrawIndexedInstanced` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_draw_indexed_instanced)(
                self.object.raw(),
                index_count_per_instance,
                instance_count,
                start_index_location,
                base_vertex_location,
                start_instance_location,
            )
        };
        let () = returned;
    }
    fn dispatch(
        &self,
        thread_group_count_x: u32,
        thread_group_count_y: u32,
        thread_group_count_z: u32,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::Dispatch, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::Dispatch` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().graphics_command_list_dispatch)(
                self.object.raw(),
                thread_group_count_x,
                thread_group_count_y,
                thread_group_count_z,
            )
        };
        let () = returned;
    }
    fn copy_buffer_region(
        &self,
        dst_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        dst_offset: u64,
        src_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        src_offset: u64,
        num_bytes: u64,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::CopyBufferRegion, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::CopyBufferRegion` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_copy_buffer_region)(
                self.object.raw(),
                {
                    let held = dst_buffer;
                    held.raw()
                },
                dst_offset,
                {
                    let held = src_buffer;
                    held.raw()
                },
                src_offset,
                num_bytes,
            )
        };
        let () = returned;
    }
    fn copy_texture_region(
        &self,
        dst: &direct3d_12::structures::TextureCopyLocation<Self::Api>,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src: &direct3d_12::structures::TextureCopyLocation<Self::Api>,
        src_box: Option<&direct3d_12::structures::Box>,
    ) {
        let _ = dst;
        let _ = dst_x;
        let _ = dst_y;
        let _ = dst_z;
        let _ = src;
        let _ = src_box;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn ia_set_primitive_topology(
        &self,
        primitive_topology: direct3d_common::enumerations::PrimitiveTopology,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::IASetPrimitiveTopology, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::IASetPrimitiveTopology` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_ia_set_primitive_topology)(
                self.object.raw(),
                primitive_topology.0,
            )
        };
        let () = returned;
    }
    fn set_pipeline_state(
        &self,
        pipeline_state: &<Self::Api as direct3d_12::Direct3D12>::PipelineState,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::SetPipelineState, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::SetPipelineState` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_set_pipeline_state)(self.object.raw(), {
                let held = pipeline_state;
                held.raw()
            })
        };
        let () = returned;
    }
    fn resource_barrier(&self, barriers: &[direct3d_12::structures::ResourceBarrier<Self::Api>]) {
        let _ = barriers;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn set_descriptor_heaps(
        &self,
        descriptor_heaps: &[Option<&<Self::Api as direct3d_12::Direct3D12>::DescriptorHeap>],
    ) {
        let Ok(num_descriptor_heaps) = u32::try_from(descriptor_heaps.len()) else {
            return;
        };
        let descriptor_heaps_raw: Vec<Raw> = descriptor_heaps
            .iter()
            .map(|held| held.map_or(null_mut(), |held| held.raw()))
            .collect();
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::SetDescriptorHeaps, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::SetDescriptorHeaps` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_set_descriptor_heaps)(
                self.object.raw(),
                num_descriptor_heaps,
                descriptor_heaps_raw.as_ptr(),
            )
        };
        let () = returned;
    }
    fn set_compute_root_signature(
        &self,
        root_signature: Option<&<Self::Api as direct3d_12::Direct3D12>::RootSignature>,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::SetComputeRootSignature, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::SetComputeRootSignature` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_set_compute_root_signature)(
                self.object.raw(),
                root_signature.map_or(null_mut(), |held| held.raw()),
            )
        };
        let () = returned;
    }
    fn set_graphics_root_signature(
        &self,
        root_signature: Option<&<Self::Api as direct3d_12::Direct3D12>::RootSignature>,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::SetGraphicsRootSignature, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::SetGraphicsRootSignature` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_set_graphics_root_signature)(
                self.object.raw(),
                root_signature.map_or(null_mut(), |held| held.raw()),
            )
        };
        let () = returned;
    }
    fn set_graphics_root_descriptor_table(
        &self,
        root_parameter_index: u32,
        base_descriptor: direct3d_12::structures::GpuDescriptorHandle,
    ) {
        let _ = root_parameter_index;
        let _ = base_descriptor;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn set_compute_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = root_parameter_index;
        let _ = buffer_location;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn set_graphics_root_constant_buffer_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = root_parameter_index;
        let _ = buffer_location;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn set_compute_root_unordered_access_view(
        &self,
        root_parameter_index: u32,
        buffer_location: u64,
    ) {
        let _ = root_parameter_index;
        let _ = buffer_location;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn ia_set_index_buffer(&self, view: Option<&direct3d_12::structures::IndexBufferView>) {
        let _ = view;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn ia_set_vertex_buffers(
        &self,
        start_slot: u32,
        views: &[direct3d_12::structures::VertexBufferView],
    ) {
        let _ = start_slot;
        let _ = views;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn so_set_targets(
        &self,
        start_slot: u32,
        views: &[direct3d_12::structures::StreamOutputBufferView],
    ) {
        let _ = start_slot;
        let _ = views;
        // Refused by its form (the report's NOT GLUED): the system is not called.
    }
    fn om_set_render_targets(
        &self,
        num_render_target_descriptors: u32,
        render_target_descriptors: Option<&direct3d_12::structures::CpuDescriptorHandle>,
        r_ts_single_handle_to_descriptor_range: bool,
        depth_stencil_descriptor: Option<&direct3d_12::structures::CpuDescriptorHandle>,
    ) {
        let _ = num_render_target_descriptors;
        let _ = render_target_descriptors;
        let _ = r_ts_single_handle_to_descriptor_range;
        let _ = depth_stencil_descriptor;
        // Refused by its kinds (src/kinds.rs), the system never called: pRenderTargetDescriptors: D3D12_CPU_DESCRIPTOR_HANDLE.ptr: an integer of pointer width, which the system may take as an address; pDepthStencilDescriptor: D3D12_CPU_DESCRIPTOR_HANDLE.ptr: an integer of pointer width, which the system may take as an address.
    }
    fn clear_render_target_view(
        &self,
        render_target_view: direct3d_12::structures::CpuDescriptorHandle,
        color_rgba: &[f32; direct3d_12::constants::GRAPHICS_COMMAND_LIST_CLEAR_RENDER_TARGET_VIEW_COLOR_RGBA_LENGTH],
        rects: &[component_object_model::structures::Rect],
    ) {
        let _ = render_target_view;
        let _ = color_rgba;
        let _ = rects;
        // Refused by its kinds (src/kinds.rs), the system never called: RenderTargetView: D3D12_CPU_DESCRIPTOR_HANDLE.ptr: an integer of pointer width, which the system may take as an address.
    }
    fn begin_query(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        index: u32,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::BeginQuery, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::BeginQuery` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().graphics_command_list_begin_query)(
                self.object.raw(),
                {
                    let held = query_heap;
                    held.raw()
                },
                type_.0,
                index,
            )
        };
        let () = returned;
    }
    fn end_query(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        index: u32,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::EndQuery, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::EndQuery` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.object.entry_points().graphics_command_list_end_query)(
                self.object.raw(),
                {
                    let held = query_heap;
                    held.raw()
                },
                type_.0,
                index,
            )
        };
        let () = returned;
    }
    fn resolve_query_data(
        &self,
        query_heap: &<Self::Api as direct3d_12::Direct3D12>::QueryHeap,
        type_: direct3d_12::enumerations::QueryType,
        start_index: u32,
        num_queries: u32,
        destination_buffer: &<Self::Api as direct3d_12::Direct3D12>::Resource,
        aligned_destination_buffer_offset: u64,
    ) {
        #[expect(
            unsafe_code,
            reason = "ID3D12GraphicsCommandList::ResolveQueryData, called out"
        )]
        // SAFETY: a live object whose table holds `ID3D12GraphicsCommandList::ResolveQueryData` where the metadata orders it; every pointer passed is ours for the call.
        let returned = unsafe {
            (self
                .object
                .entry_points()
                .graphics_command_list_resolve_query_data)(
                self.object.raw(),
                {
                    let held = query_heap;
                    held.raw()
                },
                type_.0,
                start_index,
                num_queries,
                {
                    let held = destination_buffer;
                    held.raw()
                },
                aligned_destination_buffer_offset,
            )
        };
        let () = returned;
    }
}

impl<A: System> direct3d_12::CommandList for SystemGraphicsCommandList<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemGraphicsCommandList<A> {}

impl<A: System> direct3d_12::Object for SystemGraphicsCommandList<A> {
    type Api = A;
}

impl<A: System> direct3d_12::PipelineState for SystemPipelineState<A> {}

impl<A: System> direct3d_12::Pageable for SystemPipelineState<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemPipelineState<A> {}

impl<A: System> direct3d_12::Object for SystemPipelineState<A> {
    type Api = A;
}

impl<A: System> direct3d_12::QueryHeap for SystemQueryHeap<A> {}

impl<A: System> direct3d_12::Pageable for SystemQueryHeap<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemQueryHeap<A> {}

impl<A: System> direct3d_12::Object for SystemQueryHeap<A> {
    type Api = A;
}

impl<A: System> direct3d_12::Resource for SystemResource<A> {
    fn map(
        &self,
        subresource: u32,
        read_range: Option<&direct3d_12::structures::Range>,
        data: &impl wcontainers_volatile_bytes::VolatileBytes,
    ) -> Result<(), component_object_model::structures::HResult> {
        let _ = subresource;
        let _ = read_range;
        let _ = data;
        // Refused by its kinds (src/kinds.rs), the system never called: pReadRange: D3D12_RANGE.Begin: an integer of pointer width, which the system may take as an address; pReadRange: D3D12_RANGE.End: an integer of pointer width, which the system may take as an address; ppData: an address the system writes (a 2-deep pointer), valid until a later call the metadata does not name.
        Err(E_NOTIMPL)
    }
    fn unmap(&self, subresource: u32, written_range: Option<&direct3d_12::structures::Range>) {
        let _ = subresource;
        let _ = written_range;
        // Refused by its kinds (src/kinds.rs), the system never called: pWrittenRange: D3D12_RANGE.Begin: an integer of pointer width, which the system may take as an address; pWrittenRange: D3D12_RANGE.End: an integer of pointer width, which the system may take as an address.
    }
    fn get_desc(&self) -> direct3d_12::structures::ResourceDesc {
        // Refused by its form (the report's NOT GLUED): the system is not called.
        decode_windows_win32_graphics_direct_3d12_d_3d12_resource_desc(
            &[u8::MIN; size_of::<WindowsWin32GraphicsDirect3d12D3d12ResourceDescLayout>()],
        )
    }
    fn get_gpu_virtual_address(&self) -> u64 {
        // Refused by its form (the report's NOT GLUED): the system is not called.
        Default::default()
    }
}

impl<A: System> direct3d_12::Pageable for SystemResource<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemResource<A> {}

impl<A: System> direct3d_12::Object for SystemResource<A> {
    type Api = A;
}

impl<A: System> direct3d_12::RootSignature for SystemRootSignature<A> {}

impl<A: System> direct3d_12::DeviceChild for SystemRootSignature<A> {}

impl<A: System> direct3d_12::Object for SystemRootSignature<A> {
    type Api = A;
}

/// The C side's library, as an implementation of the API's top-level trait and of those it extends.
pub struct SystemDirect3D12 {
    library: Library,
}

impl SystemDirect3D12 {
    /// Returns the API over the C side's library.
    ///
    /// # Safety
    /// Each entry is the library's own export of that name, which takes the parameters its metadata gives.
    #[expect(
        unsafe_code,
        reason = "the promise that each entry is the library's export"
    )]
    pub unsafe fn new(library: Library) -> SystemDirect3D12 {
        SystemDirect3D12 { library }
    }
}

impl direct3d_12::Direct3D12 for SystemDirect3D12 {
    type CommandAllocator = SystemCommandAllocator<Self>;
    type CommandQueue = SystemCommandQueue<Self>;
    type DescriptorHeap = SystemDescriptorHeap<Self>;
    type Device = SystemDevice<Self>;
    type Fence = SystemFence<Self>;
    type GraphicsCommandList = SystemGraphicsCommandList<Self>;
    type PipelineState = SystemPipelineState<Self>;
    type QueryHeap = SystemQueryHeap<Self>;
    type Resource = SystemResource<Self>;
    type RootSignature = SystemRootSignature<Self>;

    fn create_device(
        &self,
        adapter: Option<direct3d_12::UnknownRef<'_, Self>>,
        minimum_feature_level: direct3d_common::enumerations::FeatureLevel,
        riid: &component_object_model::structures::Guid,
    ) -> Result<direct3d_12::UnknownObject<Self>, component_object_model::structures::HResult> {
        let adapter_raw: Raw = match adapter {
            Some(direct3d_12::UnknownRef::CommandAllocator(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::CommandQueue(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::DescriptorHeap(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::Device(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::Fence(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::GraphicsCommandList(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::PipelineState(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::QueryHeap(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::Resource(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::RootSignature(object)) => object.raw(),
            Some(direct3d_12::UnknownRef::Unknown(foreign)) => {
                match foreign.object().downcast_ref::<ForeignUnknown>() {
                    Some(held) => held.raw(),
                    None => {
                        return Err(E_INVALIDARG);
                    }
                }
            }
            None => null_mut(),
        };
        let mut riid_bytes = Aligned::<{ size_of::<GuidLayout>() }>::default();
        encode_guid(riid, &mut riid_bytes.0);
        let mut device: Raw = null_mut();
        #[expect(
            unsafe_code,
            reason = "D3D12CreateDevice, the library's export, called out"
        )]
        // SAFETY: the library's export `D3D12CreateDevice`, as `new`'s caller promised; every pointer passed is ours for the call.
        let returned = unsafe {
            (self.library.d_3d12_create_device)(
                adapter_raw,
                minimum_feature_level.0,
                riid_bytes.0.as_ptr(),
                &raw mut device,
            )
        };
        succeeded(returned)?;
        #[expect(unsafe_code, reason = "D3D12CreateDevice: ppDevice, Out and ComOutPtr")]
        // SAFETY: the metadata's `ppDevice` is `Out` and `ComOutPtr`: after a success, null or one reference to an object of the interface `riid` names.
        let Some(device) = (unsafe { unknown_object_of::<Self>(riid, device) }) else {
            return Err(E_FAIL);
        };
        Ok(device)
    }
}

impl System for SystemDirect3D12 {}

impl direct3d_common::Direct3DCommon for SystemDirect3D12 {
    type Blob = direct3d_common_direct::SystemBlob<Self>;
    type DestructionNotifier = direct3d_common_direct::SystemDestructionNotifier<Self>;
}

impl direct3d_common_direct::System for SystemDirect3D12 {}
