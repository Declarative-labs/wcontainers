//! The call into dxbc-spirv (C++, built by `build.rs`).

use crate::ConvertError;
use crate::ir::{Ir, code};
use alloc::string::String;
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn wcs_dxbc_to_ir(
        data: *const u8,
        size: usize,
        out: *mut *mut u8,
        out_size: *mut usize,
        sizes: *mut *mut u32,
        sizes_words: *mut usize,
        error: *mut *mut c_char,
    ) -> c_int;
    fn wcs_free(p: *mut c_void);
}

/// DXBC to dxbc-spirv's legalised IR.
pub fn to_ir(dxbc: &[u8]) -> Result<Ir, ConvertError> {
    let mut out = core::ptr::null_mut();
    let mut size = 0;
    let mut sizes = core::ptr::null_mut();
    let mut sizes_words = 0;
    let mut error = core::ptr::null_mut();
    // SAFETY: the pointers are valid for the call; the C side allocates `out`, `sizes` and `error`
    // with malloc.
    let status = unsafe {
        wcs_dxbc_to_ir(
            dxbc.as_ptr(),
            dxbc.len(),
            &mut out,
            &mut size,
            &mut sizes,
            &mut sizes_words,
            &mut error,
        )
    };
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
    // SAFETY: `out` holds `size` bytes and `sizes` holds `sizes_words` words written by the C side,
    // both freed right after.
    let ir = Ir::decode(unsafe { core::slice::from_raw_parts(out, size) });
    let declared: alloc::vec::Vec<u32> =
        unsafe { core::slice::from_raw_parts(sizes, sizes_words) }.to_vec();
    unsafe {
        wcs_free(out.cast());
        wcs_free(sizes.cast());
    }
    let mut ir = ir.ok_or_else(|| ConvertError::Malformed("the IR did not decode".into()))?;
    declared_sizes(&mut ir, &declared);
    ir.spaces = crate::container::shader_model(dxbc) >= Some(DIRECT3D_12_NUMBERING);
    Ok(ir)
}

/// The first shader model whose resources Direct3D 12 numbers by register space.
const DIRECT3D_12_NUMBERING: (u32, u32) = (5, 1);

/// The largest constant buffer dxbc-spirv declares for one it cannot size.
const MAX_CONSTANTS: u32 = 4096;

/// dxbc-spirv declares every dynamically indexed constant buffer with 4096 vectors; WebGPU then requires
/// a 64 KiB binding. The bytecode's own declared size, which dxbc-spirv's parser reads as (space,
/// register, size) triples in `declared`, is what a program binds at least, so it is used.
fn declared_sizes(ir: &mut Ir, declared: &[u32]) {
    for op in ir.ops.iter_mut().filter(|op| op.code == code::DCL_CBV) {
        let at = (op.lits.first().copied(), op.lits.get(1).copied());
        let size = declared
            .as_chunks::<3>()
            .0
            .iter()
            .find(|d| (Some(u64::from(d[0])), Some(u64::from(d[1]))) == at);
        if let (Some(&[_, _, size]), [dim]) = (size, &mut op.ty.dims[..])
            && *dim == MAX_CONSTANTS
            && size > 0
        {
            *dim = size.min(MAX_CONSTANTS);
        }
    }
}
