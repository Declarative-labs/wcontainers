//! d3d12.dll: forwards each export to wcontainers_directx.dll, with no code of its own.

#![cfg(windows)]

const EXPORTS: &str = " /EXPORT:D3D12CreateDevice=wcontainers_directx.wc_D3D12CreateDevice /EXPORT:D3D12GetDebugInterface=wcontainers_directx.wc_D3D12GetDebugInterface /EXPORT:D3D12SerializeRootSignature=wcontainers_directx.wc_D3D12SerializeRootSignature /EXPORT:D3D12SerializeVersionedRootSignature=wcontainers_directx.wc_D3D12SerializeVersionedRootSignature ";

/// Linker directives the linker reads from this section of the object file.
#[used]
#[unsafe(link_section = ".drectve")]
static DIRECTIVES: [u8; EXPORTS.len()] = {
    let mut out = [0; EXPORTS.len()];
    let bytes = EXPORTS.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        out[i] = bytes[i];
        i += 1;
    }
    out
};
