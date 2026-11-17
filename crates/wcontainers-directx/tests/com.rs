//! COM behaviour of the library's objects: QueryInterface, identity, reference counts held by children
//! and views, the immediate context, private data and descriptions, as Microsoft documents them.
#![cfg(windows)]

mod common;

use windows::Win32::Graphics::Direct3D::{
    D3D_PRIMITIVE_TOPOLOGY, D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_NOT_FOUND, IDXGIAdapter1, IDXGIDevice, IDXGIObject,
};
use windows::core::{GUID, IUnknown, Interface};

use wcontainers_directx::com::create_device;

/// The reference count of an object, read by adding a reference and releasing it.
fn refs<I: Interface>(i: &I) -> u32 {
    unsafe {
        let raw = i.as_raw();
        let unknown = IUnknown::from_raw_borrowed(&raw).unwrap();
        (Interface::vtable(unknown).AddRef)(raw);
        (Interface::vtable(unknown).Release)(raw)
    }
}

fn device() -> (ID3D11Device, ID3D11DeviceContext) {
    let (d, c, _) = create_device(&[], 0).unwrap();
    (d, c)
}

fn buffer(device: &ID3D11Device) -> ID3D11Buffer {
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: 64,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
        ..Default::default()
    };
    let mut out = None;
    unsafe { device.CreateBuffer(&desc, None, Some(&mut out)) }.unwrap();
    out.unwrap()
}

#[test]
fn query_interface_follows_identity_and_parents() {
    let (device, _context) = device();
    let dxgi: IDXGIDevice = device.cast().unwrap();
    let _: IDXGIObject = device.cast().unwrap();
    let a: IUnknown = device.cast().unwrap();
    let b: IUnknown = dxgi.cast().unwrap();
    assert_eq!(a.as_raw(), b.as_raw(), "one IUnknown for both interfaces");
    let one: IUnknown = device.cast::<ID3D11Device1>().unwrap().cast().unwrap();
    assert_eq!(
        a.as_raw(),
        one.as_raw(),
        "Direct3D 11.1's device is the same object"
    );
    let adapter: IDXGIAdapter1 = unsafe { dxgi.GetAdapter() }.unwrap().cast().unwrap();
    let desc = unsafe { adapter.GetDesc1() }.unwrap();
    assert_eq!(desc.Flags, 0);
    // The host's monitors, each on a monitor and a child of the adapter, then none.
    let mut index = 0;
    let past = loop {
        match unsafe { adapter.EnumOutputs(index) } {
            Ok(output) => {
                let d = unsafe { output.GetDesc() }.unwrap();
                assert!(!d.Monitor.is_invalid());
                let parent: IDXGIAdapter1 = unsafe { output.GetParent() }.unwrap();
                assert_eq!(parent.as_raw(), adapter.as_raw());
                index += 1;
            }
            Err(e) => break e.code(),
        }
    };
    assert_eq!(past, DXGI_ERROR_NOT_FOUND);
    let b = buffer(&device);
    let _: ID3D11Resource = b.cast().unwrap();
    let _: ID3D11DeviceChild = b.cast().unwrap();
    assert!(b.cast::<ID3D11Texture2D>().is_err());
}

#[test]
fn children_hold_their_device() {
    let (device, context) = device();
    let before = refs(&device);
    let b = buffer(&device);
    assert_eq!(
        refs(&device),
        before + 1,
        "a child holds one reference on its device"
    );
    let got = unsafe { b.GetDevice() }.unwrap();
    assert_eq!(got.as_raw(), device.as_raw());
    drop(got);
    drop(b);
    assert_eq!(refs(&device), before);
    drop(context);
    assert_eq!(
        refs(&device),
        before - 1,
        "the immediate context held one reference"
    );
}

#[test]
fn the_immediate_context_is_one_object_while_it_lives() {
    let (device, context) = device();
    let again = unsafe { device.GetImmediateContext() }.unwrap();
    assert_eq!(again.as_raw(), context.as_raw());
    assert_eq!(refs(&context), 2);
    drop(again);
    assert_eq!(unsafe { context.GetType() }, D3D11_DEVICE_CONTEXT_IMMEDIATE);
    let owner = unsafe { context.GetDevice() }.unwrap();
    assert_eq!(owner.as_raw(), device.as_raw());
}

