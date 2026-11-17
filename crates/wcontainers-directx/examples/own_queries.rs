//! A small Direct3D 11 program on queries: an event polled to completion, occlusion predicates
//! around draws that pass samples and draws scissored to nothing, predicates that overlap, a
//! predicate ended twice, `GetData` before an end and with wrong sizes, and the interfaces and sizes
//! of each. Prints the answers, for comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::{HMODULE, RECT};
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::Interface;

    const VS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/common.vs_full.dxbc");
    const PS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/textures.ps_load.dxbc");
    const SIZE: u32 = 16;

    let code = |r: windows::core::Result<()>| r.err().map_or(0, |e| e.code().0) as u32;
    let (mut device, mut context) = (None, None);
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?
    };
    let (device, c): (ID3D11Device, ID3D11DeviceContext) = (device.unwrap(), context.unwrap());

    let desc = D3D11_TEXTURE2D_DESC {
        Width: SIZE,
        Height: SIZE,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        ..Default::default()
    };
    let (mut target, mut rtv, mut vs, mut ps) = (None, None, None, None);
    let mut scissored = None;
    unsafe {
        device.CreateTexture2D(&desc, None, Some(&mut target))?;
        device.CreateRenderTargetView(target.as_ref().unwrap(), None, Some(&mut rtv))?;
        device.CreateVertexShader(VS, None, Some(&mut vs))?;
        device.CreatePixelShader(PS, None, Some(&mut ps))?;
        device.CreateRasterizerState(
            &D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthClipEnable: true.into(),
                ScissorEnable: true.into(),
                ..Default::default()
            },
            Some(&mut scissored),
        )?;
        c.OMSetRenderTargets(Some(&[rtv.clone()]), None);
        c.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
        c.RSSetScissorRects(Some(&[RECT::default()]));
        c.VSSetShader(vs.as_ref(), None);
        c.PSSetShader(ps.as_ref(), None);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    }
    // A draw over the whole target, and one scissored to nothing.
    let full = || unsafe {
        c.RSSetState(None);
        c.Draw(3, 0);
    };
    let empty = || unsafe {
        c.RSSetState(scissored.as_ref());
        c.Draw(3, 0);
    };
    let make = |kind: D3D11_QUERY| -> windows::core::Result<ID3D11Query> {
        let mut q = None;
        unsafe {
            device.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: kind,
                    MiscFlags: 0,
                },
                Some(&mut q),
            )?
        };
        Ok(q.unwrap())
    };
    // `GetData`'s own HRESULT (the windows crate's wrapper reads `S_FALSE` as success).
    let get = |q: &ID3D11Query, data: *mut std::ffi::c_void, size: u32| -> u32 {
        let a: ID3D11Asynchronous = q.cast().unwrap();
        unsafe { (Interface::vtable(&c).GetData)(Interface::as_raw(&c), a.as_raw(), data, size, 0) }
            .0 as u32
    };
    // Polls a query until it answers, flushing, and prints the answer.
    let poll = |name: &str, q: &ID3D11Query| {
        let mut value = 0xffff_ffffu32;
        let mut hr = 1;
        for _ in 0..10_000_000 {
            hr = get(q, (&mut value as *mut u32).cast(), 4);
            if hr != 1 {
                break;
            }
        }
        println!("{name} GetData {hr:#010x} {value}");
    };

    let event = make(D3D11_QUERY_EVENT)?;
    full();
    unsafe { c.End(&event) };
    poll("event", &event);

    let a = make(D3D11_QUERY_OCCLUSION_PREDICATE)?;
    println!("fresh GetData {:#010x}", get(&a, std::ptr::null_mut(), 0));
    unsafe { c.Begin(&a) };
    println!("running GetData {:#010x}", get(&a, std::ptr::null_mut(), 0));
    full();
    unsafe { c.End(&a) };
    poll("drawn", &a);
    let mut word = 0u16;
    let r = get(&a, (&mut word as *mut u16).cast(), 2);
    println!("two bytes GetData {r:#010x}");
    println!(
        "no pointer GetData {:#010x}",
        get(&a, std::ptr::null_mut(), 4)
    );
    println!("no data GetData {:#010x}", get(&a, std::ptr::null_mut(), 0));

    let b = make(D3D11_QUERY_OCCLUSION_PREDICATE)?;
    unsafe { c.Begin(&b) };
    empty();
    unsafe { c.End(&b) };
    poll("scissored", &b);

    // Overlapping: `outer` around everything, `inner` around one scissored draw, `late` from the
    // full draw past the outer's end.
    let (outer, inner, late) = (
        make(D3D11_QUERY_OCCLUSION_PREDICATE)?,
        make(D3D11_QUERY_OCCLUSION_PREDICATE)?,
        make(D3D11_QUERY_OCCLUSION_PREDICATE)?,
    );
    unsafe { c.Begin(&outer) };
    empty();
    unsafe { c.Begin(&inner) };
    empty();
    unsafe { c.End(&inner) };
    unsafe { c.Begin(&late) };
    full();
    unsafe { c.End(&outer) };
    empty();
    unsafe { c.End(&late) };
    poll("outer", &outer);
    poll("inner", &inner);
    poll("late", &late);

    let twice = make(D3D11_QUERY_OCCLUSION_PREDICATE)?;
    unsafe {
        c.Begin(&twice);
        full();
        c.End(&twice);
        c.End(&twice);
    }
    poll("ended twice", &twice);

    for (name, kind) in [
        ("event", D3D11_QUERY_EVENT),
        ("predicate", D3D11_QUERY_OCCLUSION_PREDICATE),
    ] {
        let q = make(kind)?;
        let size = unsafe { q.GetDataSize() };
        let d = unsafe { q.GetDesc() };
        let p = q.cast::<ID3D11Predicate>().map(|_| ());
        println!(
            "{name} size {size} desc {} {} predicate {:#010x}",
            d.Query.0,
            d.MiscFlags,
            code(p)
        );
    }
    let mut p = None;
    let r = unsafe {
        device.CreatePredicate(
            &D3D11_QUERY_DESC {
                Query: D3D11_QUERY_EVENT,
                MiscFlags: 0,
            },
            Some(&mut p),
        )
    };
    println!("CreatePredicate(event) {:#010x}", code(r));
    let r = unsafe {
        device.CreatePredicate(
            &D3D11_QUERY_DESC {
                Query: D3D11_QUERY_OCCLUSION_PREDICATE,
                MiscFlags: 0,
            },
            Some(&mut p),
        )
    };
    println!("CreatePredicate(occlusion) {:#010x}", code(r));

    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
