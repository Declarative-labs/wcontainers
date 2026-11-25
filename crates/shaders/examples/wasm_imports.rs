//! The conversion alone, as a WebAssembly library, so the WASI imports it needs can be listed:
//! `cargo build -p wcontainers-shaders --target wasm32-wasip1 --example wasm_imports`, then
//! `wasm-tools print` on the result.

/// Converts the DXBC at `ptr` (`len` bytes) with the default variant; returns 1 when it converted.
///
/// # Safety
/// `ptr` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn convert_dxbc(ptr: *const u8, len: usize) -> u32 {
    // SAFETY: the caller's promise.
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    let variant = wcontainers_gpu::VariantKey::default();
    wcontainers_shaders::convert(wcontainers_shaders::Source::Dxbc(bytes), &variant, None).is_ok()
        as u32
}