#[test]
fn views_hold_their_resource_and_return_it() {
    let (device, _context) = device();
    let desc = D3D11_TEXTURE2D_DESC {
        Width: 8,
        Height: 8,
        MipLevels: 1,
        ArraySize: 1,
        Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        ..Default::default()
    };
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.unwrap();
    let texture = texture.unwrap();
    let mut got = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut got) };
    assert_eq!(got, desc);
    let resource: ID3D11Resource = texture.cast().unwrap();
    let before = refs(&texture);
    let mut view = None;
    unsafe { device.CreateRenderTargetView(&resource, None, Some(&mut view)) }.unwrap();
    let view = view.unwrap();
    // The view's reference is the library's own: the program's count does not show it.
    assert_eq!(refs(&texture), before);
    let back = unsafe { view.GetResource() }.unwrap();
    assert_eq!(back.as_raw(), resource.as_raw());
    assert_eq!(refs(&texture), before + 1);
    // Released by the program, the texture lives on in the view and comes back from it.
    drop((back, resource));
    let raw = texture.as_raw();
    assert_eq!(
        unsafe { (Interface::vtable(&texture).base__.base__.base__.Release)(raw) },
        0
    );
    std::mem::forget(texture);
    let back = unsafe { view.GetResource() }.unwrap();
    assert_eq!((back.as_raw(), refs(&back)), (raw, 1));
    let mut vd = D3D11_RENDER_TARGET_VIEW_DESC::default();
    unsafe { view.GetDesc(&mut vd) };
    assert_eq!(vd.ViewDimension, D3D11_RTV_DIMENSION_TEXTURE2D);
    assert_eq!(vd.Format, desc.Format);
    // A view of a texture without the bind flag is refused, and a null out pointer only checks.
    let mut srv = None;
    assert!(unsafe { device.CreateShaderResourceView(&back, None, Some(&mut srv)) }.is_err());
    assert!(unsafe { device.CreateRenderTargetView(&back, None, None) }.is_ok());
}

#[test]
fn private_data_round_trips() {
    let (device, _context) = device();
    let b = buffer(&device);
    let guid = GUID::from_u128(0x429b8c22_9188_4b0c_8742_acb0bf85c200);
    let name = b"vertices";
    unsafe { b.SetPrivateData(&guid, name.len() as u32, Some(name.as_ptr().cast())) }.unwrap();
    let mut size = 0u32;
    unsafe { b.GetPrivateData(&guid, &mut size, None) }.unwrap();
    assert_eq!(size, name.len() as u32);
    let mut small = [0u8; 4];
    let mut room = 4u32;
    assert!(
        unsafe { b.GetPrivateData(&guid, &mut room, Some(small.as_mut_ptr().cast())) }.is_err()
    );
    let mut out = [0u8; 8];
    let mut room = 8u32;
    unsafe { b.GetPrivateData(&guid, &mut room, Some(out.as_mut_ptr().cast())) }.unwrap();
    assert_eq!(&out, name);
    let other = GUID::from_u128(1);
    let mut size = 5u32;
    assert!(unsafe { b.GetPrivateData(&other, &mut size, None) }.is_err());
    assert_eq!(size, 0);
}

#[test]
fn unserved_shaders_are_refused_not_faked() {
    let (device, _context) = device();
    // Bytes that are not a shader: E_INVALIDARG, nothing made.
    let mut code = b"DXBC".to_vec();
    code.resize(64, 7);
    let mut out = None;
    let r = unsafe { device.CreateVertexShader(&code, None, Some(&mut out)) };
    assert_eq!(
        r.err().map(|e| e.code()),
        Some(windows::Win32::Foundation::E_INVALIDARG)
    );
    assert!(out.is_none());
    // A valid pixel shader reading a value the converter does not provide yet: E_NOTIMPL.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../wcontainers-graphics-test/shaders/dxbc/geometry.ps_id.dxbc"
    );
    let code = std::fs::read(path).unwrap();
    let mut out = None;
    let r = unsafe { device.CreatePixelShader(&code, None, Some(&mut out)) };
    assert_eq!(
        r.err().map(|e| e.code()),
        Some(windows::Win32::Foundation::E_NOTIMPL)
    );
    assert!(out.is_none());
}

#[test]
fn the_forwarding_d3d11_reaches_the_library() {
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::core::{HSTRING, s};
    let dir = common::libraries();
    let load = |name: &str| unsafe { LoadLibraryW(&HSTRING::from(dir.join(name).as_os_str())) };
    let library = load("wcontainers_directx.dll").unwrap();
    let forwarder = load("d3d11.dll").unwrap();
    assert_ne!(library, forwarder);
    for name in [
        s!("D3D11CreateDevice"),
        s!("D3D11CreateDeviceAndSwapChain"),
        s!("D3D11On12CreateDevice"),
    ] {
        let direct = unsafe { GetProcAddress(library, name) }.unwrap();
        let forwarded = unsafe { GetProcAddress(forwarder, name) }.unwrap();
        assert_eq!(direct as usize, forwarded as usize);
    }
}

