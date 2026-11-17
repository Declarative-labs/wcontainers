//! COM objects around the fronts, and the exported entry points. In this build every call runs on the
//! calling thread straight into the front, one call at a time under the library's lock; the call
//! buffer and device thread come with the channel.

mod context;
mod convert;
pub mod counted;
mod device;
mod dxgi;
mod generated;
mod objects;

use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Mutex;

use parking_lot::{ReentrantMutex, ReentrantMutexGuard};

use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, HMODULE};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_MORE_DATA, DXGI_ERROR_NOT_FOUND,
    DXGI_ERROR_SDK_COMPONENT_MISSING, DXGI_ERROR_UNSUPPORTED, DXGI_ERROR_WAS_STILL_DRAWING,
};
use windows::core::{Error, GUID, HRESULT, IUnknown, Interface, OutRef, Ref, Result};

pub use context::Context;
pub use device::{Device, create_device};
pub use dxgi::Adapter;
pub use objects::*;

use context::Context_Impl;
use device::Device_Impl;
use dxgi::Adapter_Impl;

/// The back end the library draws through.
pub type Chosen = wcontainers_gpu_webgpu::WebGpu;
pub type Front = wcontainers_direct3d_11::Device<Chosen>;
pub type Shared = Rc<RefCell<Front>>;

/// Makes the back end on the most capable GPU `wgpu` finds through Direct3D 12, or none if there is
/// none. Direct3D 12 is what browsers' WebGPU draws through on Windows, and it rasterises aliased
/// lines by Direct3D's rule, which `wgpu`'s Vulkan path does not.
pub fn backend() -> Option<Chosen> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&Chosen::device_descriptor(&adapter))).ok()?;
    Some(Chosen::new(&adapter, device, queue))
}

/// Held through every COM method and every drop of what the objects share, so a program's threads
/// (Direct3D 11's device is free-threaded) reach the front's single-threaded state one at a time. It
/// is re-entrant: a call may release objects, whose drops take it again.
static LOCK: ReentrantMutex<()> = ReentrantMutex::new(());

pub fn lock() -> ReentrantMutexGuard<'static, ()> {
    LOCK.lock()
}

/// Held through each method of the device and the context: the library's lock and, on leaving, the
/// front's new slow-path warnings shown.
pub struct Call<'a> {
    front: &'a Shared,
    _lock: ReentrantMutexGuard<'static, ()>,
}

impl<'a> Call<'a> {
    pub fn new(front: &'a Shared) -> Self {
        Call {
            _lock: lock(),
            front,
        }
    }
}

impl Drop for Call<'_> {
    fn drop(&mut self) {
        // A call made from inside another (a drop releasing objects) leaves the warnings to it.
        if let Ok(mut front) = self.front.try_borrow_mut() {
            for w in front.take_warnings() {
                eprintln!("wcontainers direct3d 11: slow path taken: {w}");
            }
        }
    }
}

/// A value shared between COM objects that is touched only under the library's lock, dropped
/// under it too (COM objects are released on any thread).
pub struct Locked<T>(ManuallyDrop<T>);

impl<T> Locked<T> {
    pub fn new(value: T) -> Self {
        Locked(ManuallyDrop::new(value))
    }
}

impl<T> Deref for Locked<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> Drop for Locked<T> {
    fn drop(&mut self) {
        let _lock = lock();
        unsafe { ManuallyDrop::drop(&mut self.0) }
    }
}

static WARNED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// Warns once per name that a call is not served.
fn warn_once(what: &'static str) {
    let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
    if !warned.contains(&what) {
        warned.push(what);
        eprintln!("wcontainers direct3d: not served: {what}");
    }
}

/// The answer of a COM method this library does not serve yet: `E_NOTIMPL`, warned.
pub fn missing(what: &'static str) -> Error {
    warn_once(what);
    E_NOTIMPL.into()
}

/// A method without a result this library does not serve yet, on an object that is not the device
/// or context: warned, and nothing else changes.
pub fn missing_void(what: &'static str) {
    warn_once(what);
}

/// What `GetDeviceRemovedReason` answers for a removal.
pub fn removal(r: Option<wcontainers_direct3d_11::Removal>) -> Result<()> {
    use wcontainers_direct3d_11::Removal;
    use wcontainers_gpu::LossReason;
    use windows::Win32::Graphics::Dxgi::{
        DXGI_ERROR_DEVICE_RESET, DXGI_ERROR_DRIVER_INTERNAL_ERROR,
    };
    match r {
        None => Ok(()),
        Some(Removal::Lost(LossReason::Reset)) => Err(DXGI_ERROR_DEVICE_RESET.into()),
        Some(Removal::Lost(LossReason::Removed)) => Err(DXGI_ERROR_DEVICE_REMOVED.into()),
        // A request the front cannot serve, an invalid command on the back end's API and memory the
        // back end ran out of are the driver's failures to the program (Direct3D 11 lists no memory
        // reason).
        Some(Removal::Unserved | Removal::Lost(LossReason::Error | LossReason::OutOfMemory)) => {
            Err(DXGI_ERROR_DRIVER_INTERNAL_ERROR.into())
        }
    }
}

