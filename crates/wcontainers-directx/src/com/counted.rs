//! Device children counted as Direct3D counts them: the program's references (`AddRef`,
//! `Release`, and what `Release` answers) apart from the library's own (a view's resource, what the
//! context has bound, the device's live state objects), which the program never sees. An object
//! lives while either kind is held, and holds its device while the program holds it.

use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering, fence};

use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::core::{ComObjectInterface, IUnknownImpl, Interface};

use super::Child;

/// The two counts. The program's references together hold one of the library's.
pub struct Counts {
    public: AtomicU32,
    private: AtomicU32,
}

impl Counts {
    /// One reference of the program's, which holds one of the library's.
    pub const fn new() -> Self {
        Counts {
            public: AtomicU32::new(1),
            private: AtomicU32::new(1),
        }
    }
}

impl Default for Counts {
    fn default() -> Self {
        Self::new()
    }
}

/// An object counted this way (made by [`counted!`]).
///
/// # Safety
/// The object is in a box made by its `ComObject` and is destroyed only by [`release_private`].
pub unsafe trait Counted: IUnknownImpl + Sized + 'static {
    fn counts(&self) -> &Counts;
    fn child(&self) -> &Child;
}

/// How many references the program holds.
pub fn public_count<O: Counted>(o: &O) -> u32 {
    o.counts().public.load(Ordering::Relaxed)
}

/// The program takes a reference: the first since it had none takes one of the library's and one
/// on the device.
pub fn add_public<O: Counted>(o: &O) -> u32 {
    let n = o.counts().public.fetch_add(1, Ordering::Relaxed) + 1;
    if n == 1 {
        o.counts().private.fetch_add(1, Ordering::Relaxed);
        std::mem::forget(o.child().device().clone());
    }
    n
}

/// The program drops a reference: the last gives back the device's and one of the library's. A
/// release when the program holds none (an object the library still holds, released once too
/// often) changes nothing and answers what Direct3D answers, the count wrapped below zero.
///
/// # Safety
/// `o` is a live object whose reference the caller held, or one the library holds.
pub unsafe fn release_public<O: Counted>(o: *mut O) -> u32 {
    let public = &unsafe { &*o }.counts().public;
    let Ok(held) = public.try_update(Ordering::Release, Ordering::Relaxed, |n| n.checked_sub(1))
    else {
        return u32::MAX;
    };
    let n = held - 1;
    if n == 0 {
        unsafe { &*o }.child().released(o as usize);
        let device = unsafe { &*o }.child().device().as_raw();
        unsafe { release_private(o) };
        // The object may be gone; its device reference is given back through the copied pointer.
        drop(unsafe { ID3D11Device::from_raw(device) });
    }
    n
}

/// One of the library's references goes; the object is destroyed when none of either is left.
///
/// # Safety
/// `o` is a live object one of whose library references the caller held.
pub unsafe fn release_private<O: Counted>(o: *mut O) {
    if unsafe { &*o }
        .counts()
        .private
        .fetch_sub(1, Ordering::Release)
        == 1
    {
        fence(Ordering::Acquire);
        drop(unsafe { Box::from_raw(o) });
    }
}

/// One of the library's references to an object of ours.
pub struct Held<O: Counted>(NonNull<O>);

impl<O: Counted> Held<O> {
    pub fn new(o: &O) -> Self {
        o.counts().private.fetch_add(1, Ordering::Relaxed);
        Held(NonNull::from(o))
    }

    /// The object as interface `I`, with a reference of the program's.
    pub fn public<I: Interface>(&self) -> I
    where
        O: ComObjectInterface<I>,
    {
        self.deref().as_interface_ref().to_owned()
    }

    /// Whether two are the same object.
    pub fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    /// The object's address, which names it.
    pub fn address(&self) -> usize {
        self.0.as_ptr() as usize
    }
}

impl<O: Counted> Deref for Held<O> {
    type Target = O;
    fn deref(&self) -> &O {
        unsafe { self.0.as_ref() }
    }
}

impl<O: Counted> Clone for Held<O> {
    fn clone(&self) -> Self {
        Held::new(self.deref())
    }
}

impl<O: Counted> Drop for Held<O> {
    fn drop(&mut self) {
        unsafe { release_private(self.0.as_ptr()) }
    }
}

