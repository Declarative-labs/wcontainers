//! The converters linked into the crate's own WebAssembly build, where no engine runs inside the
//! module to hold them: every entry is called through the one signature of `cpp/boundary.h`.

use super::{BUFFER_SLOTS, Entry, Failure, exports, no_reason, slot};
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{c_int, c_void};

/// The boundary's one signature: `(data, size, samples, slots) -> status`.
type Function = unsafe extern "C" fn(*const u8, usize, u32, *mut usize) -> c_int;

unsafe extern "C" {
    fn wcs_dxbc_to_ir(data: *const u8, size: usize, samples: u32, slots: *mut usize) -> c_int;
    fn wcs_dxil_to_spirv(data: *const u8, size: usize, samples: u32, slots: *mut usize) -> c_int;
    fn wcs_dxil_constant_sizes(
        data: *const u8,
        size: usize,
        samples: u32,
        slots: *mut usize,
    ) -> c_int;
    fn wcs_dxil_compared_gather_channels(
        data: *const u8,
        size: usize,
        samples: u32,
        slots: *mut usize,
    ) -> c_int;
    fn wcs_dxil_declared_registers(
        data: *const u8,
        size: usize,
        samples: u32,
        slots: *mut usize,
    ) -> c_int;
    fn wcs_dxil_views(data: *const u8, size: usize, samples: u32, slots: *mut usize) -> c_int;
    fn wcs_free(p: *mut c_void);
}

/// The linked function an entry names.
fn function<const BUFFERS: usize>(entry: &Entry<BUFFERS>) -> Function {
    match entry.name {
        exports::DXBC_TO_IR => wcs_dxbc_to_ir,
        exports::DXIL_TO_SPIRV => wcs_dxil_to_spirv,
        exports::DXIL_CONSTANT_SIZES => wcs_dxil_constant_sizes,
        exports::DXIL_COMPARED_GATHER_CHANNELS => wcs_dxil_compared_gather_channels,
        exports::DXIL_DECLARED_REGISTERS => wcs_dxil_declared_registers,
        exports::DXIL_VIEWS => wcs_dxil_views,
        name => unreachable!("no linked function {name}"),
    }
}

pub(super) fn call<const BUFFERS: usize>(
    entry: &Entry<BUFFERS>,
    data: &[u8],
    samples: u32,
) -> Result<[Vec<u8>; BUFFERS], Failure> {
    let mut slots = [0usize; slot::COUNT];
    // SAFETY: `data` is readable for its length and `slots` writable for `slot::COUNT` words, as the
    // boundary's signature asks; the function writes only the slots.
    let status = unsafe { function(entry)(data.as_ptr(), data.len(), samples, slots.as_mut_ptr()) };
    // Every slot's buffer and reason, copied out, then freed.
    let copy = |at: usize, bytes: usize| -> Vec<u8> {
        let at = core::ptr::with_exposed_provenance_mut::<u8>(slots[at]);
        if at.is_null() {
            return Vec::new();
        }
        // SAFETY: the boundary allocated `bytes` bytes at `at` with malloc, freed here once.
        unsafe {
            let copied = core::slice::from_raw_parts(at, bytes).to_vec();
            wcs_free(at.cast());
            copied
        }
    };
    let reason = copy(slot::REASON, slots[slot::REASON_LENGTH]);
    let outputs = core::array::from_fn(|i| {
        let [buffer, count] = BUFFER_SLOTS[i];
        copy(buffer, slots[count] * entry.outputs[i].bytes())
    });
    match status {
        0 => Ok(outputs),
        _ if slots[slot::REASON] == 0 => Err(Failure::Refused(no_reason(entry))),
        _ => Err(Failure::Refused(
            String::from_utf8_lossy(&reason).into_owned(),
        )),
    }
}
