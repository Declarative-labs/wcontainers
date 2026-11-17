//! Methods returning a structure. COM's ABI passes a pointer for the structure and returns that
//! pointer, which C callers read; the windows crate's shims write the structure and return nothing.
//! Objects with such methods get a copy of their vtable whose entries also return the pointer.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;

use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::Direct3D12::*;
use windows::core::{GUID, HRESULT, IUnknown_Vtbl, Interface};

/// Replaces `$vtbl.$field`, a method writing a `$out` through its first argument, with a shim that
/// calls it and returns that argument.
macro_rules! returns_pointer {
    ($vtbl:ident . $field:ident ($($arg:ident: $ty:ty),*) -> $out:ty) => {{
        static ORIGINAL: OnceLock<unsafe extern "system" fn(*mut c_void, *mut $out $(, $ty)*)> =
            OnceLock::new();
        unsafe extern "system" fn shim(this: *mut c_void, out: *mut $out $(, $arg: $ty)*) -> *mut $out {
            // SAFETY: the original method, called as the caller called the shim.
            unsafe { ORIGINAL.get().expect("set before the vtable is used")(this, out $(, $arg)*) };
            out
        }
        ORIGINAL.get_or_init(|| $vtbl.$field);
        let shim: unsafe extern "system" fn(*mut c_void, *mut $out $(, $ty)*) -> *mut $out = shim;
        // SAFETY: the same arguments; the windows crate's callers ignore the returned value.
        $vtbl.$field = unsafe { std::mem::transmute::<
            unsafe extern "system" fn(*mut c_void, *mut $out $(, $ty)*) -> *mut $out,
            unsafe extern "system" fn(*mut c_void, *mut $out $(, $ty)*),
        >(shim) };
    }};
}

/// Interfaces whose vtable has methods returning a structure.
pub trait StructReturns: Interface<Vtable: 'static> {
    fn patched(original: &Self::Vtable) -> &'static Self::Vtable;
}

/// `object` with its vtable replaced by the patched copy.
pub fn fixed<I: StructReturns>(object: I) -> I {
    let slot = object.as_raw() as *mut *const I::Vtable;
    // SAFETY: an interface pointer points to its vtable pointer, in the object's own memory.
    unsafe {
        let patched = I::patched(&**slot);
        *slot = patched;
    }
    object
}

impl StructReturns for ID3D12Device8 {
    fn patched(original: &ID3D12Device8_Vtbl) -> &'static ID3D12Device8_Vtbl {
        static VTBL: OnceLock<ID3D12Device8_Vtbl> = OnceLock::new();
        VTBL.get_or_init(|| {
            // SAFETY: a bitwise copy of a table of function pointers.
            let mut v = unsafe { std::ptr::read(original) };
            returns_pointer!(
                v.GetResourceAllocationInfo2(
                    mask: u32,
                    count: u32,
                    descs: *const D3D12_RESOURCE_DESC1,
                    infos: *mut D3D12_RESOURCE_ALLOCATION_INFO1
                ) -> D3D12_RESOURCE_ALLOCATION_INFO
            );
            let v4 = &mut v.base__.base__.base__.base__;
            returns_pointer!(
                v4.GetResourceAllocationInfo1(
                    mask: u32,
                    count: u32,
                    descs: *const D3D12_RESOURCE_DESC,
                    infos: *mut D3D12_RESOURCE_ALLOCATION_INFO1
                ) -> D3D12_RESOURCE_ALLOCATION_INFO
            );
            let v0 = &mut v4.base__.base__.base__.base__;
            returns_pointer!(v0.GetAdapterLuid() -> LUID);
            returns_pointer!(
                v0.GetResourceAllocationInfo(mask: u32, count: u32, descs: *const D3D12_RESOURCE_DESC)
                    -> D3D12_RESOURCE_ALLOCATION_INFO
            );
            returns_pointer!(
                v0.GetCustomHeapProperties(mask: u32, kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES
            );
            v
        })
    }
}

impl StructReturns for ID3D12Resource2 {
    fn patched(original: &ID3D12Resource2_Vtbl) -> &'static ID3D12Resource2_Vtbl {
        static VTBL: OnceLock<ID3D12Resource2_Vtbl> = OnceLock::new();
        VTBL.get_or_init(|| {
            // SAFETY: a bitwise copy of a table of function pointers.
            let mut v = unsafe { std::ptr::read(original) };
            returns_pointer!(v.GetDesc1() -> D3D12_RESOURCE_DESC1);
            let v0 = &mut v.base__.base__;
            returns_pointer!(v0.GetDesc() -> D3D12_RESOURCE_DESC);
            v
        })
    }
}

