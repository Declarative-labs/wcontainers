//! d3d10core.dll's entry points: Direct3D 10's device, made as a Direct3D 11 device whose
//! immediate context starts with a state emulating Direct3D 10 active. Calls from this library,
//! the back end's libraries and the system's go on to the system's, as `D3D11CreateDevice`'s do,
//! but those of the system's Direct3D 10 runtime (d3d10.dll, d3d10_1.dll), which asks on the
//! program's behalf.

use std::ffi::c_void;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_0};
use windows::Win32::Graphics::Direct3D10::ID3D10Device;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_SDK_VERSION, ID3D11Device1, ID3D11DeviceContext1, ID3DDeviceContextState,
};
use windows::Win32::Graphics::Dxgi::DXGI_ERROR_UNSUPPORTED;
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
    if !factory::called_by_program_or(&["d3d10.dll", "d3d10_1.dll"]) {
        // SAFETY: the system's export of this name has this type.
        return match unsafe {
            factory::system::<CreateDevice>("d3d10core.dll", c"D3D10CoreCreateDevice")
        } {
            Some(f) => unsafe { f(factory, adapter, flags, level, out) },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    if out.is_null() {
        return windows::Win32::Foundation::E_INVALIDARG;
    }
    unsafe { *out = std::ptr::null_mut() };
    match make(flags, level) {
        Ok(device) => {
            unsafe { *out = device.into_raw() };
            windows::Win32::Foundation::S_OK
        }
        Err(e) => e.code(),
    }
}

fn make(flags: u32, level: D3D_FEATURE_LEVEL) -> windows::core::Result<ID3D10Device> {
    let (device, context, _) = {
        let _lock = lock();
        super::create_device(&[level.0 as u32], flags)?
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
        return match unsafe { factory::system::<CreateDevice10>("d3d10.dll", c"D3D10CreateDevice") }
        {
            Some(f) => unsafe { f(adapter, driver_type, software, flags, sdk_version, out) },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    unsafe {
        create10(
            driver_type,
            flags,
            sdk_version == SDK_VERSION,
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
            factory::system::<CreateDevice101>("d3d10_1.dll", c"D3D10CreateDevice1")
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
    unsafe { create10(driver_type, flags, sdk_version == SDK_VERSION_1, level, out) }
}

/// `D3D10_SDK_VERSION` and `D3D10_1_SDK_VERSION`.
const SDK_VERSION: u32 = 29;
const SDK_VERSION_1: u32 = 0x20;
/// `D3D10_DRIVER_TYPE_HARDWARE` and `D3D10_CREATE_DEVICE_DEBUG`.
const DRIVER_HARDWARE: i32 = 0;
const CREATE_DEBUG: u32 = 2;

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
/// reference and null drivers are not served), no debug layer.
unsafe fn create10(
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
    if driver_type != DRIVER_HARDWARE {
        super::warn_once("D3D10CreateDevice with a software, reference, WARP or null driver");
        return DXGI_ERROR_UNSUPPORTED;
    }
    if flags & CREATE_DEBUG != 0 {
        return windows::Win32::Graphics::Dxgi::DXGI_ERROR_SDK_COMPONENT_MISSING;
    }
    match make(flags, level) {
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
    unsafe { factory::system::<GetVersion>("d3d10core.dll", c"D3D10CoreGetVersion") }
        .map_or(0, |f| unsafe { f() })
}
