//! A small Direct3D 11 program on queries: an event polled to completion, occlusion predicates
//! around draws that pass samples and draws scissored to nothing, predicates that overlap, a
//! predicate ended twice, `GetData` before an end and with wrong sizes, counted occlusion queries
//! (overlapping, against depth the draws write, across a depth clear and a buffer write), pipeline
//! statistics, timestamps and their disjoint query answering, and the interfaces and sizes of each. Prints the answers, for
//! comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::{HMODULE, RECT};
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::Interface;

    const VS: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/common.vs_full.dxbc");
    const PS: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/textures.ps_load.dxbc");
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

    // Counted: every draw covers the 16 by 16 target once.
    let poll64 = |name: &str, q: &ID3D11Query| {
        let mut value = u64::MAX;
        let mut hr = 1;
        for _ in 0..10_000_000 {
            hr = get(q, (&mut value as *mut u64).cast(), 8);
            if hr != 1 {
                break;
            }
        }
        println!("{name} GetData {hr:#010x} {value}");
    };
    let (outer, inner, none) = (
        make(D3D11_QUERY_OCCLUSION)?,
        make(D3D11_QUERY_OCCLUSION)?,
        make(D3D11_QUERY_OCCLUSION)?,
    );
    unsafe {
        c.Begin(&outer);
        full();
        c.Begin(&inner);
        full();
        empty();
        c.End(&inner);
        c.Begin(&none);
        empty();
        c.End(&outer);
        c.End(&none);
    }
    poll64("counted outer", &outer);
    poll64("counted inner", &inner);
    poll64("counted scissored", &none);

    // Against depth: the second draw at the same depth fails the test; after a clear, and a write
    // of a buffer the draws do not read, it passes again.
    let (mut depth, mut dsv, mut less, mut cb) = (None, None, None, None);
    unsafe {
        device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
                BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
                ..desc
            },
            None,
            Some(&mut depth),
        )?;
        device.CreateDepthStencilView(depth.as_ref().unwrap(), None, Some(&mut dsv))?;
        device.CreateDepthStencilState(
            &D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D11_COMPARISON_LESS,
                ..Default::default()
            },
            Some(&mut less),
        )?;
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut cb),
        )?;
        let dsv = dsv.as_ref().unwrap();
        c.OMSetRenderTargets(Some(&[rtv.clone()]), dsv);
        c.OMSetDepthStencilState(less.as_ref(), 0);
        c.ClearDepthStencilView(dsv, D3D11_CLEAR_DEPTH.0, 1.0, 0);
        let q = make(D3D11_QUERY_OCCLUSION)?;
        c.Begin(&q);
        full();
        full();
        c.End(&q);
        poll64("counted depth", &q);
        c.Begin(&q);
        full();
        c.ClearDepthStencilView(dsv, D3D11_CLEAR_DEPTH.0, 1.0, 0);
        full();
        c.UpdateSubresource(
            cb.as_ref().unwrap(),
            0,
            None,
            [0u8; 16].as_ptr().cast(),
            0,
            0,
        );
        full();
        c.End(&q);
        poll64("counted cleared", &q);
        c.OMSetDepthStencilState(None, 0);
        c.OMSetRenderTargets(Some(&[rtv.clone()]), None);
    }

    // Pipeline statistics: the input assembler, vertex shader and clipper counts of draws made
    // directly, instanced, indirectly and scissored to nothing, and of an indexed strip cut in two;
    // pixel shader invocations at least the pixels covered (helper pixels may add some).
    let statistics = |name: &str, q: &ID3D11Query| {
        let mut s = D3D11_QUERY_DATA_PIPELINE_STATISTICS::default();
        let mut hr = 1;
        for _ in 0..10_000_000 {
            hr = get(
                q,
                (&mut s as *mut D3D11_QUERY_DATA_PIPELINE_STATISTICS).cast(),
                88,
            );
            if hr != 1 {
                break;
            }
        }
        // Vertex shader invocations are the device's: it may reuse vertices, as real Direct3D
        // on the RTX 3080 does even in draws without indices.
        println!(
            "{name} GetData {hr:#010x} ia {} {} gs {} {} clipper {} {} hs {} ds {} cs {}",
            s.IAVertices,
            s.IAPrimitives,
            s.GSInvocations,
            s.GSPrimitives,
            s.CInvocations,
            s.CPrimitives,
            s.HSInvocations,
            s.DSInvocations,
            s.CSInvocations
        );
        s.PSInvocations
    };
    let args = {
        let words: Vec<u8> = [3u32, 2, 0, 0]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        let mut b = None;
        unsafe {
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: 16,
                    Usage: D3D11_USAGE_DEFAULT,
                    MiscFlags: D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS.0 as u32,
                    ..Default::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: words.as_ptr().cast(),
                    ..Default::default()
                }),
                Some(&mut b),
            )?
        };
        b.unwrap()
    };
    let indices = {
        let words: Vec<u8> = [0u16, 1, 2, 0xffff, 0, 1, 2, 1]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        let mut b = None;
        unsafe {
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: words.len() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_INDEX_BUFFER.0 as u32,
                    ..Default::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: words.as_ptr().cast(),
                    ..Default::default()
                }),
                Some(&mut b),
            )?
        };
        b.unwrap()
    };
    let (direct, strip) = (
        make(D3D11_QUERY_PIPELINE_STATISTICS)?,
        make(D3D11_QUERY_PIPELINE_STATISTICS)?,
    );
    unsafe {
        c.Begin(&direct);
        full();
        empty();
        c.RSSetState(None);
        c.DrawInstanced(3, 2, 0, 0);
        c.DrawInstancedIndirect(&args, 0);
        c.End(&direct);
    }
    let pixels = statistics("statistics direct", &direct);
    println!(
        "statistics direct pixels at least 4 x 256 {}",
        pixels >= 4 * 256
    );
    unsafe {
        c.Begin(&strip);
        c.IASetIndexBuffer(&indices, DXGI_FORMAT_R16_UINT, 0);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        c.DrawIndexed(8, 0, 0);
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        c.End(&strip);
    }
    let pixels = statistics("statistics strip", &strip);
    println!("statistics strip pixels at least 256 {}", pixels >= 256);
    let unused = make(D3D11_QUERY_PIPELINE_STATISTICS)?;
    unsafe {
        c.Begin(&unused);
        c.End(&unused);
    }
    let pixels = statistics("statistics none", &unused);
    println!("statistics none pixels {pixels}");

    // Stream output's queries around a draw that streams nothing out.
    let (so, overflow) = (
        make(D3D11_QUERY_SO_STATISTICS)?,
        make(D3D11_QUERY_SO_OVERFLOW_PREDICATE)?,
    );
    unsafe {
        c.Begin(&so);
        c.Begin(&overflow);
        full();
        c.End(&overflow);
        c.End(&so);
    }
    let mut written = [u64::MAX; 2];
    let mut hr = 1;
    for _ in 0..10_000_000 {
        hr = get(&so, written.as_mut_ptr().cast(), 16);
        if hr != 1 {
            break;
        }
    }
    println!("stream output statistics GetData {hr:#010x} {written:?}");
    poll("stream output overflow", &overflow);

    // Timestamps answer; their values, and whether the counter was continuous, are the device's.
    let (disjoint, start, end) = (
        make(D3D11_QUERY_TIMESTAMP_DISJOINT)?,
        make(D3D11_QUERY_TIMESTAMP)?,
        make(D3D11_QUERY_TIMESTAMP)?,
    );
    unsafe {
        c.Begin(&disjoint);
        c.End(&start);
        full();
        c.End(&end);
        c.End(&disjoint);
    }
    let mut stamp = [0u8; 16];
    for (name, q, size) in [("disjoint", &disjoint, 16), ("timestamp", &end, 8)] {
        let mut hr = 1;
        for _ in 0..10_000_000 {
            hr = get(q, stamp.as_mut_ptr().cast(), size);
            if hr != 1 {
                break;
            }
        }
        println!("{name} GetData {hr:#010x}");
    }

    for (name, kind) in [
        ("event", D3D11_QUERY_EVENT),
        ("predicate", D3D11_QUERY_OCCLUSION_PREDICATE),
        ("occlusion", D3D11_QUERY_OCCLUSION),
        ("timestamp", D3D11_QUERY_TIMESTAMP),
        ("disjoint", D3D11_QUERY_TIMESTAMP_DISJOINT),
        ("statistics", D3D11_QUERY_PIPELINE_STATISTICS),
        (
            "stream output statistics",
            D3D11_QUERY_SO_STATISTICS_STREAM2,
        ),
        (
            "stream output overflow",
            D3D11_QUERY_SO_OVERFLOW_PREDICATE_STREAM3,
        ),
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
