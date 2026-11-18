//! DXGI's entry points (`CreateDXGIFactory`, `1` and `2`), which `dxgi.dll` forwards here. A
//! program's own calls get our factory, whose adapter is the back end's. Calls from this library
//! (wgpu under the back end), from the libraries the back end loads and from the system's libraries
//! load DXGI by name, which in such a process finds this library, and go on to the system's; so do
//! their calls of Direct3D 11's entry points.

use std::ffi::c_void;

use windows::Win32::Foundation::{E_NOINTERFACE, E_POINTER, HMODULE};
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_UNSUPPORTED, IDXGIFactory6};
use windows::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW, GetProcAddress, LoadLibraryW,
};
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows::core::{GUID, HRESULT, Interface, PCSTR, PCWSTR};

use super::dxgi::Adapter;
use super::swapchain::Factory;

const EXPORTS: &str = " /EXPORT:CreateDXGIFactory=wc_CreateDXGIFactory /EXPORT:CreateDXGIFactory1=wc_CreateDXGIFactory1 /EXPORT:CreateDXGIFactory2=wc_CreateDXGIFactory2 ";

/// Linker directives the linker reads from this section of the object file: DXGI's entry points
/// under their own names, whose definitions are `wc_` ones so they never meet an import of the
/// system's.
#[used]
#[unsafe(link_section = ".drectve")]
static DIRECTIVES: [u8; EXPORTS.len()] = {
    let mut out = [0; EXPORTS.len()];
    let bytes = EXPORTS.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        out[i] = bytes[i];
        i += 1;
    }
    out
};

/// The module holding `address`.
fn module_of(address: *const c_void) -> Option<HMODULE> {
    let mut module = HMODULE::default();
    // SAFETY: looks the address up without taking a reference on the module.
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(address.cast()),
            &mut module,
        )
    }
    .ok()?;
    Some(module)
}

/// The system directory, lower case, ending in a backslash.
fn system_directory() -> String {
    let mut dir = [0u16; 260];
    // SAFETY: a buffer of the length given.
    let len = unsafe { GetSystemDirectoryW(Some(&mut dir)) } as usize;
    String::from_utf16_lossy(&dir[..len.min(dir.len())]).to_lowercase() + "\\"
}

/// Whether the caller of the entry point that calls this is the program's code, not this library's,
/// the back end's or the system's.
#[inline(never)]
pub(super) fn called_by_program() -> bool {
    let mut frames = [std::ptr::null_mut(); 1];
    // Frame 0 is this function, 1 the entry point, 2 its caller.
    // SAFETY: writes at most one frame into the array.
    if unsafe { RtlCaptureStackBackTrace(2, &mut frames, None) } == 0 {
        return true;
    }
    let (Some(caller), Some(own)) = (
        module_of(frames[0]),
        module_of(called_by_program as *const c_void),
    ) else {
        return true;
    };
    if caller == own {
        return false;
    }
    let mut path = [0u16; 1024];
    // SAFETY: a buffer of the length given.
    let len = unsafe { GetModuleFileNameW(Some(caller), &mut path) } as usize;
    let path = String::from_utf16_lossy(&path[..len.min(path.len())]).to_lowercase();
    !path.starts_with(&system_directory())
        && !super::chosen::own_directory().is_some_and(|d| path.starts_with(d))
}

/// The export `export` of the system's library `library`, loaded by its full path.
///
/// # Safety
/// `F` is the export's function type.
pub(super) unsafe fn system<F: Copy>(library: &str, export: &std::ffi::CStr) -> Option<F> {
    let path: Vec<u16> = (system_directory() + library)
        .encode_utf16()
        .chain([0])
        .collect();
    // SAFETY: a path ended by a zero; the module stays loaded for the process.
    let module = unsafe { LoadLibraryW(PCWSTR(path.as_ptr())) }.ok()?;
    // SAFETY: a name ended by a zero.
    let f = unsafe { GetProcAddress(module, PCSTR(export.as_ptr().cast())) }?;
    // SAFETY: the caller names the type.
    Some(unsafe { std::mem::transmute_copy(&f) })
}

/// Our factory, as interface `riid`.
///
/// # Safety
/// `riid` and `out` are the caller's, as COM passes them.
unsafe fn ours(riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() || riid.is_null() {
        return E_POINTER;
    }
    // SAFETY: the caller's out pointer.
    unsafe { *out = std::ptr::null_mut() };
    let _lock = super::lock();
    let Some(caps) = super::caps() else {
        return DXGI_ERROR_UNSUPPORTED;
    };
    let factory: IDXGIFactory6 = Factory::new(Adapter::new(caps).into()).into();
    // SAFETY: both pointers were checked and come from the caller.
    unsafe { factory.query(riid, out) }
}

type WithFlags = unsafe extern "system" fn(u32, *const GUID, *mut *mut c_void) -> HRESULT;
type Plain = unsafe extern "system" fn(*const GUID, *mut *mut c_void) -> HRESULT;

/// `CreateDXGIFactory2`, as Microsoft documents it (its one flag asks for DXGI's debug layer,
/// which is not served and changes nothing else).
///
/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_CreateDXGIFactory2(
    flags: u32,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if !called_by_program() {
        // SAFETY: the system's export of this name has this type.
        return match unsafe { system::<WithFlags>("dxgi.dll", c"CreateDXGIFactory2") } {
            Some(f) => unsafe { f(flags, riid, out) },
            None => E_NOINTERFACE,
        };
    }
    // SAFETY: the caller's pointers, passed on.
    unsafe { ours(riid, out) }
}

/// `CreateDXGIFactory1`, as Microsoft documents it.
///
/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_CreateDXGIFactory1(
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if !called_by_program() {
        // SAFETY: the system's export of this name has this type.
        return match unsafe { system::<Plain>("dxgi.dll", c"CreateDXGIFactory1") } {
            Some(f) => unsafe { f(riid, out) },
            None => E_NOINTERFACE,
        };
    }
    // SAFETY: the caller's pointers, passed on.
    unsafe { ours(riid, out) }
}

/// `CreateDXGIFactory`, as Microsoft documents it.
///
/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_CreateDXGIFactory(
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if !called_by_program() {
        // SAFETY: the system's export of this name has this type.
        return match unsafe { system::<Plain>("dxgi.dll", c"CreateDXGIFactory") } {
            Some(f) => unsafe { f(riid, out) },
            None => E_NOINTERFACE,
        };
    }
    // SAFETY: the caller's pointers, passed on.
    unsafe { ours(riid, out) }
}
