//! The conversion alone, as a WebAssembly command, so the WASI imports it needs can be listed:
//! `cargo build -p wcontainers-shaders --target wasm32-wasip1 --example wasm_imports`, then
//! `wasm-tools print` on the result (the command's own start adds the imports of its arguments,
//! environment and exit).

use std::process::ExitCode;

/// A program for the conversion to run on, so that none of it is left out of the build.
const PROGRAM: &[u8] = include_bytes!("../tests/own/cs_atomics.dxbc");

/// Converts `PROGRAM` with the default variant; fails when it did not convert.
fn main() -> ExitCode {
    let variant = wcontainers_gpu::VariantKey::default();
    match wcontainers_shaders::convert(wcontainers_shaders::Source::Dxbc(PROGRAM), &variant, None) {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
