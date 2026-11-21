//! d3d10core.dll's entry points: Direct3D 10's device, made as a Direct3D 11 device whose
//! immediate context starts with a state emulating Direct3D 10 active. Calls from this library,
//! the back end's libraries and the system's go on to the system's, as `D3D11CreateDevice`'s do,
//! but those of the system's Direct3D 10 runtime (d3d10.dll, d3d10_1.dll), which asks on the
//! program's behalf.

use std::ffi::c_void;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_0};
use windows::Win32::Graphics::Direct3D10::{
    D3D10_CREATE_DEVICE_DEBUG, D3D10_CREATE_DEVICE_SINGLETHREADED, D3D10_DRIVER_TYPE_HARDWARE,
    D3D10_SDK_VERSION, ID3D10Device, ID3D10Multithread,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_SDK_VERSION, ID3D11Device1, ID3D11DeviceContext1, ID3DDeviceContextState,
};
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_UNSUPPORTED, IDXGIFactory1};
use windows::core::{HRESULT, Interface};

use super::{factory, lock};

type CreateDevice = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    u32,
    D3D_FEATURE_LEVEL,
    *mut *mut c_void,
) -> HRESULT;
type GetVersion = unsafe extern "system" fn() -> u32;

/// `D3D10CoreCreateDevice`: a device at `level` in Direct3D 10's behaviour, as `ID3D10Device`.
/// The factory and adapter name the system's; the device is made on ours, as
/// `D3D11CreateDevice` with an adapter is.
///
/// # Safety
/// The pointers are what the caller of `D3D10CoreCreateDevice` passes.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D10CoreCreateDevice(
    factory: *mut c_void,
    adapter: *mut c_void,
    flags: u32,
    level: D3D_FEATURE_LEVEL,
    out: *mut *mut c_void,
) -> HRESULT {
    // Direct3D 10's runtime makes its devices here for the program.
    if !factory::called_by_program_or(&[factory::D3D10_LIBRARY, factory::D3D10_1_LIBRARY]) {
        // SAFETY: the system's export of this name has this type.
        return match unsafe {
            factory::system::<CreateDevice>(
                factory::D3D10CORE_LIBRARY,
                factory::entry::D3D10_CORE_CREATE_DEVICE,
            )
        } {
            Some(f) => unsafe { f(factory, adapter, flags, level, out) },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    if out.is_null() {
        return windows::Win32::Foundation::E_INVALIDARG;
    }
    unsafe { *out = std::ptr::null_mut() };
    // The factory the system's runtime made: one of DXGI 1.0 answers no later interface.
    let dxgi10 = unsafe { windows::core::IUnknown::from_raw_borrowed(&factory) }
        .is_none_or(|f| f.cast::<IDXGIFactory1>().is_err());
    match make(flags, level, dxgi10) {
        Ok(device) => {
            unsafe { *out = device.into_raw() };
            windows::Win32::Foundation::S_OK
        }
        Err(e) => e.code(),
    }
}

/// The device; `dxgi10` gives it the adapter of a factory of DXGI 1.0.
fn make(flags: u32, level: D3D_FEATURE_LEVEL, dxgi10: bool) -> windows::core::Result<ID3D10Device> {
    let (device, context, _) = {
        let _lock = lock();
        super::create_device(&[level.0 as u32], flags, dxgi10)?
    };
    let device1: ID3D11Device1 = device.cast()?;
    let context1: ID3D11DeviceContext1 = context.cast()?;
    let mut state: Option<ID3DDeviceContextState> = None;
    unsafe {
        device1.CreateDeviceContextState(
            0,
            &[level],
            D3D11_SDK_VERSION,
            &ID3D10Device::IID,
            None,
            Some(&mut state),
        )?;
        context1.SwapDeviceContextState(state.as_ref(), None);
        // Direct3D 10's device serialises its calls unless made single-threaded.
        if flags & D3D10_CREATE_DEVICE_SINGLETHREADED.0 as u32 == 0 {
            // The answer is the earlier setting, off on a new device.
            let _ = device
                .cast::<ID3D10Multithread>()?
                .SetMultithreadProtected(true);
        }
    }
    device.cast()
}

/// `D3D10CreateDevice` (d3d10.dll's), as Microsoft documents it: a device at 10_0 on the hardware
/// driver. Calls from this library, the back end's libraries and the system's go on to the
/// system's.
///
/// # Safety
/// The pointers are what the caller of `D3D10CreateDevice` passes.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D10CreateDevice(
    adapter: *mut c_void,
    driver_type: i32,
    software: HMODULE,
    flags: u32,
    sdk_version: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !factory::called_by_program() {
        // SAFETY: the system's export of this name has this type.
        return match unsafe {
            factory::system::<CreateDevice10>(
                factory::D3D10_LIBRARY,
                factory::entry::D3D10_CREATE_DEVICE,
            )
        } {
            Some(f) => unsafe { f(adapter, driver_type, software, flags, sdk_version, out) },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    unsafe {
        create10(
            adapter.is_null(),
            driver_type,
            flags,
            sdk_version == D3D10_SDK_VERSION,
            D3D_FEATURE_LEVEL_10_0,
            out,
        )
    }
}

/// `D3D10CreateDevice1` (d3d10_1.dll's), as Microsoft documents it: a device at `level` on the
/// hardware driver, handed out as `ID3D10Device1`.
///
/// # Safety
/// The pointers are what the caller of `D3D10CreateDevice1` passes.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D10CreateDevice1(
    adapter: *mut c_void,
    driver_type: i32,
    software: HMODULE,
    flags: u32,
    level: D3D_FEATURE_LEVEL,
    sdk_version: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    if !factory::called_by_program() {
        // SAFETY: the system's export of this name has this type.
        return match unsafe {
            factory::system::<CreateDevice101>(
                factory::D3D10_1_LIBRARY,
                factory::entry::D3D10_CREATE_DEVICE1,
            )
        } {
            Some(f) => unsafe {
                f(
                    adapter,
                    driver_type,
                    software,
                    flags,
                    level,
                    sdk_version,
                    out,
                )
            },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    unsafe {
        create10(
            adapter.is_null(),
            driver_type,
            flags,
            sdk_version == D3D10_1_SDK_VERSION,
            level,
            out,
        )
    }
}

/// d3d10_1.h's `D3D10_1_SDK_VERSION`, which the windows crate does not carry.
const D3D10_1_SDK_VERSION: u32 = 0x20;

/// Warned when a Direct3D 10 device is asked of a driver other than the hardware one.
const NOT_HARDWARE_10: &str = "Direct3D 10 devices of a software, reference, WARP or null driver";

type CreateDevice10 =
    unsafe extern "system" fn(*mut c_void, i32, HMODULE, u32, u32, *mut *mut c_void) -> HRESULT;
type CreateDevice101 = unsafe extern "system" fn(
    *mut c_void,
    i32,
    HMODULE,
    u32,
    D3D_FEATURE_LEVEL,
    u32,
    *mut *mut c_void,
) -> HRESULT;

/// Direct3D 10's device creation for the program: the hardware driver only (the software, WARP,
/// reference and null drivers are not served), no debug layer. Without an adapter the device's
/// is one of a factory of DXGI 1.0, as Direct3D 10 makes it; a given adapter stands for ours.
unsafe fn create10(
    no_adapter: bool,
    driver_type: i32,
    flags: u32,
    sdk_ok: bool,
    level: D3D_FEATURE_LEVEL,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        unsafe { *out = std::ptr::null_mut() };
    }
    if !sdk_ok || out.is_null() {
        return windows::Win32::Foundation::E_INVALIDARG;
    }
    if driver_type != D3D10_DRIVER_TYPE_HARDWARE.0 {
        super::warn_once(NOT_HARDWARE_10);
        return DXGI_ERROR_UNSUPPORTED;
    }
    if flags & D3D10_CREATE_DEVICE_DEBUG.0 as u32 != 0 {
        return windows::Win32::Graphics::Dxgi::DXGI_ERROR_SDK_COMPONENT_MISSING;
    }
    match make(flags, level, no_adapter) {
        Ok(device) => {
            unsafe { *out = device.into_raw() };
            windows::Win32::Foundation::S_OK
        }
        Err(e) => e.code(),
    }
}

/// `D3D10CoreRegisterLayers`: no debug layers are served.
#[unsafe(no_mangle)]
pub extern "system" fn D3D10CoreRegisterLayers() -> HRESULT {
    windows::Win32::Foundation::E_NOTIMPL
}

/// `D3D10CoreGetVersion`: the system's runtime's, which this one serves the interfaces of; none
/// where it has no d3d10core.dll.
#[unsafe(no_mangle)]
pub extern "system" fn D3D10CoreGetVersion() -> u32 {
    // SAFETY: the system's export of this name has this type.
    unsafe {
        factory::system::<GetVersion>(
            factory::D3D10CORE_LIBRARY,
            factory::entry::D3D10_CORE_GET_VERSION,
        )
    }
    .map_or(0, |f| unsafe { f() })
}
