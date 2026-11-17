//! dxgi.dll: forwards each export to our d3d12.dll, which holds DXGI too, with no code of its own.

#![cfg(windows)]

const EXPORTS: &str = " /EXPORT:CreateDXGIFactory=d3d12.wc_CreateDXGIFactory /EXPORT:CreateDXGIFactory1=d3d12.wc_CreateDXGIFactory1 /EXPORT:CreateDXGIFactory2=d3d12.wc_CreateDXGIFactory2 ";

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
