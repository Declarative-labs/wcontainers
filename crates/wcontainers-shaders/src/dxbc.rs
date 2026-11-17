//! The call into dxbc-spirv (C++, built by `build.rs`).

use crate::ConvertError;
use crate::ir::{Ir, code};
use alloc::string::String;
use alloc::vec::Vec;
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
    let mut ir = ir.ok_or_else(|| ConvertError::Malformed("the IR did not decode".into()))?;
    declared_sizes(&mut ir, &dynamic_constant_buffers(dxbc));
    Ok(ir)
}

/// The largest constant buffer dxbc-spirv declares for one it cannot size.
const MAX_CONSTANTS: u32 = 4096;

/// dxbc-spirv declares every dynamically indexed constant buffer with 4096 vectors; WebGPU then requires
/// a 64 KiB binding. The bytecode's own declared size is what a program binds at least, so it is used.
fn declared_sizes(ir: &mut Ir, declared: &[(u32, u32)]) {
    for op in ir.ops.iter_mut().filter(|op| op.code == code::DCL_CBV) {
        let register = op.lits.get(1).map(|&r| r as u32);
        let size = declared.iter().find(|d| Some(d.0) == register);
        if let (Some(&(_, size)), [dim]) = (size, &mut op.ty.dims[..])
            && *dim == MAX_CONSTANTS
            && size > 0
        {
            *dim = size.min(MAX_CONSTANTS);
        }
    }
}

/// The (register, size in vectors) of every dynamically indexed constant buffer the shader declares,
/// read from its SHDR or SHEX chunk.
fn dynamic_constant_buffers(container: &[u8]) -> Vec<(u32, u32)> {
    let word = |at: usize| {
        container
            .get(at..at + 4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    };
    let mut out = Vec::new();
    let chunks = word(28).unwrap_or(0) as usize;
    let Some(code) = (0..chunks).find_map(|i| {
        let at = word(32 + 4 * i)? as usize;
        matches!(container.get(at..at + 4), Some(b"SHDR" | b"SHEX")).then_some(at + 8)
    }) else {
        return out;
    };
    let Some(version) = word(code) else {
        return out;
    };
    let sm51 = (version >> 4) & 0xf == 5 && version & 0xf == 1;
    let end = code + 4 * word(code + 4).unwrap_or(0) as usize;
    let mut at = code + 8;
    while at < end {
        let Some(token) = word(at) else { break };
        let opcode = token & 0x7ff;
        let length = match opcode {
            // Custom data: its length is the next word.
            0x35 => word(at + 4).unwrap_or(0),
            _ => (token >> 24) & 0x7f,
        } as usize;
        if length == 0 {
            break;
        }
        // dcl_constantbuffer with the dynamically indexed access pattern.
        if opcode == 0x59 && token & 0x800 != 0 {
            let operand = word(at + 4).unwrap_or(0);
            let first = at + 8 + 4 * (operand >> 31) as usize;
            let entry = if sm51 {
                // [id][lower][upper], then the size.
                (word(first + 4), word(first + 12))
            } else {
                // [register][size].
                (word(first), word(first + 4))
            };
            if let (Some(register), Some(size)) = entry {
                out.push((register, size));
            }
        }
        at += 4 * length;
    }
    out
}
