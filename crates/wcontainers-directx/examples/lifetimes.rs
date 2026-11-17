//! A Direct3D 11 program making a seeded random sequence of object lifetimes: buffers, textures,
//! views and blend states made, referenced, released, bound to the context, fetched back from it
//! and from views. After each step it prints the device's reference count and that of every object
//! it holds, so runs beside the system's library and beside ours can be compared line by line.
//! Usage: `lifetimes <seed> <steps>`.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::collections::HashMap;

    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::{IUnknown, Interface};

    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(1);
    let steps: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(200);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

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
    let (device, context): (ID3D11Device, ID3D11DeviceContext) =
        (device.unwrap(), context.unwrap());

    /// The reference count, read by adding a reference and releasing it.
    fn refs(i: &IUnknown) -> u32 {
        unsafe {
            let raw = i.as_raw();
            (Interface::vtable(i).AddRef)(raw);
            (Interface::vtable(i).Release)(raw)
        }
    }

    /// One reference the program holds, on object number `id`.
    #[derive(Clone)]
    enum Obj {
        Buffer(ID3D11Buffer),
        Texture(ID3D11Texture2D),
        Rtv(ID3D11RenderTargetView),
        Srv(ID3D11ShaderResourceView),
        Blend(ID3D11BlendState),
    }
    impl Obj {
        fn unknown(&self) -> IUnknown {
            match self {
                Obj::Buffer(o) => o.clone().into(),
                Obj::Texture(o) => o.clone().into(),
                Obj::Rtv(o) => o.clone().into(),
                Obj::Srv(o) => o.clone().into(),
                Obj::Blend(o) => o.clone().into(),
            }
        }
        fn raw(&self) -> *mut std::ffi::c_void {
            match self {
                Obj::Buffer(o) => o.as_raw(),
                Obj::Texture(o) => o.as_raw(),
                Obj::Rtv(o) => o.as_raw(),
                Obj::Srv(o) => o.as_raw(),
                Obj::Blend(o) => o.as_raw(),
            }
        }
    }

    let mut held: Vec<(usize, Obj)> = Vec::new();
    // Object numbers by address, set when an object is made (an address may be reused after).
    let mut ids: HashMap<usize, usize> = HashMap::new();
    let mut next = 0;
    let mut name = |ids: &mut HashMap<usize, usize>, raw: *mut std::ffi::c_void| {
        next += 1;
        ids.insert(raw as usize, next);
        next
    };
    let pick = |rng: &mut ChaCha8Rng, held: &[(usize, Obj)], f: &dyn Fn(&Obj) -> bool| {
        let c: Vec<usize> = (0..held.len()).filter(|&i| f(&held[i].1)).collect();
        (!c.is_empty() && rng.random_bool(0.85)).then(|| c[rng.random_range(0..c.len())])
    };
    let code = |r: windows::core::Result<()>| r.err().map_or(0, |e| e.code().0) as u32;

    for step in 0..steps {
        let op = rng.random_range(0..16);
        let what: String = match op {
            0 => {
                let desc = D3D11_BUFFER_DESC {
                    ByteWidth: 16,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_VERTEX_BUFFER.0 | D3D11_BIND_CONSTANT_BUFFER.0) as u32,
                    ..Default::default()
                };
                let mut out = None;
                let r = unsafe { device.CreateBuffer(&desc, None, Some(&mut out)) };
                if let Some(b) = out {
                    let id = name(&mut ids, b.as_raw());
                    held.push((id, Obj::Buffer(b)));
                }
                format!("CreateBuffer {:#x}", code(r))
            }
            1 => {
                let desc = D3D11_TEXTURE2D_DESC {
                    Width: 8,
                    Height: 8,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
                    ..Default::default()
                };
                let mut out = None;
                let r = unsafe { device.CreateTexture2D(&desc, None, Some(&mut out)) };
                if let Some(t) = out {
                    let id = name(&mut ids, t.as_raw());
                    held.push((id, Obj::Texture(t)));
                }
                format!("CreateTexture2D {:#x}", code(r))
            }
            2 | 3 => {
                let Some(i) = pick(&mut rng, &held, &|o| matches!(o, Obj::Texture(_))) else {
                    continue;
                };
                let Obj::Texture(t) = held[i].1.clone() else {
                    unreachable!()
                };
                let tid = held[i].0;
                if op == 2 {
                    let mut out = None;
                    let r = unsafe { device.CreateRenderTargetView(&t, None, Some(&mut out)) };
                    if let Some(v) = out {
                        let id = name(&mut ids, v.as_raw());
                        held.push((id, Obj::Rtv(v)));
                    }
                    format!("CreateRenderTargetView of {tid} {:#x}", code(r))
                } else {
                    let mut out = None;
                    let r = unsafe { device.CreateShaderResourceView(&t, None, Some(&mut out)) };
                    if let Some(v) = out {
                        let id = name(&mut ids, v.as_raw());
                        held.push((id, Obj::Srv(v)));
                    }
                    format!("CreateShaderResourceView of {tid} {:#x}", code(r))
                }
            }
            4 => {
                let k = rng.random_range(0..2);
                let mut desc = D3D11_BLEND_DESC::default();
                desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: (k == 1).into(),
                    SrcBlend: D3D11_BLEND_ONE,
                    DestBlend: D3D11_BLEND_ZERO,
                    BlendOp: D3D11_BLEND_OP_ADD,
                    SrcBlendAlpha: D3D11_BLEND_ONE,
                    DestBlendAlpha: D3D11_BLEND_ZERO,
                    BlendOpAlpha: D3D11_BLEND_OP_ADD,
                    RenderTargetWriteMask: 0xf,
                };
                let mut out = None;
                let r = unsafe { device.CreateBlendState(&desc, Some(&mut out)) };
                if let Some(s) = out {
                    // Equal descriptions give one object: it keeps its number while held.
                    let id = match held.iter().find(|(_, o)| o.raw() == s.as_raw()) {
                        Some((id, _)) => *id,
                        None => name(&mut ids, s.as_raw()),
                    };
                    held.push((id, Obj::Blend(s)));
                }
                format!("CreateBlendState {k} {:#x}", code(r))
            }
            5 => {
                let Some(i) = pick(&mut rng, &held, &|_| true) else {
                    continue;
                };
                let (id, _) = held.swap_remove(i);
                format!("Release {id}")
            }
            6 => {
                let Some(i) = pick(&mut rng, &held, &|_| true) else {
                    continue;
                };
                held.push(held[i].clone());
                format!("AddRef {}", held[i].0)
            }
            7 | 8 => {
                let slot = rng.random_range(0..3);
                let b = pick(&mut rng, &held, &|o| matches!(o, Obj::Buffer(_))).map(|i| {
                    let Obj::Buffer(b) = &held[i].1 else {
                        unreachable!()
                    };
                    (held[i].0, b.clone())
                });
                let id = b.as_ref().map_or(0, |b| b.0);
                let b = b.map(|b| b.1);
                unsafe {
                    if op == 7 {
                        context.IASetVertexBuffers(slot, 1, Some(&b), Some(&16), Some(&0));
                    } else {
                        context.VSSetConstantBuffers(slot, Some(&[b]));
                    }
                }
                format!(
                    "Set{} slot {slot} to {id}",
                    ["VertexBuffers", "ConstantBuffers"][op - 7]
                )
            }
            9 => {
                let slot = rng.random_range(0..3);
                let v = pick(&mut rng, &held, &|o| matches!(o, Obj::Srv(_))).map(|i| {
                    let Obj::Srv(v) = &held[i].1 else {
                        unreachable!()
                    };
                    (held[i].0, v.clone())
                });
                let id = v.as_ref().map_or(0, |v| v.0);
                unsafe { context.PSSetShaderResources(slot, Some(&[v.map(|v| v.1)])) };
                format!("PSSetShaderResources slot {slot} to {id}")
            }
            10 => {
                let v = pick(&mut rng, &held, &|o| matches!(o, Obj::Rtv(_))).map(|i| {
                    let Obj::Rtv(v) = &held[i].1 else {
                        unreachable!()
                    };
                    (held[i].0, v.clone())
                });
                let id = v.as_ref().map_or(0, |v| v.0);
                unsafe {
                    context.OMSetRenderTargets(
                        Some(&[v.map(|v| v.1)]),
                        None::<&ID3D11DepthStencilView>,
                    )
                };
                format!("OMSetRenderTargets to {id}")
            }
            11 => {
                let s = pick(&mut rng, &held, &|o| matches!(o, Obj::Blend(_))).map(|i| {
                    let Obj::Blend(s) = &held[i].1 else {
                        unreachable!()
                    };
                    (held[i].0, s.clone())
                });
                let id = s.as_ref().map_or(0, |s| s.0);
                unsafe { context.OMSetBlendState(s.map(|s| s.1).as_ref(), None, u32::MAX) };
                format!("OMSetBlendState to {id}")
            }
            12 => {
                // Every getter: what it hands out is held like any other reference.
                let mut got: Vec<Obj> = Vec::new();
                unsafe {
                    let mut buffers: [Option<ID3D11Buffer>; 3] = Default::default();
                    context.IAGetVertexBuffers(0, 3, Some(buffers.as_mut_ptr()), None, None);
                    got.extend(buffers.into_iter().flatten().map(Obj::Buffer));
                    let mut buffers: [Option<ID3D11Buffer>; 3] = Default::default();
                    context.VSGetConstantBuffers(0, Some(&mut buffers));
                    got.extend(buffers.into_iter().flatten().map(Obj::Buffer));
                    let mut views: [Option<ID3D11ShaderResourceView>; 3] = Default::default();
                    context.PSGetShaderResources(0, Some(&mut views));
                    got.extend(views.into_iter().flatten().map(Obj::Srv));
                    let mut targets: [Option<ID3D11RenderTargetView>; 1] = Default::default();
                    context.OMGetRenderTargets(Some(&mut targets), None);
                    got.extend(targets.into_iter().flatten().map(Obj::Rtv));
                    let mut blend = None;
                    context.OMGetBlendState(Some(&mut blend), None, None);
                    got.extend(blend.map(Obj::Blend));
                }
                let mut list = String::new();
                for o in got {
                    let id = ids[&(o.raw() as usize)];
                    list += &format!(" {id}");
                    held.push((id, o));
                }
                format!("Get{list}")
            }
            13 => {
                let Some(i) = pick(&mut rng, &held, &|o| matches!(o, Obj::Rtv(_) | Obj::Srv(_)))
                else {
                    continue;
                };
                let r = unsafe {
                    match &held[i].1 {
                        Obj::Rtv(v) => v.GetResource(),
                        Obj::Srv(v) => v.GetResource(),
                        _ => unreachable!(),
                    }
                }?;
                let t: ID3D11Texture2D = r.cast()?;
                drop(r);
                let id = ids[&(t.as_raw() as usize)];
                held.push((id, Obj::Texture(t)));
                format!("GetResource of {} gives {id}", held[i].0)
            }
            14 => {
                unsafe { context.ClearState() };
                "ClearState".into()
            }
            _ => {
                let Some(i) = pick(&mut rng, &held, &|_| true) else {
                    continue;
                };
                let owner = match &held[i].1 {
                    Obj::Buffer(o) => unsafe { o.GetDevice() },
                    Obj::Texture(o) => unsafe { o.GetDevice() },
                    Obj::Rtv(o) => unsafe { o.GetDevice() },
                    Obj::Srv(o) => unsafe { o.GetDevice() },
                    Obj::Blend(o) => unsafe { o.GetDevice() },
                }?;
                format!("GetDevice of {} same {}", held[i].0, owner == device)
            }
        };
        let mut counts: Vec<(usize, u32)> = held
            .iter()
            .map(|(id, o)| (*id, refs(&o.unknown()) - 1))
            .collect();
        counts.sort();
        counts.dedup();
        let counts: Vec<String> = counts.iter().map(|(id, n)| format!("{id}:{n}")).collect();
        println!(
            "{step} {what} | device {} | {}",
            refs(&device.cast::<IUnknown>()?) - 1,
            counts.join(" ")
        );
    }

    held.clear();
    println!(
        "held released | device {}",
        refs(&device.cast::<IUnknown>()?) - 1
    );
    drop(context);
    let raw = device.as_raw();
    std::mem::forget(device);
    let last =
        unsafe { (Interface::vtable(IUnknown::from_raw_borrowed(&raw).unwrap()).Release)(raw) };
    println!("context released, device Release {last}");
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
