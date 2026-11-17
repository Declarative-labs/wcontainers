//! The Windows library of the Direct3D family on the WebGPU back end: DXGI's and Direct3D 12's entry
//! points and COM objects over the front, with one device thread owning the back end. `d3d12.dll`
//! and `dxgi.dll` forward their exports here (examples/d3d12.rs, examples/dxgi.rs) to the same names
//! with `wc_` before them, which cannot clash with the system's imports this library links. It lives here
//! until the family's own crate (`wcontainers-directx`, plan section 10) takes it over.

#![cfg(windows)]
#![allow(non_snake_case)]

mod com;
mod gpu;
mod objects;

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

/// # Safety
/// The pointers are the caller's, as the API defines them.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn wc_CreateDXGIFactory2(
    _flags: u32,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let factory: IDXGIFactory4 = Factory.into();
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
    _adapter: *mut c_void,
    minimum_level: i32,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let level = gpu::call(|d| features::feature_level(d.backend().caps()));
    match level {
        Some(l) if l >= minimum_level as u32 => {}
        _ => return DXGI_ERROR_UNSUPPORTED,
    }
    if out.is_null() {
        return S_FALSE;
    }
    let device: ID3D12Device = Device.into();
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
