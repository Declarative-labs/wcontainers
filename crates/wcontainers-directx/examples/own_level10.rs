//! Cases on a device of feature level 10_0, as the library built on WebGL 2 reports, one per
//! argument: `targets` (three targets blended and masked each its own way, over a depth test, then
//! again with two targets writing nothing; first what each form of per-target blending answers),
//! `resinfo` (a pixel shader asking views of a texture and of an array, every level and layer as
//! the library built on WebGL 2 serves views, for their sizes and level counts at their first levels) and `resinfo_empty` (also an empty slot). Shaders are compiled by the
//! system's d3dcompiler_47.dll. Prints each answer and the targets' bytes as hex, for comparing
//! with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::{PCSTR, s};

    let case = std::env::args().nth(1).unwrap_or_default();
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

    let (mut device, mut context, mut level) = (None, None, D3D_FEATURE_LEVEL::default());
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            Some(&[D3D_FEATURE_LEVEL_10_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut level),
            Some(&mut context),
        )?
    };
    println!("feature level {:#x}", level.0);
    let (device, context): (ID3D11Device, ID3D11DeviceContext) =
        (device.unwrap(), context.unwrap());

    let texture = |w: u32, h: u32, mips: u32, layers: u32, format, bind: D3D11_BIND_FLAG| {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: mips,
            ArraySize: layers,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind.0 as u32,
            ..Default::default()
        };
        let mut out = None;
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut out)).unwrap() };
        out.unwrap()
    };
    // Prints a texture's first level through a staging copy.
    let print = |what: &str, t: &ID3D11Texture2D, texel: u32| {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { t.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MipLevels = 1;
        let mut staging = None;
        let r = unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) };
        println!("{what}: staging {:#010x}", code(r));
        let Some(staging) = staging else { return };
        unsafe { context.CopySubresourceRegion(&staging, 0, 0, 0, 0, t, 0, None) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let r = unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) };
        let ok = r.is_ok();
        println!("{what}: Map {:#010x}", code(r));
        if ok {
            for y in 0..desc.Height {
                let row = unsafe {
                    std::slice::from_raw_parts(
                        mapped
                            .pData
                            .cast::<u8>()
                            .add((y * mapped.RowPitch) as usize),
                        (desc.Width * texel) as usize,
                    )
                };
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                println!("{what}: row {y} {}", hex.join(""));
            }
            unsafe { context.Unmap(&staging, 0) };
        }
    };
    let rtv = |t: &ID3D11Texture2D| {
        let mut v = None;
        unsafe {
            device
                .CreateRenderTargetView(t, None, Some(&mut v))
                .unwrap()
        };
        v
    };
    let vs = |src: &str| {
        let mut out = None;
        unsafe {
            device
                .CreateVertexShader(&compile(src, s!("vs_4_0")), None, Some(&mut out))
                .unwrap()
        };
        out
    };
    let ps = |src: &str| {
        let mut out = None;
        unsafe {
            device
                .CreatePixelShader(&compile(src, s!("ps_4_0")), None, Some(&mut out))
                .unwrap()
        };
        out
    };
    let viewport = |w: u32, h: u32| unsafe {
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: w as f32,
            Height: h as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
    };
    unsafe { context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST) };

    match case.as_str() {
        "targets" => {
            // Two triangles, the first near and red, the second far and green over half of it.
            let shader = vs(
                "struct V { float4 pos : SV_Position; float4 color : COLOR; };
                V main(uint id : SV_VertexID) {
                    V v;
                    uint k = id % 3;
                    bool second = id >= 3;
                    float2 p = float2(k == 1 ? 1.0 : -1.0, k == 2 ? -1.0 : 1.0);
                    if (second) p.x = k == 2 ? 1.0 : p.x;
                    v.pos = float4(p, second ? 0.6 : 0.3, 1.0);
                    v.color = second ? float4(0.0, 1.0, 0.0, 0.25) : float4(1.0, 0.0, 0.0, 0.25);
                    return v;
                }",
            );
            let pixel = ps(
                "struct V { float4 pos : SV_Position; float4 color : COLOR; };
                struct O { float4 a : SV_Target0; float4 b : SV_Target1; float4 c : SV_Target2; };
                O main(V v) {
                    O o;
                    o.a = v.color;
                    o.b = v.color * 0.4;
                    o.c = v.color.gbra;
                    return o;
                }",
            );
            let (w, h) = (8, 8);
            let targets: Vec<_> = (0..3)
                .map(|_| {
                    texture(
                        w,
                        h,
                        1,
                        1,
                        DXGI_FORMAT_R8G8B8A8_UNORM,
                        D3D11_BIND_RENDER_TARGET,
                    )
                })
                .collect();
            let views: Vec<_> = targets.iter().map(rtv).collect();
            let depth = texture(w, h, 1, 1, DXGI_FORMAT_D32_FLOAT, D3D11_BIND_DEPTH_STENCIL);
            let mut dsv = None;
            unsafe { device.CreateDepthStencilView(&depth, None, Some(&mut dsv))? };
            let ds_desc = D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D11_COMPARISON_LESS,
                ..Default::default()
            };
            let mut ds = None;
            unsafe { device.CreateDepthStencilState(&ds_desc, Some(&mut ds))? };
            let target = |enable: bool, src, dst, op, mask: D3D11_COLOR_WRITE_ENABLE| {
                D3D11_RENDER_TARGET_BLEND_DESC {
                    BlendEnable: enable.into(),
                    SrcBlend: src,
                    DestBlend: dst,
                    BlendOp: op,
                    SrcBlendAlpha: D3D11_BLEND_ONE,
                    DestBlendAlpha: D3D11_BLEND_ZERO,
                    BlendOpAlpha: D3D11_BLEND_OP_ADD,
                    RenderTargetWriteMask: mask.0 as u8,
                }
            };
            // Feature level 10_0 has Direct3D 10's per-target blend enables and write masks, with
            // one set of factors and operations: what it answers to each form.
            let alpha = |enable: bool, mask| {
                target(
                    enable,
                    D3D11_BLEND_SRC_ALPHA,
                    D3D11_BLEND_INV_SRC_ALPHA,
                    D3D11_BLEND_OP_ADD,
                    mask,
                )
            };
            let rg = D3D11_COLOR_WRITE_ENABLE(
                D3D11_COLOR_WRITE_ENABLE_RED.0 | D3D11_COLOR_WRITE_ENABLE_GREEN.0,
            );
            let mut factors = D3D11_BLEND_DESC {
                IndependentBlendEnable: true.into(),
                ..Default::default()
            };
            factors.RenderTarget[0] = alpha(true, D3D11_COLOR_WRITE_ENABLE_ALL);
            factors.RenderTarget[1] = target(
                true,
                D3D11_BLEND_ONE,
                D3D11_BLEND_ONE,
                D3D11_BLEND_OP_ADD,
                D3D11_COLOR_WRITE_ENABLE_ALL,
            );
            let mut same = factors;
            same.RenderTarget[1] = same.RenderTarget[0];
            let mut blend = factors;
            blend.RenderTarget[1] = alpha(false, rg);
            blend.RenderTarget[2] = alpha(true, D3D11_COLOR_WRITE_ENABLE_ALL);
            for (name, d) in [
                ("factors differing", &factors),
                ("targets alike", &same),
                ("enables and masks differing", &blend),
            ] {
                let mut state = None;
                let r = unsafe { device.CreateBlendState(d, Some(&mut state)) };
                println!("CreateBlendState {name} {:#010x}", code(r));
            }
            let mut state = None;
            unsafe { device.CreateBlendState(&blend, Some(&mut state))? };
            let clears = [
                [0.0, 0.0, 1.0, 1.0],
                [0.2, 0.2, 0.2, 1.0],
                [0.5, 0.25, 0.75, 0.0],
            ];
            unsafe {
                viewport(w, h);
                for (v, c) in views.iter().zip(&clears) {
                    context.ClearRenderTargetView(v.as_ref().unwrap(), c);
                }
                context.ClearDepthStencilView(dsv.as_ref().unwrap(), D3D11_CLEAR_DEPTH.0, 1.0, 0);
                context.VSSetShader(shader.as_ref(), None);
                context.PSSetShader(pixel.as_ref(), None);
                context.OMSetRenderTargets(Some(&views), dsv.as_ref());
                context.OMSetDepthStencilState(ds.as_ref(), 0);
                context.OMSetBlendState(state.as_ref(), None, u32::MAX);
                context.Draw(6, 0);
            }
            for (i, t) in targets.iter().enumerate() {
                print(&format!("blended target {i}"), t, 4);
            }
            // Again with the depth test passing equal depths: only the first target writes.
            blend.RenderTarget[1].RenderTargetWriteMask = 0;
            blend.RenderTarget[2].RenderTargetWriteMask = 0;
            blend.RenderTarget[0].BlendEnable = false.into();
            let mut state = None;
            unsafe { device.CreateBlendState(&blend, Some(&mut state))? };
            let ds_desc = D3D11_DEPTH_STENCIL_DESC {
                DepthFunc: D3D11_COMPARISON_LESS_EQUAL,
                ..ds_desc
            };
            let mut ds = None;
            unsafe {
                device.CreateDepthStencilState(&ds_desc, Some(&mut ds))?;
                context.OMSetDepthStencilState(ds.as_ref(), 0);
                context.OMSetBlendState(state.as_ref(), None, u32::MAX);
                context.Draw(6, 0);
            }
            for (i, t) in targets.iter().enumerate() {
                print(&format!("masked target {i}"), t, 4);
            }
        }
        "resinfo" | "resinfo_empty" => {
            let pixel = ps("Texture2D<float4> a : register(t0);
                Texture2DArray<float4> b : register(t1);
                Texture2D<float4> c : register(t2);
                uint4 main(float4 pos : SV_Position) : SV_Target {
                    uint mip = (uint)pos.x, w, h, n, l;
                    if (pos.y < 1.0) { a.GetDimensions(mip, w, h, n); return uint4(w, h, n, mip); }
                    if (pos.y < 2.0) { b.GetDimensions(mip, w, h, l, n); return uint4(w, h, l, n); }
                    c.GetDimensions(mip, w, h, n);
                    return uint4(w, h, n, 7);
                }");
            let full = vs("float4 main(uint id : SV_VertexID) : SV_Position {
                    float2 uv = float2((id << 1) & 2, id & 2);
                    return float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
                }");
            let plain = texture(
                32,
                16,
                5,
                1,
                DXGI_FORMAT_R8G8B8A8_UNORM,
                D3D11_BIND_SHADER_RESOURCE,
            );
            let layered = texture(
                8,
                24,
                0,
                5,
                DXGI_FORMAT_R16G16B16A16_FLOAT,
                D3D11_BIND_SHADER_RESOURCE,
            );
            let view = |t: &ID3D11Texture2D, desc: D3D11_SHADER_RESOURCE_VIEW_DESC| {
                let mut v = None;
                let r = unsafe { device.CreateShaderResourceView(t, Some(&desc), Some(&mut v)) };
                println!("CreateShaderResourceView {:#010x}", code(r));
                v
            };
            let a = view(
                &plain,
                D3D11_SHADER_RESOURCE_VIEW_DESC {
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
                    Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D11_TEX2D_SRV {
                            MostDetailedMip: 0,
                            MipLevels: u32::MAX,
                        },
                    },
                },
            );
            let b = view(
                &layered,
                D3D11_SHADER_RESOURCE_VIEW_DESC {
                    Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
                    ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2DARRAY,
                    Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2DArray: D3D11_TEX2D_ARRAY_SRV {
                            MostDetailedMip: 0,
                            MipLevels: u32::MAX,
                            FirstArraySlice: 0,
                            ArraySize: 5,
                        },
                    },
                },
            );
            let out = texture(
                3,
                3,
                1,
                1,
                DXGI_FORMAT_R32G32B32A32_UINT,
                D3D11_BIND_RENDER_TARGET,
            );
            let rt = rtv(&out);
            unsafe {
                viewport(3, 3);
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[9.0; 4]);
                context.VSSetShader(full.as_ref(), None);
                context.PSSetShader(pixel.as_ref(), None);
                context.PSSetShaderResources(0, Some(&[a, b, None]));
                if case == "resinfo" {
                    // Row 2 asks the empty slot: drawn only in `resinfo_empty`.
                    viewport(3, 2);
                }
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                context.Draw(3, 0);
            }
            print("sizes", &out, 16);
        }
        other => panic!("unknown case {other:?}"),
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