/// Makes `$outer`, the COM object around `$inner` (which has a `child: Child` field), counted as
/// above, answering `QueryInterface` for each interface listed (with its vtable type) and those it
/// derives from. The first is its identity's (with the interfaces it derives from that Rust code
/// asks for after `|`); each after it names its place among the vtable pointers (-2, -3, ...) and
/// may name a method of `$inner` saying whether the object offers it.
macro_rules! counted {
    ($inner:ident => $outer:ident { $first:ident: $first_iface:ident $(| $also:ident)*, $first_vtbl:ident $(; $field:ident: $iface:ident, $vtbl:ident, $offset:expr $(, if $offers:ident)?)* $(;)? }) => {
        #[repr(C)]
        #[allow(non_camel_case_types)]
        pub struct $outer {
            identity: &'static windows_core::IInspectable_Vtbl,
            $first: &'static $first_vtbl,
            $($field: &'static $vtbl,)*
            this: $inner,
            counts: $crate::com::counted::Counts,
        }

        impl std::ops::Deref for $outer {
            type Target = $inner;
            #[inline(always)]
            fn deref(&self) -> &$inner {
                &self.this
            }
        }

        unsafe impl $crate::com::counted::Counted for $outer {
            #[inline(always)]
            fn counts(&self) -> &$crate::com::counted::Counts {
                &self.counts
            }
            #[inline(always)]
            fn child(&self) -> &$crate::com::Child {
                &self.this.child
            }
        }

        impl windows_core::IUnknownImpl for $outer {
            type Impl = $inner;

            #[inline(always)]
            fn get_impl(&self) -> &$inner {
                &self.this
            }

            #[inline(always)]
            fn get_impl_mut(&mut self) -> &mut $inner {
                &mut self.this
            }

            fn into_inner(self) -> $inner {
                self.this
            }

            unsafe fn QueryInterface(
                &self,
                iid: *const windows_core::GUID,
                interface: *mut *mut std::ffi::c_void,
            ) -> windows_core::HRESULT {
                use windows_core::Interface;
                if iid.is_null() || interface.is_null() {
                    return windows_core::imp::E_POINTER;
                }
                let iid = unsafe { *iid };
                if iid == windows_core::DYNAMIC_CAST_IID {
                    // Not an interface: the object itself for `cast_object_ref`, without a reference.
                    unsafe {
                        (interface as *mut *const dyn std::any::Any)
                            .write(self as &dyn std::any::Any as *const dyn std::any::Any)
                    };
                    return windows_core::HRESULT(0);
                }
                let found: *const std::ffi::c_void = if iid == windows_core::IUnknown::IID {
                    &self.identity as *const _ as *const std::ffi::c_void
                } else if $first_vtbl::matches(&iid) {
                    &self.$first as *const _ as *const std::ffi::c_void
                }
                $(else if $vtbl::matches(&iid) $(&& self.this.$offers())? {
                    &self.$field as *const _ as *const std::ffi::c_void
                })*
                else {
                    unsafe { *interface = std::ptr::null_mut() };
                    return windows_core::imp::E_NOINTERFACE;
                };
                unsafe { *interface = found as *mut std::ffi::c_void };
                self.AddRef();
                windows_core::HRESULT(0)
            }

            #[inline(always)]
            fn AddRef(&self) -> u32 {
                $crate::com::counted::add_public(self)
            }

            #[inline(always)]
            unsafe fn Release(self_: *mut Self) -> u32 {
                unsafe { $crate::com::counted::release_public(self_) }
            }

            fn is_reference_count_one(&self) -> bool {
                use $crate::com::counted::Counted;
                self.counts().is_one()
            }

            unsafe fn GetTrustLevel(&self, value: *mut i32) -> windows_core::HRESULT {
                if value.is_null() {
                    return windows_core::imp::E_POINTER;
                }
                unsafe { *value = 0 };
                windows_core::HRESULT(0)
            }

            fn to_object(&self) -> windows_core::ComObject<$inner> {
                self.AddRef();
                unsafe { windows_core::ComObject::from_raw(std::ptr::NonNull::from(self)) }
            }
        }

        impl windows_core::ComObjectInner for $inner {
            type Outer = $outer;

            fn into_object(self) -> windows_core::ComObject<Self> {
                let outer = $outer {
                    identity: &const {
                        windows_core::IInspectable_Vtbl::new::<$outer, $first_iface, 0>()
                    },
                    $first: &const { $first_vtbl::new::<$outer, -1>() },
                    $($field: &const { $vtbl::new::<$outer, { $offset }>() },)*
                    this: self,
                    counts: $crate::com::counted::Counts::new(),
                };
                let ptr = Box::into_raw(Box::new(outer));
                unsafe { windows_core::ComObject::from_raw(std::ptr::NonNull::new_unchecked(ptr)) }
            }
        }

        impl windows_core::ComObjectInterface<windows_core::IUnknown> for $outer {
            #[inline(always)]
            fn as_interface_ref(&self) -> windows_core::InterfaceRef<'_, windows_core::IUnknown> {
                unsafe { std::mem::transmute(&self.identity) }
            }
        }

        impl windows_core::ComObjectInterface<$first_iface> for $outer {
            #[inline(always)]
            fn as_interface_ref(&self) -> windows_core::InterfaceRef<'_, $first_iface> {
                unsafe { std::mem::transmute(&self.$first) }
            }
        }

        $(impl windows_core::ComObjectInterface<$also> for $outer {
            #[inline(always)]
            fn as_interface_ref(&self) -> windows_core::InterfaceRef<'_, $also> {
                unsafe { std::mem::transmute(&self.$first) }
            }
        })*

        $(impl windows_core::ComObjectInterface<$iface> for $outer {
            #[inline(always)]
            fn as_interface_ref(&self) -> windows_core::InterfaceRef<'_, $iface> {
                unsafe { std::mem::transmute(&self.$field) }
            }
        })*

        impl From<$inner> for $first_iface {
            fn from(this: $inner) -> Self {
                windows_core::ComObject::new(this).into_interface()
            }
        }

        $(impl From<$inner> for $also {
            fn from(this: $inner) -> Self {
                $first_iface::from(this).into()
            }
        })*
    };
}

pub(crate) use counted;

impl Counts {
    pub fn is_one(&self) -> bool {
        self.public.load(Ordering::Acquire) == 1 && self.private.load(Ordering::Acquire) == 1
    }
}
