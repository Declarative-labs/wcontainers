//! The system's libraries reached from outside their direct crates, with no `unsafe`: this test is the recorder's, a
//! crate under the workspace's `forbid(unsafe_code)`, and forbids it again. Each `System<Api>::system()` is the safe
//! way the implementations track asked for: the direct crate keeps the promise `new` asks for where it declares the
//! exports, so a crate of ours gets d3d11.dll, dxgi.dll and d3d12.dll by a plain call. Hostile arguments are refused by
//! the calling glue before the system sees them; honest calls get their objects. Windows only (AC9 runs it).
#![cfg(windows)]
#![forbid(unsafe_code)]

use wcontainers_component_object_model::constants::{E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL};
use wcontainers_component_object_model::structures::{Guid, Hmodule};
use wcontainers_direct3d_11::Direct3D11;
use wcontainers_direct3d_11::constants::D3D11_SDK_VERSION;
use wcontainers_direct3d_11::enumerations::CreateDeviceFlag;
use wcontainers_direct3d_11_direct::SystemDirect3D11;
use wcontainers_direct3d_12::Direct3D12;
use wcontainers_direct3d_12::constants::IID_ID3D12_DEVICE;
use wcontainers_direct3d_12_direct::SystemDirect3D12;
use wcontainers_direct3d_common::enumerations::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0};
use wcontainers_directx_graphics_infrastructure::DirectXGraphicsInfrastructure;
use wcontainers_directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1;
use wcontainers_directx_graphics_infrastructure::enumerations::CreateFactoryFlags;
use wcontainers_directx_graphics_infrastructure_direct::SystemDirectXGraphicsInfrastructure;

/// An interface identifier no interface has.
const NO_INTERFACE: Guid = Guid { data1: 0xabab_abab, data2: 0xabab, data3: 0xabab, data4: [0xab; 8] };

/// Direct3D 11: a software module, the switch-to-reference flag (0x2) and seventeen feature levels are refused with
/// E_INVALIDARG; WARP at 11_0 gives a device at 11_0.
#[test]
fn direct3d_11_from_outside_its_direct_crate() {
    let system = SystemDirect3D11::system();
    let level = [D3D_FEATURE_LEVEL_11_0];
    let refused = [
        system.create_device(None, D3D_DRIVER_TYPE_WARP, Hmodule(0x1000), CreateDeviceFlag(0), &level, D3D11_SDK_VERSION).err(),
        system.create_device(None, D3D_DRIVER_TYPE_WARP, Hmodule(0), CreateDeviceFlag(0x2), &level, D3D11_SDK_VERSION).err(),
        system.create_device(None, D3D_DRIVER_TYPE_WARP, Hmodule(0), CreateDeviceFlag(0), &[D3D_FEATURE_LEVEL_11_0; 17], D3D11_SDK_VERSION).err(),
    ];
    assert_eq!(refused, [Some(E_INVALIDARG); 3]);
    let made = system.create_device(None, D3D_DRIVER_TYPE_WARP, Hmodule(0), CreateDeviceFlag(0), &level, D3D11_SDK_VERSION);
    assert_eq!(made.ok().map(|(_, level, _)| level), Some(D3D_FEATURE_LEVEL_11_0));
}

/// DXGI: the debug flag (0x1) is refused with E_INVALIDARG; DXGIDisableVBlankVirtualization, which MinGW's dxgi.a does
/// not carry, answers E_NOTIMPL; CreateDXGIFactory1 gives a factory.
#[test]
fn dxgi_from_outside_its_direct_crate() {
    let system = SystemDirectXGraphicsInfrastructure::system();
    assert_eq!(system.create_dxgi_factory2(CreateFactoryFlags(0x1), &IID_IDXGI_FACTORY1).err(), Some(E_INVALIDARG));
    assert_eq!(system.dxgi_disable_v_blank_virtualization(), Err(E_NOTIMPL));
    assert!(matches!(system.create_dxgi_factory1(&IID_IDXGI_FACTORY1), Ok(wcontainers_directx_graphics_infrastructure::UnknownObject::Factory(_))));
}

/// Direct3D 12: an interface no device has answers E_NOINTERFACE; the default adapter at 11_0 gives a device.
#[test]
fn direct3d_12_from_outside_its_direct_crate() {
    let system = SystemDirect3D12::system();
    assert_eq!(system.create_device(None, D3D_FEATURE_LEVEL_11_0, &NO_INTERFACE).err(), Some(E_NOINTERFACE));
    assert!(matches!(system.create_device(None, D3D_FEATURE_LEVEL_11_0, &IID_ID3D12_DEVICE), Ok(wcontainers_direct3d_12::UnknownObject::Device(_))));
}