/// The HRESULT of a front error.
pub fn hresult(e: wcontainers_direct3d_11::Error) -> Error {
    use wcontainers_direct3d_11::Error as F;
    match e {
        F::InvalidArg(_) => E_INVALIDARG.into(),
        F::NotImplemented(_) => E_NOTIMPL.into(),
        F::Unsupported => DXGI_ERROR_UNSUPPORTED.into(),
        F::StillDrawing => DXGI_ERROR_WAS_STILL_DRAWING.into(),
        F::OutOfMemory => windows::Win32::Foundation::E_OUTOFMEMORY.into(),
        F::DeviceRemoved => DXGI_ERROR_DEVICE_REMOVED.into(),
    }
}

/// One stored private datum: bytes, or an interface held with a reference.
enum Datum {
    Bytes(Vec<u8>),
    Interface(IUnknown),
}

/// Private data of an object (`SetPrivateData` and friends).
#[derive(Default)]
pub struct Private {
    data: RefCell<Vec<(GUID, Datum)>>,
}

impl Private {
    pub(crate) fn get_private_data(
        &self,
        guid: *const GUID,
        size: *mut u32,
        data: *mut c_void,
    ) -> Result<()> {
        if guid.is_null() || size.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let guid = unsafe { *guid };
        let store = self.data.borrow();
        let Some((_, datum)) = store.iter().find(|(g, _)| *g == guid) else {
            unsafe { *size = 0 };
            return Err(DXGI_ERROR_NOT_FOUND.into());
        };
        let bytes: Vec<u8> = match datum {
            Datum::Bytes(b) => b.clone(),
            Datum::Interface(i) => (i.as_raw() as usize).to_ne_bytes().to_vec(),
        };
        let room = unsafe { *size } as usize;
        unsafe { *size = bytes.len() as u32 };
        if data.is_null() {
            return Ok(());
        }
        if room < bytes.len() {
            return Err(DXGI_ERROR_MORE_DATA.into());
        }
        if let Datum::Interface(i) = datum {
            std::mem::forget(i.clone());
        }
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast(), bytes.len()) };
        Ok(())
    }

    pub(crate) fn set_private_data(
        &self,
        guid: *const GUID,
        size: u32,
        data: *const c_void,
    ) -> Result<()> {
        if guid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let guid = unsafe { *guid };
        let mut store = self.data.borrow_mut();
        store.retain(|(g, _)| *g != guid);
        if !data.is_null() {
            let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size as usize) };
            store.push((guid, Datum::Bytes(bytes.to_vec())));
        }
        Ok(())
    }

    pub(crate) fn set_private_data_interface(
        &self,
        guid: *const GUID,
        object: Ref<IUnknown>,
    ) -> Result<()> {
        if guid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let guid = unsafe { *guid };
        let mut store = self.data.borrow_mut();
        store.retain(|(g, _)| *g != guid);
        if let Some(object) = object.as_ref() {
            store.push((guid, Datum::Interface(object.clone())));
        }
        Ok(())
    }
}

/// What every device child has: its device and its private data. The device is held with one
/// reference while the program holds the child ([`counted`]); the context, which is not counted that
/// way, gives its reference back when it goes.
pub struct Child {
    device: ManuallyDrop<ID3D11Device>,
    pub private: Private,
}

impl Child {
    /// A child holding `device`'s reference.
    pub fn new(device: ID3D11Device) -> Self {
        Child {
            device: ManuallyDrop::new(device),
            private: Private::default(),
        }
    }

    pub fn device(&self) -> &ID3D11Device {
        &self.device
    }

    pub fn get_device(&self, out: OutRef<ID3D11Device>) {
        let _ = out.write(Some(self.device().clone()));
    }

    pub(crate) fn get_private_data(
        &self,
        guid: *const GUID,
        size: *mut u32,
        data: *mut c_void,
    ) -> Result<()> {
        self.private.get_private_data(guid, size, data)
    }

    pub(crate) fn set_private_data(
        &self,
        guid: *const GUID,
        size: u32,
        data: *const c_void,
    ) -> Result<()> {
        self.private.set_private_data(guid, size, data)
    }