impl StructReturns for ID3D12DescriptorHeap {
    fn patched(original: &ID3D12DescriptorHeap_Vtbl) -> &'static ID3D12DescriptorHeap_Vtbl {
        static VTBL: OnceLock<ID3D12DescriptorHeap_Vtbl> = OnceLock::new();
        VTBL.get_or_init(|| {
            // SAFETY: a bitwise copy of a table of function pointers.
            let mut v = unsafe { std::ptr::read(original) };
            returns_pointer!(v.GetDesc() -> D3D12_DESCRIPTOR_HEAP_DESC);
            returns_pointer!(v.GetCPUDescriptorHandleForHeapStart() -> D3D12_CPU_DESCRIPTOR_HANDLE);
            returns_pointer!(v.GetGPUDescriptorHandleForHeapStart() -> D3D12_GPU_DESCRIPTOR_HANDLE);
            v
        })
    }
}

impl StructReturns for ID3D12CommandQueue {
    fn patched(original: &ID3D12CommandQueue_Vtbl) -> &'static ID3D12CommandQueue_Vtbl {
        static VTBL: OnceLock<ID3D12CommandQueue_Vtbl> = OnceLock::new();
        VTBL.get_or_init(|| {
            // SAFETY: a bitwise copy of a table of function pointers.
            let mut v = unsafe { std::ptr::read(original) };
            returns_pointer!(v.GetDesc() -> D3D12_COMMAND_QUEUE_DESC);
            v
        })
    }
}

impl StructReturns for ID3D12Heap {
    fn patched(original: &ID3D12Heap_Vtbl) -> &'static ID3D12Heap_Vtbl {
        static VTBL: OnceLock<ID3D12Heap_Vtbl> = OnceLock::new();
        VTBL.get_or_init(|| {
            // SAFETY: a bitwise copy of a table of function pointers.
            let mut v = unsafe { std::ptr::read(original) };
            returns_pointer!(v.GetDesc() -> D3D12_HEAP_DESC);
            let unknown = &mut v.base__.base__.base__.base__;
            // SAFETY: a bitwise copy of a table of function pointers.
            HEAP_UNKNOWN.get_or_init(|| unsafe { std::ptr::read(unknown) });
            unknown.QueryInterface = heap_query;
            unknown.AddRef = heap_add_ref;
            unknown.Release = heap_release;
            v
        })
    }
}

// A heap's object reference count is one for all the program's references (while it has any) plus
// one for each placed resource's `HeapHold`; the program's own count is kept in the heap.

/// The heap vtable's own `IUnknown` entries.
static HEAP_UNKNOWN: OnceLock<IUnknown_Vtbl> = OnceLock::new();

fn heap_unknown() -> &'static IUnknown_Vtbl {
    HEAP_UNKNOWN.get().expect("set before the vtable is used")
}

unsafe extern "system" fn heap_query(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    // SAFETY: called as COM calls it, on a live heap.
    unsafe {
        let r = (heap_unknown().QueryInterface)(this, iid, out);
        // The object count rose by one: keep it only if the program had no references.
        if r.is_ok()
            && crate::objects::heap_program_count(&this).fetch_add(1, Ordering::AcqRel) != 0
        {
            (heap_unknown().Release)(this);
        }
        r
    }
}

unsafe extern "system" fn heap_add_ref(this: *mut c_void) -> u32 {
    // SAFETY: called as COM calls it, on a live heap.
    unsafe {
        let n = crate::objects::heap_program_count(&this).fetch_add(1, Ordering::AcqRel) + 1;
        if n == 1 {
            (heap_unknown().AddRef)(this);
        }
        n
    }
}

unsafe extern "system" fn heap_release(this: *mut c_void) -> u32 {
    // SAFETY: called as COM calls it, on a live heap; the object may end in the last call.
    unsafe {
        let n = crate::objects::heap_program_count(&this).fetch_sub(1, Ordering::AcqRel) - 1;
        if n == 0 {
            (heap_unknown().Release)(this);
        }
        n
    }
}

/// Takes a placed resource's reference on heap `this`.
///
/// # Safety
/// `this` is a live heap of this library.
pub unsafe fn heap_hold(this: *mut c_void) {
    // SAFETY: as the caller promises.
    unsafe { (heap_unknown().AddRef)(this) };
}

/// Lets go of a reference `heap_hold` took; the heap may end.
///
/// # Safety
/// `this` is a heap `heap_hold` was called on, once for each call.
pub unsafe fn heap_let_go(this: *mut c_void) {
    // SAFETY: as the caller promises.
    unsafe { (heap_unknown().Release)(this) };
}
