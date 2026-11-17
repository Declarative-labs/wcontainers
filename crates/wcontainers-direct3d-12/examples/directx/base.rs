//! What every Direct3D 12 object of this library shares: private data (`ID3D12Object`) and, for the
//! device's children, a reference to the device (`ID3D12DeviceChild::GetDevice`).

use std::ffi::c_void;
use std::sync::Mutex;

use windows::Win32::Foundation::{E_INVALIDARG, E_POINTER};
use windows::Win32::Graphics::Direct3D::WKPDID_D3DDebugObjectNameW;
use windows::Win32::Graphics::Direct3D12::ID3D12Device;
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_MORE_DATA, DXGI_ERROR_NOT_FOUND};
use windows::core::{GUID, IUnknown, Interface, PCWSTR, Ref, Result, Weak};

/// The live device: Direct3D 12 hands out the same device for an adapter while it lives.
static DEVICE: Mutex<Option<WeakDevice>> = Mutex::new(None);

struct WeakDevice(Weak<ID3D12Device>);
// SAFETY: the weak reference is to a free-threaded object of this library.
unsafe impl Send for WeakDevice {}

/// The live device, if there is one.
pub fn live_device() -> Option<ID3D12Device> {
    DEVICE.lock().unwrap().as_ref().and_then(|w| w.0.upgrade())
}

/// Records `device` as the live one.
pub fn set_live_device(device: &ID3D12Device) {
    *DEVICE.lock().unwrap() = device.downgrade().ok().map(WeakDevice);
}

enum Data {
    Bytes(Vec<u8>),
    Object(IUnknown),
}

#[derive(Default)]
pub struct Base {
    data: Mutex<Vec<(GUID, Data)>>,
    device: Option<ID3D12Device>,
}

/// Objects with a [`Base`].
pub trait HasBase {
    fn base(&self) -> &Base;
}

impl Base {
    /// The base of a child of the live device, which it keeps alive.
    pub fn child() -> Self {
        Base {
            data: Mutex::default(),
            device: live_device(),
        }
    }

    pub fn GetDevice(&self, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        if riid.is_null() || out.is_null() {
            return Err(E_POINTER.into());
        }
        let device = self.device.as_ref().ok_or(E_INVALIDARG)?;
        // SAFETY: both pointers were checked.
        unsafe { device.query(riid, out).ok() }
    }

    pub fn GetPrivateData(
        &self,
        guid: *const GUID,
        size: *mut u32,
        out: *mut c_void,
    ) -> Result<()> {
        // SAFETY: the caller's pointers, checked for null.
        let (guid, size) = unsafe { (guid.as_ref(), size.as_mut()) };
        let (Some(guid), Some(size)) = (guid, size) else {
            return Err(E_INVALIDARG.into());
        };
        let data = self.data.lock().unwrap();
        let Some((_, value)) = data.iter().find(|(g, _)| g == guid) else {
            *size = 0;
            return Err(DXGI_ERROR_NOT_FOUND.into());
        };
        let bytes: &[u8] = match value {
            Data::Bytes(b) => b,
            Data::Object(o) => {
                // SAFETY: the pointer's own bytes.
                unsafe { std::slice::from_raw_parts((o as *const IUnknown).cast(), 8) }
            }
        };
        let needed = bytes.len() as u32;
        if out.is_null() {
            *size = needed;
            return Ok(());
        }
        if *size < needed {
            *size = needed;
            return Err(DXGI_ERROR_MORE_DATA.into());
        }
        *size = needed;
        if let Data::Object(o) = value {
            // The caller gets a reference of its own.
            std::mem::forget(o.clone());
        }
        // SAFETY: the caller's buffer holds at least `needed` bytes.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast(), bytes.len()) };
        Ok(())
    }

    pub fn SetPrivateData(&self, guid: *const GUID, size: u32, data: *const c_void) -> Result<()> {
        // SAFETY: the caller's GUID, checked for null.
        let guid = *unsafe { guid.as_ref() }.ok_or(E_INVALIDARG)?;
        let value = if data.is_null() || size == 0 {
            None
        } else {
            // SAFETY: the caller's `size` bytes.
            let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size as usize) };
            Some(Data::Bytes(bytes.to_vec()))
        };
        self.set(guid, value);
        Ok(())
    }

    pub fn SetPrivateDataInterface(&self, guid: *const GUID, object: Ref<IUnknown>) -> Result<()> {
        // SAFETY: the caller's GUID, checked for null.
        let guid = *unsafe { guid.as_ref() }.ok_or(E_INVALIDARG)?;
        self.set(guid, object.as_ref().map(|o| Data::Object(o.clone())));
        Ok(())
    }

    /// The name is the private data `WKPDID_D3DDebugObjectNameW`, as Direct3D 12 keeps it.
    pub fn SetName(&self, name: &PCWSTR) -> Result<()> {
        if name.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: a string ended by a zero.
        let wide = unsafe { name.as_wide() };
        let bytes: Vec<u8> = wide
            .iter()
            .chain(&[0])
            .flat_map(|c| c.to_le_bytes())
            .collect();
        self.set(WKPDID_D3DDebugObjectNameW, Some(Data::Bytes(bytes)));
        Ok(())
    }

    fn set(&self, guid: GUID, value: Option<Data>) {
        let mut data = self.data.lock().unwrap();
        data.retain(|(g, _)| *g != guid);
        if let Some(v) = value {
            data.push((guid, v));
        }
    }
}
