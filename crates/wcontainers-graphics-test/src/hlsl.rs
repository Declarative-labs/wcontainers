//! HLSL to DXBC through the system's d3dcompiler_47.dll, for both Direct3D renderers.

use std::ffi::CString;

use anyhow::{Result, anyhow};
use windows::Win32::Graphics::Direct3D::Fxc::{
    D3DCOMPILE_ENABLE_STRICTNESS, D3DCOMPILE_OPTIMIZATION_LEVEL3, D3DCompile,
};
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::core::PCSTR;

/// Compiles one entry point of `source` for a shader model target such as `vs_5_0`.
pub fn compile(source: &str, entry: &str, target: &str) -> Result<Vec<u8>> {
    let entry_c = CString::new(entry)?;
    let target_c = CString::new(target)?;
    let mut code = None;
    let mut errors = None;
    let compiled = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR::null(),
            None,
            None::<&windows::Win32::Graphics::Direct3D::ID3DInclude>,
            PCSTR(entry_c.as_ptr().cast()),
            PCSTR(target_c.as_ptr().cast()),
            D3DCOMPILE_ENABLE_STRICTNESS | D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    match (compiled, code) {
        (Ok(()), Some(code)) => Ok(blob_bytes(&code).to_vec()),
        (result, _) => {
            let message = errors.map(|e| String::from_utf8_lossy(blob_bytes(&e)).into_owned());
            Err(anyhow!(
                "compiling {entry} for {target}: {result:?} {}",
                message.unwrap_or_default()
            ))
        }
    }
}

pub fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}
