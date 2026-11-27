//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The system's d3d12.dll reached by a caller that writes no `unsafe` (this file forbids it): `SystemDirect3D12::system`
//! keeps the promise `new` asks for, so any crate under the workspace's `forbid` gets the system's Direct3D 12.
#![cfg(windows)]
#![forbid(unsafe_code)]

use wcontainers_component_object_model::constants::E_NOINTERFACE;
use wcontainers_component_object_model::structures::Guid;
use wcontainers_direct3d_12::constants::IID_ID3D12_DEVICE;
use wcontainers_direct3d_12::{Direct3D12, UnknownObject};
use wcontainers_direct3d_12_direct::SystemDirect3D12;
use wcontainers_direct3d_common::enumerations::D3D_FEATURE_LEVEL_11_0;

/// An interface identifier no interface has: COM's answer for an interface an object does not have.
const NO_INTERFACE: Guid = Guid {
    data1: 0xabab_abab,
    data2: 0xabab,
    data3: 0xabab,
    data4: [0xab; 8],
};

/// D3D12CreateDevice asked for an interface no device has answers E_NOINTERFACE, and the glue makes no object of it.
#[test]
fn an_interface_no_device_has_is_refused() {
    let system = SystemDirect3D12::system();
    let refused = system
        .create_device(None, D3D_FEATURE_LEVEL_11_0, &NO_INTERFACE)
        .err();
    assert_eq!(refused, Some(E_NOINTERFACE));
}

/// The default adapter (Microsoft's basic render driver where there is no other) at feature level 11_0 gives a device.
#[test]
fn an_honest_call_gives_a_device() {
    let system = SystemDirect3D12::system();
    let made = system.create_device(None, D3D_FEATURE_LEVEL_11_0, &IID_ID3D12_DEVICE);
    assert!(matches!(made, Ok(UnknownObject::Device(_))));
}
