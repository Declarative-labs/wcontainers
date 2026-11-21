//! COM objects around the fronts, and the exported entry points. In this build every call runs on the
//! calling thread straight into the front, one call at a time under the library's lock; the call
//! buffer and device thread come with the channel.

mod cache;
mod chosen;
mod context;
mod convert;
pub mod counted;
mod d3d10;
mod d3d10core;
mod device;
mod device10;
mod dxgi;
mod factory;
mod generated;
mod objects;
mod output;
mod sharing;
mod swapchain;

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
    DXGI_SWAP_CHAIN_DESC, IDXGIDevice, IDXGIFactory, IDXGISwapChain,
};
use windows::core::{Error, GUID, HRESULT, IUnknown, Interface, OutRef, Ref, Result};

pub use context::Context;
pub use device::{Device, create_device};
pub use dxgi::Adapter;
pub use objects::*;

use context::{Context_Impl, DeviceContextState_Impl};
use device::Device_Impl;
use dxgi::Adapter_Impl;
use output::Output_Impl;
use swapchain::{Factory, Factory_Impl, SwapChain_Impl};

pub use chosen::{Chosen, FEATURES_OFF};
pub use factory::{
    D3D10_1_LIBRARY, D3D10_LIBRARY, D3D10CORE_LIBRARY, D3D11_LIBRARY, DXGI_LIBRARY, entry,
};
pub type Front = wcontainers_direct3d_11::Device<Chosen>;
pub type Shared = Rc<RefCell<Front>>;

/// Makes the back end, or none if there is no GPU to make it on.
pub fn backend() -> Option<Chosen> {
    let b = chosen::make()?;
    let _ = CAPS.set(wcontainers_gpu::Backend::caps(&b).clone());
    Some(b)
}

/// The caps of the first back end made.
static CAPS: std::sync::OnceLock<wcontainers_gpu::Caps> = std::sync::OnceLock::new();

/// The back end's caps, for factories made before any device: those of the first back end made, or
/// of one made for them and dropped while no other exists (two native WebGL 2 back ends on a thread
/// would share EGL's display and current context).
pub fn caps() -> Option<&'static wcontainers_gpu::Caps> {
    if CAPS.get().is_none() {
        drop(backend()?);
    }
    CAPS.get()
}

/// Held through every COM method and every drop of what the objects share, so a program's threads
/// (Direct3D 11's device is free-threaded) reach the front's single-threaded state one at a time. It
/// is re-entrant: a call may release objects, whose drops take it again.
static LOCK: ReentrantMutex<()> = ReentrantMutex::new(());

pub fn lock() -> ReentrantMutexGuard<'static, ()> {
    LOCK.lock()
}

/// Runs a creation whose object goes to `out`, which is cleared when it fails, as Direct3D clears it.
pub fn creating<T: windows::core::Type<T>>(
    out: OutRef<T>,
    make: impl FnOnce(OutRef<T>) -> Result<()>,
) -> Result<()> {
    // SAFETY: `OutRef` is a transparent pointer.
    let raw: *mut *mut c_void = unsafe { std::mem::transmute_copy(&out) };
    let r = make(out);
    if r.is_err() && !raw.is_null() {
        unsafe { *raw = std::ptr::null_mut() };
    }
    r
}

/// Gives back one hold of the library's lock whose guard was forgotten.
///
/// # Safety
/// The calling thread holds the lock through a forgotten guard.
pub unsafe fn unlock() {
    unsafe { LOCK.force_unlock() }
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
        // Reported as Direct3D's debug layer reports an invalid call.
        F::InvalidArg(why) => {
            eprintln!("wcontainers direct3d 11: invalid: {why}");
            E_INVALIDARG.into()
        }
        F::NotImplemented(_) => E_NOTIMPL.into(),
        F::Unsupported => DXGI_ERROR_UNSUPPORTED.into(),
        F::StillDrawing => DXGI_ERROR_WAS_STILL_DRAWING.into(),
        F::OutOfMemory => windows::Win32::Foundation::E_OUTOFMEMORY.into(),
        F::DeviceRemoved => DXGI_ERROR_DEVICE_REMOVED.into(),
        F::InvalidCall(_) => windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL.into(),
    }
}

