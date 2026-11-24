//! A model of the kernel's side of the three calls, for Kani and Miri: it touches every byte the call lets the kernel touch.
#![expect(
    unsafe_code,
    reason = "the model stands for the kernel, reading and writing through the pointers the calls give it"
)]

use std::ffi::{c_int, c_void};

use libc::socklen_t;

/// Reads the first and last of the `length` bytes, as far as the kernel may read, and writes the last back, as
/// `IPV6_FLOWLABEL_MGR` does.
pub unsafe fn setsockopt(
    _socket: c_int,
    _level: c_int,
    _name: c_int,
    value: *const c_void,
    length: socklen_t,
) -> c_int {
    let bytes = value.cast::<u8>().cast_mut();
    if let Some(last) = (length as usize).checked_sub(1) {
        // SAFETY: none; the model reads and writes where the kernel may, and Kani or Miri reports an access outside
        // the value or a write into bytes the caller only shares.
        unsafe {
            let first = bytes.read();
            bytes.add(last).write(first);
        }
    }
    0
}

/// Writes the first and last of the `*length` bytes, as far as the kernel may write, and a length no longer.
pub unsafe fn getsockopt(
    _socket: c_int,
    _level: c_int,
    _name: c_int,
    value: *mut c_void,
    length: *mut socklen_t,
) -> c_int {
    let bytes = value.cast::<u8>();
    // SAFETY: none; as above, for the length the caller gives.
    let given = unsafe { length.read() };
    if let Some(last) = (given as usize).checked_sub(1) {
        // SAFETY: none; as above.
        unsafe {
            bytes.write(u8::MAX);
            bytes.add(last).write(u8::MAX);
        }
    }
    #[cfg(kani)]
    let written = {
        let written: socklen_t = kani::any();
        kani::assume(written <= given);
        written
    };
    #[cfg(not(kani))]
    let written = given;
    // SAFETY: none; as above.
    unsafe { length.write(written) };
    0
}

/// Writes the one int every request the crate makes gives.
pub unsafe fn ioctl(_socket: c_int, _request: libc::Ioctl, value: *mut c_int) -> c_int {
    // SAFETY: none; as above.
    unsafe { value.write(c_int::MAX) };
    0
}
