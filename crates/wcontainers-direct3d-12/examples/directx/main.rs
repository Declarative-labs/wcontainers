//! d3d12.dll, the Windows library of the Direct3D family on the WebGPU back end: DXGI's and Direct3D
//! 12's entry points and COM objects over the front, with one device thread owning the back end.
//! Each entry point is `wc_` and the API's name, which cannot clash with the system's imports this
//! library links (it imports `CreateDXGIFactory1`); this library exports Direct3D 12's under their
//! own names too, and `dxgi.dll` (examples/dxgi.rs) forwards DXGI's here. It lives here until the
//! family's own crate (`wcontainers-directx`, plan section 10) takes it over.

#![cfg(windows)]
#![allow(non_snake_case)]

mod abi;
mod base;
mod com;
mod gpu;
mod objects;
mod present;
mod rootsig;

const EXPORTS: &str = " /EXPORT:D3D12CreateDevice=wc_D3D12CreateDevice,@101 /EXPORT:D3D12GetDebugInterface=wc_D3D12GetDebugInterface,@102 /EXPORT:D3D12SerializeRootSignature=wc_D3D12SerializeRootSignature,@116 /EXPORT:D3D12SerializeVersionedRootSignature=wc_D3D12SerializeVersionedRootSignature,@117 /EXPORT:D3D12CreateRootSignatureDeserializer=wc_D3D12CreateRootSignatureDeserializer,@107 /EXPORT:D3D12CreateVersionedRootSignatureDeserializer=wc_D3D12CreateVersionedRootSignatureDeserializer,@108 ";

/// Linker directives the linker reads from this section of the object file: Direct3D 12's exports
/// under their own names and the system library's ordinals.
#[used]
#[unsafe(link_section = ".drectve")]
static DIRECTIVES: [u8; EXPORTS.len()] = {
    let mut out = [0; EXPORTS.len()];
    let bytes = EXPORTS.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        out[i] = bytes[i];
        i += 1;
    }
    out
};

use std::ffi::c_void;

use wcontainers_direct3d_12::features;
use wcontainers_direct3d_12::root::{
    DescriptorRange, RootParameter, RootParameterKind, RootSignatureDesc, StaticSampler, serialize,
};
use wcontainers_gpu::Backend;
use windows::Win32::Foundation::{E_INVALIDARG, E_NOINTERFACE, E_POINTER, S_FALSE, S_OK};
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_UNSUPPORTED, IDXGIFactory4};
use windows::core::{GUID, HRESULT, Interface};

use objects::{Blob, Device, Factory};

/// Hands out `object` as the interface `riid` names.
///
/// # Safety
/// `riid` and `out` are the caller's, as COM passes them.
unsafe fn hand_out(
    object: &windows::core::IUnknown,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() || riid.is_null() {
        return E_POINTER;
    }
    // SAFETY: both pointers were checked and come from the caller.
    unsafe { object.query(riid, out) }
}

