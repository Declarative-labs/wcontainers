//! The call into dxbc-spirv (C++, built by `build.rs`).

use crate::ConvertError;
use crate::ir::Ir;
use alloc::string::String;
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn wcs_dxbc_to_ir(
        data: *const u8,
        size: usize,
        out: *mut *mut u8,
        out_size: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_free(p: *mut c_void);
}

/// DXBC to dxbc-spirv's legalised IR.
pub fn to_ir(dxbc: &[u8]) -> Result<Ir, ConvertError> {
    let mut out = core::ptr::null_mut();
    let mut size = 0;
    let mut error = core::ptr::null_mut();
    // SAFETY: the pointers are valid for the call; the C side allocates `out` and `error` with malloc.
    let status =
        unsafe { wcs_dxbc_to_ir(dxbc.as_ptr(), dxbc.len(), &mut out, &mut size, &mut error) };
    if status != 0 {
        let reason = if error.is_null() {
            String::from("dxbc-spirv gave no reason")
        } else {
            // SAFETY: a NUL-terminated string from the C side, freed right after.
            let s = unsafe { core::ffi::CStr::from_ptr(error) }
                .to_string_lossy()
                .into();
            unsafe { wcs_free(error.cast()) };
            s
        };
        return Err(ConvertError::Rejected(reason));
    }
    // SAFETY: `out` holds `size` bytes written by the C side, freed right after.
    let bytes = unsafe { core::slice::from_raw_parts(out, size) };
    let ir = Ir::decode(bytes);
    unsafe { wcs_free(out.cast()) };
    ir.ok_or_else(|| ConvertError::Malformed("the IR did not decode".into()))
}
