//! A small Direct3D 11 program sampling the two-pixel formats (`R8G8_B8G8_UNORM`,
//! `G8R8_G8B8_UNORM`) through a point sampler and loading them, copying one to a staging texture
//! and mapping it. Prints each answer and every texel drawn (as hex), for comparing with real
//! Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;

    const VS: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/common.vs_full.dxbc");
    const SAMPLE: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/textures.ps_mips.dxbc");
    const LOAD: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/textures.ps_load.dxbc");
    // The packed texture: 8 by 4 pixels, two to each four bytes.
    const WIDTH: u32 = 8;
    const HEIGHT: u32 = 4;
    // The target: 16 by 8, so the point sampler reads each texel four times.
    const TW: u32 = 16;
    const TH: u32 = 8;

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

    let texture = |format, w, h, usage, bind: u32, cpu: u32, init: Option<&[u8]>| {
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
            SysMemPitch: WIDTH * 2,
            SysMemSlicePitch: 0,
        });
        let mut t = None;
        let r = unsafe {
            device.CreateTexture2D(&desc, data.as_ref().map(|d| d as *const _), Some(&mut t))
        };
        r.map(|_| t.unwrap())
    };
    // Every byte differs, so a channel read from the wrong place shows.
    let bytes: Vec<u8> = (0..WIDTH * 2 * HEIGHT)
        .map(|i| (i * 37 + 11) as u8)
        .collect();
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
    }
    let target = texture(
        DXGI_FORMAT_R8G8B8A8_UNORM,
        TW,
        TH,
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_RENDER_TARGET.0 as u32,
        0,
        None,
    )?;
    let read = texture(
        DXGI_FORMAT_R8G8B8A8_UNORM,
        TW,
        TH,
        D3D11_USAGE_STAGING,
        0,
        D3D11_CPU_ACCESS_READ.0 as u32,
        None,
    )?;
    let mut rtv = None;
    unsafe {
        device.CreateRenderTargetView(&target, None, Some(&mut rtv))?;
        context.OMSetRenderTargets(Some(&[rtv.clone()]), None);
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.VSSetShader(vs.as_ref(), None);
        context.PSSetSamplers(0, Some(&[sampler.clone()]));
    }
    let print = |what: &str, staging: &ID3D11Texture2D, w: u32, h: u32, texel: u32| {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let r = unsafe { context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) };
        let ok = r.is_ok();
        println!("{what}: Map {:#010x}", code(r));
        if ok {
            for y in 0..h {
                let row = unsafe {
                    std::slice::from_raw_parts(
                        mapped
                            .pData
                            .cast::<u8>()
                            .add((y * mapped.RowPitch) as usize),
                        (w * texel) as usize,
                    )
                };
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                println!("{what}: row {y} {}", hex.join(""));
            }
            unsafe { context.Unmap(staging, 0) };
        }
    };

    for (name, format) in [
        ("R8G8_B8G8_UNORM", DXGI_FORMAT_R8G8_B8G8_UNORM),
        ("G8R8_G8B8_UNORM", DXGI_FORMAT_G8R8_G8B8_UNORM),
    ] {
        let packed = texture(
            format,
            WIDTH,
            HEIGHT,
            D3D11_USAGE_DEFAULT,
            D3D11_BIND_SHADER_RESOURCE.0 as u32,
            0,
            Some(&bytes),
        );
        println!(
            "{name}: CreateTexture2D {:#010x}",
            code(packed.as_ref().map(|_| ()).map_err(Clone::clone))
        );
        let Ok(packed) = packed else { continue };
        let mut srv = None;
        let r = unsafe { device.CreateShaderResourceView(&packed, None, Some(&mut srv)) };
        println!("{name}: CreateShaderResourceView {:#010x}", code(r));
        // Loads draw only the texture's own size, so every texel loaded is inside it.
        for (how, ps, w, h) in [
            ("sampled", &sample, TW, TH),
            ("loaded", &load, WIDTH, HEIGHT),
        ] {
            unsafe {
                context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                    Width: w as f32,
                    Height: h as f32,
                    MaxDepth: 1.0,
                    ..Default::default()
                }]));
                context.ClearRenderTargetView(rtv.as_ref().unwrap(), &[0.0, 0.0, 0.0, 0.0]);
                context.PSSetShader(ps.as_ref(), None);
                context.PSSetShaderResources(0, Some(&[srv.clone()]));
                context.Draw(3, 0);
                context.CopyResource(&read, &target);
            }
            print(&format!("{name} {how}"), &read, TW, TH, 4);
        }
        // The texture's own bytes, through a copy to a staging texture of its format.
        let staging = texture(
            format,
            WIDTH,
            HEIGHT,
            D3D11_USAGE_STAGING,
            0,
            D3D11_CPU_ACCESS_READ.0 as u32,
            None,
        )?;
        unsafe { context.CopyResource(&staging, &packed) };
        print(&format!("{name} copied"), &staging, WIDTH / 2, HEIGHT, 4);
    }
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
