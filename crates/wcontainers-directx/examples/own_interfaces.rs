//! A small Direct3D 11 program on the interfaces beside `ID3D11Device` and `ID3D11DeviceContext`:
//! Direct3D 11.1's device and context (states with their 11.1 descriptions, constant buffer
//! ranges), `IDXGIDevice1`, `ID3D11Multithread`, annotations, DXGI's resource and surface on
//! resources of each dimension, and what failed creations and maps leave in their out pointers.
//! Prints the answers, for comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::ffi::c_void;

    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::core::{GUID, IUnknown, Interface, w};

    let code = |r: windows::core::Result<()>| r.err().map_or(0, |e| e.code().0) as u32;
    let (mut device, mut context) = (None, None);
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?
    };
    let (device, c): (ID3D11Device, ID3D11DeviceContext) = (device.unwrap(), context.unwrap());
    let unknown = |i: &IUnknown| i.as_raw() as usize;

    // The device's interfaces.
    for (name, iid) in [
        ("ID3D11Device1", ID3D11Device1::IID),
        ("IDXGIDevice", IDXGIDevice::IID),
        ("IDXGIDevice1", IDXGIDevice1::IID),
    ] {
        let mut p = std::ptr::null_mut();
        let hr = unsafe { device.query(&iid, &mut p) };
        println!("device {name} {:08x}", hr.0 as u32);
        if !p.is_null() {
            drop(unsafe { IUnknown::from_raw(p) });
        }
    }
    let device1: ID3D11Device1 = device.cast()?;
    let dxgi: IDXGIDevice1 = device.cast()?;
    unsafe {
        println!("frame latency {}", dxgi.GetMaximumFrameLatency()?);
        println!("set latency 5 {:08x}", code(dxgi.SetMaximumFrameLatency(5)));
        println!("frame latency {}", dxgi.GetMaximumFrameLatency()?);
        println!("set latency 0 {:08x}", code(dxgi.SetMaximumFrameLatency(0)));
        println!("frame latency {}", dxgi.GetMaximumFrameLatency()?);
        println!(
            "set latency 17 {:08x}",
            code(dxgi.SetMaximumFrameLatency(17))
        );
        println!("frame latency {}", dxgi.GetMaximumFrameLatency()?);
    }

    // The immediate context's.
    for (name, iid) in [
        ("ID3D11DeviceContext1", ID3D11DeviceContext1::IID),
        ("ID3D11Multithread", ID3D11Multithread::IID),
        ("ID3DUserDefinedAnnotation", ID3DUserDefinedAnnotation::IID),
    ] {
        let mut p = std::ptr::null_mut();
        let hr = unsafe { c.query(&iid, &mut p) };
        println!("context {name} {:08x}", hr.0 as u32);
        if !p.is_null() {
            drop(unsafe { IUnknown::from_raw(p) });
        }
    }
    let c1: ID3D11DeviceContext1 = c.cast()?;
    let c1b = unsafe { device1.GetImmediateContext1()? };
    println!(
        "same context1 {}",
        unknown(&c1.cast()?) == unknown(&c1b.cast()?)
    );
    let mt: ID3D11Multithread = c.cast()?;
    unsafe {
        println!("protected {}", mt.GetMultithreadProtected().as_bool());
        println!(
            "set protected was {}",
            mt.SetMultithreadProtected(true).as_bool()
        );
        println!("protected {}", mt.GetMultithreadProtected().as_bool());
        mt.Enter();
        c.Flush();
        mt.Leave();
        println!(
            "set protected was {}",
            mt.SetMultithreadProtected(false).as_bool()
        );
    }
    let note: ID3DUserDefinedAnnotation = c.cast()?;
    unsafe {
        println!("begin event {}", note.BeginEvent(w!("frame")));
        note.SetMarker(w!("marker"));
        println!("end event {}", note.EndEvent());
        println!("annotation status {}", note.GetStatus().as_bool());
    }

    // Direct3D 11.1's states: one object for equal descriptions made either way.
    let mut blend = D3D11_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = 0xf;
    blend.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    let mut blend1 = D3D11_BLEND_DESC1::default();
    blend1.RenderTarget[0].RenderTargetWriteMask = 0xf;
    blend1.RenderTarget[0].SrcBlend = D3D11_BLEND_SRC_ALPHA;
    blend1.RenderTarget[0].LogicOp = D3D11_LOGIC_OP_NOOP;
    let (mut b0, mut b1) = (None, None);
    unsafe {
        device.CreateBlendState(&blend, Some(&mut b0))?;
        device1.CreateBlendState1(&blend1, Some(&mut b1))?;
    }
    let (b0, b1) = (b0.unwrap(), b1.unwrap());
    println!(
        "same blend {}",
        unknown(&b0.cast()?) == unknown(&b1.cast()?)
    );
    let mut d1 = D3D11_BLEND_DESC1::default();
    unsafe { b0.cast::<ID3D11BlendState1>()?.GetDesc1(&mut d1) };
    let t = &d1.RenderTarget[0];
    println!(
        "blend desc1 {} {} {} {} {} {} {}",
        t.BlendEnable.as_bool(),
        t.LogicOpEnable.as_bool(),
        t.SrcBlend.0,
        t.DestBlend.0,
        t.LogicOp.0,
        t.RenderTargetWriteMask,
        d1.RenderTarget[3].LogicOp.0
    );
    let raster1 = D3D11_RASTERIZER_DESC1 {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_BACK,
        DepthClipEnable: true.into(),
        ..Default::default()
    };
    let mut r1 = None;
    unsafe { device1.CreateRasterizerState1(&raster1, Some(&mut r1))? };
    let mut rd = D3D11_RASTERIZER_DESC1::default();
    unsafe { r1.unwrap().GetDesc1(&mut rd) };
    println!(
        "raster desc1 {} {} {} {}",
        rd.FillMode.0,
        rd.CullMode.0,
        rd.DepthClipEnable.as_bool(),
        rd.ForcedSampleCount
    );

    // Constant buffer ranges, and what a whole buffer reads back as.
    let cb_desc = D3D11_BUFFER_DESC {
        ByteWidth: 4096,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        ..Default::default()
    };
    let mut cb = None;
    unsafe { device.CreateBuffer(&cb_desc, None, Some(&mut cb))? };
    let cb = cb.unwrap();
    let ranges = |label: &str| {
        let mut out: [Option<ID3D11Buffer>; 3] = [None, None, None];
        let (mut first, mut count) = ([7u32; 3], [7u32; 3]);
        unsafe {
            c1.VSGetConstantBuffers1(
                0,
                3,
                Some(out.as_mut_ptr()),
                Some(first.as_mut_ptr()),
                Some(count.as_mut_ptr()),
            )
        };
        let bound: Vec<bool> = out.iter().map(Option::is_some).collect();
        println!("{label} bound {bound:?} first {first:?} count {count:?}");
    };
    unsafe {
        c1.VSSetConstantBuffers1(
            0,
            2,
            Some([Some(cb.clone()), Some(cb.clone())].as_ptr()),
            Some([16, 32].as_ptr()),
            Some([16, 48].as_ptr()),
        );
    }
    ranges("ranges");
    unsafe { c.VSSetConstantBuffers(1, Some(&[Some(cb.clone())])) };
    ranges("whole");
    unsafe { c1.VSSetConstantBuffers1(0, 1, Some([Some(cb.clone())].as_ptr()), None, None) };
    ranges("null ranges");

    // DXGI on resources of each kind.
    let tex1 = |mips, layers| -> windows::core::Result<ID3D11Resource> {
        let mut t = None;
        unsafe {
            device.CreateTexture1D(
                &D3D11_TEXTURE1D_DESC {
                    Width: 64,
                    MipLevels: mips,
                    ArraySize: layers,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut t),
            )?
        };
        t.unwrap().cast()
    };
    let tex2 = |mips, layers, bind: u32| -> windows::core::Result<ID3D11Resource> {
        let mut t = None;
        unsafe {
            device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: 64,
                    Height: 32,
                    MipLevels: mips,
                    ArraySize: layers,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: bind,
                    ..Default::default()
                },
                None,
                Some(&mut t),
            )?
        };
        t.unwrap().cast()
    };
    let mut t3 = None;
    unsafe {
        device.CreateTexture3D(
            &D3D11_TEXTURE3D_DESC {
                Width: 8,
                Height: 8,
                Depth: 8,
                MipLevels: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut t3),
        )?
    };
    let srv = D3D11_BIND_SHADER_RESOURCE.0 as u32;
    let rt = D3D11_BIND_RENDER_TARGET.0 as u32;
    let resources: Vec<(&str, ID3D11Resource)> = vec![
        ("buffer", cb.cast()?),
        ("1d", tex1(1, 1)?),
        ("1d mips", tex1(0, 1)?),
        ("1d array", tex1(1, 2)?),
        ("2d", tex2(1, 1, srv | rt)?),
        ("2d target", tex2(1, 1, rt)?),
        ("2d mips", tex2(2, 1, srv)?),
        ("2d array", tex2(1, 2, srv)?),
        ("3d", t3.unwrap().cast()?),
    ];
    let guid = GUID::from_u128(0x6a4c1d1e_0f7b_4b5e_9a27_4c0d2b1e8f33);
    for (name, r) in &resources {
        let resource = r.cast::<IDXGIResource>();
        let surface = r.cast::<IDXGISurface>();
        println!(
            "{name} resource {:08x} surface {:08x}",
            code(resource.as_ref().map(drop).map_err(Clone::clone)),
            code(surface.as_ref().map(drop).map_err(Clone::clone))
        );
        let Ok(resource) = resource else { continue };
        unsafe {
            println!("{name} usage {:x}", resource.GetUsage()?.0);
            println!("{name} priority {:08x}", resource.GetEvictionPriority()?.0);
            let hr = resource.GetParent::<IDXGISurface>().map(drop);
            println!("{name} parent surface {:08x}", code(hr));
            let parent = resource.GetParent::<IDXGIDevice>()?;
            println!(
                "{name} parent is device {}",
                unknown(&parent.cast()?) == unknown(&device.cast()?)
            );
            let d: ID3D11Device = resource.GetDevice()?;
            println!(
                "{name} device is device {}",
                unknown(&d.cast()?) == unknown(&device.cast()?)
            );
            let value = 0x1234u32;
            resource.SetPrivateData(&guid, 4, (&raw const value).cast())?;
            let (mut got, mut size) = (0u32, 4u32);
            r.GetPrivateData(&guid, &mut size, Some((&raw mut got).cast()))?;
            println!("{name} private data {got:x} {size}");
            let sub: IDXGIDeviceSubObject = resource.cast()?;
            let object: IDXGIObject = resource.cast()?;
            println!(
                "{name} sub-object is resource {} object {}",
                sub.as_raw() == resource.as_raw(),
                object.as_raw() == resource.as_raw()
            );
            if let Ok(surface) = surface {
                let d = surface.GetDesc()?;
                println!(
                    "{name} surface desc {} {} {} {} {}",
                    d.Width, d.Height, d.Format.0, d.SampleDesc.Count, d.SampleDesc.Quality
                );
                let back: IDXGIResource = surface.cast()?;
                let sub: IDXGIDeviceSubObject = surface.cast()?;
                println!(
                    "{name} surface to resource {} sub-object {}",
                    back.as_raw() == resource.as_raw(),
                    sub.as_raw() == resource.as_raw()
                );
            }
        }
    }

    // Failed creations and maps.
    let mut out: Option<ID3D11Texture2D> =
        Some(unsafe { ID3D11Texture2D::from_raw(0xdead_beef_usize as *mut c_void) });
    let bad = D3D11_TEXTURE2D_DESC {
        Width: 0,
        Height: 4,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: srv,
        ..Default::default()
    };
    let hr = code(unsafe { device.CreateTexture2D(&bad, None, Some(&mut out)) });
    println!("failed creation {hr:08x} out null {}", out.is_none());
    std::mem::forget(out);
    let mut out: Option<ID3D11RenderTargetView> =
        Some(unsafe { ID3D11RenderTargetView::from_raw(0xdead_beef_usize as *mut c_void) });
    let hr = code(unsafe { device.CreateRenderTargetView(&resources[6].1, None, Some(&mut out)) });
    println!("failed view {hr:08x} out null {}", out.is_none());
    std::mem::forget(out);
    let mut mapped = D3D11_MAPPED_SUBRESOURCE {
        pData: 0xdead_beef_usize as *mut c_void,
        RowPitch: 0xabab,
        DepthPitch: 0xcdcd,
    };
    let hr = code(unsafe { c.Map(&cb, 1, D3D11_MAP_READ, 0, Some(&mut mapped)) });
    println!(
        "failed map {hr:08x} data null {} pitches {:x} {:x}",
        mapped.pData.is_null(),
        mapped.RowPitch,
        mapped.DepthPitch
    );
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
