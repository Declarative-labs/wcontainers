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
    assert!(
        device.cast::<ID3D11Device1>().is_err(),
        "Direct3D 11.1 is not offered"
    );
    let adapter: IDXGIAdapter1 = unsafe { dxgi.GetAdapter() }.unwrap().cast().unwrap();
    let desc = unsafe { adapter.GetDesc1() }.unwrap();
    assert_eq!(desc.Flags, 0);
    assert_eq!(
        unsafe { adapter.EnumOutputs(0) }.err().map(|e| e.code()),
        Some(DXGI_ERROR_NOT_FOUND)
    );
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
    let dir = common::fresh("forwarder");
    common::place_libraries(&dir);
    let load = |name: &str| unsafe { LoadLibraryW(&HSTRING::from(dir.join(name).as_os_str())) };
    let library = load("wcontainers_directx.dll").unwrap();
    let forwarder = load("d3d11.dll").unwrap();
    assert_ne!(library, forwarder);
    let direct = unsafe { GetProcAddress(library, s!("D3D11CreateDevice")) }.unwrap();
    let forwarded = unsafe { GetProcAddress(forwarder, s!("D3D11CreateDevice")) }.unwrap();
    assert_eq!(direct as usize, forwarded as usize);
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
