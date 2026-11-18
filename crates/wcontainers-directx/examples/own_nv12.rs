//! A small Direct3D 11 program using an NV12 texture: made with initial data, viewed through
//! each plane (loaded and sampled by pixel shaders, cleared as targets), written by
//! `UpdateSubresource` and copied by `CopySubresourceRegion` at even and odd boxes, and written
//! and read through staging maps. Prints each answer and the texture's bytes after each step (as
//! hex, in the layout a map gives), for comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;

    const VS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/common.vs_full.dxbc");
    const SAMPLE: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/textures.ps_mips.dxbc");
    const LOAD: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/textures.ps_load.dxbc");
    const W: u32 = 8;
    const H: u32 = 4;

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
    let (device, context): (ID3D11Device, ID3D11DeviceContext) =
        (device.unwrap(), context.unwrap());

    let r = unsafe { device.CheckFormatSupport(DXGI_FORMAT_NV12) };
    let support = r.as_ref().map_or(0, |s| *s);
    // The uses this program relies on; real drivers add video uses no browser has.
    let used = (D3D11_FORMAT_SUPPORT_TEXTURE2D.0
        | D3D11_FORMAT_SUPPORT_SHADER_LOAD.0
        | D3D11_FORMAT_SUPPORT_SHADER_SAMPLE.0
        | D3D11_FORMAT_SUPPORT_RENDER_TARGET.0) as u32;
    println!(
        "CheckFormatSupport {:#010x} {:#x}",
        code(r.map(|_| ())),
        support & used
    );

    let texture = |w, h, format, usage, bind: u32, cpu: u32, init: Option<&[u8]>| {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: usage,
            BindFlags: bind,
            CPUAccessFlags: cpu,
            ..Default::default()
        };
        let data = init.map(|bytes| D3D11_SUBRESOURCE_DATA {
            pSysMem: bytes.as_ptr().cast(),
            SysMemPitch: w,
            SysMemSlicePitch: 0,
        });
        let mut t = None;
        let r = unsafe {
            device.CreateTexture2D(&desc, data.as_ref().map(|d| d as *const _), Some(&mut t))
        };
        r.map(|_| t.unwrap())
    };
    let bound = (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32;
    let rw = (D3D11_CPU_ACCESS_READ.0 | D3D11_CPU_ACCESS_WRITE.0) as u32;
    // Every byte differs: the luma rows, then the chroma rows (U V pairs).
    let bytes: Vec<u8> = (0..W * H * 3 / 2).map(|i| (i * 37 + 11) as u8).collect();

    for (w, h) in [(7, 4), (8, 3)] {
        let r = texture(w, h, DXGI_FORMAT_NV12, D3D11_USAGE_DEFAULT, bound, 0, None);
        println!("CreateTexture2D({w}x{h}) {:#010x}", code(r.map(|_| ())));
    }
    let nv12 = texture(
        W,
        H,
        DXGI_FORMAT_NV12,
        D3D11_USAGE_DEFAULT,
        bound,
        0,
        Some(&bytes),
    )?;
    let other = texture(
        W,
        H,
        DXGI_FORMAT_NV12,
        D3D11_USAGE_DEFAULT,
        bound,
        0,
        Some(&bytes),
    )?;
    let staging = texture(W, H, DXGI_FORMAT_NV12, D3D11_USAGE_STAGING, 0, rw, None)?;

    // The texture's bytes in the layout a map gives: luma rows, then chroma rows.
    let show = |what: &str, t: &ID3D11Texture2D| {
        unsafe { context.CopyResource(&staging, t) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let r = unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) };
        let ok = r.is_ok();
        println!("{what}: Map {:#010x}", code(r));
        if ok {
            println!(
                "{what}: whole image in the slice pitch {}",
                mapped.DepthPitch >= mapped.RowPitch * H * 3 / 2
            );
            for y in 0..H * 3 / 2 {
                let row = unsafe {
                    std::slice::from_raw_parts(
                        mapped
                            .pData
                            .cast::<u8>()
                            .add((y * mapped.RowPitch) as usize),
                        W as usize,
                    )
                };
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                println!("{what}: row {y} {}", hex.join(""));
            }
            unsafe { context.Unmap(&staging, 0) };
        }
    };
    show("initial", &nv12);

    // Views: only each plane's formats are taken.
    let srv = |format| {
        let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        let mut v = None;
        let r = unsafe { device.CreateShaderResourceView(&nv12, Some(&desc), Some(&mut v)) };
        (r, v)
    };
    let mut none = None;
    let r = unsafe { device.CreateShaderResourceView(&nv12, None, Some(&mut none)) };
    println!("CreateShaderResourceView(default) {:#010x}", code(r));
    for (name, format) in [
        ("NV12", DXGI_FORMAT_NV12),
        ("R8_SNORM", DXGI_FORMAT_R8_SNORM),
        ("R8G8B8A8_UNORM", DXGI_FORMAT_R8G8B8A8_UNORM),
    ] {
        println!(
            "CreateShaderResourceView({name}) {:#010x}",
            code(srv(format).0)
        );
    }
    let (r, luma) = srv(DXGI_FORMAT_R8_UNORM);
    println!("CreateShaderResourceView(R8_UNORM) {:#010x}", code(r));
    let (r, chroma) = srv(DXGI_FORMAT_R8G8_UNORM);
    println!("CreateShaderResourceView(R8G8_UNORM) {:#010x}", code(r));

    // Each plane loaded and sampled into an RGBA target of its size.
    let sampler_desc = D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
        AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
        AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
        AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
        ComparisonFunc: D3D11_COMPARISON_NEVER,
        MaxLOD: f32::MAX,
        ..Default::default()
    };
    let (mut sampler, mut vs, mut sample, mut load) = (None, None, None, None);
    unsafe {
        device.CreateSamplerState(&sampler_desc, Some(&mut sampler))?;
        device.CreateVertexShader(VS, None, Some(&mut vs))?;
        device.CreatePixelShader(SAMPLE, None, Some(&mut sample))?;
        device.CreatePixelShader(LOAD, None, Some(&mut load))?;
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.VSSetShader(vs.as_ref(), None);
        context.PSSetSamplers(0, Some(&[sampler.clone()]));
    }
    let target = texture(
        W,
        H,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_RENDER_TARGET.0 as u32,
        0,
        None,
    )?;
    let read = texture(
        W,
        H,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        D3D11_USAGE_STAGING,
        0,
        D3D11_CPU_ACCESS_READ.0 as u32,
        None,
    )?;
    let mut rtv = None;
    unsafe { device.CreateRenderTargetView(&target, None, Some(&mut rtv))? };
    for (plane, view, w, h) in [("luma", &luma, W, H), ("chroma", &chroma, W / 2, H / 2)] {
        for (how, ps) in [("loaded", &load), ("sampled", &sample)] {
            unsafe {
                context.OMSetRenderTargets(Some(&[rtv.clone()]), None);
                context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    Width: w as f32,
                    Height: h as f32,
                    MaxDepth: 1.0,
                    ..Default::default()
                }]));
                context.ClearRenderTargetView(rtv.as_ref().unwrap(), &[0.0, 0.0, 0.0, 0.0]);
                context.PSSetShader(ps.as_ref(), None);
                context.PSSetShaderResources(0, Some(std::slice::from_ref(view)));
                context.Draw(3, 0);
                context.PSSetShaderResources(0, Some(&[None]));
                context.OMSetRenderTargets(None, None);
                context.CopyResource(&read, &target);
            }
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            let r = unsafe { context.Map(&read, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) };
            let ok = r.is_ok();
            println!("{plane} {how}: Map {:#010x}", code(r));
            if ok {
                for y in 0..h {
                    let row = unsafe {
                        std::slice::from_raw_parts(
                            mapped
                                .pData
                                .cast::<u8>()
                                .add((y * mapped.RowPitch) as usize),
                            (w * 4) as usize,
                        )
                    };
                    let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                    println!("{plane} {how}: row {y} {}", hex.join(""));
                }
                unsafe { context.Unmap(&read, 0) };
            }
        }
    }

    // Each plane cleared through a target view of its format.
    for (name, format, value) in [
        ("R8_UNORM", DXGI_FORMAT_R8_UNORM, 0.25f32),
        ("R8G8_UNORM", DXGI_FORMAT_R8G8_UNORM, 0.75),
        ("NV12", DXGI_FORMAT_NV12, 0.5),
    ] {
        let desc = D3D11_RENDER_TARGET_VIEW_DESC {
            Format: format,
            ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 },
            },
        };
        let mut v = None;
        let r = unsafe { device.CreateRenderTargetView(&other, Some(&desc), Some(&mut v)) };
        println!("CreateRenderTargetView({name}) {:#010x}", code(r));
        if let Some(v) = v {
            unsafe { context.ClearRenderTargetView(&v, &[value, value / 2.0, 0.0, 0.0]) };
        }
    }
    show("cleared", &other);

    // UpdateSubresource and CopySubresourceRegion at even and odd boxes: Direct3D writes the
    // chroma under an even box and nothing under an odd one.
    let source: Vec<u8> = (0..4 * 2 * 3 / 2).map(|i| 0xa0 + i as u8).collect();
    for (name, b) in [
        ("even", (2u32, 2u32)),
        ("odd left", (1, 2)),
        ("odd top", (2, 1)),
    ] {
        let dst_box = D3D11_BOX {
            left: b.0,
            top: b.1,
            front: 0,
            right: b.0 + 4,
            bottom: b.1 + 2,
            back: 1,
        };
        unsafe {
            context.UpdateSubresource(&nv12, 0, Some(&dst_box), source.as_ptr().cast(), 4, 0);
        }
        show(&format!("updated {name}"), &nv12);
        unsafe {
            context.CopySubresourceRegion(&other, 0, b.0, b.1, 0, &nv12, 0, Some(&dst_box));
        }
        show(&format!("copied {name}"), &other);
    }

    // A staging texture written through a map and copied in whole.
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    let r = unsafe { context.Map(&staging, 0, D3D11_MAP_WRITE, 0, Some(&mut mapped)) };
    let ok = r.is_ok();
    println!("Map(WRITE) {:#010x}", code(r));
    if ok {
        for y in 0..H * 3 / 2 {
            for x in 0..W {
                unsafe {
                    *mapped
                        .pData
                        .cast::<u8>()
                        .add((y * mapped.RowPitch + x) as usize) = (y * 16 + x) as u8;
                }
            }
        }
        unsafe {
            context.Unmap(&staging, 0);
            context.CopyResource(&nv12, &staging);
        }
    }
    show("mapped", &nv12);
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
