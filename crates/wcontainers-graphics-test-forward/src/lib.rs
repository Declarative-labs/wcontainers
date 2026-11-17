//! A stand-in d3d11.dll that writes the name of each call it receives to standard error and passes
//! the call, untouched, to the system's d3d11.dll. A program that loads it from its own directory
//! shows that a replacement placed beside a program is the one it uses. With WC_OUT set it also
//! takes a census of the program's Direct3D calls and saves chosen frames (census.rs).

#![cfg(windows)]

mod census;

use std::ffi::c_void;

use std::ffi::CStr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

include!(concat!(env!("OUT_DIR"), "/exports.rs"));

/// The real function behind each export, once found.
static RESOLVED: [AtomicUsize; 512] = [const { AtomicUsize::new(0) }; 512];
const _: () = assert!(EXPORTS.len() <= RESOLVED.len());
/// Calls of each export.
static CALLS: [AtomicU64; 512] = [const { AtomicU64::new(0) }; 512];

/// The exports called, with how often.
fn export_counts() -> Vec<(String, u64)> {
    EXPORTS
        .iter()
        .zip(&CALLS)
        .map(|(name, calls)| {
            (
                name.to_string_lossy().into_owned(),
                calls.load(Ordering::Relaxed),
            )
        })
        .filter(|(_, calls)| *calls > 0)
        .collect()
}

fn export(name: &str) -> usize {
    let index = EXPORTS
        .iter()
        .position(|e| e.to_bytes() == name.as_bytes())
        .unwrap();
    resolve(index as u32)
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
unsafe extern "system" fn wc_typed_D3D11CreateDevice(
    adapter: *mut c_void,
    driver: i32,
    software: *mut c_void,
    flags: u32,
    levels: *const i32,
    level_count: u32,
    sdk: u32,
    device: *mut *mut c_void,
    level: *mut i32,
    context: *mut *mut c_void,
) -> i32 {
    type Real = unsafe extern "system" fn(
        *mut c_void,
        i32,
        *mut c_void,
        u32,
        *const i32,
        u32,
        u32,
        *mut *mut c_void,
        *mut i32,
        *mut *mut c_void,
    ) -> i32;
    let real: Real = unsafe { std::mem::transmute(export("D3D11CreateDevice")) };
    let result = unsafe {
        real(
            adapter,
            driver,
            software,
            flags,
            levels,
            level_count,
            sdk,
            device,
            level,
            context,
        )
    };
    if result >= 0 && !device.is_null() {
        unsafe { census::created(*device, context, std::ptr::null_mut()) };
    }
    result
}

#[allow(non_snake_case, clippy::too_many_arguments)]
#[unsafe(no_mangle)]
unsafe extern "system" fn wc_typed_D3D11CreateDeviceAndSwapChain(
    adapter: *mut c_void,
    driver: i32,
    software: *mut c_void,
    flags: u32,
    levels: *const i32,
    level_count: u32,
    sdk: u32,
    desc: *const c_void,
    chain: *mut *mut c_void,
    device: *mut *mut c_void,
    level: *mut i32,
    context: *mut *mut c_void,
) -> i32 {
    type Real = unsafe extern "system" fn(
        *mut c_void,
        i32,
        *mut c_void,
        u32,
        *const i32,
        u32,
        u32,
        *const c_void,
        *mut *mut c_void,
        *mut *mut c_void,
        *mut i32,
        *mut *mut c_void,
    ) -> i32;
    let real: Real = unsafe { std::mem::transmute(export("D3D11CreateDeviceAndSwapChain")) };
    let result = unsafe {
        real(
            adapter,
            driver,
            software,
            flags,
            levels,
            level_count,
            sdk,
            desc,
            chain,
            device,
            level,
            context,
        )
    };
    if result >= 0 && !device.is_null() {
        let get = |p: *mut *mut c_void| {
            if p.is_null() {
                std::ptr::null_mut()
            } else {
                unsafe { *p }
            }
        };
        unsafe { census::created(*device, context, get(chain)) };
    }
    result
}

/// Writes the census when the process ends normally.
#[unsafe(no_mangle)]
extern "system" fn DllMain(_module: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    const DLL_PROCESS_DETACH: u32 = 0;
    if reason == DLL_PROCESS_DETACH {
        census::write();
    }
    1
}

/// Called by the thunk of export `index`: logs the call and returns the system function to jump to.
/// There is no caller to give an error to, so a missing library or function ends the process.
extern "C" fn resolve(index: u32) -> usize {
    let name = EXPORTS[index as usize];
    eprintln!("{LIBRARY} forward: {}", name.to_string_lossy());
    CALLS[index as usize].fetch_add(1, Ordering::Relaxed);
    let slot = &RESOLVED[index as usize];
    let known = slot.load(Ordering::Acquire);
    if known != 0 {
        return known;
    }
    let Some(function) = (unsafe { GetProcAddress(system_library(), name.as_ptr().cast()) }) else {
        eprintln!("{LIBRARY} forward: the system library lacks this export");
        std::process::abort();
    };
    slot.store(function as usize, Ordering::Release);
    function as usize
}

/// The library calls go to, loaded by its full path (by name alone, Windows would hand back this
/// one): the system's, or the replacement at the path in WC_D3D11, so a census and frame capture
/// can sit in front of a replacement.
fn system_library() -> HMODULE {
    static MODULE: OnceLock<usize> = OnceLock::new();
    *MODULE.get_or_init(|| {
        // The variables are this library's alone: the library it loads, which may be another
        // copy of it, must not see them.
        census::configure();
        let target = std::env::var("WC_D3D11");
        for name in ["WC_OUT", "WC_CAPTURE", "WC_D3D11"] {
            // Read and removed once, on the program's first call into Direct3D 11.
            unsafe { std::env::remove_var(name) };
        }
        let path = match target {
            Ok(path) => path,
            Err(_) => {
                let mut dir = [0u16; 260];
                let length =
                    unsafe { GetSystemDirectoryW(dir.as_mut_ptr(), dir.len() as u32) } as usize;
                assert!(length > 0 && length < dir.len(), "GetSystemDirectoryW");
                format!(
                    "{}\\{LIBRARY}.dll",
                    String::from_utf16_lossy(&dir[..length])
                )
            }
        };
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            eprintln!("{LIBRARY} forward: cannot load {path}");
            std::process::abort();
        }
        module as usize
    }) as HMODULE
}