/// The system's own export `export` of library `name`, for calls made on the device thread: the
/// GPU driver under the back end loads DXGI or Direct3D 12 by name from that thread, which in this
/// process finds this library, and must have the system's.
///
/// # Safety
/// `F` is the export's function type.
unsafe fn system<F: Copy>(name: &str, export: &std::ffi::CStr) -> Option<F> {
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut dir = [0u16; 260];
    // SAFETY: a buffer of the length given.
    let len = unsafe { GetSystemDirectoryW(Some(&mut dir)) } as usize;
    let path: Vec<u16> = String::from_utf16_lossy(&dir[..len])
        .chars()
        .chain(format!("\\{name}").chars())
        .collect::<String>()
        .encode_utf16()
        .chain([0])
        .collect();
    // SAFETY: a path ended by a zero; the module stays loaded for the process.
    let module = unsafe { LoadLibraryW(windows::core::PCWSTR(path.as_ptr())) }.ok()?;
    // SAFETY: a name ended by a zero.
    let f = unsafe { GetProcAddress(module, windows::core::PCSTR(export.as_ptr().cast())) }?;
    // SAFETY: the caller names the type.
    Some(unsafe { std::mem::transmute_copy(&f) })
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_CreateDXGIFactory2(
    flags: u32,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if gpu::on_device_thread() {
        type F = unsafe extern "system" fn(u32, *const GUID, *mut *mut c_void) -> HRESULT;
        // SAFETY: the system's export of this name has this signature.
        return match unsafe { system::<F>("dxgi.dll", c"CreateDXGIFactory2") } {
            Some(f) => unsafe { f(flags, riid, out) },
            None => E_NOINTERFACE,
        };
    }
    let factory: IDXGIFactory4 = Factory(flags).into();
    // SAFETY: as the caller passed them.
    unsafe { hand_out(&factory.cast().unwrap(), riid, out) }
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_CreateDXGIFactory1(
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    // SAFETY: the caller's pointers, passed on.
    unsafe { wc_CreateDXGIFactory2(0, riid, out) }
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_CreateDXGIFactory(
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    // SAFETY: the caller's pointers, passed on.
    unsafe { wc_CreateDXGIFactory2(0, riid, out) }
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_D3D12CreateDevice(
    adapter: *mut c_void,
    minimum_level: i32,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if gpu::on_device_thread() {
        type F =
            unsafe extern "system" fn(*mut c_void, i32, *const GUID, *mut *mut c_void) -> HRESULT;
        // SAFETY: the system's export of this name has this signature.
        return match unsafe { system::<F>("d3d12.dll", c"D3D12CreateDevice") } {
            Some(f) => unsafe { f(adapter, minimum_level, riid, out) },
            None => DXGI_ERROR_UNSUPPORTED,
        };
    }
    let level = gpu::call(|d| features::feature_level(d.backend().caps()));
    match level {
        Some(l) if l >= minimum_level as u32 => {}
        _ => return DXGI_ERROR_UNSUPPORTED,
    }
    if out.is_null() {
        return S_FALSE;
    }
    // One device per adapter: while it lives, it is handed out again.
    let device = base::live_device().unwrap_or_else(|| {
        let device: ID3D12Device = abi::fixed(Device::default().into());
        base::set_live_device(&device);
        device
    });
    // SAFETY: as the caller passed them.
    unsafe { hand_out(&device.cast().unwrap(), riid, out) }
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_D3D12GetDebugInterface(
    _riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if !out.is_null() {
        // SAFETY: the caller's out pointer.
        unsafe { *out = std::ptr::null_mut() };
    }
    E_NOINTERFACE
}

/// `n` values at `p`.
///
/// # Safety
/// `p` points to `n` values, or `n` is zero.
unsafe fn many<'a, T>(p: *const T, n: u32) -> &'a [T] {
    if n == 0 || p.is_null() {
        &[]
    } else {
        // SAFETY: as the caller promises.
        unsafe { std::slice::from_raw_parts(p, n as usize) }
    }
}

/// The root signature of version 1.0 or 1.1 the caller describes, in the front's form.
///
/// # Safety
/// `desc` points to a valid description, as the API requires.
unsafe fn root_desc(
    desc: &D3D12_VERSIONED_ROOT_SIGNATURE_DESC,
) -> Result<RootSignatureDesc, HRESULT> {
    let sampler = |s: &D3D12_STATIC_SAMPLER_DESC| {
        StaticSampler([
            s.Filter.0 as u32,
            s.AddressU.0 as u32,
            s.AddressV.0 as u32,
            s.AddressW.0 as u32,
            s.MipLODBias.to_bits(),
            s.MaxAnisotropy,
            s.ComparisonFunc.0 as u32,
            s.BorderColor.0 as u32,
            s.MinLOD.to_bits(),
            s.MaxLOD.to_bits(),
            s.ShaderRegister,
            s.RegisterSpace,
            s.ShaderVisibility.0 as u32,
        ])
    };
    // SAFETY: the union member read is the one `Version` names.
    unsafe {
        match desc.Version {
            D3D_ROOT_SIGNATURE_VERSION_1_0 => {
                let d = &desc.Anonymous.Desc_1_0;
                let parameters = many(d.pParameters, d.NumParameters)
                    .iter()
                    .map(|p| {
                        let kind = match p.ParameterType {
                            D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE => {
                                let t = &p.Anonymous.DescriptorTable;
                                RootParameterKind::Table(
                                    many(t.pDescriptorRanges, t.NumDescriptorRanges)
                                        .iter()
                                        .map(|r| DescriptorRange {
                                            kind: r.RangeType.0 as u32,
                                            count: r.NumDescriptors,
                                            register: r.BaseShaderRegister,
                                            space: r.RegisterSpace,
                                            flags: 0,
                                            offset: r.OffsetInDescriptorsFromTableStart,
                                        })
                                        .collect(),
                                )
                            }
                            D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS => {
                                let c = &p.Anonymous.Constants;
                                RootParameterKind::Constants {
                                    register: c.ShaderRegister,
                                    space: c.RegisterSpace,
                                    count: c.Num32BitValues,
                                }
                            }
                            t => {
                                let r = &p.Anonymous.Descriptor;
                                RootParameterKind::Descriptor {
                                    kind: t.0 as u32,
                                    register: r.ShaderRegister,
                                    space: r.RegisterSpace,
                                    flags: 0,
                                }
                            }
                        };
                        RootParameter {
                            kind,
                            visibility: p.ShaderVisibility.0 as u32,
                        }
                    })
                    .collect();
                Ok(RootSignatureDesc {
                    version: 1,
                    parameters,
                    samplers: many(d.pStaticSamplers, d.NumStaticSamplers)
                        .iter()
                        .map(sampler)
                        .collect(),
                    flags: d.Flags.0 as u32,
                })
            }
            D3D_ROOT_SIGNATURE_VERSION_1_1 => {
                let d = &desc.Anonymous.Desc_1_1;
                let parameters = many(d.pParameters, d.NumParameters)
                    .iter()
                    .map(|p| {
                        let kind = match p.ParameterType {
                            D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE => {
                                let t = &p.Anonymous.DescriptorTable;
                                RootParameterKind::Table(
                                    many(t.pDescriptorRanges, t.NumDescriptorRanges)
                                        .iter()
                                        .map(|r| DescriptorRange {
                                            kind: r.RangeType.0 as u32,
                                            count: r.NumDescriptors,
                                            register: r.BaseShaderRegister,
                                            space: r.RegisterSpace,
                                            flags: r.Flags.0 as u32,
                                            offset: r.OffsetInDescriptorsFromTableStart,
                                        })
                                        .collect(),
                                )
                            }
                            D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS => {
                                let c = &p.Anonymous.Constants;
                                RootParameterKind::Constants {
                                    register: c.ShaderRegister,
                                    space: c.RegisterSpace,
                                    count: c.Num32BitValues,
                                }
                            }
                            t => {
                                let r = &p.Anonymous.Descriptor;
                                RootParameterKind::Descriptor {
                                    kind: t.0 as u32,
                                    register: r.ShaderRegister,
                                    space: r.RegisterSpace,
                                    flags: r.Flags.0 as u32,
                                }
                            }
                        };
                        RootParameter {
                            kind,
                            visibility: p.ShaderVisibility.0 as u32,
                        }
                    })
                    .collect();
                Ok(RootSignatureDesc {
                    version: 2,
                    parameters,
                    samplers: many(d.pStaticSamplers, d.NumStaticSamplers)
                        .iter()
                        .map(sampler)
                        .collect(),
                    flags: d.Flags.0 as u32,
                })
            }
            _ => Err(E_INVALIDARG),
        }
    }
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_D3D12SerializeVersionedRootSignature(
    desc: *const D3D12_VERSIONED_ROOT_SIGNATURE_DESC,
    blob: *mut *mut c_void,
    errors: *mut *mut c_void,
) -> HRESULT {
    if !errors.is_null() {
        // SAFETY: the caller's out pointer.
        unsafe { *errors = std::ptr::null_mut() };
    }
    if desc.is_null() || blob.is_null() {
        return E_POINTER;
    }
    // SAFETY: the caller's description.
    let desc = match unsafe { root_desc(&*desc) } {
        Ok(d) => d,
        Err(e) => return e,
    };
    let out: ID3DBlob = Blob(serialize(&desc)).into();
    // SAFETY: the caller's out pointer; the reference is handed over.
    unsafe { *blob = out.into_raw() };
    S_OK
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_D3D12SerializeRootSignature(
    desc: *const D3D12_ROOT_SIGNATURE_DESC,
    version: D3D_ROOT_SIGNATURE_VERSION,
    blob: *mut *mut c_void,
    errors: *mut *mut c_void,
) -> HRESULT {
    if desc.is_null() || version != D3D_ROOT_SIGNATURE_VERSION_1_0 {
        return E_INVALIDARG;
    }
    let versioned = D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
        Version: version,
        Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
            // SAFETY: the caller's description, copied bit for bit.
            Desc_1_0: unsafe { std::ptr::read(desc) },
        },
    };
    // SAFETY: the caller's pointers, passed on.
    unsafe { wc_D3D12SerializeVersionedRootSignature(&versioned, blob, errors) }
}

/// The root signature serialized in the caller's `length` bytes at `data`, read.
///
/// # Safety
/// `data` points to `length` bytes, as the API requires.
unsafe fn read_root(data: *const c_void, length: usize) -> Result<RootSignatureDesc, HRESULT> {
    if data.is_null() || length == 0 {
        return Err(E_INVALIDARG);
    }
    // SAFETY: as the caller promises.
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), length) };
    wcontainers_direct3d_12::root::deserialize(bytes).map_err(|_| E_INVALIDARG)
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_D3D12CreateRootSignatureDeserializer(
    data: *const c_void,
    length: usize,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    // SAFETY: the caller's bytes.
    match unsafe { read_root(data, length) } {
        Ok(desc) => {
            let d: ID3D12RootSignatureDeserializer = rootsig::Deserializer::new(&desc).into();
            // SAFETY: the caller's pointers.
            unsafe { hand_out(&d.cast().unwrap(), riid, out) }
        }
        Err(e) => e,
    }
}

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_D3D12CreateVersionedRootSignatureDeserializer(
    data: *const c_void,
    length: usize,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    // SAFETY: the caller's bytes.
    match unsafe { read_root(data, length) } {
        Ok(desc) => {
            let d: ID3D12VersionedRootSignatureDeserializer =
                rootsig::VersionedDeserializer::new(&desc).into();
            // SAFETY: the caller's pointers.
            unsafe { hand_out(&d.cast().unwrap(), riid, out) }
        }
        Err(e) => e,
    }
}
