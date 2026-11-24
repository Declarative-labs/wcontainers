//! A model of the kernel's side of the three calls, for Kani and Miri: it touches every byte the call lets the kernel
//! touch, and through the two addresses of an `MPTCP_FULL_INFO` request, every byte of the arrays they name.
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
        // the region.
        unsafe {
            let first = bytes.read();
            bytes.add(last).write(first);
        }
    }
    0
}

/// The byte offsets in `struct mptcp_full_info` of the user sizes, the count, and the two addresses.
const TCP_INFO_USER: usize = 4;
const SUBFLOW_INFO_USER: usize = 12;
const ARRAYS_USER: usize = 20;
const SUBFLOW_INFO: usize = 24;
const TCP_INFO: usize = 32;
/// `MPTCP_FULL_INFO`'s name.
const MPTCP_FULL_INFO: c_int = 4;

/// Writes the first and last byte of `count` elements of `size` bytes at the address, as the kernel fills an array.
///
/// # Safety
/// None; the model writes where the kernel would.
unsafe fn fill_array(address: u64, size: u32, count: u32) {
    let length = size as usize * count as usize;
    if let Some(last) = length.checked_sub(1) {
        let start = std::ptr::with_exposed_provenance_mut::<u8>(address as usize);
        // SAFETY: none; as above.
        unsafe {
            start.write(u8::MAX);
            start.add(last).write(u8::MAX);
        }
    }
}

/// Writes the first and last of the `*length` bytes, as far as the kernel may write, and a length no longer; for
/// `MPTCP_FULL_INFO`, also fills both arrays its request names.
pub unsafe fn getsockopt(
    _socket: c_int,
    _level: c_int,
    name: c_int,
    value: *mut c_void,
    length: *mut socklen_t,
) -> c_int {
    let bytes = value.cast::<u8>();
    // SAFETY: none; as above, for the length the caller gives.
    let given = unsafe { length.read() };
    if name == MPTCP_FULL_INFO && given as usize >= TCP_INFO + size_of::<u64>() {
        // SAFETY: none; as above.
        unsafe {
            let word = |at: usize| bytes.add(at).cast::<u32>().read_unaligned();
            let address = |at: usize| bytes.add(at).cast::<u64>().read_unaligned();
            let count = word(ARRAYS_USER);
            fill_array(address(SUBFLOW_INFO), word(SUBFLOW_INFO_USER), count);
            fill_array(address(TCP_INFO), word(TCP_INFO_USER), count);
        }
    }
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
pub unsafe fn ioctl(_socket: c_int, _request: libc::Ioctl, value: *mut c_void) -> c_int {
    // SAFETY: none; as above.
    unsafe { value.cast::<c_int>().write_unaligned(c_int::MAX) };
    0
}
