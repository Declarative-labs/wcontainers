//! The adapter a device reports (`IDXGIDevice::GetAdapter`), described by the shared infrastructure
//! crate from the back end's caps.

use std::ffi::c_void;

use windows::Win32::Foundation::LUID;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{GUID, IUnknownImpl, Interface, Result, implement};

use wcontainers_directx_graphics_infrastructure::AdapterDesc;

use super::*;

#[implement(IDXGIAdapter1)]
pub struct Adapter {
    pub(super) private: Private,
    desc: AdapterDesc,
}

impl Adapter {
    pub fn new(caps: &wcontainers_gpu::Caps) -> Self {
        Adapter {
            private: Private::default(),
            desc: AdapterDesc::from_caps(caps),
        }
    }

    /// The shared infrastructure's description: its adapter identifier, and memory sizes of zero,
    /// since no browser API reports them.
    pub(super) fn get_desc1(&self) -> Result<DXGI_ADAPTER_DESC1> {
        let d = &self.desc;
        Ok(DXGI_ADAPTER_DESC1 {
            Description: d.description_utf16(),
            VendorId: d.vendor_id,
            DeviceId: d.device_id,
            SubSysId: d.sub_sys_id,
            Revision: d.revision,
            DedicatedVideoMemory: d.dedicated_video_memory as usize,
            DedicatedSystemMemory: d.dedicated_system_memory as usize,
            SharedSystemMemory: d.shared_system_memory as usize,
            AdapterLuid: LUID {
                LowPart: d.luid as u32,
                HighPart: (d.luid >> 32) as i32,
            },
            Flags: if d.software {
                DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32
            } else {
                0
            },
        })
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
}

impl Adapter_Impl {
    /// `CheckInterfaceSupport`, which DXGI answers for Direct3D 10 device interfaces with the user
    /// mode driver's version: this adapter makes none, and has no such driver.
    pub(super) fn check_interface_support(&self, _name: *const GUID) -> Result<i64> {
        Err(DXGI_ERROR_UNSUPPORTED.into())
    }

    /// The host's monitors, the primary first.
    pub(super) fn enum_outputs(&self, index: u32) -> Result<IDXGIOutput> {
        let (desc, monitor) = super::output::monitors()
            .into_iter()
            .nth(index as usize)
            .ok_or(DXGI_ERROR_NOT_FOUND)?;
        Ok(super::output::Output::new(desc, monitor, self.to_interface()).into())
    }

    /// A factory enumerating this adapter, which makes swap chains of its devices.
    pub(super) fn get_parent(&self, riid: *const GUID, parent: *mut *mut c_void) -> Result<()> {
        let factory: IDXGIFactory2 = Factory::new(self.to_interface()).into();
        unsafe { factory.query(riid, parent).ok() }
    }
}
