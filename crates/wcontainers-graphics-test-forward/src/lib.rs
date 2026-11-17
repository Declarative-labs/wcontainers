//! A stand-in d3d11.dll that writes the name of each call it receives to standard error and passes
//! the call, untouched, to the system's d3d11.dll. A program that loads it from its own directory
//! shows that a replacement placed beside a program is the one it uses.

#![cfg(windows)]

use std::ffi::CStr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

include!(concat!(env!("OUT_DIR"), "/exports.rs"));

/// The real function behind each export, once found.
static RESOLVED: [AtomicUsize; 512] = [const { AtomicUsize::new(0) }; 512];
const _: () = assert!(EXPORTS.len() <= RESOLVED.len());

/// Called by the thunk of export `index`: logs the call and returns the system function to jump to.
/// There is no caller to give an error to, so a missing library or function ends the process.
extern "C" fn resolve(index: u32) -> usize {
    let name = EXPORTS[index as usize];
    eprintln!("{LIBRARY} forward: {}", name.to_string_lossy());
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

/// The system's library, loaded by its full path: by name alone, Windows would hand back this one.
fn system_library() -> HMODULE {
    static MODULE: OnceLock<usize> = OnceLock::new();
    *MODULE.get_or_init(|| {
        let mut dir = [0u16; 260];
        let length = unsafe { GetSystemDirectoryW(dir.as_mut_ptr(), dir.len() as u32) } as usize;
        assert!(length > 0 && length < dir.len(), "GetSystemDirectoryW");
        let path: Vec<u16> = String::from_utf16_lossy(&dir[..length])
            .encode_utf16()
            .chain(format!("\\{LIBRARY}.dll").encode_utf16())
            .chain([0])
            .collect();
        let module = unsafe { LoadLibraryW(path.as_ptr()) };
        if module.is_null() {
            eprintln!("{LIBRARY} forward: cannot load the system {LIBRARY}.dll");
            std::process::abort();
        }
        module as usize
    }) as HMODULE
}
