//! A small program on Direct3D 10's device inside a Direct3D 11 device: `ID3D10Device1` before
//! and after a context state is made, its calls while a Direct3D 11 state is active, then a
//! context state emulating Direct3D 10 swapped in and a triangle drawn, copied and mapped through
//! Direct3D 10's calls only (with an occlusion query and an event), Direct3D 11's context calls
//! while it is active, and the state each side keeps across the swaps. Prints the answers, for
//! comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::ffi::c_void;

    use windows::Win32::Foundation::{HMODULE, RECT};
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D10::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::{BOOL, IUnknown, Interface, PCSTR, s};

    let code = |r: windows::core::Result<()>| r.err().map_or(0, |e| e.code().0) as u32;
    let compile = |src: &str, target: PCSTR| -> Vec<u8> {
        let (mut blob, mut errors) = (None, None);
        let r = unsafe {
            D3DCompile(
                src.as_ptr().cast(),
                src.len(),
                PCSTR::null(),
                None,
                None::<&ID3DInclude>,
                s!("main"),
                target,
                0,
                0,
                &mut blob,
                Some(&mut errors),
            )
        };
        if let Err(e) = r {
            let text = errors.map(|b| unsafe {
                String::from_utf8_lossy(std::slice::from_raw_parts(
                    b.GetBufferPointer().cast::<u8>(),
                    b.GetBufferSize(),
                ))
                .into_owned()
            });
            panic!("compile: {e} {text:?}");
        }
        let blob = blob.unwrap();
        unsafe {
            std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize())
                .to_vec()
        }
    };
    let query_hr = |o: &IUnknown, iid: &windows::core::GUID| -> u32 {
        let mut p = std::ptr::null_mut();
        let hr = unsafe { o.query(iid, &mut p) };
        if !p.is_null() {
            drop(unsafe { IUnknown::from_raw(p) });
        }
        hr.0 as u32
    };

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
    let device1: ID3D11Device1 = device.cast()?;
    let c1: ID3D11DeviceContext1 = c.cast()?;
    let unknown: IUnknown = device.cast()?;
    println!(
        "before a state ID3D10Device {:08x} ID3D10Device1 {:08x}",
        query_hr(&unknown, &ID3D10Device::IID),
        query_hr(&unknown, &ID3D10Device1::IID)
    );

    let state = |iid: windows::core::GUID, level: D3D_FEATURE_LEVEL| {
        let mut chosen = D3D_FEATURE_LEVEL(0);
        let mut made = None;
        let hr = unsafe {
            device1.CreateDeviceContextState(
                0,
                &[level],
                D3D11_SDK_VERSION,
                &iid,
                Some(&mut chosen),
                Some(&mut made),
            )
        };
        println!("state made {:08x} level {:x}", code(hr), chosen.0);
        made.unwrap()
    };
    let state11 = state(ID3D11Device1::IID, D3D_FEATURE_LEVEL_11_0);
    println!(
        "after a state ID3D10Device {:08x} ID3D10Device1 {:08x}",
        query_hr(&unknown, &ID3D10Device::IID),
        query_hr(&unknown, &ID3D10Device1::IID)
    );
    let d10: ID3D10Device1 = device.cast()?;
    let same = |a: &IUnknown, b: &IUnknown| a.as_raw() == b.as_raw();
    println!("same object {}", same(&d10.cast::<IUnknown>()?, &unknown));
    unsafe {
        println!("feature level {:x}", d10.GetFeatureLevel().0);
    }

    // Direct3D 10's calls while Direct3D 11's behaviour is active.
    unsafe {
        d10.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        println!(
            "11 mode: 10 set, 11 reads {} 10 reads {}",
            c.IAGetPrimitiveTopology().0,
            d10.IAGetPrimitiveTopology().0
        );
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINELIST);
        println!(
            "11 mode: 11 set, 11 reads {} 10 reads {}",
            c.IAGetPrimitiveTopology().0,
            d10.IAGetPrimitiveTopology().0
        );
        c.RSSetViewports(Some(&[D3D11_VIEWPORT {
            TopLeftX: 1.0,
            TopLeftY: 2.0,
            Width: 3.0,
            Height: 4.0,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]));
        let mut n = 4;
        let mut vp = [D3D10_VIEWPORT::default(); 4];
        d10.RSGetViewports(&mut n, Some(vp.as_mut_ptr()));
        println!(
            "11 mode: 10 reads {n} viewports, first {} {} {} {}",
            vp[0].TopLeftX, vp[0].TopLeftY, vp[0].Width, vp[0].Height
        );
        let desc = D3D10_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D10_USAGE_DEFAULT,
            BindFlags: D3D10_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut b = None;
        let hr = d10.CreateBuffer(&desc, None, Some(&mut b));
        println!("11 mode: 10 CreateBuffer {:08x} {}", code(hr), b.is_some());
        if let Some(b) = &b {
            let parent = b.GetDevice().ok();
            println!("buffer's 10 device given {}", parent.is_some());
            let b11: ID3D11Buffer = b.cast()?;
            let mut d = D3D11_BUFFER_DESC::default();
            b11.GetDesc(&mut d);
            println!("buffer's 11 bind {:x} usage {}", d.BindFlags, d.Usage.0);
        }
    }

    // A state emulating Direct3D 10 swapped in.
    let state10 = state(ID3D10Device1::IID, D3D_FEATURE_LEVEL_10_1);
    let mut previous = None;
    unsafe { c1.SwapDeviceContextState(&state10, Some(&mut previous)) };
    println!("previous given {}", previous.is_some());
    drop(state10);

    unsafe {
        println!(
            "10 mode: fresh 11 reads {} 10 reads {}",
            c.IAGetPrimitiveTopology().0,
            d10.IAGetPrimitiveTopology().0
        );
        c.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINESTRIP);
        println!(
            "10 mode: 11 set, 11 reads {} 10 reads {}",
            c.IAGetPrimitiveTopology().0,
            d10.IAGetPrimitiveTopology().0
        );
        d10.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        println!(
            "10 mode: 10 set, 11 reads {} 10 reads {}",
            c.IAGetPrimitiveTopology().0,
            d10.IAGetPrimitiveTopology().0
        );
    }

    // A triangle drawn through Direct3D 10's calls only.
    let vs = compile(
        "float4 main(float2 p : POSITION) : SV_Position { return float4(p, 0.5, 1); }",
        s!("vs_4_0"),
    );
    let ps = compile(
        "cbuffer c : register(b0) { float4 colour; }\n\
         float4 main() : SV_Target { return colour; }",
        s!("ps_4_0"),
    );
    unsafe {
        let texture =
            |bind: u32, usage: D3D10_USAGE, cpu: u32| -> windows::core::Result<ID3D10Texture2D> {
                d10.CreateTexture2D(
                    &D3D10_TEXTURE2D_DESC {
                        Width: 4,
                        Height: 4,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: usage,
                        BindFlags: bind,
                        CPUAccessFlags: cpu,
                        MiscFlags: 0,
                    },
                    None,
                )
            };
        let target = texture(D3D10_BIND_RENDER_TARGET.0 as u32, D3D10_USAGE_DEFAULT, 0)?;
        let staging = texture(0, D3D10_USAGE_STAGING, D3D10_CPU_ACCESS_READ.0 as u32)?;
        let mut rtv = None;
        println!(
            "10 mode: target view {:08x}",
            code(d10.CreateRenderTargetView(&target, None, Some(&mut rtv)))
        );
        let rtv = rtv.unwrap();
        let vertices: [f32; 6] = [-1.0, -1.0, -1.0, 1.0, 1.0, -1.0];
        let mut vb = None;
        d10.CreateBuffer(
            &D3D10_BUFFER_DESC {
                ByteWidth: 24,
                Usage: D3D10_USAGE_IMMUTABLE,
                BindFlags: D3D10_BIND_VERTEX_BUFFER.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            },
            Some(&D3D10_SUBRESOURCE_DATA {
                pSysMem: vertices.as_ptr().cast(),
                SysMemPitch: 0,
                SysMemSlicePitch: 0,
            }),
            Some(&mut vb),
        )?;
        let mut cb = None;
        d10.CreateBuffer(
            &D3D10_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D10_USAGE_DEFAULT,
                BindFlags: D3D10_BIND_CONSTANT_BUFFER.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            },
            None,
            Some(&mut cb),
        )?;
        let colour: [f32; 4] = [1.0, 0.5, 0.0, 1.0];
        d10.UpdateSubresource(cb.as_ref().unwrap(), 0, None, colour.as_ptr().cast(), 0, 0);
        let mut layout = None;
        d10.CreateInputLayout(
            &[D3D10_INPUT_ELEMENT_DESC {
                SemanticName: s!("POSITION"),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D10_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            }],
            &vs,
            Some(&mut layout),
        )?;
        let (mut v, mut p) = (None, None);
        d10.CreateVertexShader(&vs, Some(&mut v))?;
        d10.CreatePixelShader(&ps, Some(&mut p))?;
        let mut blend = D3D10_BLEND_DESC {
            SrcBlend: D3D10_BLEND_ONE,
            DestBlend: D3D10_BLEND_ZERO,
            BlendOp: D3D10_BLEND_OP_ADD,
            SrcBlendAlpha: D3D10_BLEND_ONE,
            DestBlendAlpha: D3D10_BLEND_ZERO,
            BlendOpAlpha: D3D10_BLEND_OP_ADD,
            ..Default::default()
        };
        blend.RenderTargetWriteMask = [D3D10_COLOR_WRITE_ENABLE_ALL.0 as u8; 8];
        blend.RenderTargetWriteMask[0] = (D3D10_COLOR_WRITE_ENABLE_RED.0
            | D3D10_COLOR_WRITE_ENABLE_GREEN.0
            | D3D10_COLOR_WRITE_ENABLE_ALPHA.0) as u8;
        let mut bs = None;
        d10.CreateBlendState(&blend, Some(&mut bs))?;

        let vb = vb.unwrap();
        let (stride, offset) = (8u32, 0u32);
        d10.IASetVertexBuffers(0, 1, Some(&Some(vb.clone())), Some(&stride), Some(&offset));
        d10.IASetInputLayout(layout.as_ref());
        d10.VSSetShader(v.as_ref());
        d10.PSSetShader(p.as_ref());
        d10.PSSetConstantBuffers(0, Some(&[cb.clone()]));
        d10.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        d10.OMSetBlendState(bs.as_ref(), &[0.0; 4], 0xffff_ffff);
        d10.RSSetViewports(Some(&[D3D10_VIEWPORT {
            TopLeftX: 0,
            TopLeftY: 0,
            Width: 4,
            Height: 4,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }]));
        d10.ClearRenderTargetView(&rtv, &[0.25, 0.5, 0.75, 1.0]);

        let mut occlusion = None;
        d10.CreateQuery(
            &D3D10_QUERY_DESC {
                Query: D3D10_QUERY_OCCLUSION,
                MiscFlags: 0,
            },
            Some(&mut occlusion),
        )?;
        let occlusion = occlusion.unwrap();
        occlusion.Begin();
        d10.Draw(3, 0);
        occlusion.End();
        let get = |q: &ID3D10Query, data: *mut c_void, size: u32| -> u32 {
            let a: ID3D10Asynchronous = q.cast().unwrap();
            (Interface::vtable(&a).GetData)(a.as_raw(), data, size, 0).0 as u32
        };
        let mut samples = 0u64;
        while get(&occlusion, (&mut samples as *mut u64).cast(), 8) != 0 {}
        println!(
            "10 mode: occlusion {samples} size {}",
            occlusion.GetDataSize()
        );
        for (name, kind) in [
            ("event", D3D10_QUERY_EVENT),
            ("statistics", D3D10_QUERY_PIPELINE_STATISTICS),
            ("disjoint", D3D10_QUERY_TIMESTAMP_DISJOINT),
        ] {
            let mut q = None;
            d10.CreateQuery(
                &D3D10_QUERY_DESC {
                    Query: kind,
                    MiscFlags: 0,
                },
                Some(&mut q),
            )?;
            println!("10 mode: {name} size {}", q.unwrap().GetDataSize());
        }

        d10.CopyResource(&staging, &target);
        let mapped = staging.Map(0, D3D10_MAP_READ, 0)?;
        for y in 0..4 {
            let row = std::slice::from_raw_parts(
                mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                16,
            );
            println!("10 mode: row {y} {row:02x?}");
        }
        staging.Unmap(0);

        // What Direct3D 10's getters give back.
        println!(
            "10 mode: vertex shader back {}",
            d10.VSGetShader().ok().map(|s| s.as_raw()) == v.as_ref().map(|s| s.as_raw())
        );
        let (mut back_bs, mut factor, mut mask) = (None, [9.0f32; 4], 0u32);
        d10.OMGetBlendState(Some(&mut back_bs), Some(&mut factor), Some(&mut mask));
        println!(
            "10 mode: blend back {} factor {factor:?} mask {mask:x}",
            back_bs.map(|b| b.as_raw()) == bs.as_ref().map(|b| b.as_raw())
        );
        let mut n = 4;
        let mut vp = [D3D10_VIEWPORT::default(); 4];
        d10.RSGetViewports(&mut n, Some(vp.as_mut_ptr()));
        println!(
            "10 mode: 10 reads {n} viewports, first {} {} {} {}",
            vp[0].TopLeftX, vp[0].TopLeftY, vp[0].Width, vp[0].Height
        );
        let (mut back_vb, mut s2, mut o2) = (None, 0u32, 9u32);
        d10.IAGetVertexBuffers(0, 1, Some(&mut back_vb), Some(&mut s2), Some(&mut o2));
        println!(
            "10 mode: vertex buffer back {} stride {s2} offset {o2}",
            back_vb.map(|b| b.as_raw()) == Some(vb.as_raw())
        );
        let mut targets = [None, None];
        d10.OMGetRenderTargets(Some(&mut targets), None);
        println!(
            "10 mode: targets back {} {}",
            targets[0].as_ref().map(|t| t.as_raw()) == Some(rtv.as_raw()),
            targets[1].is_some()
        );
        let mut rects = [RECT::default(); 2];
        let mut nr = 2;
        d10.RSGetScissorRects(&mut nr, Some(rects.as_mut_ptr()));
        println!("10 mode: scissor rects {nr}");

        // Direct3D 11's context calls while Direct3D 10's behaviour is active.
        let mut v11 = None;
        c.VSGetShader(&mut v11, None, None);
        println!("10 mode: 11 reads a vertex shader {}", v11.is_some());
        let target11: ID3D11Resource = target.cast()?;
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        println!(
            "10 mode: 11 Map {:08x}",
            code(c.Map(&target11, 0, D3D11_MAP_READ, 0, Some(&mut m)))
        );
        c.ClearState();
        println!(
            "10 mode: after 11 ClearState 10 reads {}",
            d10.IAGetPrimitiveTopology().0
        );
        let mut pred = None;
        let mut value = BOOL(7);
        c.GetPredication(Some(&mut pred), Some(&mut value));
        println!("10 mode: 11 predication {} {}", pred.is_some(), value.0);
    }

    // Back to Direct3D 11's state.
    let mut ours10 = None;
    unsafe { c1.SwapDeviceContextState(previous.as_ref(), Some(&mut ours10)) };
    unsafe {
        println!(
            "back: 11 reads {} 10 reads {}",
            c.IAGetPrimitiveTopology().0,
            d10.IAGetPrimitiveTopology().0
        );
        d10.ClearState();
        println!(
            "back: after 10 ClearState 11 reads {}",
            c.IAGetPrimitiveTopology().0
        );
    }
    unsafe { c1.SwapDeviceContextState(ours10.as_ref(), None) };
    unsafe {
        println!(
            "10 again: 11 reads {} 10 reads {}",
            c.IAGetPrimitiveTopology().0,
            d10.IAGetPrimitiveTopology().0
        );
    }
    drop(state11);
    println!("done");
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
