//! Small Direct3D 11 programs for single cases, one per argument: `atomics` (texture atomics
//! from a compute shader), `raw_signed_atomics` (signed and unsigned minimums and maximums of
//! raw words from every pixel and thread), `clear_view` (`ClearView` of a target's rectangles and
//! of a whole storage view), `cs_min_lod` (a compute shader sampling, loading and
//! gathering under `SetResourceMinLOD` clamps), `typed` (stores to `R11G11B10_FLOAT` and `R16G16_UNORM` storage
//! views), `split` (a pixel shader's storage view read by the next draw on the same targets),
//! `depth_filter` (an `R32_FLOAT` depth and a colour texture sampled through one linear
//! sampler), `deep` (a depth buffer larger than its target), `msaa_copy` (a copy between
//! multisampled textures), `stream` (stream output whose stream is also drawn), `early`
//! (`[earlydepthstencil]` with storage writes, depth writes off), `early_writes` (the same with
//! depth writes on), `stencil` (a stencil view read after clears and a stencil draw),
//! `msaa_lines` (lines under `MultisampleEnable`, each pixel's covered samples counted),
//! `aliased_msaa_lines` (the same lines without it),
//! `biased_lines` (a constant depth bias on a line and points), `staged_bias` (the same bias on
//! what geometry and tessellation stages rasterise), `unshaded` (depth-only draws without a
//! pixel shader, depth clamped and clipped), `aa_lines` (alpha-antialiased lines
//! blended by their coverage), `aa_strip_indexed` (the same as an indexed strip),
//! `format_support` (both words of every format's support) and `bgrx` (what a vertex shader reads
//! from a `B8G8R8X8_UNORM` element). Shaders are compiled by
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
        "format_support" => {
            for format in 0..=DXGI_FORMAT_V408.0 {
                let mut support2 = D3D11_FEATURE_DATA_FORMAT_SUPPORT2 {
                    InFormat: DXGI_FORMAT(format),
                    OutFormatSupport2: 0,
                };
                let r = unsafe {
                    device.CheckFeatureSupport(
                        D3D11_FEATURE_FORMAT_SUPPORT2,
                        (&raw mut support2).cast(),
                        size_of_val(&support2) as u32,
                    )
                };
                let support1 = unsafe { device.CheckFormatSupport(DXGI_FORMAT(format)) };
                println!(
                    "format {format} {:#010x} {:#010x} {:#010x}",
                    code(r),
                    support1.unwrap_or(0),
                    support2.OutFormatSupport2
                );
            }
        }
        "bgrx" => {
            // One point whose B8G8R8X8_UNORM element (its X byte not 255) the vertex shader
            // passes on whole; the target holds the four components it read.
            let bytecode = compile(
                "struct V { float4 p : SV_Position; float4 c : COLOR; };
                V main(float4 c : COLOR) { V v; v.p = float4(0, 0, 0, 1); v.c = c; return v; }",
                s!("vs_5_0"),
            );
            let pixel = ps(
                "float4 main(float4 p : SV_Position, float4 c : COLOR) : SV_Target {
                    return c;
                }",
            );
            let element = D3D11_INPUT_ELEMENT_DESC {
                SemanticName: s!("COLOR"),
                Format: DXGI_FORMAT_B8G8R8X8_UNORM,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                ..Default::default()
            };
            let mut layout = None;
            let r = unsafe { device.CreateInputLayout(&[element], &bytecode, Some(&mut layout)) };
            println!("CreateInputLayout {:#010x}", code(r));
            let Some(layout) = layout else { return Ok(()) };
            let vertex: [u8; 4] = [0x40, 0x80, 0xc0, 0x20];
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: size_of_val(&vertex) as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
                ..Default::default()
            };
            let data = D3D11_SUBRESOURCE_DATA {
                pSysMem: vertex.as_ptr().cast(),
                ..Default::default()
            };
            let mut buffer = None;
            unsafe { device.CreateBuffer(&desc, Some(&data), Some(&mut buffer))? };
            let target = make(
                &Tex {
                    w: 1,
                    h: 1,
                    format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let mut shader = None;
            unsafe {
                device.CreateVertexShader(&bytecode, None, Some(&mut shader))?;
                context.IASetInputLayout(&layout);
                let stride = size_of_val(&vertex) as u32;
                context.IASetVertexBuffers(0, 1, Some(&buffer), Some(&stride), Some(&0));
                context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);
                context.VSSetShader(shader.as_ref(), None);
                context.PSSetShader(pixel.as_ref(), None);
                context.OMSetRenderTargets(Some(&[rtv(&target)]), None);
            }
            viewport(1, 1);
            unsafe { context.Draw(1, 0) };
            print("B8G8R8X8_UNORM element", &target, 16);
        }
        "xr" => {
            // R10G10B10_XR_BIAS_A2_UNORM textures: which shapes and bindings are taken, their
            // bytes kept through initial data, updates, CPU writes and copies, and copies with
            // R10G10B10A2 textures.
            const XR: DXGI_FORMAT = DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM;
            let (side, texel) = (2u32, 4u32);
            let pitch = side * texel;
            let first: Vec<u8> = (0..pitch * side).map(|i| (i * 37 + 11) as u8).collect();
            let second: Vec<u8> = first.iter().map(|b| b ^ 0xa5).collect();
            let tex = |format, bind| Tex {
                w: side,
                h: side,
                format,
                samples: 1,
                bind,
            };
            for (what, bind) in [
                ("shader resource", D3D11_BIND_SHADER_RESOURCE),
                ("render target", D3D11_BIND_RENDER_TARGET),
                ("unordered access", D3D11_BIND_UNORDERED_ACCESS),
            ] {
                println!(
                    "{what}: {:#010x}",
                    code(make(&tex(XR, bind), None).map(drop))
                );
            }
            let mut one = None;
            let d1 = D3D11_TEXTURE1D_DESC {
                Width: side,
                MipLevels: 1,
                ArraySize: 1,
                Format: XR,
                Usage: D3D11_USAGE_DEFAULT,
                ..Default::default()
            };
            let r = unsafe { device.CreateTexture1D(&d1, None, Some(&mut one)) };
            println!("1D: {:#010x}", code(r));
            let mut three = None;
            let d3 = D3D11_TEXTURE3D_DESC {
                Width: side,
                Height: side,
                Depth: side,
                MipLevels: 1,
                Format: XR,
                Usage: D3D11_USAGE_DEFAULT,
                ..Default::default()
            };
            let r = unsafe { device.CreateTexture3D(&d3, None, Some(&mut three)) };
            println!("3D: {:#010x}", code(r));
            let mut levels = 0;
            let r = unsafe {
                let mips = D3D11_TEXTURE2D_DESC {
                    Width: side,
                    Height: side,
                    MipLevels: 0,
                    ArraySize: 1,
                    Format: XR,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    ..Default::default()
                };
                let mut t = None;
                device.CreateTexture2D(&mips, None, Some(&mut t)).map(|_| {
                    let mut d = D3D11_TEXTURE2D_DESC::default();
                    t.unwrap().GetDesc(&mut d);
                    levels = d.MipLevels;
                })
            };
            println!("mip chain: {:#010x} levels {levels}", code(r));
            let t = make(&tex(XR, D3D11_BIND_FLAG(0)), Some((&first, pitch)))?;
            print("initialised", &t, texel);
            unsafe { context.UpdateSubresource(&t, 0, None, second.as_ptr().cast(), pitch, 0) };
            print("updated", &t, texel);
            for other in [
                DXGI_FORMAT_R10G10B10A2_UNORM,
                DXGI_FORMAT_R10G10B10A2_TYPELESS,
            ] {
                let o = make(&tex(other, D3D11_BIND_FLAG(0)), Some((&first, pitch)))?;
                unsafe { context.CopyResource(&o, &t) };
                print(&format!("copied into {}", other.0), &o, texel);
                let x = make(&tex(XR, D3D11_BIND_FLAG(0)), Some((&second, pitch)))?;
                unsafe { context.CopyResource(&x, &o) };
                print(&format!("copied from {}", other.0), &x, texel);
            }
            let staging = D3D11_TEXTURE2D_DESC {
                Width: side,
                Height: side,
                MipLevels: 1,
                ArraySize: 1,
                Format: XR,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: (D3D11_CPU_ACCESS_READ.0 | D3D11_CPU_ACCESS_WRITE.0) as u32,
                ..Default::default()
            };
            let mut s = None;
            unsafe { device.CreateTexture2D(&staging, None, Some(&mut s))? };
            let s = s.unwrap();
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe { context.Map(&s, 0, D3D11_MAP_WRITE, 0, Some(&mut mapped))? };
            for y in 0..side {
                let row = &first[(y * pitch) as usize..][..pitch as usize];
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        row.as_ptr(),
                        mapped
                            .pData
                            .cast::<u8>()
                            .add((y * mapped.RowPitch) as usize),
                        row.len(),
                    )
                };
            }
            unsafe { context.Unmap(&s, 0) };
            unsafe { context.CopyResource(&t, &s) };
            print("written by the CPU", &t, texel);
            let removed = unsafe { device.GetDeviceRemovedReason() };
            println!("removed: {:#010x}", code(removed));
        }
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
        "raw_signed_atomics" => {
            // Every pixel of a draw (some discarding first) and every thread of a dispatch takes
            // signed and unsigned minimums and maximums of the same raw words, which commute, so
            // the words end fixed.
            let side = 16;
            let start = [3i32; 8];
            let desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of_val(&start) as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32,
                ..Default::default()
            };
            let data = D3D11_SUBRESOURCE_DATA {
                pSysMem: start.as_ptr().cast(),
                ..Default::default()
            };
            let mut words = None;
            unsafe { device.CreateBuffer(&desc, Some(&data), Some(&mut words))? };
            let words = words.unwrap();
            let raw = D3D11_UNORDERED_ACCESS_VIEW_DESC {
                Format: DXGI_FORMAT_R32_TYPELESS,
                ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
                Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                    Buffer: D3D11_BUFFER_UAV {
                        FirstElement: 0,
                        NumElements: start.len() as u32,
                        Flags: D3D11_BUFFER_UAV_FLAG_RAW.0 as u32,
                    },
                },
            };
            let mut view = None;
            unsafe { device.CreateUnorderedAccessView(&words, Some(&raw), Some(&mut view))? };
            let target = make(
                &Tex {
                    w: side,
                    h: side,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let rt = rtv(&target);
            let pixels = ps("RWByteAddressBuffer u : register(u1);
                float4 main(float4 pos : SV_Position) : SV_Target {
                    if (((uint)pos.x + (uint)pos.y) % 5 == 0) discard;
                    int v = (int)pos.x - (int)pos.y * 3;
                    u.InterlockedMax(0, v);
                    u.InterlockedMin(4, v);
                    u.InterlockedMax(8, (uint)v);
                    u.InterlockedMin(12, (uint)v);
                    return 0;
                }");
            let threads = cs("RWByteAddressBuffer u : register(u0);
                [numthreads(8, 8, 1)]
                void main(uint3 id : SV_DispatchThreadID) {
                    int v = (int)id.x * 2 - (int)id.y * 5;
                    uint r;
                    u.InterlockedMax(16, v, r);
                    u.InterlockedMin(20, v, r);
                    u.InterlockedMax(24, (uint)v, r);
                    u.InterlockedMin(28, (uint)v, r);
                }");
            unsafe {
                viewport(side, side);
                context.VSSetShader(vs(FULL).as_ref(), None);
                context.OMSetRenderTargetsAndUnorderedAccessViews(
                    Some(std::slice::from_ref(&rt)),
                    None,
                    1,
                    1,
                    Some([view.clone()].as_ptr()),
                    None,
                );
                context.PSSetShader(pixels.as_ref(), None);
                context.Draw(3, 0);
                context.OMSetRenderTargets(None, None);
                context.CSSetShader(threads.as_ref(), None);
                context.CSSetUnorderedAccessViews(0, 1, Some([view].as_ptr()), None);
                context.Dispatch(2, 2, 1);
                context.CSSetUnorderedAccessViews(0, 1, Some([None].as_ptr()), None);
            }
            print_buffer("raw signed atomics", &words, desc.ByteWidth);
        }
        "clear_view" => {
            // `ClearView` of a target's rectangles (one reaching past its edge) and of a whole
            // storage view.
            let at = |format, bind| Tex {
                w: 8,
                h: 4,
                format,
                samples: 1,
                bind,
            };
            let target = make(
                &at(DXGI_FORMAT_R8G8B8A8_UNORM, D3D11_BIND_RENDER_TARGET),
                None,
            )?;
            let stored = make(
                &at(DXGI_FORMAT_R32_FLOAT, D3D11_BIND_UNORDERED_ACCESS),
                None,
            )?;
            let context1: ID3D11DeviceContext1 = windows::core::Interface::cast(&context)?;
            let rects = [
                windows::Win32::Foundation::RECT {
                    left: 1,
                    top: 1,
                    right: 4,
                    bottom: 3,
                },
                windows::Win32::Foundation::RECT {
                    left: 6,
                    top: 0,
                    right: 12,
                    bottom: 2,
                },
            ];
            let (target_view, stored_view) = (rtv(&target).unwrap(), uav(&stored).unwrap());
            unsafe {
                context.ClearRenderTargetView(&target_view, &[0.0; 4]);
                context1.ClearView(&target_view, &[1.0, 0.5, 0.25, 1.0], Some(&rects));
                context1.ClearView(&stored_view, &[0.75, 0.0, 0.0, 0.0], None);
            }
            print("clear view rects", &target, 4);
            print("clear view storage", &stored, 4);

            let mut options = D3D11_FEATURE_DATA_D3D11_OPTIONS::default();
            let r = unsafe {
                device.CheckFeatureSupport(
                    D3D11_FEATURE_D3D11_OPTIONS,
                    (&raw mut options).cast(),
                    size_of_val(&options) as u32,
                )
            };
            println!(
                "options {:#010x} ClearView {}",
                code(r),
                options.ClearView.as_bool()
            );

            // Rectangles of storage views: floats, integers (above their range, fractions,
            // NaN), and targets of an integer format, below their range too. Negative values
            // into integer storage views are left out: the RTX 3080 does not clamp them.
            let odd = [300.0, -5.0, 2.7, f32::NAN];
            for (name, format, bind, value, texel) in [
                (
                    "storage rects float",
                    DXGI_FORMAT_R32_FLOAT,
                    D3D11_BIND_UNORDERED_ACCESS,
                    [0.75, 0.0, 0.0, 0.0],
                    4,
                ),
                (
                    "storage rects unorm",
                    DXGI_FORMAT_R8G8B8A8_UNORM,
                    D3D11_BIND_UNORDERED_ACCESS,
                    [1.5, 0.5, -1.0, 0.25],
                    4,
                ),
                (
                    "storage rects snorm",
                    DXGI_FORMAT_R8G8B8A8_SNORM,
                    D3D11_BIND_UNORDERED_ACCESS,
                    [-1.0, 0.5, 2.0, -0.25],
                    4,
                ),
                (
                    "storage rects uint",
                    DXGI_FORMAT_R8G8B8A8_UINT,
                    D3D11_BIND_UNORDERED_ACCESS,
                    [300.0, 2.7, f32::NAN, 0.0],
                    4,
                ),
                (
                    "storage rects sint",
                    DXGI_FORMAT_R16G16B16A16_SINT,
                    D3D11_BIND_UNORDERED_ACCESS,
                    [40000.0, 3.9, 0.0, 7.0],
                    8,
                ),
                (
                    "storage rects r32 uint",
                    DXGI_FORMAT_R32_UINT,
                    D3D11_BIND_UNORDERED_ACCESS,
                    [5e9, 0.0, 0.0, 0.0],
                    4,
                ),
                (
                    "target rects uint",
                    DXGI_FORMAT_R8G8B8A8_UINT,
                    D3D11_BIND_RENDER_TARGET,
                    odd,
                    4,
                ),
            ] {
                let zeros = vec![0u8; (8 * 4 * texel) as usize];
                let t = make(&at(format, bind), Some((&zeros, 8 * texel)))?;
                unsafe {
                    if bind == D3D11_BIND_RENDER_TARGET {
                        context1.ClearView(&rtv(&t).unwrap(), &value, Some(&rects));
                    } else {
                        context1.ClearView(&uav(&t).unwrap(), &value, Some(&rects));
                    }
                }
                print(name, &t, texel);
            }

            // Rectangles of a storage view of both layers of a two-layer array.
            let desc = D3D11_TEXTURE2D_DESC {
                Width: 8,
                Height: 4,
                MipLevels: 1,
                ArraySize: 2,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                ..Default::default()
            };
            let mut array = None;
            let zeros = [0u8; 8 * 4 * 8];
            let layers = [D3D11_SUBRESOURCE_DATA {
                pSysMem: zeros.as_ptr().cast(),
                SysMemPitch: 8 * 8,
                SysMemSlicePitch: 0,
            }; 2];
            unsafe { device.CreateTexture2D(&desc, Some(layers.as_ptr()), Some(&mut array))? };
            let array = array.unwrap();
            let mut view = None;
            unsafe {
                device.CreateUnorderedAccessView(&array, None, Some(&mut view))?;
                context1.ClearView(&view.unwrap(), &[0.5, -2.0, 0.0, 0.0], Some(&rects));
            }
            let layer = make(&at(DXGI_FORMAT_R32G32_FLOAT, D3D11_BIND_FLAG(0)), None)?;
            let whole = make(
                &at(DXGI_FORMAT_R8G8B8A8_UINT, D3D11_BIND_RENDER_TARGET),
                None,
            )?;
            unsafe { context.ClearRenderTargetView(&rtv(&whole).unwrap(), &odd) };
            print("target whole uint", &whole, 4);
            for l in 0..desc.ArraySize {
                unsafe { context.CopySubresourceRegion(&layer, 0, 0, 0, 0, &array, l, None) };
                print(&format!("storage rects layer {l}"), &layer, 8);
            }

            // Views of buffers and of 1D and 3D textures, holding 0x11 bytes before, cleared
            // whole, then the textures' rectangles.
            use windows::Win32::Foundation::RECT;
            let filled = [0x11u8; 64];
            let data = D3D11_SUBRESOURCE_DATA {
                pSysMem: filled.as_ptr().cast(),
                SysMemPitch: 16,
                SysMemSlicePitch: 32,
            };
            let unordered = D3D11_BIND_UNORDERED_ACCESS.0 as u32;
            let mut buffer = None;
            let mut buffer_view = None;
            unsafe {
                device.CreateBuffer(
                    &D3D11_BUFFER_DESC {
                        ByteWidth: 16,
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: unordered,
                        ..Default::default()
                    },
                    Some(&data),
                    Some(&mut buffer),
                )?;
                let buffer = buffer.as_ref().unwrap();
                device.CreateUnorderedAccessView(
                    buffer,
                    Some(&D3D11_UNORDERED_ACCESS_VIEW_DESC {
                        Format: DXGI_FORMAT_R32_UINT,
                        ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
                        Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                            Buffer: D3D11_BUFFER_UAV {
                                FirstElement: 0,
                                NumElements: 4,
                                Flags: 0,
                            },
                        },
                    }),
                    Some(&mut buffer_view),
                )?;
                context1.ClearView(&buffer_view.unwrap(), &[7.0, 0.0, 0.0, 0.0], None);
            }
            print_buffer("storage buffer", buffer.as_ref().unwrap(), 16);

            let mut line = None;
            let mut line_view = None;
            unsafe {
                device.CreateTexture1D(
                    &D3D11_TEXTURE1D_DESC {
                        Width: 4,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_R32_FLOAT,
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: unordered,
                        ..Default::default()
                    },
                    Some(&data),
                    Some(&mut line),
                )?;
                let line = line.as_ref().unwrap();
                device.CreateUnorderedAccessView(line, None, Some(&mut line_view))?;
                let line_view = line_view.unwrap();
                context1.ClearView(&line_view, &[0.75, 0.0, 0.0, 0.0], None);
                context1.ClearView(
                    &line_view,
                    &[0.5, 0.0, 0.0, 0.0],
                    Some(&[RECT {
                        left: 1,
                        top: 0,
                        right: 3,
                        bottom: 1,
                    }]),
                );
            }
            let mut volume = None;
            let (mut volume_target, mut volume_view) = (None, None);
            unsafe {
                device.CreateTexture3D(
                    &D3D11_TEXTURE3D_DESC {
                        Width: 4,
                        Height: 2,
                        Depth: 2,
                        MipLevels: 1,
                        Format: DXGI_FORMAT_R32_FLOAT,
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: unordered | D3D11_BIND_RENDER_TARGET.0 as u32,
                        ..Default::default()
                    },
                    Some(&data),
                    Some(&mut volume),
                )?;
                let volume = volume.as_ref().unwrap();
                device.CreateRenderTargetView(volume, None, Some(&mut volume_target))?;
                device.CreateUnorderedAccessView(volume, None, Some(&mut volume_view))?;
                let (volume_view, volume_target) = (volume_view.unwrap(), volume_target.unwrap());
                context1.ClearView(&volume_view, &[0.25, 0.0, 0.0, 0.0], None);
                context1.ClearView(&volume_target, &[0.5, 0.0, 0.0, 0.0], None);
                let rect = |left, top, right, bottom| RECT {
                    left,
                    top,
                    right,
                    bottom,
                };
                context1.ClearView(
                    &volume_view,
                    &[1.0, 0.0, 0.0, 0.0],
                    Some(&[rect(1, 0, 3, 1)]),
                );
                context1.ClearView(
                    &volume_target,
                    &[2.0, 0.0, 0.0, 0.0],
                    Some(&[rect(0, 1, 2, 2)]),
                );
            }
            // Both read back through a staging copy, all of their bytes in order.
            let read = |what: &str, resource: &ID3D11Resource, staging: &ID3D11Resource| {
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                unsafe {
                    context.CopyResource(staging, resource);
                    context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
                    for row in 0..filled.len() / 16 {
                        let (slice, y) = (row / 2, row % 2);
                        let at = slice * mapped.DepthPitch as usize + y * mapped.RowPitch as usize;
                        let bytes =
                            std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(at), 16);
                        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                        println!("{what}: row {row} {}", hex.join(""));
                    }
                    context.Unmap(staging, 0);
                }
                windows::core::Result::Ok(())
            };
            let staged = D3D11_CPU_ACCESS_READ.0 as u32;
            let (mut line_staging, mut volume_staging) = (None, None);
            unsafe {
                device.CreateTexture1D(
                    &D3D11_TEXTURE1D_DESC {
                        Width: 4,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: DXGI_FORMAT_R32_FLOAT,
                        Usage: D3D11_USAGE_STAGING,
                        CPUAccessFlags: staged,
                        ..Default::default()
                    },
                    None,
                    Some(&mut line_staging),
                )?;
                device.CreateTexture3D(
                    &D3D11_TEXTURE3D_DESC {
                        Width: 4,
                        Height: 2,
                        Depth: 2,
                        MipLevels: 1,
                        Format: DXGI_FORMAT_R32_FLOAT,
                        Usage: D3D11_USAGE_STAGING,
                        CPUAccessFlags: staged,
                        ..Default::default()
                    },
                    None,
                    Some(&mut volume_staging),
                )?;
            }
            // The 1D texture holds one row of 16 bytes.
            let cast = |r: &windows::core::IUnknown| -> windows::core::Result<ID3D11Resource> {
                windows::core::Interface::cast(r)
            };
            let line_staging = cast(&line_staging.unwrap().into())?;
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                context.CopyResource(&line_staging, &cast(&line.unwrap().into())?);
                context.Map(&line_staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
                let bytes = std::slice::from_raw_parts(mapped.pData.cast::<u8>(), 16);
                let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                println!("storage 1d: row 0 {}", hex.join(""));
                context.Unmap(&line_staging, 0);
            }
            read(
                "target and storage 3d",
                &cast(&volume.unwrap().into())?,
                &cast(&volume_staging.unwrap().into())?,
            )?;
        }
        "cs_min_lod" => {
            // An 8x8 chain of four levels; texel (x, y) of level l is (l * 60, x * 32, y * 32, 255).
            const SIDE: u32 = 8;
            const LEVELS: u32 = 4;
            let levels: Vec<Vec<u8>> = (0..LEVELS)
                .map(|l| {
                    let n = SIDE >> l;
                    (0..n * n)
                        .flat_map(|i| [(l * 60) as u8, (i % n * 32) as u8, (i / n * 32) as u8, 255])
                        .collect()
                })
                .collect();
            let data: Vec<D3D11_SUBRESOURCE_DATA> = levels
                .iter()
                .enumerate()
                .map(|(l, bytes)| D3D11_SUBRESOURCE_DATA {
                    pSysMem: bytes.as_ptr().cast(),
                    SysMemPitch: (SIDE >> l) * 4,
                    SysMemSlicePitch: 0,
                })
                .collect();
            let desc = D3D11_TEXTURE2D_DESC {
                Width: SIDE,
                Height: SIDE,
                MipLevels: LEVELS,
                ArraySize: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_RESOURCE_CLAMP.0 as u32,
                ..Default::default()
            };
            let mut chain = None;
            unsafe { device.CreateTexture2D(&desc, Some(data.as_ptr()), Some(&mut chain))? };
            let chain = chain.unwrap();
            let mut view = None;
            unsafe { device.CreateShaderResourceView(&chain, None, Some(&mut view))? };
            let sampler = {
                let desc = D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MaxLOD: f32::MAX,
                    ..Default::default()
                };
                let mut s = None;
                unsafe { device.CreateSamplerState(&desc, Some(&mut s))? };
                s
            };
            // Each clamp's five results in a row: two samples, two loads and a gather.
            let clamps = [1.5f32, 5.0, 0.25];
            let results = make(
                &Tex {
                    w: 5,
                    h: clamps.len() as u32,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_UNORDERED_ACCESS,
                },
                None,
            )?;
            let shader = cs("Texture2D<float4> t : register(t0);
                SamplerState s : register(s0);
                RWTexture2D<float4> o : register(u0);
                cbuffer c : register(b0) { uint row; }
                [numthreads(1, 1, 1)]
                void main() {
                    float2 uv = float2(0.3, 0.6);
                    o[uint2(0, row)] = t.SampleLevel(s, uv, 0);
                    o[uint2(1, row)] = t.SampleLevel(s, uv, 2.25);
                    o[uint2(2, row)] = t.Load(int3(1, 1, 0));
                    o[uint2(3, row)] = t.Load(int3(0, 0, 2));
                    o[uint2(4, row)] = t.GatherRed(s, uv);
                }");
            unsafe {
                context.CSSetShader(shader.as_ref(), None);
                context.CSSetShaderResources(0, Some(&[view]));
                context.CSSetSamplers(0, Some(&[sampler]));
                context.CSSetUnorderedAccessViews(0, 1, Some([uav(&results)].as_ptr()), None);
                for (row, &clamp) in clamps.iter().enumerate() {
                    context.SetResourceMinLOD(&chain, clamp);
                    let row = constants([f32::from_bits(row as u32), 0.0, 0.0, 0.0]);
                    context.CSSetConstantBuffers(0, Some(&[row]));
                    context.Dispatch(1, 1, 1);
                }
            }
            print("filtered cs min lod", &results, 4);
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
        "msaa_lines" | "aliased_msaa_lines" => {
            // Lines under MultisampleEnable (or, `aliased_msaa_lines`, aliased lines without it)
            // into four samples, each pixel's covered samples counted.
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
                MultisampleEnable: (case == "msaa_lines").into(),
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
        "unshaded" => {
            // Depth-only draws without a pixel shader or a target, with depth clipping off (each
            // depth clamped to its viewport's range) and on, into 32-bit float depth cleared to
            // 0.5; each draw covers a quarter of the depth buffer through its own viewport.
            let full = vs(FULL);
            let depth = make(
                &Tex {
                    w: 16,
                    h: 4,
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
            let mut always = None;
            let dss = D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                DepthFunc: D3D11_COMPARISON_ALWAYS,
                ..Default::default()
            };
            unsafe { device.CreateDepthStencilState(&dss, Some(&mut always))? };
            let raster = |clip: bool| {
                let desc = D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: clip.into(),
                    ..Default::default()
                };
                let mut out = None;
                unsafe { device.CreateRasterizerState(&desc, Some(&mut out)).unwrap() };
                out
            };
            let (clamped, clipped) = (raster(false), raster(true));
            unsafe {
                context.ClearDepthStencilView(dsv.as_ref().unwrap(), D3D11_CLEAR_DEPTH.0, 0.5, 0);
                context.OMSetRenderTargets(None, dsv.as_ref());
                context.OMSetDepthStencilState(always.as_ref(), 0);
                context.VSSetShader(full.as_ref(), None);
                context.PSSetShader(None, None);
            }
            for (x, y, z, state) in [
                (0.0, 0.0, 1.5, &clamped),
                (8.0, 0.0, -0.5, &clamped),
                (0.0, 2.0, 1.5, &clipped),
                (8.0, 2.0, 0.25, &clamped),
            ] {
                let c = constants([z, 0.0, 0.0, 0.0]);
                unsafe {
                    context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                        TopLeftX: x,
                        TopLeftY: y,
                        Width: 8.0,
                        Height: 2.0,
                        MinDepth: 0.25,
                        MaxDepth: 0.75,
                    }]));
                    context.RSSetState(state.as_ref());
                    context.VSSetConstantBuffers(0, Some(std::slice::from_ref(&c)));
                    context.Draw(3, 0);
                }
            }
            unsafe {
                context.RSSetState(None);
                context.OMSetDepthStencilState(None, 0);
                context.OMSetRenderTargets(None, None);
            }
            print("unshaded depth", &depth, 4);
        }
        "staged_bias" => {
            // A constant depth bias on what geometry and tessellation stages rasterise: triangles
            // made from points, from lines and from a patch, and a line strip made from a
            // triangle, into 16-bit depth cleared to 0.5.
            let corners = vs("float4 main(uint id : SV_VertexID) : SV_Position {
                    float2 q[3] = { float2(-0.5, 0.0), float2(0.5, 0.0), float2(0.0, 0.6) };
                    return float4(q[id], 0.25, 1);
                }");
            let control = vs("struct V { float4 p : POS; };
                V main(uint id : SV_VertexID) {
                    float2 q[3] = { float2(-0.9, -0.9), float2(0.0, 0.9), float2(0.9, -0.9) };
                    V v; v.p = float4(q[id], 0.25, 1); return v;
                }");
            let geometry = |src: &str| {
                let mut gs = None;
                let r = unsafe {
                    device.CreateGeometryShader(&compile(src, s!("gs_5_0")), None, Some(&mut gs))
                };
                println!("CreateGeometryShader {:#010x}", code(r));
                gs
            };
            let from_points = geometry(
                "struct V { float4 p : SV_Position; };
                [maxvertexcount(4)] void main(point V i[1], inout TriangleStream<V> s) {
                    float2 d[4] = { float2(-0.2, 0.4), float2(0.2, 0.4), float2(-0.2, -0.4),
                        float2(0.2, -0.4) };
                    V o;
                    for (int k = 0; k < 4; ++k) { o.p = i[0].p + float4(d[k], 0, 0); s.Append(o); }
                }",
            );
            let from_lines = geometry(
                "struct V { float4 p : SV_Position; };
                [maxvertexcount(4)] void main(line V i[2], inout TriangleStream<V> s) {
                    V o;
                    o.p = i[0].p + float4(0, 0.3, 0, 0); s.Append(o);
                    o.p = i[1].p + float4(0, 0.3, 0, 0); s.Append(o);
                    o.p = i[0].p - float4(0, 0.3, 0, 0); s.Append(o);
                    o.p = i[1].p - float4(0, 0.3, 0, 0); s.Append(o);
                }",
            );
            let outline = geometry(
                "struct V { float4 p : SV_Position; };
                [maxvertexcount(4)] void main(triangle V i[3], inout LineStream<V> s) {
                    for (int k = 0; k < 4; ++k) s.Append(i[k % 3]);
                }",
            );
            let (mut hs, mut ds) = (None, None);
            unsafe {
                let r = device.CreateHullShader(
                    &compile(
                        "struct V { float4 p : POS; };
                        struct C { float e[3] : SV_TessFactor; float i : SV_InsideTessFactor; };
                        C pc(InputPatch<V, 3> ip) {
                            C c; c.e[0] = 2; c.e[1] = 2; c.e[2] = 2; c.i = 2; return c;
                        }
                        [domain(\"tri\")] [partitioning(\"integer\")]
                        [outputtopology(\"triangle_cw\")] [outputcontrolpoints(3)]
                        [patchconstantfunc(\"pc\")]
                        V main(InputPatch<V, 3> ip, uint i : SV_OutputControlPointID) {
                            return ip[i];
                        }",
                        s!("hs_5_0"),
                    ),
                    None,
                    Some(&mut hs),
                );
                println!("CreateHullShader {:#010x}", code(r));
                let r = device.CreateDomainShader(
                    &compile(
                        "struct V { float4 p : POS; };
                        struct C { float e[3] : SV_TessFactor; float i : SV_InsideTessFactor; };
                        [domain(\"tri\")] float4 main(C c, float3 uvw : SV_DomainLocation,
                            const OutputPatch<V, 3> q) : SV_Position {
                            return q[0].p * uvw.x + q[1].p * uvw.y + q[2].p * uvw.z;
                        }",
                        s!("ds_5_0"),
                    ),
                    None,
                    Some(&mut ds),
                );
                println!("CreateDomainShader {:#010x}", code(r));
            }
            let flat = ps("float4 main(float4 p : SV_Position) : SV_Target { return 1; }");
            let target = make(
                &Tex {
                    w: 16,
                    h: 8,
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
            for name in ["points", "lines", "patches", "outline"] {
                let depth = make(
                    &Tex {
                        w: 16,
                        h: 8,
                        format: DXGI_FORMAT_R16_TYPELESS,
                        samples: 1,
                        bind: D3D11_BIND_DEPTH_STENCIL,
                    },
                    None,
                )?;
                let mut dsv = None;
                let desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
                    Format: DXGI_FORMAT_D16_UNORM,
                    ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
                    ..Default::default()
                };
                unsafe { device.CreateDepthStencilView(&depth, Some(&desc), Some(&mut dsv))? };
                unsafe {
                    viewport(16, 8);
                    context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                    context.ClearDepthStencilView(
                        dsv.as_ref().unwrap(),
                        D3D11_CLEAR_DEPTH.0,
                        0.5,
                        0,
                    );
                    context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), dsv.as_ref());
                    context.OMSetDepthStencilState(always.as_ref(), 0);
                    context.RSSetState(raster.as_ref());
                    context.PSSetShader(flat.as_ref(), None);
                    let (shader, topology) = match name {
                        "points" => (&corners, D3D_PRIMITIVE_TOPOLOGY_POINTLIST),
                        "lines" => (&corners, D3D_PRIMITIVE_TOPOLOGY_LINELIST),
                        "outline" => (&corners, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST),
                        _ => (&control, D3D_PRIMITIVE_TOPOLOGY_3_CONTROL_POINT_PATCHLIST),
                    };
                    context.VSSetShader(shader.as_ref(), None);
                    match name {
                        "points" => context.GSSetShader(from_points.as_ref(), None),
                        "lines" => context.GSSetShader(from_lines.as_ref(), None),
                        "outline" => context.GSSetShader(outline.as_ref(), None),
                        _ => {
                            context.HSSetShader(hs.as_ref(), None);
                            context.DSSetShader(ds.as_ref(), None);
                        }
                    }
                    context.IASetPrimitiveTopology(topology);
                    context.Draw(
                        if matches!(name, "points" | "lines") {
                            2
                        } else {
                            3
                        },
                        0,
                    );
                    context.GSSetShader(None, None);
                    context.HSSetShader(None, None);
                    context.DSSetShader(None, None);
                    context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    context.RSSetState(None);
                    context.OMSetDepthStencilState(None, 0);
                    context.OMSetRenderTargets(None, None);
                }
                print(&format!("staged bias {name} depth"), &depth, 2);
            }
        }
        "aa_lines" | "aa_strip_indexed" => {
            // Alpha-antialiased lines (antialiased lines without multisampling, back faces culled,
            // which lines never are) with colours from their vertices, at perspective depths and
            // through a viewport off the target's corner, blended by their alpha over transparent
            // black, so the target's alpha holds each pixel's coverage: a list and a strip, or
            // (`aa_strip_indexed`) an indexed strip cut by the restart index.
            let side = 32;
            let target = make(
                &Tex {
                    w: side,
                    h: side,
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    samples: 1,
                    bind: D3D11_BIND_RENDER_TARGET,
                },
                None,
            )?;
            let rt = rtv(&target);
            let lines = vs("struct V { float4 p : SV_Position; float4 c : COLOR; };
                V main(uint id : SV_VertexID) {
                    float2 p[11] = { float2(-0.9, 0.53125), float2(0.9, 0.53125),
                        float2(-0.8, 0), float2(0.8, 0),
                        float2(-0.9, -0.9), float2(0.7, -0.2),
                        float2(0.3, -0.95), float2(0.45, 0.95),
                        float2(-0.95, 0.9), float2(-0.2, 0.7), float2(0.6, 0.85) };
                    float4 c[3] = { float4(1, 0, 0, 1), float4(0, 1, 0, 1), float4(0, 0, 1, 1) };
                    V v;
                    float w = 1 + (id % 4) * 0.25;
                    v.p = float4(p[id] * w, 0.5 * w, w);
                    v.c = c[id % 3];
                    return v;
                }");
            let coloured = ps(
                "float4 main(float4 p : SV_Position, float4 c : COLOR) : SV_Target {
                    return c;
                }",
            );
            let mut raster = None;
            let desc = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_BACK,
                DepthClipEnable: true.into(),
                AntialiasedLineEnable: true.into(),
                ..Default::default()
            };
            unsafe { device.CreateRasterizerState(&desc, Some(&mut raster))? };
            let mut blend_desc = D3D11_BLEND_DESC::default();
            blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            let mut blend = None;
            unsafe { device.CreateBlendState(&blend_desc, Some(&mut blend))? };
            // Vertices 0 to 5, cut, then the strip's three; drawn from base vertex 0.
            let strip: [u16; 10] = [0, 1, 2, 3, 4, 5, u16::MAX, 8, 9, 10];
            let index_desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of_val(&strip) as u32,
                Usage: D3D11_USAGE_IMMUTABLE,
                BindFlags: D3D11_BIND_INDEX_BUFFER.0 as u32,
                ..Default::default()
            };
            let index_data = D3D11_SUBRESOURCE_DATA {
                pSysMem: strip.as_ptr().cast(),
                ..Default::default()
            };
            let mut indices = None;
            unsafe { device.CreateBuffer(&index_desc, Some(&index_data), Some(&mut indices))? };
            unsafe {
                context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    TopLeftX: 6.0,
                    TopLeftY: 4.0,
                    Width: 22.0,
                    Height: 26.0,
                    MaxDepth: 1.0,
                    ..Default::default()
                }]));
                context.ClearRenderTargetView(rt.as_ref().unwrap(), &[0.0; 4]);
                context.OMSetRenderTargets(Some(std::slice::from_ref(&rt)), None);
                context.OMSetBlendState(blend.as_ref(), None, u32::MAX);
                context.RSSetState(raster.as_ref());
                context.VSSetShader(lines.as_ref(), None);
                context.PSSetShader(coloured.as_ref(), None);
                if case == "aa_lines" {
                    context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINELIST);
                    context.Draw(8, 0);
                    context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINESTRIP);
                    context.Draw(3, 8);
                } else {
                    context.IASetIndexBuffer(indices.as_ref(), DXGI_FORMAT_R16_UINT, 0);
                    context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINESTRIP);
                    context.DrawIndexed(strip.len() as u32, 0, 0);
                    context.IASetIndexBuffer(None, DXGI_FORMAT_R16_UINT, 0);
                }
                context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                context.RSSetState(None);
                context.OMSetBlendState(None, None, u32::MAX);
                context.OMSetRenderTargets(None, None);
            }
            print("aa lines", &target, 4);
        }
        _ => println!("no case {case:?}"),
    }
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
