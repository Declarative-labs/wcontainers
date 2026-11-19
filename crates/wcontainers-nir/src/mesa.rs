//! NIR from the pinned Mesa, owned, with the steps the routes take on it (through `bridge/bridge.c`).

use crate::ConvertError;
use std::ffi::{CString, c_char};
use std::sync::Once;

/// The program stages the routes take (`enum wc_stage`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Fragment,
    Compute,
}

#[repr(C)]
struct NirShader {
    _opaque: [u8; 0],
}

#[repr(C)]
struct Bytes {
    data: *mut u8,
    size: usize,
}

#[repr(C)]
struct Words {
    data: *mut u32,
    count: usize,
}

unsafe extern "C" {
    fn wc_init();
    fn wc_spirv_to_nir(
        words: *const u32,
        count: usize,
        stage: Stage,
        entry_point: *const c_char,
    ) -> *mut NirShader;
    fn wc_nir_serialize(nir: *const NirShader, out: *mut Bytes) -> bool;
    fn wc_nir_to_spirv(nir: *mut NirShader, out: *mut Words) -> bool;
    fn wc_nir_free(nir: *mut NirShader);
    fn wc_bytes_free(bytes: *mut Bytes);
    fn wc_words_free(words: *mut Words);
}

/// One shader's NIR, freed on drop.
pub struct Nir(*mut NirShader);

/// Mesa's type tables, held for the life of the process (Mesa counts their references).
fn initialised() {
    static INIT: Once = Once::new();
    // SAFETY: wc_init only takes a reference on Mesa's process-wide type tables.
    INIT.call_once(|| unsafe { wc_init() });
}

impl Nir {
    /// The NIR of one entry point of a Vulkan SPIR-V module.
    pub fn from_spirv(words: &[u32], stage: Stage, entry_point: &str) -> Result<Nir, ConvertError> {
        initialised();
        let entry = CString::new(entry_point).map_err(|_| ConvertError::Mesa(SPIRV_TO_NIR))?;
        // SAFETY: the words and the name outlive the call; Mesa copies what it keeps.
        let nir = unsafe { wc_spirv_to_nir(words.as_ptr(), words.len(), stage, entry.as_ptr()) };
        if nir.is_null() {
            return Err(ConvertError::Mesa(SPIRV_TO_NIR));
        }
        Ok(Nir(nir))
    }

    /// NIR's serialised bytes, the cache's key with the converter's version (plan 15.3).
    pub fn serialized(&self) -> Result<Vec<u8>, ConvertError> {
        let mut bytes = Bytes {
            data: std::ptr::null_mut(),
            size: 0,
        };
        // SAFETY: self.0 is a live shader; the bridge fills `bytes` with memory it allocated.
        if !unsafe { wc_nir_serialize(self.0, &mut bytes) } {
            return Err(ConvertError::Mesa(NIR_SERIALIZE));
        }
        // SAFETY: data holds size bytes until wc_bytes_free.
        let copy = unsafe { std::slice::from_raw_parts(bytes.data, bytes.size) }.to_vec();
        // SAFETY: bytes came from the bridge and is freed once.
        unsafe { wc_bytes_free(&mut bytes) };
        Ok(copy)
    }

    /// Candidate A's first step: SPIR-V written by Zink's compiler (the shader is prepared for it
    /// in place, so it is consumed).
    pub fn into_spirv(self) -> Result<Vec<u32>, ConvertError> {
        let mut words = Words {
            data: std::ptr::null_mut(),
            count: 0,
        };
        // SAFETY: self.0 is a live shader; the bridge fills `words` with memory it allocated.
        if !unsafe { wc_nir_to_spirv(self.0, &mut words) } {
            return Err(ConvertError::Mesa(NIR_TO_SPIRV));
        }
        // SAFETY: data holds count words until wc_words_free.
        let copy = unsafe { std::slice::from_raw_parts(words.data, words.count) }.to_vec();
        // SAFETY: words came from the bridge and is freed once.
        unsafe { wc_words_free(&mut words) };
        Ok(copy)
    }
}

impl Drop for Nir {
    fn drop(&mut self) {
        // SAFETY: self.0 came from the bridge and is freed once.
        unsafe { wc_nir_free(self.0) };
    }
}

/// The steps named in `ConvertError::Mesa`.
pub const SPIRV_TO_NIR: &str = "spirv_to_nir";
pub const NIR_SERIALIZE: &str = "nir_serialize";
pub const NIR_TO_SPIRV: &str = "nir_to_spirv";
