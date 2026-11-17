//! A small Direct3D 11 program copying a cleared texture of depth with stencil (`D24_UNORM_S8_UINT`)
//! by `CopyResource` and `CopySubresourceRegion`, then loading the copy's depth through a view. Prints
//! each answer, the first texel of the top and bottom rows (as hex) and the device's removal reason, for
//! comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;

    const VS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/common.vs_full.dxbc");
    const PS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/textures.ps_load.dxbc");
    const SIZE: u32 = 4;

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

    let texture = |format, usage, bind: u32, cpu: u32| {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: SIZE,
            Height: SIZE,
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
        let mut t = None;
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut t)) }.map(|_| t.unwrap())
    };
    let depth_bind = (D3D11_BIND_DEPTH_STENCIL.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32;
    let a = texture(
        DXGI_FORMAT_R24G8_TYPELESS,
        D3D11_USAGE_DEFAULT,
        depth_bind,
        0,
    )?;
    let b = texture(
        DXGI_FORMAT_R24G8_TYPELESS,
        D3D11_USAGE_DEFAULT,
        depth_bind,
        0,
    )?;
    let dsv_desc = D3D11_DEPTH_STENCIL_VIEW_DESC {
        Format: DXGI_FORMAT_D24_UNORM_S8_UINT,
        ViewDimension: D3D11_DSV_DIMENSION_TEXTURE2D,
        ..Default::default()
    };
    let mut dsv = None;
    unsafe {
        device.CreateDepthStencilView(&a, Some(&dsv_desc), Some(&mut dsv))?;
        context.ClearDepthStencilView(
            dsv.as_ref().unwrap(),
            D3D11_CLEAR_DEPTH.0 | D3D11_CLEAR_STENCIL.0,
            0.25,
            7,
        );
        context.CopyResource(&b, &a);
        context.CopySubresourceRegion(&a, 0, 0, 0, 0, &b, 0, None);
        context.CopyResource(&b, &a);
        context.Flush();
    }
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason after the copies {:#010x}", code(r));

    // The copy's depth, loaded through an R24_UNORM_X8_TYPELESS view into a float target.
    let target = texture(
        DXGI_FORMAT_R32G32B32A32_FLOAT,
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_RENDER_TARGET.0 as u32,
        0,
    )?;
    let staging = texture(
        DXGI_FORMAT_R32G32B32A32_FLOAT,
        D3D11_USAGE_STAGING,
        0,
        D3D11_CPU_ACCESS_READ.0 as u32,
    )?;
    let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
        ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
        Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
            Texture2D: D3D11_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
            },
        },
    };
    let (mut srv, mut rtv, mut vs, mut ps) = (None, None, None, None);
    unsafe {
        device.CreateShaderResourceView(&b, Some(&srv_desc), Some(&mut srv))?;
        device.CreateRenderTargetView(&target, None, Some(&mut rtv))?;
        device.CreateVertexShader(VS, None, Some(&mut vs))?;
        device.CreatePixelShader(PS, None, Some(&mut ps))?;
        context.OMSetRenderTargets(Some(&[rtv.clone()]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.VSSetShader(vs.as_ref(), None);
        context.PSSetShader(ps.as_ref(), None);
        context.PSSetShaderResources(0, Some(&[srv.clone()]));
        context.Draw(3, 0);
        context.CopyResource(&staging, &target);
    }
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    let r = unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) };
    let ok = r.is_ok();
    println!("Map {:#010x}", code(r));
    if ok {
        for y in [0, SIZE - 1] {
            let row = unsafe {
                std::slice::from_raw_parts(
                    mapped
                        .pData
                        .cast::<u8>()
                        .add((y * mapped.RowPitch) as usize),
                    (SIZE * 16) as usize,
                )
            };
            let first: Vec<String> = row[..16].iter().map(|b| format!("{b:02x}")).collect();
            println!("row {y} {}", first.join(""));
        }
        unsafe { context.Unmap(&staging, 0) };
    }
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