    pub(crate) fn set_private_data_interface(
        &self,
        guid: *const GUID,
        object: Ref<IUnknown>,
    ) -> Result<()> {
        self.private.set_private_data_interface(guid, object)
    }

    /// The front device behind the child's device.
    pub fn front(&self) -> Shared {
        self.device
            .cast_object_ref::<Device>()
            .expect("a child's device is ours")
            .front
            .clone()
    }
}

/// `D3D11_CREATE_DEVICE_DEBUG`.
const CREATE_DEBUG: u32 = 0x2;
/// `D3D_DRIVER_TYPE_UNKNOWN` and `D3D_DRIVER_TYPE_HARDWARE`.
const DRIVER_UNKNOWN: i32 = 0;
const DRIVER_HARDWARE: i32 = 1;

/// `D3D11CreateDevice`, as Microsoft documents it.
///
/// # Safety
/// The pointers are what the caller of `D3D11CreateDevice` passes.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D11CreateDevice(
    adapter: *mut c_void,
    driver_type: i32,
    software: HMODULE,
    flags: u32,
    levels: *const D3D_FEATURE_LEVEL,
    level_count: u32,
    _sdk_version: u32,
    device: *mut *mut c_void,
    level: *mut D3D_FEATURE_LEVEL,
    context: *mut *mut c_void,
) -> HRESULT {
    // With an adapter the driver type is unknown; without one it is named.
    if !software.is_invalid() || adapter.is_null() == (driver_type == DRIVER_UNKNOWN) {
        return E_INVALIDARG;
    }
    if adapter.is_null() && driver_type != DRIVER_HARDWARE {
        warn_once("D3D11CreateDevice with a software, reference, WARP or null driver");
        return DXGI_ERROR_UNSUPPORTED;
    }
    if flags & CREATE_DEBUG != 0 {
        return DXGI_ERROR_SDK_COMPONENT_MISSING;
    }
    let asked: Vec<u32> = if levels.is_null() || level_count == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(levels, level_count as usize) }
            .iter()
            .map(|l| l.0 as u32)
            .collect()
    };
    let (d, c, l) = match create_device(&asked, flags) {
        Ok(made) => made,
        Err(e) => return e.code(),
    };
    unsafe {
        if !level.is_null() {
            *level = D3D_FEATURE_LEVEL(l as i32);
        }
        if !context.is_null() {
            *context = c.into_raw();
        }
        if device.is_null() {
            return windows::Win32::Foundation::S_FALSE;
        }
        *device = d.into_raw();
    }
    windows::Win32::Foundation::S_OK
}

/// `D3D11CreateDeviceAndSwapChain`, as Microsoft documents it: without a swap chain asked for, the
/// device as `D3D11CreateDevice` makes it.
///
/// # Safety
/// The pointers are what the caller of `D3D11CreateDeviceAndSwapChain` passes.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D11CreateDeviceAndSwapChain(
    adapter: *mut c_void,
    driver_type: i32,
    software: HMODULE,
    flags: u32,
    levels: *const D3D_FEATURE_LEVEL,
    level_count: u32,
    sdk_version: u32,
    swap_chain_desc: *const c_void,
    swap_chain: *mut *mut c_void,
    device: *mut *mut c_void,
    level: *mut D3D_FEATURE_LEVEL,
    context: *mut *mut c_void,
) -> HRESULT {
    if !swap_chain.is_null() {
        unsafe { *swap_chain = std::ptr::null_mut() };
        if swap_chain_desc.is_null() {
            return E_INVALIDARG;
        }
        return missing("D3D11CreateDeviceAndSwapChain with a swap chain").code();
    }
    unsafe {
        D3D11CreateDevice(
            adapter,
            driver_type,
            software,
            flags,
            levels,
            level_count,
            sdk_version,
            device,
            level,
            context,
        )
    }
}

/// `D3D11On12CreateDevice`: not served (no Direct3D 12 device of ours to layer on yet).
///
/// # Safety
/// Writes nothing; the arguments are not read.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D11On12CreateDevice(
    _device: *mut c_void,
    _flags: u32,
    _levels: *const D3D_FEATURE_LEVEL,
    _level_count: u32,
    _queues: *const *mut c_void,
    _queue_count: u32,
    _node_mask: u32,
    out_device: *mut *mut c_void,
    out_context: *mut *mut c_void,
    _level: *mut D3D_FEATURE_LEVEL,
) -> HRESULT {
    for out in [out_device, out_context] {
        if !out.is_null() {
            unsafe { *out = std::ptr::null_mut() };
        }
    }
    missing("D3D11On12CreateDevice").code()
}
