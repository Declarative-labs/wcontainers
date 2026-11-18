//! A stand-in for d3d11.dll and d3d12.dll (copied under either name) that writes the name of each
//! call it receives to standard error and passes the call, untouched, to the system's library of
//! that export. A program that loads it from its own directory shows that a replacement placed
//! beside a program is the one it uses. With WC_OUT set it also takes a census of the program's
//! Direct3D calls and saves chosen frames (census.rs), on a clock that moves by frames (clock.rs).

#![cfg(windows)]

mod census;
mod clock;

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

/// An export's name as the census and the log write it: "<library>.dll <name>", or its ordinal.
fn export_label(index: usize) -> String {
    let (library, name, ordinal) = EXPORTS[index];
    match name {
        Some(name) => format!("{}.dll {}", LIBRARIES[library], name.to_string_lossy()),
        None => format!("{}.dll @{ordinal}", LIBRARIES[library]),
    }
}

/// The exports called, with how often.
fn export_counts() -> Vec<(String, u64)> {
    (0..EXPORTS.len())
        .map(|i| (export_label(i), CALLS[i].load(Ordering::Relaxed)))
        .filter(|(_, calls)| *calls > 0)
        .collect()
}

fn export(name: &str) -> usize {
    let index = EXPORTS
        .iter()
        .position(|e| e.1.is_some_and(|n| n.to_bytes() == name.as_bytes()))
        .unwrap();
    resolve(index as u32)
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
unsafe extern "system" fn wc_typed_D3D12CreateDevice(
    adapter: *mut c_void,
    level: i32,
    riid: *const windows::core::GUID,
    device: *mut *mut c_void,
) -> i32 {
    type Real = unsafe extern "system" fn(
        *mut c_void,
        i32,
        *const windows::core::GUID,
        *mut *mut c_void,
    ) -> i32;
    let real: Real = unsafe { std::mem::transmute(export("D3D12CreateDevice")) };
    let result = unsafe { real(adapter, level, riid, device) };
    // With a null device the call only asks whether a device could be made.
    if result >= 0 && !device.is_null() {
        unsafe { census::created12(*device) };
    }
    result
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

/// Starts the frame clock when frames are captured, as early as the program can be given it, and
/// writes the census when the process ends normally.
#[unsafe(no_mangle)]
extern "system" fn DllMain(_module: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    const DLL_PROCESS_DETACH: u32 = 0;
    const DLL_PROCESS_ATTACH: u32 = 1;
    match reason {
        DLL_PROCESS_ATTACH if std::env::var_os("WC_CAPTURE").is_some_and(|v| !v.is_empty()) => {
            for missed in clock::install() {
                census::note(format!("clock not detoured: {missed}"));
            }
        }
        DLL_PROCESS_DETACH => census::write(),
        _ => {}
    }
    1
}

/// Called by the thunk of export `index`: logs the call and returns the system function to jump to.
/// There is no caller to give an error to, so a missing library or function ends the process.
extern "C" fn resolve(index: u32) -> usize {
    let (library, name, ordinal) = EXPORTS[index as usize];
    let library_name = LIBRARIES[library];
    match name {
        Some(name) => eprintln!("{library_name} forward: {}", name.to_string_lossy()),
        None => eprintln!("{library_name} forward: @{ordinal}"),
    }
    CALLS[index as usize].fetch_add(1, Ordering::Relaxed);
    let slot = &RESOLVED[index as usize];
    let known = slot.load(Ordering::Acquire);
    if known != 0 {
        return known;
    }
    // By name, or by ordinal for an export without one (an ordinal passes as a pointer below 0x10000).
    let symbol = name.map_or(ordinal as usize as *const u8, |n| n.as_ptr().cast());
    let Some(function) = (unsafe { GetProcAddress(system_library(library), symbol) }) else {
        eprintln!("{library_name} forward: the system library lacks this export");
        std::process::abort();
    };
    slot.store(function as usize, Ordering::Release);
    function as usize
}

/// The variables naming a replacement for each library, read by `configure`.
static TARGETS: OnceLock<[Option<String>; 2]> = OnceLock::new();

/// Reads this library's variables once and removes them: the library it loads, which may be
/// another copy of it, must not see them.
fn configure() -> &'static [Option<String>; 2] {
    TARGETS.get_or_init(|| {
        census::configure();
        let targets = ["WC_D3D11", "WC_D3D12"].map(|name| std::env::var(name).ok());
        for name in ["WC_OUT", "WC_CAPTURE", "WC_D3D11", "WC_D3D12"] {
            unsafe { std::env::remove_var(name) };
        }
        targets
    })
}

/// The library `library`'s calls go to, loaded by its full path (by name alone, Windows would hand
/// back this one): the system's, or the replacement at the path in WC_D3D11 or WC_D3D12, so a
/// census and frame capture can sit in front of a replacement.
fn system_library(library: usize) -> HMODULE {
    static MODULES: [OnceLock<usize>; 2] = [OnceLock::new(), OnceLock::new()];
    let name = LIBRARIES[library];
    *MODULES[library].get_or_init(|| {
        let target = configure()[library].clone().ok_or(());
        let path = match target {
            Ok(path) => path,
            Err(_) => {
                let mut dir = [0u16; 260];
                let length =
                    unsafe { GetSystemDirectoryW(dir.as_mut_ptr(), dir.len() as u32) } as usize;
                assert!(length > 0 && length < dir.len(), "GetSystemDirectoryW");
                format!("{}\\{name}.dll", String::from_utf16_lossy(&dir[..length]))
            }
        };
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            eprintln!("{name} forward: cannot load {path}");
            std::process::abort();
        }
        module as usize
    }) as HMODULE
}