/// One stored private datum: bytes, or an interface held with a reference (or none).
enum Datum {
    Bytes(Vec<u8>),
    Interface(Option<IUnknown>),
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
            Datum::Interface(i) => (i.as_ref().map_or(0, |i| i.as_raw() as usize))
                .to_ne_bytes()
                .to_vec(),
        };
        let room = unsafe { *size } as usize;
        unsafe { *size = bytes.len() as u32 };
        if data.is_null() {
            return Ok(());
        }
        if room < bytes.len() {
            return Err(DXGI_ERROR_MORE_DATA.into());
        }
        if let Datum::Interface(Some(i)) = datum {
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
        let had = store.len();
        store.retain(|(g, _)| *g != guid);
        if data.is_null() {
            // No data removes the entry; with none to remove Direct3D answers S_FALSE.
            if store.len() == had {
                return Err(windows::Win32::Foundation::S_FALSE.into());
            }
            return Ok(());
        }
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size as usize) };
        store.push((guid, Datum::Bytes(bytes.to_vec())));
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
        // A null interface is kept too, and read back as a null pointer.
        store.push((guid, Datum::Interface(object.as_ref().cloned())));
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
    /// The program released its last reference to this child, at `address`: under Direct3D 10's
    /// behaviour the device's immediate context unbinds it.
    pub fn released(&self, address: usize) {
        if let Ok(d) = self.device.cast_object_ref::<device::Device>() {
            d.unbind_released(address);
        }
    }

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

    /// DXGI's parent and device of a resource: its device, as interface `riid`.
    pub(crate) fn query_device(&self, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        unsafe { self.device.query(riid, out).ok() }
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

type CreateDevice = unsafe extern "system" fn(
    *mut c_void,
    i32,
    HMODULE,
    u32,
    *const D3D_FEATURE_LEVEL,
    u32,
    u32,
    *mut *mut c_void,
    *mut D3D_FEATURE_LEVEL,
    *mut *mut c_void,
) -> HRESULT;

type CreateDeviceAndSwapChain = unsafe extern "system" fn(
    *mut c_void,
    i32,
    HMODULE,
    u32,
    *const D3D_FEATURE_LEVEL,
    u32,
    u32,
    *const c_void,
    *mut *mut c_void,
    *mut *mut c_void,
    *mut D3D_FEATURE_LEVEL,
    *mut *mut c_void,
) -> HRESULT;

/// `D3D11CreateDevice`, as Microsoft documents it. Calls from this library, the back end's
/// libraries and the system's go on to the system's.
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
    sdk_version: u32,
    device: *mut *mut c_void,
    level: *mut D3D_FEATURE_LEVEL,
    context: *mut *mut c_void,
) -> HRESULT {
    if !factory::called_by_program() {
        // SAFETY: the system's export of this name has this type.
        return match unsafe {
            factory::system::<CreateDevice>(
                factory::D3D11_LIBRARY,
                factory::entry::D3D11_CREATE_DEVICE,
            )
        } {
            Some(f) => unsafe {
                f(
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
            },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    unsafe {
        create_for_program(
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

/// The feature levels Direct3D 11 knows, which a program's list may name.
const KNOWN_LEVELS: [windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL; 11] = {
    use windows::Win32::Graphics::Direct3D::*;
    [
        D3D_FEATURE_LEVEL_1_0_CORE,
        D3D_FEATURE_LEVEL_9_1,
        D3D_FEATURE_LEVEL_9_2,
        D3D_FEATURE_LEVEL_9_3,
        D3D_FEATURE_LEVEL_10_0,
        D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_12_0,
        D3D_FEATURE_LEVEL_12_1,
        D3D_FEATURE_LEVEL_12_2,
    ]
};

/// Warned when a device is asked of a driver other than the hardware one.
const NOT_HARDWARE: &str = "D3D11CreateDevice with a software, reference, WARP or null driver";

/// Warned when a device is asked to have a video device, which is not served.
const NO_VIDEO: &str = "D3D11CreateDevice with D3D11_CREATE_DEVICE_VIDEO_SUPPORT";

/// `D3D11CreateDevice` for the program.
///
/// # Safety
/// The pointers are what the caller of `D3D11CreateDevice` passes.
#[allow(clippy::too_many_arguments)]
unsafe fn create_for_program(
    adapter: *mut c_void,
    driver_type: i32,
    software: HMODULE,
    flags: u32,
    levels: *const D3D_FEATURE_LEVEL,
    level_count: u32,
    sdk_version: u32,
    device: *mut *mut c_void,
    level: *mut D3D_FEATURE_LEVEL,
    context: *mut *mut c_void,
) -> HRESULT {
    // Direct3D clears what it hands out before it checks anything.
    unsafe {
        if !level.is_null() {
            *level = D3D_FEATURE_LEVEL(0);
        }
        if !device.is_null() {
            *device = std::ptr::null_mut();
        }
        if !context.is_null() {
            *context = std::ptr::null_mut();
        }
    }
    // With an adapter the driver type is unknown; without one it is named. A software rasteriser's
    // module goes with the software driver alone.
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_SOFTWARE, D3D_DRIVER_TYPE_UNKNOWN,
    };
    if adapter.is_null() == (driver_type == D3D_DRIVER_TYPE_UNKNOWN.0)
        || software.is_invalid() == (driver_type == D3D_DRIVER_TYPE_SOFTWARE.0)
        || sdk_version != windows::Win32::Graphics::Direct3D11::D3D11_SDK_VERSION
    {
        return E_INVALIDARG;
    }
    if adapter.is_null() && driver_type != D3D_DRIVER_TYPE_HARDWARE.0 {
        warn_once(NOT_HARDWARE);
        return DXGI_ERROR_UNSUPPORTED;
    }
    if flags & windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_DEBUG.0 != 0 {
        return DXGI_ERROR_SDK_COMPONENT_MISSING;
    }
    // No video device is served, so a device that must have one is refused as Direct3D refuses it
    // on an adapter without video.
    if flags & windows::Win32::Graphics::Direct3D11::D3D11_CREATE_DEVICE_VIDEO_SUPPORT.0 != 0 {
        warn_once(NO_VIDEO);
        return DXGI_ERROR_UNSUPPORTED;
    }
    // No list is Direct3D's default list; an empty one names no level to make.
    let asked: Vec<u32> = if levels.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(levels, level_count as usize) }
            .iter()
            .map(|l| l.0 as u32)
            .collect()
    };
    if asked
        .iter()
        .any(|&l| !KNOWN_LEVELS.iter().any(|k| k.0 as u32 == l))
    {
        return E_INVALIDARG;
    }
    if !levels.is_null() && asked.is_empty() {
        return DXGI_ERROR_UNSUPPORTED;
    }
    let (d, c, l) = match create_device(&asked, flags, false) {
        Ok(made) => made,
        Err(e) => return e.code(),
    };
    unsafe {
        if !level.is_null() {
            *level = D3D_FEATURE_LEVEL(l as i32);
        }
        // Without either out pointer Direct3D only says it could make the device.
        if device.is_null() && context.is_null() {
            return windows::Win32::Foundation::S_FALSE;
        }
        if !context.is_null() {
            *context = c.into_raw();
        }
        if !device.is_null() {
            *device = d.into_raw();
        }
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
    if !factory::called_by_program() {
        // SAFETY: the system's export of this name has this type.
        return match unsafe {
            factory::system::<CreateDeviceAndSwapChain>(
                factory::D3D11_LIBRARY,
                factory::entry::D3D11_CREATE_DEVICE_AND_SWAP_CHAIN,
            )
        } {
            Some(f) => unsafe {
                f(
                    adapter,
                    driver_type,
                    software,
                    flags,
                    levels,
                    level_count,
                    sdk_version,
                    swap_chain_desc,
                    swap_chain,
                    device,
                    level,
                    context,
                )
            },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    if let Some(chain) = unsafe { swap_chain.as_mut() } {
        *chain = std::ptr::null_mut();
    }
    // Without a description Direct3D makes the device alone, whether a swap chain is asked for.
    let desc = unsafe { swap_chain_desc.cast::<DXGI_SWAP_CHAIN_DESC>().as_ref() };
    let Some(desc) = desc.filter(|_| !swap_chain.is_null()) else {
        return unsafe {
            create_for_program(
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
        };
    };
    // The device, then its swap chain from the factory of the device's adapter, as DXGI makes it.
    let mut made: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        create_for_program(
            adapter,
            driver_type,
            software,
            flags,
            levels,
            level_count,
            sdk_version,
            &mut made,
            level,
            std::ptr::null_mut(),
        )
    };
    if hr.is_err() {
        return hr;
    }
    // SAFETY: `D3D11CreateDevice` handed out this reference.
    let d = unsafe { ID3D11Device::from_raw(made) };
    let chain = (|| -> Result<IDXGISwapChain> {
        let factory: IDXGIFactory = unsafe { d.cast::<IDXGIDevice>()?.GetAdapter()?.GetParent()? };
        let mut chain = None;
        unsafe { factory.CreateSwapChain(&d, desc, &mut chain) }.ok()?;
        chain.ok_or_else(|| E_INVALIDARG.into())
    })();
    let chain = match chain {
        Ok(c) => c,
        Err(e) => return e.code(),
    };
    unsafe {
        *swap_chain = chain.into_raw();
        if !context.is_null() {
            *context = d
                .GetImmediateContext()
                .map_or(std::ptr::null_mut(), Interface::into_raw);
        }
        if !device.is_null() {
            *device = d.into_raw();
        }
    }
    windows::Win32::Foundation::S_OK
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