#[test]
fn device_creation_answers_as_direct3d() {
    // Through the built library, as a program calls it: the entry points pass calls from the
    // library's own module (here the test, which links it in) on to the system's.
    use std::ffi::c_void;
    use windows::Win32::Foundation::{E_INVALIDARG, HMODULE};
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Dxgi::DXGI_ERROR_INVALID_CALL;
    use windows::Win32::Graphics::Dxgi::{DXGI_SWAP_CHAIN_DESC, IDXGIAdapter};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::core::{HRESULT, HSTRING, s};
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
    type CreateBoth = unsafe extern "system" fn(
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
    let dir = common::libraries();
    let library = unsafe {
        LoadLibraryW(&HSTRING::from(
            dir.join("wcontainers_directx.dll").as_os_str(),
        ))
    }
    .unwrap();
    let create: CreateDevice =
        unsafe { std::mem::transmute(GetProcAddress(library, s!("D3D11CreateDevice")).unwrap()) };
    let create_both: CreateBoth = unsafe {
        std::mem::transmute(GetProcAddress(library, s!("D3D11CreateDeviceAndSwapChain")).unwrap())
    };
    let null: *mut c_void = std::ptr::null_mut();
    let make = |adapter: *mut c_void, driver: D3D_DRIVER_TYPE, out: *mut *mut c_void| unsafe {
        create(
            adapter,
            driver.0,
            HMODULE::default(),
            0,
            std::ptr::null(),
            0,
            7,
            out,
            null.cast(),
            null.cast(),
        )
    };
    let mut out = null;
    assert_eq!(make(null, D3D_DRIVER_TYPE_HARDWARE, &mut out).0, 0);
    let device = unsafe { ID3D11Device::from_raw(out) };
    let adapter: IDXGIAdapter =
        unsafe { device.cast::<IDXGIDevice>().unwrap().GetAdapter() }.unwrap();
    // An adapter goes with an unknown driver type, and no adapter with a named one.
    assert_eq!(
        make(adapter.as_raw(), D3D_DRIVER_TYPE_HARDWARE, null.cast()),
        E_INVALIDARG
    );
    assert_eq!(
        make(null, D3D_DRIVER_TYPE_UNKNOWN, null.cast()),
        E_INVALIDARG
    );
    let mut out = null;
    assert_eq!(
        make(adapter.as_raw(), D3D_DRIVER_TYPE_UNKNOWN, &mut out).0,
        0
    );
    drop(unsafe { ID3D11Device::from_raw(out) });
    // Without a swap chain asked for, the device as D3D11CreateDevice makes it.
    let both = |desc: *const DXGI_SWAP_CHAIN_DESC,
                chain: *mut *mut c_void,
                out: *mut *mut c_void| unsafe {
        create_both(
            null,
            D3D_DRIVER_TYPE_HARDWARE.0,
            HMODULE::default(),
            0,
            std::ptr::null(),
            0,
            7,
            desc.cast(),
            chain,
            out,
            null.cast(),
            null.cast(),
        )
    };
    let mut out = null;
    assert_eq!(both(std::ptr::null(), null.cast(), &mut out).0, 0);
    drop(unsafe { ID3D11Device::from_raw(out) });
    // A swap chain without a window is an invalid call, as DXGI answers it.
    let desc = DXGI_SWAP_CHAIN_DESC::default();
    let mut chain: *mut c_void = std::ptr::NonNull::dangling().as_ptr();
    assert_eq!(
        both(&desc, &mut chain, null.cast()),
        DXGI_ERROR_INVALID_CALL
    );
    assert!(chain.is_null());
    assert_eq!(
        both(std::ptr::null(), &mut chain, null.cast()),
        E_INVALIDARG
    );
}

#[test]
fn device_calls_from_several_threads_reach_the_front_one_at_a_time() {
    // Direct3D 11's device is free-threaded: eight threads make, query and release buffers at once.
    let (device, _context) = device();
    let before = refs(&device);
    let raw = device.as_raw() as usize;
    let threads: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(move || {
                let raw = raw as *mut std::ffi::c_void;
                let device = unsafe { ID3D11Device::from_raw_borrowed(&raw) }
                    .unwrap()
                    .clone();
                for _ in 0..200 {
                    let b = buffer(&device);
                    let r: ID3D11Resource = b.cast().unwrap();
                    assert_eq!(refs(&r), 2);
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(refs(&device), before);
}

#[test]
fn equal_state_descriptions_give_the_live_object() {
    let (device, _context) = device();
    let target = D3D11_RENDER_TARGET_BLEND_DESC {
        SrcBlend: D3D11_BLEND_ONE,
        DestBlend: D3D11_BLEND_ZERO,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_ZERO,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: 0xf,
        ..Default::default()
    };
    let blend = |enable: bool| {
        let desc = D3D11_BLEND_DESC {
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: enable.into(),
                ..target
            }; 8],
            ..Default::default()
        };
        let mut out = None;
        unsafe { device.CreateBlendState(&desc, Some(&mut out)) }.unwrap();
        out.unwrap()
    };
    let (a, b) = (blend(false), blend(false));
    assert_eq!(a.as_raw(), b.as_raw());
    assert_eq!(refs(&a), 2);
    assert_ne!(blend(true).as_raw(), a.as_raw());
    drop((a, b));
    // Once the program's references went, an equal description hands out one again.
    assert_eq!(refs(&blend(false)), 1);

    let sampler = |filter| {
        let desc = D3D11_SAMPLER_DESC {
            Filter: filter,
            AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let mut out = None;
        unsafe { device.CreateSamplerState(&desc, Some(&mut out)) }.unwrap();
        out.unwrap()
    };
    let (a, b) = (
        sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR),
        sampler(D3D11_FILTER_MIN_MAG_MIP_LINEAR),
    );
    assert_eq!(a.as_raw(), b.as_raw());
    assert_ne!(sampler(D3D11_FILTER_MIN_MAG_MIP_POINT).as_raw(), a.as_raw());
}

#[test]
fn the_context_hands_back_what_is_bound() {
    let (device, context) = device();
    let b = buffer(&device);
    unsafe {
        context.IASetVertexBuffers(2, 1, Some(&Some(b.clone())), Some(&16), Some(&4));
        let mut buffers: [Option<ID3D11Buffer>; 3] = Default::default();
        let (mut strides, mut offsets) = ([9u32; 3], [9u32; 3]);
        context.IAGetVertexBuffers(
            1,
            3,
            Some(buffers.as_mut_ptr()),
            Some(strides.as_mut_ptr()),
            Some(offsets.as_mut_ptr()),
        );
        assert!(buffers[0].is_none() && buffers[2].is_none());
        assert_eq!(buffers[1].as_ref().map(Interface::as_raw), Some(b.as_raw()));
        assert_eq!((strides, offsets), ([0, 16, 0], [0, 4, 0]));
        // The context's reference is the library's own; the getter hands out one of the program's.
        assert_eq!(refs(&b), 2);
        drop(buffers);
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        assert_eq!(
            context.IAGetPrimitiveTopology(),
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP
        );
        let mut targets: [Option<ID3D11RenderTargetView>; 2] = Default::default();
        let mut depth = None;
        context.OMGetRenderTargets(Some(&mut targets), Some(&mut depth));
        assert!(targets.iter().all(Option::is_none) && depth.is_none());
        let (mut state, mut factor, mut mask) = (None, [0.0; 4], 0);
        context.OMGetBlendState(Some(&mut state), Some(&mut factor), Some(&mut mask));
        assert!(state.is_none());
        assert_eq!((factor, mask), ([1.0; 4], u32::MAX));
        let viewport = D3D11_VIEWPORT {
            Width: 8.0,
            Height: 4.0,
            MaxDepth: 1.0,
            ..Default::default()
        };
        context.RSSetViewports(Some(&[viewport]));
        let mut n = 0;
        context.RSGetViewports(&mut n, None);
        assert_eq!(n, 1);
        let mut got = [D3D11_VIEWPORT::default(); 2];
        n = 2;
        context.RSGetViewports(&mut n, Some(got.as_mut_ptr()));
        assert_eq!(got, [viewport, D3D11_VIEWPORT::default()]);
        context.ClearState();
        assert_eq!(refs(&b), 1);
        assert_eq!(context.IAGetPrimitiveTopology(), D3D_PRIMITIVE_TOPOLOGY(0));
    }
}

#[test]
fn stages_the_front_lacks_take_unbinding_and_report_nothing_bound() {
    let (device, context) = device();
    unsafe {
        context.GSSetShader(None, None);
        context.HSSetShader(None, None);
        context.DSSetShader(None, None);
        context.CSSetShader(None, None);
        context.GSSetConstantBuffers(0, Some(&[None, None]));
        let mut shader: Option<ID3D11GeometryShader> = None;
        context.GSGetShader(&mut shader, None, None);
        assert!(shader.is_none());
        let mut buffers: [Option<ID3D11Buffer>; 2] = Default::default();
        context.CSGetConstantBuffers(0, Some(&mut buffers));
        assert!(buffers.iter().all(Option::is_none));
        assert!(device.GetDeviceRemovedReason().is_ok());
    }
}

/// Constant buffers and samplers bound to the geometry, hull and domain stages are kept for the
/// getters, as Direct3D keeps them, though no shader of those stages can be made.
#[test]
fn stages_the_front_lacks_keep_constant_buffers_and_samplers() {
    let (device, context) = device();
    unsafe {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        };
        let mut b = None;
        device.CreateBuffer(&desc, None, Some(&mut b)).unwrap();
        let b = b.unwrap();
        let mut s = None;
        let sampler = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MaxLOD: f32::MAX,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            ..Default::default()
        };
        device.CreateSamplerState(&sampler, Some(&mut s)).unwrap();
        let s = s.unwrap();
        context.GSSetConstantBuffers(1, Some(&[Some(b.clone())]));
        context.HSSetSamplers(2, Some(&[Some(s.clone())]));
        context.DSSetConstantBuffers(13, Some(&[Some(b.clone()), Some(b.clone())]));
        let mut buffers: [Option<ID3D11Buffer>; 2] = Default::default();
        context.GSGetConstantBuffers(0, Some(&mut buffers));
        assert!(buffers[0].is_none());
        assert_eq!(buffers[1].as_ref(), Some(&b));
        let mut samplers: [Option<ID3D11SamplerState>; 1] = Default::default();
        context.HSGetSamplers(2, Some(&mut samplers));
        assert_eq!(samplers[0].as_ref(), Some(&s));
        // Past the last slot: dropped.
        let mut last: [Option<ID3D11Buffer>; 1] = Default::default();
        context.DSGetConstantBuffers(13, Some(&mut last));
        assert!(last[0].is_none());
        context.ClearState();
        context.GSGetConstantBuffers(1, Some(&mut last));
        assert!(last[0].is_none());
        assert!(device.GetDeviceRemovedReason().is_ok());
    }
}

