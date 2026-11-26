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
    /// The adapter of a factory of DXGI 1.0 (Direct3D 10's devices made without an adapter),
    /// which does not answer `IDXGIFactory1`.
    dxgi10: bool,
    /// The factory that enumerated this adapter, its parent; none for a device's adapter.
    parent: Option<IDXGIFactory6>,
}

impl Adapter {
    pub fn new(caps: &wcontainers_gpu::caps::Caps, dxgi10: bool) -> Self {
        Adapter {
            private: Private::default(),
            desc: AdapterDesc::from_caps(caps),
            dxgi10,
            parent: None,
        }
    }

    /// Another object for the same adapter, as each enumeration of DXGI's gives, whose parent is
    /// the enumerating `factory`.
    pub(super) fn another(&self, factory: IDXGIFactory6) -> Self {
        Adapter {
            private: Private::default(),
            desc: self.desc.clone(),
            dxgi10: self.dxgi10,
            parent: Some(factory),
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
                HighPart: (d.luid >> u32::BITS) as i32,
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

    /// The factory that enumerated this adapter, or for a device's adapter a new factory
    /// enumerating it, which makes swap chains of its devices.
    pub(super) fn get_parent(&self, riid: *const GUID, parent: *mut *mut c_void) -> Result<()> {
        if let Some(factory) = &self.parent {
            return unsafe { factory.query(riid, parent).ok() };
        }
        let factory: IDXGIFactory6 = Factory::new(self.to_interface()).into();
        if self.dxgi10 {
            as_dxgi10(&factory)?;
        }
        unsafe { factory.query(riid, parent).ok() }
    }
}

/// Gives `factory` DXGI 1.0's form. Microsoft's page on `CreateDXGIFactory`: DXGI 1.0's factory
/// does not give `IDXGIFactory1` (DXGI 1.0 and 1.1 are not mixed); the later interfaces it gives.
pub(super) fn as_dxgi10(factory: &IDXGIFactory6) -> Result<()> {
    // SAFETY: both are interface pointers of our factory, of the tables named.
    unsafe {
        restrict::<IDXGIFactory6_Vtbl, 0>(factory.as_raw());
        restrict::<windows::core::IInspectable_Vtbl, 1>(
            factory.cast::<windows::core::IUnknown>()?.as_raw(),
        );
    }
    Ok(())
}

type QueryInterface =
    unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> windows::core::HRESULT;
/// The windows crate's `QueryInterface` entries of a factory's two tables.
static QUERY_INTERFACE: [std::sync::OnceLock<QueryInterface>; FACTORY_TABLES] =
    [const { std::sync::OnceLock::new() }; FACTORY_TABLES];
/// A factory's tables: `IDXGIFactory6`'s and its identity's.
const FACTORY_TABLES: usize = 2;

/// Points interface pointer `p`'s table at a copy (made once, for table `N`) whose
/// `QueryInterface` answers as DXGI 1.0's factory (`query_factory10`).
///
/// # Safety
/// `p` is an interface pointer of a factory of ours, and `V` its table's type.
unsafe fn restrict<V: 'static, const N: usize>(p: *mut c_void) {
    static TABLES: [std::sync::OnceLock<usize>; FACTORY_TABLES] =
        [const { std::sync::OnceLock::new() }; FACTORY_TABLES];
    let slot = p as *mut *const V;
    let copy = *TABLES[N].get_or_init(|| {
        // SAFETY: the windows crate's table, copied bit for bit; every table starts with
        // IUnknown's.
        let mut table = unsafe { std::ptr::read(*slot) };
        let unknown =
            unsafe { &mut *(&mut table as *mut V).cast::<windows::core::IUnknown_Vtbl>() };
        let _ = QUERY_INTERFACE[N].set(unknown.QueryInterface);
        unknown.QueryInterface = query_factory10::<N>;
        Box::leak(Box::new(table)) as *const V as usize
    });
    unsafe { *slot = copy as *const V };
}

/// `QueryInterface` of DXGI 1.0's factory: `IDXGIFactory1` is not answered.
unsafe extern "system" fn query_factory10<const N: usize>(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> windows::core::HRESULT {
    if !iid.is_null() && unsafe { *iid } == IDXGIFactory1::IID {
        if !out.is_null() {
            unsafe { *out = std::ptr::null_mut() };
        }
        return windows::Win32::Foundation::E_NOINTERFACE;
    }
    let entry = QUERY_INTERFACE[N].get().expect("the table was patched");
    unsafe { entry(this, iid, out) }
}
