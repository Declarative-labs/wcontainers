//! d3d12.dll: forwards each export to wcontainers_d3d12.dll under the system library's ordinals,
//! with no code of its own. Both forwarders name the library by its own name, so a library standing
//! in front of either (a capture layer loading ours behind it) leaves one copy of ours in the process.

#![cfg(windows)]

#[macro_use]
#[path = "directx/exports.rs"]
mod exports;

linker_directives!(
    " /EXPORT:D3D12CreateDevice=wcontainers_d3d12.wc_D3D12CreateDevice,@101 /EXPORT:D3D12GetDebugInterface=wcontainers_d3d12.wc_D3D12GetDebugInterface,@102 /EXPORT:D3D12SerializeRootSignature=wcontainers_d3d12.wc_D3D12SerializeRootSignature,@116 /EXPORT:D3D12SerializeVersionedRootSignature=wcontainers_d3d12.wc_D3D12SerializeVersionedRootSignature,@117 /EXPORT:D3D12CreateRootSignatureDeserializer=wcontainers_d3d12.wc_D3D12CreateRootSignatureDeserializer,@107 /EXPORT:D3D12CreateVersionedRootSignatureDeserializer=wcontainers_d3d12.wc_D3D12CreateVersionedRootSignatureDeserializer,@108 "
);