#[test]
fn binding_a_target_unbinds_its_shader_views() {
    let (device, context) = device();
    let desc = D3D11_TEXTURE2D_DESC {
        Width: 8,
        Height: 8,
        MipLevels: 2,
        ArraySize: 1,
        Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        ..Default::default()
    };
    let mut texture = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.unwrap();
    let texture = texture.unwrap();
    let srv = |mip: u32| {
        let d = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: desc.Format,
            ViewDimension: windows::Win32::Graphics::Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: mip,
                    MipLevels: 1,
                },
            },
        };
        let mut out = None;
        unsafe { device.CreateShaderResourceView(&texture, Some(&d), Some(&mut out)) }.unwrap();
        out
    };
    let (top, second) = (srv(0), srv(1));
    let mut rtv = None;
    unsafe { device.CreateRenderTargetView(&texture, None, Some(&mut rtv)) }.unwrap();
    let bound = || {
        let mut views: [Option<ID3D11ShaderResourceView>; 2] = Default::default();
        unsafe { context.PSGetShaderResources(0, Some(&mut views)) };
        views.map(|v| v.is_some())
    };
    unsafe {
        context.PSSetShaderResources(0, Some(&[top.clone(), second.clone()]));
        assert_eq!(bound(), [true, true]);
        // The target is level 0: its view is unbound, the view of level 1 stays.
        context.OMSetRenderTargets(Some(&[rtv]), None::<&ID3D11DepthStencilView>);
        assert_eq!(bound(), [false, true]);
        // A view of a level bound as a target is bound as none.
        context.PSSetShaderResources(0, Some(std::slice::from_ref(&top)));
        assert_eq!(bound(), [false, true]);
        context.OMSetRenderTargets(None, None::<&ID3D11DepthStencilView>);
        context.PSSetShaderResources(0, Some(&[top]));
        assert_eq!(bound(), [true, true]);
    }
}

