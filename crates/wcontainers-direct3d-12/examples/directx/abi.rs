//! Methods returning a structure. COM's ABI passes a pointer for the structure and returns that
//! pointer, which C callers read; the windows crate's shims write the structure and return nothing.
//! Objects with such methods get a copy of their vtable whose entries also return the pointer.

use std::ffi::c_void;
use std::sync::OnceLock;

use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::Direct3D12::*;
use windows::core::Interface;

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

impl StructReturns for ID3D12Device {
    fn patched(original: &ID3D12Device_Vtbl) -> &'static ID3D12Device_Vtbl {
        static VTBL: OnceLock<ID3D12Device_Vtbl> = OnceLock::new();
        VTBL.get_or_init(|| {
            // SAFETY: a bitwise copy of a table of function pointers.
            let mut v = unsafe { std::ptr::read(original) };
            returns_pointer!(v.GetAdapterLuid() -> LUID);
            returns_pointer!(
                v.GetResourceAllocationInfo(mask: u32, count: u32, descs: *const D3D12_RESOURCE_DESC)
                    -> D3D12_RESOURCE_ALLOCATION_INFO
            );
            returns_pointer!(
                v.GetCustomHeapProperties(mask: u32, kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES
            );
            v
        })
    }
}

impl StructReturns for ID3D12Resource {
    fn patched(original: &ID3D12Resource_Vtbl) -> &'static ID3D12Resource_Vtbl {
        static VTBL: OnceLock<ID3D12Resource_Vtbl> = OnceLock::new();
        VTBL.get_or_init(|| {
            // SAFETY: a bitwise copy of a table of function pointers.
            let mut v = unsafe { std::ptr::read(original) };
            returns_pointer!(v.GetDesc() -> D3D12_RESOURCE_DESC);
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
