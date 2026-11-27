//! dxgi.dll: forwards each export to wcontainers_d3d12.dll, which holds DXGI too, with no code of
//! its own.

#![cfg(windows)]

#[macro_use]
#[path = "directx/exports.rs"]
mod exports;

linker_directives!(
    " /EXPORT:CreateDXGIFactory=wcontainers_d3d12.wc_CreateDXGIFactory /EXPORT:CreateDXGIFactory1=wcontainers_d3d12.wc_CreateDXGIFactory1 /EXPORT:CreateDXGIFactory2=wcontainers_d3d12.wc_CreateDXGIFactory2 "
);