#[test]
fn capability_queries_claim_only_what_creation_serves() {
    use windows::Win32::Graphics::Dxgi::Common::*;
    let (device, _context) = device();
    let mut claimed = 0;
    for f in 1..=132 {
        let format = DXGI_FORMAT(f);
        let Ok(bits) = (unsafe { device.CheckFormatSupport(format) }) else {
            continue;
        };
        // A quality level for 4 samples makes a texture of 4 samples drawn into, as a target or
        // for depth (a typeless group's bits do not say which).
        let levels = unsafe { device.CheckMultisampleQualityLevels(format, 4) }.unwrap();
        if levels > 0 {
            let made = [D3D11_BIND_RENDER_TARGET, D3D11_BIND_DEPTH_STENCIL]
                .iter()
                .any(|bind| {
                    let desc = D3D11_TEXTURE2D_DESC {
                        Width: 4,
                        Height: 4,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: format,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 4,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: bind.0 as u32,
                        ..Default::default()
                    };
                    let mut out = None;
                    unsafe { device.CreateTexture2D(&desc, None, Some(&mut out)) }.is_ok()
                });
            assert!(made, "format {f} with 4 samples");
        }
        if bits & D3D11_FORMAT_SUPPORT_TEXTURE2D.0 as u32 == 0 {
            continue;
        }
        claimed += 1;
        for (bit, bind) in [
            (D3D11_FORMAT_SUPPORT_TEXTURE2D, 0),
            (
                D3D11_FORMAT_SUPPORT_SHADER_LOAD,
                D3D11_BIND_SHADER_RESOURCE.0,
            ),
            (
                D3D11_FORMAT_SUPPORT_RENDER_TARGET,
                D3D11_BIND_RENDER_TARGET.0,
            ),
            (
                D3D11_FORMAT_SUPPORT_DEPTH_STENCIL,
                D3D11_BIND_DEPTH_STENCIL.0,
            ),
        ] {
            if bits & bit.0 as u32 == 0 {
                continue;
            }
            let desc = D3D11_TEXTURE2D_DESC {
                Width: 4,
                Height: 4,
                MipLevels: 1,
                ArraySize: 1,
                Format: format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: bind as u32,
                ..Default::default()
            };
            let mut out = None;
            let r = unsafe { device.CreateTexture2D(&desc, None, Some(&mut out)) };
            assert!(r.is_ok(), "format {f} bind {bind:#x}: {r:?}");
        }
        assert_eq!(
            unsafe { device.CheckMultisampleQualityLevels(format, 1) }.unwrap(),
            1
        );
    }
    assert!(claimed > 20, "{claimed} formats claimed");
    let rgba = unsafe { device.CheckFormatSupport(DXGI_FORMAT_R8G8B8A8_UNORM) }.unwrap();
    let want = D3D11_FORMAT_SUPPORT_TEXTURE2D.0
        | D3D11_FORMAT_SUPPORT_SHADER_SAMPLE.0
        | D3D11_FORMAT_SUPPORT_RENDER_TARGET.0
        | D3D11_FORMAT_SUPPORT_BLENDABLE.0;
    assert_eq!(rgba & want as u32, want as u32);
    // Each feature answers with its own structure's size only; absent features are zeros.
    let mut threading = D3D11_FEATURE_DATA_THREADING {
        DriverConcurrentCreates: true.into(),
        DriverCommandLists: true.into(),
    };
    let size = size_of::<D3D11_FEATURE_DATA_THREADING>() as u32;
    let p = (&raw mut threading).cast();
    unsafe { device.CheckFeatureSupport(D3D11_FEATURE_THREADING, p, size) }.unwrap();
    assert!(
        !threading.DriverConcurrentCreates.as_bool() && !threading.DriverCommandLists.as_bool()
    );
    let r = unsafe { device.CheckFeatureSupport(D3D11_FEATURE_THREADING, p, size + 4) };
    assert_eq!(
        r.map_err(|e| e.code()),
        Err(windows::Win32::Foundation::E_INVALIDARG)
    );
    let mut support = D3D11_FEATURE_DATA_FORMAT_SUPPORT {
        InFormat: DXGI_FORMAT_R8G8B8A8_UNORM,
        OutFormatSupport: 0,
    };
    let size = size_of::<D3D11_FEATURE_DATA_FORMAT_SUPPORT>() as u32;
    unsafe {
        device.CheckFeatureSupport(
            D3D11_FEATURE_FORMAT_SUPPORT,
            (&raw mut support).cast(),
            size,
        )
    }
    .unwrap();
    assert_eq!(support.OutFormatSupport, rgba);
}
