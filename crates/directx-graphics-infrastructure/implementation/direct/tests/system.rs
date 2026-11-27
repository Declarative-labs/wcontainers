//! Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.
//!
//! The system's dxgi.dll reached by a caller that writes no `unsafe` (this file forbids it):
//! `SystemDirectXGraphicsInfrastructure::system` keeps the promise `new` asks for, so any crate under the workspace's
//! `forbid` gets the system's DXGI. Hostile flags are refused by the calling glue; an honest call gets a factory.
#![cfg(windows)]
#![forbid(unsafe_code)]

use wcontainers_component_object_model::constants::E_INVALIDARG;
use wcontainers_directx_graphics_infrastructure::constants::IID_IDXGI_FACTORY1;
use wcontainers_directx_graphics_infrastructure::enumerations::CreateFactoryFlags;
use wcontainers_directx_graphics_infrastructure::{DirectXGraphicsInfrastructure, UnknownObject};
use wcontainers_directx_graphics_infrastructure_direct::SystemDirectXGraphicsInfrastructure;

/// The debug flag (0x1, src/api.rs: refused) and a flag no header names are refused before the system sees them.
#[test]
fn hostile_factory_flags_are_refused() {
    let system = SystemDirectXGraphicsInfrastructure::system();
    for flags in [0x1_u32, 0x8000_0000] {
        let refused = system
            .create_dxgi_factory2(CreateFactoryFlags(flags), &IID_IDXGI_FACTORY1)
            .err();
        assert_eq!(refused, Some(E_INVALIDARG), "flags {flags:#x}");
    }
}

/// CreateDXGIFactory1 asked for IDXGIFactory1 gives a factory.
#[test]
fn an_honest_call_gives_a_factory() {
    let system = SystemDirectXGraphicsInfrastructure::system();
    let made = system.create_dxgi_factory1(&IID_IDXGI_FACTORY1);
    assert!(matches!(made, Ok(UnknownObject::Factory(_))));
}
