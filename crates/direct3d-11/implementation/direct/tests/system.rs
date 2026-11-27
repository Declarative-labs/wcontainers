//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The system's d3d11.dll reached by a caller that writes no `unsafe` (this file forbids it): `SystemDirect3D11::system`
//! keeps the promise `new` asks for, so any crate under the workspace's `forbid` gets the system's Direct3D 11.
//! Hostile arguments are refused by the calling glue before the system sees them; an honest call gets a device.
#![cfg(windows)]
#![forbid(unsafe_code)]

use wcontainers_component_object_model::constants::E_INVALIDARG;
use wcontainers_component_object_model::structures::Hmodule;
use wcontainers_direct3d_11::Direct3D11;
use wcontainers_direct3d_11::constants::D3D11_SDK_VERSION;
use wcontainers_direct3d_11::enumerations::CreateDeviceFlag;
use wcontainers_direct3d_11_direct::SystemDirect3D11;
use wcontainers_direct3d_common::enumerations::{
    D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0, FeatureLevel,
};

/// One more feature level than the glue lets through (src/api.rs: at most 16).
const TOO_MANY_LEVELS: usize = 17;

/// A software rasterizer module (not null) is pinned to refusal: the system would load and run it.
#[test]
fn a_software_module_is_refused() {
    let system = SystemDirect3D11::system();
    let refused = system
        .create_device(
            None,
            D3D_DRIVER_TYPE_WARP,
            Hmodule(0x1000),
            CreateDeviceFlag(0),
            &[D3D_FEATURE_LEVEL_11_0],
            D3D11_SDK_VERSION,
        )
        .err();
    assert_eq!(refused, Some(E_INVALIDARG));
}

/// The switch-to-reference flag (0x2) and a flag no header names are refused.
#[test]
fn hostile_flags_are_refused() {
    let system = SystemDirect3D11::system();
    for flags in [0x2_u32, 0x8000_0000] {
        let refused = system
            .create_device(
                None,
                D3D_DRIVER_TYPE_WARP,
                Hmodule(0),
                CreateDeviceFlag(flags),
                &[D3D_FEATURE_LEVEL_11_0],
                D3D11_SDK_VERSION,
            )
            .err();
        assert_eq!(refused, Some(E_INVALIDARG), "flags {flags:#x}");
    }
}

/// A list of feature levels longer than the glue lets through is refused before the system reads it.
#[test]
fn too_many_feature_levels_are_refused() {
    let system = SystemDirect3D11::system();
    let levels = [D3D_FEATURE_LEVEL_11_0; TOO_MANY_LEVELS];
    let refused = system
        .create_device(
            None,
            D3D_DRIVER_TYPE_WARP,
            Hmodule(0),
            CreateDeviceFlag(0),
            &levels,
            D3D11_SDK_VERSION,
        )
        .err();
    assert_eq!(refused, Some(E_INVALIDARG));
}

/// WARP asked for feature level 11_0 alone gives a device at 11_0 (D3D11CreateDevice answers the first level of the
/// list the driver supports, and WARP supports 11_0).
#[test]
fn warp_gives_a_device_at_the_level_asked() {
    let system = SystemDirect3D11::system();
    let made = system.create_device(
        None,
        D3D_DRIVER_TYPE_WARP,
        Hmodule(0),
        CreateDeviceFlag(0),
        &[D3D_FEATURE_LEVEL_11_0],
        D3D11_SDK_VERSION,
    );
    let level: Option<FeatureLevel> = made.ok().map(|(_, level, _)| level);
    assert_eq!(level, Some(D3D_FEATURE_LEVEL_11_0));
}
