//! The converters linked into the program, natively and in its own WebAssembly build (where no engine runs inside the
//! module to hold them): every function is called through the one signature of `cpp/boundary.h`. A converter that
//! ends here ends the process (natively, past what C++ exceptions catch) or the module.

use core::ffi::{c_int, c_void};

use wcontainers_shader_converters::{
    BUFFER_SLOTS, Buffers, CONVERTER_NAMES, ELEMENT_SIZES, Failure, Function, NO_BUFFER,
    SLOT_COUNT, SLOT_REASON, SLOT_REASON_LENGTH, SUCCEEDED, ShaderConverters,
    StandaloneShaderConverters,
};
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};

/// The boundary's one signature: `(data, size, flags, slots) -> status`.
type BoundaryFunction = unsafe extern "C" fn(*const u8, usize, u32, *mut usize) -> c_int;

#[expect(
    unsafe_code,
    reason = "the converters' C boundary, every function of which this crate calls"
)]
unsafe extern "C" {
    fn wcs_dxbc_to_ir(data: *const u8, size: usize, flags: u32, slots: *mut usize) -> c_int;
    fn wcs_dxil_to_spirv(data: *const u8, size: usize, flags: u32, slots: *mut usize) -> c_int;
    fn wcs_dxil_constant_sizes(
        data: *const u8,
        size: usize,
        flags: u32,
        slots: *mut usize,
    ) -> c_int;
    fn wcs_dxil_compared_gather_channels(
        data: *const u8,
        size: usize,
        flags: u32,
        slots: *mut usize,
    ) -> c_int;
    fn wcs_dxil_declared_registers(
        data: *const u8,
        size: usize,
        flags: u32,
        slots: *mut usize,
    ) -> c_int;
    fn wcs_dxil_views(data: *const u8, size: usize, flags: u32, slots: *mut usize) -> c_int;
    fn wcs_free(pointer: *mut c_void);
}

/// The converters linked into the program, which keep no state between calls.
pub struct LinkedShaderConverters;

impl StandaloneShaderConverters for LinkedShaderConverters {
    type Config = ();

    fn new(_config: ()) -> LinkedShaderConverters {
        LinkedShaderConverters
    }
}

impl ShaderConverters for LinkedShaderConverters {
    fn call(&self, function: Function, bytes: &[u8], flags: u32) -> Result<Buffers, Failure> {
        let mut slots = [0usize; SLOT_COUNT];
        #[expect(unsafe_code, reason = "the call into the converters' C boundary")]
        // SAFETY: `bytes` is readable for its length and `slots` writable for `SLOT_COUNT` words, as the boundary's
        // signature asks; the function writes only the slots.
        let status =
            unsafe { linked(function)(bytes.as_ptr(), bytes.len(), flags, slots.as_mut_ptr()) };
        let reason = given(slots[SLOT_REASON], slots[SLOT_REASON_LENGTH]);
        let sizes = ELEMENT_SIZES[function as usize];
        let outputs = core::array::from_fn(|i| match sizes[i] {
            NO_BUFFER => Vec::new(),
            size => {
                let [buffer, count] = BUFFER_SLOTS[i];
                given(slots[buffer], slots[count] * size)
            }
        });
        match status {
            SUCCEEDED => Ok(outputs),
            _ if slots[SLOT_REASON] == 0 => Err(Failure::Refused(no_reason(function))),
            _ => Err(Failure::Refused(
                String::from_utf8_lossy(&reason).into_owned(),
            )),
        }
    }
}

/// Returns the linked function that `function` names.
fn linked(function: Function) -> BoundaryFunction {
    match function {
        Function::DxbcToIr => wcs_dxbc_to_ir,
        Function::DxilToSpirv => wcs_dxil_to_spirv,
        Function::DxilConstantSizes => wcs_dxil_constant_sizes,
        Function::DxilComparedGatherChannels => wcs_dxil_compared_gather_channels,
        Function::DxilDeclaredRegisters => wcs_dxil_declared_registers,
        Function::DxilViews => wcs_dxil_views,
    }
}

/// Copies out the `length` bytes the boundary gave at the address `at`, then frees them; nothing for no address.
fn given(at: usize, length: usize) -> Vec<u8> {
    let at = core::ptr::with_exposed_provenance_mut::<u8>(at);
    if at.is_null() {
        return Vec::new();
    }
    #[expect(
        unsafe_code,
        reason = "the promise about a buffer the converters allocated"
    )]
    // SAFETY: the boundary allocated `length` bytes at `at` with malloc, which only this function reaches until it frees
    // them below, from this thread, and which no code forms a reference over.
    let config = unsafe { PointerAndLength::new(at, length) };
    let mut copied = vec![0; length];
    PointerBytes::new(config).read(&mut copied);
    #[expect(
        unsafe_code,
        reason = "the call that frees a buffer the converters allocated"
    )]
    // SAFETY: the boundary allocated `at` with malloc and it is freed here once, after its last read.
    unsafe {
        wcs_free(at.cast());
    }
    copied
}

/// Returns the reason given when the converter set none.
fn no_reason(function: Function) -> String {
    format!("{} gave no reason", CONVERTER_NAMES[function as usize])
}
