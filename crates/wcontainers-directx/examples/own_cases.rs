//! Small Direct3D 11 programs for single cases, one per argument: `atomics` (texture atomics
//! from a compute shader), `typed` (stores to `R11G11B10_FLOAT` and `R16G16_UNORM` storage
//! views), `split` (a pixel shader's storage view read by the next draw on the same targets),
//! `depth_filter` (an `R32_FLOAT` depth and a colour texture sampled through one linear
//! sampler), `deep` (a depth buffer larger than its target), `msaa_copy` (a copy between
//! multisampled textures), `stream` (stream output whose stream is also drawn), `early`
//! (`[earlydepthstencil]` with storage writes, depth writes off), `early_writes` (the same with
//! depth writes on), `stencil` (a stencil view read after clears and a stencil draw),
//! `msaa_lines` (lines under `MultisampleEnable`, each pixel's covered samples counted) and
//! `biased_lines` (a constant depth bias on a line and points). Shaders are compiled by
//! the system's d3dcompiler_47.dll. Prints each answer and the resources' bytes as hex, for
//! comparing with real Direct3D line by line.

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

    struct Tex {
        w: u32,
        h: u32,
        format: DXGI_FORMAT,
        samples: u32,
        bind: D3D11_BIND_FLAG,
    }
    let make = |t: &Tex, init: Option<(&[u8], u32)>| -> windows::core::Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: t.w,
            Height: t.h,
            MipLevels: 1,
            ArraySize: 1,
            Format: t.format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: t.samples,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: t.bind.0 as u32,
            ..Default::default()
        };
        let data = init.map(|(bytes, pitch)| D3D11_SUBRESOURCE_DATA {
            pSysMem: bytes.as_ptr().cast(),
            SysMemPitch: pitch,
            SysMemSlicePitch: 0,
        });
        let mut out = None;
        unsafe {
            device.CreateTexture2D(&desc, data.as_ref().map(|d| d as *const _), Some(&mut out))?
        };
        Ok(out.unwrap())
    };
    // Prints a single-sampled texture's rows through a staging copy of its format.
    let print = |what: &str, t: &ID3D11Texture2D, texel: u32| {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { t.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        let r = unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) };
        println!("{what}: staging {:#010x}", code(r));
        let Some(staging) = staging else { return };
        unsafe { context.CopyResource(&staging, t) };
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
    let print_buffer = |what: &str, b: &ID3D11Buffer, bytes: u32| {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: bytes,
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..Default::default()
        };
        let mut staging = None;
        unsafe {
            device
                .CreateBuffer(&desc, None, Some(&mut staging))
                .unwrap();
        }
        let staging = staging.unwrap();
        unsafe { context.CopyResource(&staging, b) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let r = unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) };
        let ok = r.is_ok();
        println!("{what}: Map {:#010x}", code(r));
        if ok {
            let all =
                unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), bytes as usize) };
            for (i, row) in all.chunks(32).enumerate() {
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                println!("{what}: bytes {} {}", i * 32, hex.join(""));
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
    let srv = |t: &ID3D11Texture2D, format: DXGI_FORMAT| {
        let mut v = None;
        let desc = (format != DXGI_FORMAT_UNKNOWN).then_some(D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        });
        unsafe {
            device
                .CreateShaderResourceView(t, desc.as_ref().map(|d| d as *const _), Some(&mut v))
                .unwrap()
        };
        v
    };
    let uav = |t: &ID3D11Texture2D| {
        let mut v = None;
        unsafe {
            device
                .CreateUnorderedAccessView(t, None, Some(&mut v))
                .unwrap()
        };
        v
    };
    let vs = |src: &str| {
        let mut out = None;
        unsafe {
            device
                .CreateVertexShader(&compile(src, s!("vs_5_0")), None, Some(&mut out))
                .unwrap()
        };
        out
    };
    let ps = |src: &str| {
        let mut out = None;
        unsafe {
            device
                .CreatePixelShader(&compile(src, s!("ps_5_0")), None, Some(&mut out))
                .unwrap()
        };
        out
    };
    let cs = |src: &str| {
        let mut out = None;
        unsafe {
            device
                .CreateComputeShader(&compile(src, s!("cs_5_0")), None, Some(&mut out))
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
    // A triangle covering the target at depth `z` (a constant buffer's first value, or 0).
    const FULL: &str = "cbuffer c : register(b0) { float z; }
        float4 main(uint id : SV_VertexID) : SV_Position {
            float2 uv = float2((id << 1) & 2, id & 2);
            return float4(uv * float2(2, -2) + float2(-1, 1), z, 1);
        }";
    let constants = |values: [f32; 4]| {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: values.as_ptr().cast(),
            ..Default::default()
        };
        let mut b = None;
        unsafe {
            device
                .CreateBuffer(&desc, Some(&data), Some(&mut b))
                .unwrap()
        };
        b
    };
    unsafe { context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST) };

    match case.as_str() {
        "atomics" => {
            // Each texel gets four threads' operations, which commute, so the result is fixed.
            let shader = cs("RWTexture2D<uint> u : register(u0);
                RWTexture2D<int> lo : register(u1);
                RWTexture2D<int> hi : register(u2);
                RWTexture2D<uint> o : register(u3);
                [numthreads(4, 4, 1)]
                void main(uint3 id : SV_DispatchThreadID) {
                    uint2 p = id.xy % 4;
                    uint before;
                    InterlockedAdd(u[p], id.x + 1, before);
                    InterlockedMin(lo[p], 3 - (int)(id.x * id.y));
                    InterlockedMax(hi[p.yx], 20 - (int)(id.x * 3));
                    InterlockedOr(o[p], 1u << (id.x + id.y));
                    InterlockedXor(o[p], 0x100u << id.y);
                }");
            let start: Vec<u8> = (0..16u32).flat_map(|i| (i * 3).to_le_bytes()).collect();
            let at = |format| Tex {
                w: 4,
                h: 4,
                format,
                samples: 1,
                bind: D3D11_BIND_UNORDERED_ACCESS,
            };
            let u = make(&at(DXGI_FORMAT_R32_UINT), Some((&start, 16)))?;
            let lo = make(&at(DXGI_FORMAT_R32_SINT), Some((&start, 16)))?;
            let hi = make(&at(DXGI_FORMAT_R32_SINT), Some((&start, 16)))?;
            let o = make(&at(DXGI_FORMAT_R32_UINT), Some((&start, 16)))?;
            unsafe {
                context.CSSetShader(shader.as_ref(), None);
                let views = [uav(&u), uav(&lo), uav(&hi), uav(&o)];
                context.CSSetUnorderedAccessViews(0, 4, Some(views.as_ptr()), None);
                context.Dispatch(2, 2, 1);
            }
            print("add", &u, 4);
            print("min", &lo, 4);
            print("max", &hi, 4);
            print("or xor", &o, 4);
        }
        "typed" => {
            // Values each format holds exactly.
            let shader = cs("RWTexture2D<float3> a : register(u0);
                RWTexture2D<float2> b : register(u1);
                RWTexture2D<float4> c : register(u2);
                [numthreads(4, 4, 1)]
                void main(uint3 id : SV_DispatchThreadID) {
                    a[id.xy] = float3(id.x * 0.25, id.y * 0.5 + 1.0, exp2(-(float)id.x));
                    b[id.xy] = float2(id.x / 3.0, 1.0 - id.y / 3.0);
                    c[id.xy] = float4(id.x * 0.5, -(float)id.y, 0.125, 2.0);
                }");
            let at = |format| Tex {
                w: 4,
                h: 4,
                format,
                samples: 1,
                bind: D3D11_BIND_UNORDERED_ACCESS,
            };
            let a = make(&at(DXGI_FORMAT_R11G11B10_FLOAT), None)?;
            let b = make(&at(DXGI_FORMAT_R16G16_UNORM), None)?;
            let c = make(&at(DXGI_FORMAT_R16G16B16A16_FLOAT), None)?;
            unsafe {
                context.CSSetShader(shader.as_ref(), None);
                let views = [uav(&a), uav(&b), uav(&c)];
                context.CSSetUnorderedAccessViews(0, 3, Some(views.as_ptr()), None);
                context.Dispatch(1, 1, 1);
            }
            print("R11G11B10_FLOAT", &a, 4);
            print("R16G16_UNORM", &b, 4);
            print("R16G16B16A16_FLOAT", &c, 8);
        }
        "split" => {
            let target = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let both =
                D3D11_BIND_FLAG(D3D11_BIND_UNORDERED_ACCESS.0 | D3D11_BIND_SHADER_RESOURCE.0);
            let stored = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: both,
                },
                None,
            )?;
            let write = ps("RWTexture2D<unorm float4> t : register(u1);
                float4 main(float4 pos : SV_Position) : SV_Target {
                    t[uint2(pos.xy)] = float4(pos.x / 4.0, pos.y / 4.0, 0.5 + pos.x / 16.0, 1.0);
                    return float4(0.25, 0.0, 0.0, 1.0);
                }");
            let read = ps("Texture2D<float4> t : register(t0);
                float4 main(float4 pos : SV_Position) : SV_Target {
                    float4 v = t.Load(int3(pos.xy, 0));
                    return float4(v.y, v.x, v.z * 0.5, 1.0);
                }");
            let rt = rtv(&target);
            unsafe {
                viewport(4, 4);
                context.VSSetShader(vs(FULL).as_ref(), None);
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                context.OMSetRenderTargetsAndUnorderedAccessViews(
                    Some(std::slice::from_ref(&rt)),
                    None,
                    1,
                    1,
                    Some([uav(&stored)].as_ptr()),
                    None,
                );
                context.PSSetShader(write.as_ref(), None);
                context.Draw(3, 0);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                context.PSSetShader(read.as_ref(), None);
                context.PSSetShaderResources(0, Some(&[srv(&stored, DXGI_FORMAT_UNKNOWN)]));
                context.Draw(3, 0);
            }
            print("target", &target, 4);
            print("stored", &stored, 4);
        }
        "depth_filter" => {
            let both = D3D11_BIND_FLAG(D3D11_BIND_DEPTH_STENCIL.0 | D3D11_BIND_SHADER_RESOURCE.0);
            let depth = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R32_TYPELESS,
                    samples: 1,
                    bind: both,
                },
                None,
            )?;
            let mut dsv = None;
            let desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
                Format: DXGI_FORMAT_D32_FLOAT,
                ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
                ..Default::default()
            };
            unsafe { device.CreateDepthStencilView(&depth, Some(&desc), Some(&mut dsv))? };
            let colours: Vec<u8> = (0..16u8).map(|i| i * 16 + 3).collect();
            let colour = make(
                &Tex {
                    w: 2,
                    h: 2,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_SHADER_RESOURCE,
                },
                Some((&colours, 8)),
            )?;
            // The depth of each texel from its place, exact in floats.
            let deepen = ps("float main(float4 pos : SV_Position) : SV_Depth {
                    return (pos.x + 4.0 * pos.y) * 0.03125;
                }");
            let lit = ps("Texture2D<float> d : register(t0);
                Texture2D<float4> c : register(t1);
                SamplerState s : register(s0);
                float4 main(float4 pos : SV_Position) : SV_Target {
                    float2 uv = pos.xy / 8.0;
                    return float4(d.Sample(s, uv), c.Sample(s, uv).yzw);
                }");
            let target = make(
                &Tex {
                    w: 8,
                    h: 8,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let rt = rtv(&target);
            let sampler_desc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                MaxLOD: f32::MAX,
                ..Default::default()
            };
            let ds_desc = D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D11_COMPARISON_ALWAYS,
                ..Default::default()
            };
            let (mut sampler, mut state) = (None, None);
            unsafe {
                device.CreateSamplerState(&sampler_desc, Some(&mut sampler))?;
                device.CreateDepthStencilState(&ds_desc, Some(&mut state))?;
                context.VSSetShader(vs(FULL).as_ref(), None);
                viewport(4, 4);
                context.OMSetDepthStencilState(state.as_ref(), 0);
                context.OMSetRenderTargets(None, dsv.as_ref());
                context.PSSetShader(deepen.as_ref(), None);
                context.Draw(3, 0);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                viewport(8, 8);
                context.PSSetShader(lit.as_ref(), None);
                context.PSSetSamplers(0, Some(&[sampler.clone()]));
                context.PSSetShaderResources(
                    0,
                    Some(&[
                        srv(&depth, DXGI_FORMAT_R32_FLOAT),
                        srv(&colour, DXGI_FORMAT_UNKNOWN),
                    ]),
                );
                context.Draw(3, 0);
            }
            print("depth", &depth, 4);
            // Filtering rounds within Direct3D's tolerance: compared within one unit.
            print("filtered", &target, 4);
        }
        "deep" => {
            let target = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let depth = make(
                &Tex {
                    w: 8,
                    h: 6,
                    format: DXGI_FORMAT_R32_TYPELESS,
                    samples: 1,
                    bind: D3D11_BIND_DEPTH_STENCIL,
                },
                None,
            )?;
            let mut dsv = None;
            let desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
                Format: DXGI_FORMAT_D32_FLOAT,
                ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
                ..Default::default()
            };
            unsafe { device.CreateDepthStencilView(&depth, Some(&desc), Some(&mut dsv))? };
            let colour = ps("cbuffer c : register(b0) { float z; float g; }
                float4 main(float4 pos : SV_Position) : SV_Target { return float4(pos.x / 4.0, g, z, 1); }");
            let rt = rtv(&target);
            unsafe {
                context.VSSetShader(vs(FULL).as_ref(), None);
                context.PSSetShader(colour.as_ref(), None);
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                context.ClearDepthStencilView(dsv.as_ref().unwrap(), D3D11_CLEAR_DEPTH.0, 0.5, 0);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), dsv.as_ref());
                // Nearer, over the target; then farther, failing; then nearer over a viewport
                // past the target.
                for (z, g, w, h) in [(0.25, 0.5, 4, 4), (0.75, 1.0, 4, 4), (0.125, 0.75, 8, 6)] {
                    let c = constants([z, g, 0.0, 0.0]);
                    context.VSSetConstantBuffers(0, Some(std::slice::from_ref(&c)));
                    context.PSSetConstantBuffers(0, Some(&[c]));
                    viewport(w, h);
                    context.Draw(3, 0);
                }
            }
            print("target", &target, 4);
            print("depth", &depth, 4);
        }
        "stencil" => {
            // A stencil view read after a clear of the stencil alone, after a stencil
            // replacing draw, and beside a depth view of the same texture.
            let target = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let depth = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R24G8_TYPELESS,
                    samples: 1,
                    bind: D3D11_BIND_DEPTH_STENCIL | D3D11_BIND_SHADER_RESOURCE,
                },
                None,
            )?;
            let mut dsv = None;
            let desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
                Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
                ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
                ..Default::default()
            };
            unsafe { device.CreateDepthStencilView(&depth, Some(&desc), Some(&mut dsv))? };
            let stencil = srv(&depth, DXGI_FORMAT_X24_TYPELESS_G8_UINT);
            let depth_view = srv(&depth, DXGI_FORMAT_R24_UNORM_X8_TYPELESS);
            let read = ps("Texture2D<uint4> t : register(t0); Texture2D d : register(t1);
                float4 main(float4 p : SV_Position) : SV_Target {
                    float2 s; t.GetDimensions(s.x, s.y);
                    return float4(t.Load(int3(p.xy, 0)).y, t[uint2(p.xy)].y, s.x, d.Load(int3(p.xy, 0)).r);
                }");
            let mut replace = None;
            let op = D3D11_DEPTH_STENCILOP_DESC {
                StencilFailOp: D3D11_STENCIL_OP_KEEP,
                StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
                StencilPassOp: D3D11_STENCIL_OP_REPLACE,
                StencilFunc: D3D11_COMPARISON_ALWAYS,
            };
            let dss = D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: false.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ZERO,
                DepthFunc: D3D11_COMPARISON_ALWAYS,
                StencilEnable: true.into(),
                StencilReadMask: D3D11_DEFAULT_STENCIL_READ_MASK as u8,
                StencilWriteMask: D3D11_DEFAULT_STENCIL_WRITE_MASK as u8,
                FrontFace: op,
                BackFace: op,
            };
            unsafe { device.CreateDepthStencilState(&dss, Some(&mut replace))? };
            let rt = rtv(&target);
            let none = ps("float4 main(float4 p : SV_Position) : SV_Target { return 0; }");
            unsafe {
                context.VSSetShader(vs(FULL).as_ref(), None);
                viewport(4, 4);
                context.ClearDepthStencilView(
                    dsv.as_ref().unwrap(),
                    D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0,
                    0.5,
                    0,
                );
                context.ClearDepthStencilView(
                    dsv.as_ref().unwrap(),
                    D3D11_CLEAR_STENCIL.0,
                    0.0,
                    100,
                );
            }
            let draw_read = |what: &str| unsafe {
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                context.OMSetDepthStencilState(None, 0);
                context.PSSetShader(read.as_ref(), None);
                context.PSSetShaderResources(0, Some(&[stencil.clone(), depth_view.clone()]));
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                context.Draw(3, 0);
                context.PSSetShaderResources(0, Some(&[None, None]));
                print(what, &target, 16);
            };
            draw_read("after clear");
            unsafe {
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), dsv.as_ref());
                context.OMSetDepthStencilState(replace.as_ref(), 7);
                context.PSSetShader(none.as_ref(), None);
                context.Draw(3, 0);
            }
            draw_read("after replace");
        }
        "msaa_copy" => {
            let ms = |bind| Tex {
                w: 4,
                h: 4,
                format: DXGI_FORMAT_R8G8B8A8_UNORM,
                samples: 4,
                bind,
            };
            let both = D3D11_BIND_FLAG(D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0);
            let drawn = make(&ms(both), None)?;
            let copied = make(&ms(both), None)?;
            let region = make(&ms(both), None)?;
            // A triangle over part of the target, so edge pixels hold some samples.
            let corner = vs("float4 main(uint id : SV_VertexID) : SV_Position {
                    float2 p[3] = { float2(-0.9, 0.8), float2(0.7, 0.3), float2(-0.4, -0.95) };
                    return float4(p[id], 0, 1);
                }");
            let flat = ps("float4 main(float4 pos : SV_Position) : SV_Target {
                    return float4(0.25, pos.x / 4.0, 0.75, 1);
                }");
            // Each sample of each pixel as a column of its own.
            let samples = ps("Texture2DMS<float4, 4> t : register(t0);
                float4 main(float4 pos : SV_Position) : SV_Target {
                    uint2 p = uint2(pos.xy);
                    return t.Load(int2(p.x / 4, p.y), p.x % 4);
                }");
            let out = make(
                &Tex {
                    w: 16,
                    h: 4,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let (rt, rt_out) = (rtv(&drawn), rtv(&out));
            unsafe {
                viewport(4, 4);
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0, 0.0, 0.125, 1.0]);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                context.VSSetShader(corner.as_ref(), None);
                context.PSSetShader(flat.as_ref(), None);
                context.Draw(3, 0);
                context.OMSetRenderTargets(None, None);
                context.CopyResource(&copied, &drawn);
                context.CopySubresourceRegion(&region, 0, 0, 0, 0, &drawn, 0, None);
            }
            for (what, t) in [("drawn", &drawn), ("copied", &copied), ("region", &region)] {
                unsafe {
                    viewport(16, 4);
                    context.OMSetRenderTargets(Some(std::slice::from_ref(&rt_out)), None);
                    context.VSSetShader(vs(FULL).as_ref(), None);
                    context.PSSetShader(samples.as_ref(), None);
                    context.PSSetShaderResources(0, Some(&[srv(t, DXGI_FORMAT_UNKNOWN)]));
                    context.Draw(3, 0);
                    context.PSSetShaderResources(0, Some(&[None]));
                    context.OMSetRenderTargets(None, None);
                }
                print(what, &out, 4);
            }
        }
        "stream" => {
            let vertex = vs("struct V { float4 pos : SV_Position; float4 c : COLOR; };
                V main(uint id : SV_VertexID) {
                    float2 p[3] = { float2(-1, 1), float2(1, 1), float2(-1, -1) };
                    V v; v.pos = float4(p[id], 0.5, 1); v.c = float4(id * 0.25, 0.5, 1, 1); return v;
                }");
            let gs_code = compile(
                "struct V { float4 pos : SV_Position; float4 c : COLOR; };
                [maxvertexcount(3)]
                void main(triangle V v[3], inout TriangleStream<V> s) {
                    for (uint i = 0; i < 3; i++) { V o = v[i]; o.c.w = i * 0.5; s.Append(o); }
                }",
                s!("gs_5_0"),
            );
            let entries = [
                D3D11_SO_DECLARATION_ENTRY {
                    Stream: 0,
                    SemanticName: s!("SV_Position"),
                    SemanticIndex: 0,
                    StartComponent: 0,
                    ComponentCount: 4,
                    OutputSlot: 0,
                },
                D3D11_SO_DECLARATION_ENTRY {
                    Stream: 0,
                    SemanticName: s!("COLOR"),
                    SemanticIndex: 0,
                    StartComponent: 0,
                    ComponentCount: 4,
                    OutputSlot: 0,
                },
            ];
            let mut gs = None;
            let r = unsafe {
                device.CreateGeometryShaderWithStreamOutput(
                    &gs_code,
                    Some(&entries),
                    Some(&[32]),
                    0,
                    None::<&ID3D11ClassLinkage>,
                    Some(&mut gs),
                )
            };
            println!("CreateGeometryShaderWithStreamOutput {:#010x}", code(r));
            let colour = ps("struct V { float4 pos : SV_Position; float4 c : COLOR; };
                float4 main(V v) : SV_Target { return v.c; }");
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: 128,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_STREAM_OUTPUT.0 as u32,
                ..Default::default()
            };
            let mut so = None;
            unsafe { device.CreateBuffer(&desc, None, Some(&mut so))? };
            let target = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let rt = rtv(&target);
            unsafe {
                viewport(4, 4);
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                context.VSSetShader(vertex.as_ref(), None);
                context.GSSetShader(gs.as_ref(), None);
                context.PSSetShader(colour.as_ref(), None);
                context.SOSetTargets(1, Some([so.clone()].as_ptr()), Some([0u32].as_ptr()));
                context.Draw(3, 0);
                context.SOSetTargets(0, None, None);
                context.GSSetShader(None, None);
            }
            print("target", &target, 4);
            print_buffer("stream", so.as_ref().unwrap(), 128);
        }
        "early" | "early_writes" => {
            let target = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let depth = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_D32_FLOAT,
                    samples: 1,
                    bind: D3D11_BIND_DEPTH_STENCIL,
                },
                None,
            )?;
            let counts = make(
                &Tex {
                    w: 4,
                    h: 4,
                    format: DXGI_FORMAT_R32_UINT,
                    samples: 1,
                    bind: D3D11_BIND_UNORDERED_ACCESS,
                },
                Some((&[0u8; 64], 16)),
            )?;
            let mut dsv = None;
            unsafe { device.CreateDepthStencilView(&depth, None, Some(&mut dsv))? };
            // Counts the pixels that pass the depth test.
            let counted = ps("RWTexture2D<uint> n : register(u1);
                cbuffer c : register(b0) { float z; float g; }
                [earlydepthstencil]
                float4 main(float4 pos : SV_Position) : SV_Target {
                    InterlockedAdd(n[uint2(pos.xy)], 1);
                    return float4(g, z, 0, 1);
                }");
            let rt = rtv(&target);
            let writes = case == "early_writes";
            {
                let name = if writes { "writes on" } else { "writes off" };
                let ds_desc = D3D11_DEPTH_STENCIL_DESC {
                    DepthEnable: true.into(),
                    DepthWriteMask: if writes {
                        D3D11_DEPTH_WRITE_MASK_ALL
                    } else {
                        D3D11_DEPTH_WRITE_MASK_ZERO
                    },
                    DepthFunc: D3D11_COMPARISON_LESS,
                    ..Default::default()
                };
                let mut state = None;
                unsafe {
                    device.CreateDepthStencilState(&ds_desc, Some(&mut state))?;
                    context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                    context.ClearDepthStencilView(
                        dsv.as_ref().unwrap(),
                        D3D11_CLEAR_DEPTH.0,
                        0.5,
                        0,
                    );
                    context.ClearUnorderedAccessViewUint(uav(&counts).as_ref().unwrap(), &[0; 4]);
                    context.OMSetDepthStencilState(state.as_ref(), 0);
                    context.VSSetShader(vs(FULL).as_ref(), None);
                    context.PSSetShader(counted.as_ref(), None);
                    context.OMSetRenderTargetsAndUnorderedAccessViews(
                        Some(std::slice::from_ref(&rt)),
                        dsv.as_ref(),
                        1,
                        1,
                        Some([uav(&counts)].as_ptr()),
                        None,
                    );
                    viewport(4, 4);
                    // Passing, failing, then passing again where the first left its depth.
                    for (z, g) in [(0.25f32, 0.5f32), (0.75, 1.0), (0.375, 0.25)] {
                        let c = constants([z, g, 0.0, 0.0]);
                        context.VSSetConstantBuffers(0, Some(std::slice::from_ref(&c)));
                        context.PSSetConstantBuffers(0, Some(&[c]));
                        context.Draw(3, 0);
                    }
                    context.OMSetRenderTargets(None, None);
                }
                print(&format!("{name} target"), &target, 4);
                print(&format!("{name} counts"), &counts, 4);
                print(&format!("{name} depth"), &depth, 4);
            }
        }
        "msaa_lines" => {
            // Lines under MultisampleEnable into four samples, each pixel's covered samples
            // counted.
            let drawn = make(
                &Tex {
                    w: 16,
                    h: 16,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 4,
                    bind: D3D11_BIND_FLAG(
                        D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0,
                    ),
                },
                None,
            )?;
            let counts = make(
                &Tex {
                    w: 16,
                    h: 16,
                    format: DXGI_FORMAT_R8_UINT,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let lines = vs("float4 main(uint id : SV_VertexID) : SV_Position {
                    float2 p[6] = { float2(-0.9, 0.3), float2(0.85, 0.3),
                        float2(-0.8, -0.9), float2(0.7, 0.6),
                        float2(0.2, 0.95), float2(0.4, -0.85) };
                    return float4(p[id], 0, 1);
                }");
            let white = ps("float4 main(float4 p : SV_Position) : SV_Target { return 1; }");
            let count = ps("Texture2DMS<float4, 4> t : register(t0);
                uint main(float4 pos : SV_Position) : SV_Target {
                    uint n = 0;
                    for (uint s = 0; s < 4; ++s) n += t.Load(int2(pos.xy), s).r > 0.5;
                    return n;
                }");
            let mut raster = None;
            let desc = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthClipEnable: true.into(),
                MultisampleEnable: true.into(),
                ..Default::default()
            };
            unsafe { device.CreateRasterizerState(&desc, Some(&mut raster))? };
            let (rt, rt_counts) = (rtv(&drawn), rtv(&counts));
            unsafe {
                viewport(16, 16);
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                context.RSSetState(raster.as_ref());
                context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINELIST);
                context.VSSetShader(lines.as_ref(), None);
                context.PSSetShader(white.as_ref(), None);
                context.Draw(6, 0);
                context.RSSetState(None);
                context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt_counts)), None);
                context.VSSetShader(vs(FULL).as_ref(), None);
                context.PSSetShader(count.as_ref(), None);
                context.PSSetShaderResources(0, Some(&[srv(&drawn, DXGI_FORMAT_UNKNOWN)]));
                context.Draw(3, 0);
                context.PSSetShaderResources(0, Some(&[None]));
                context.OMSetRenderTargets(None, None);
            }
            print("msaa lines", &counts, 1);
        }
        "biased_lines" => {
            // A constant depth bias on a line along a row of pixel centres and on points at pixel
            // centres, into 16 and 24-bit depth cleared to 0.5.
            let shapes = vs("cbuffer c : register(b0) { float z; }
                float4 main(uint id : SV_VertexID) : SV_Position {
                    float2 p[5] = { float2(-1.5, 0.25), float2(1.5, 0.25),
                        float2(-0.8125, -0.25), float2(0.0625, -0.25), float2(0.6875, -0.25) };
                    return float4(p[id], z, 1);
                }");
            let flat = ps("float4 main(float4 p : SV_Position) : SV_Target { return 1; }");
            let target = make(
                &Tex {
                    w: 16,
                    h: 4,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let rt = rtv(&target);
            let mut raster = None;
            let desc = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthBias: 1000,
                DepthClipEnable: true.into(),
                ..Default::default()
            };
            unsafe { device.CreateRasterizerState(&desc, Some(&mut raster))? };
            let mut always = None;
            let dss = D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D11_COMPARISON_ALWAYS,
                ..Default::default()
            };
            unsafe { device.CreateDepthStencilState(&dss, Some(&mut always))? };
            let c = constants([0.25, 0.0, 0.0, 0.0]);
            for (name, typeless, format, texel) in [
                ("d16", DXGI_FORMAT_R16_TYPELESS, DXGI_FORMAT_D16_UNORM, 2),
                (
                    "d24",
                    DXGI_FORMAT_R24G8_TYPELESS,
                    DXGI_FORMAT_D24_UNORM_S8_UINT,
                    4,
                ),
            ] {
                let depth = make(
                    &Tex {
                        w: 16,
                        h: 4,
                        format: typeless,
                        samples: 1,
                        bind: D3D11_BIND_DEPTH_STENCIL,
                    },
                    None,
                )?;
                let mut dsv = None;
                let desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
                    Format: format,
                    ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
                    ..Default::default()
                };
                unsafe { device.CreateDepthStencilView(&depth, Some(&desc), Some(&mut dsv))? };
                unsafe {
                    viewport(16, 4);
                    context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                    context.ClearDepthStencilView(
                        dsv.as_ref().unwrap(),
                        D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0,
                        0.5,
                        0,
                    );
                    context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), dsv.as_ref());
                    context.OMSetDepthStencilState(always.as_ref(), 0);
                    context.RSSetState(raster.as_ref());
                    context.VSSetShader(shapes.as_ref(), None);
                    context.VSSetConstantBuffers(0, Some(std::slice::from_ref(&c)));
                    context.PSSetShader(flat.as_ref(), None);
                    context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINELIST);
                    context.Draw(2, 0);
                    context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);
                    context.Draw(3, 2);
                    context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    context.RSSetState(None);
                    context.OMSetDepthStencilState(None, 0);
                    context.OMSetRenderTargets(None, None);
                }
                print(&format!("{name} depth"), &depth, texel);
            }
        }
        _ => println!("no case {case:?}"),
    }
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
