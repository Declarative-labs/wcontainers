//! The adapter a device reports (`IDXGIDevice::GetAdapter`). Adapter and output discovery belong to the
//! shared infrastructure crate; until it exists this names the back end the library was built with.

use std::ffi::c_void;

use windows::Win32::Foundation::{E_NOINTERFACE, LUID};
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{GUID, Result, implement};

use super::*;

/// What the adapter calls itself.
const NAME: &str = "wcontainers recording back end (draws nothing)";

#[implement(IDXGIAdapter1)]
pub struct Adapter {
    pub(super) private: Private,
}

impl Adapter {
    pub fn new() -> Self {
        Adapter {
            private: Private::default(),
        }
    }

    pub(super) fn get_desc1(&self) -> Result<DXGI_ADAPTER_DESC1> {
        let mut d = DXGI_ADAPTER_DESC1 {
            AdapterLuid: LUID::default(),
            Flags: DXGI_ADAPTER_FLAG_NONE.0 as u32,
            ..Default::default()
        };
        for (o, c) in d.Description.iter_mut().zip(NAME.encode_utf16()) {
            *o = c;
        }
        Ok(d)
    }

    pub(super) fn get_desc(&self) -> Result<DXGI_ADAPTER_DESC> {
        let d = self.get_desc1()?;
        Ok(DXGI_ADAPTER_DESC {
            Description: d.Description,
            VendorId: d.VendorId,
            DeviceId: d.DeviceId,
            SubSysId: d.SubSysId,
            Revision: d.Revision,
            DedicatedVideoMemory: d.DedicatedVideoMemory,
            DedicatedSystemMemory: d.DedicatedSystemMemory,
            SharedSystemMemory: d.SharedSystemMemory,
            AdapterLuid: d.AdapterLuid,
        })
    }

    /// No outputs: presenting comes with the infrastructure crate.
    pub(super) fn enum_outputs(&self, _output: u32) -> Result<IDXGIOutput> {
        Err(DXGI_ERROR_NOT_FOUND.into())
    }

    /// The factory is the infrastructure crate's, not made yet.
    pub(super) fn get_parent(&self, _riid: *const GUID, parent: *mut *mut c_void) -> Result<()> {
        if let Some(p) = unsafe { parent.as_mut() } {
            *p = std::ptr::null_mut();
        }
        missing_void("IDXGIAdapter::GetParent (the factory)");
        Err(E_NOINTERFACE.into())
    }
}

impl Default for Adapter {
    fn default() -> Self {
        Self::new()
    }
}
