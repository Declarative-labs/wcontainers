//! A small Direct3D 11 program asking for what the front does not serve yet: a geometry shader, a 3D
//! texture, a structured buffer, a texture of 4 samples, and a draw blending with the pixel shader's second output. Prints what each call answered, as `<call> <hresult>`, and the device's
//! removal reason after the draw; real Direct3D answers 0 to all of them.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;

    const GS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/geometry.gs_points.dxbc");
    const VS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/msaa.vs_full.dxbc");
    const PS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/blend.ps_dual.dxbc");

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

    let mut gs = None;
    let r = unsafe { device.CreateGeometryShader(GS, None, Some(&mut gs)) };
    println!("CreateGeometryShader {:#010x}", code(r));

    let desc = D3D11_TEXTURE3D_DESC {
        Width: 4,
        Height: 4,
        Depth: 4,
        MipLevels: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        ..Default::default()
    };
    let mut volume = None;
    let r = unsafe { device.CreateTexture3D(&desc, None, Some(&mut volume)) };
    println!("CreateTexture3D {:#010x}", code(r));

    let desc = D3D11_BUFFER_DESC {
        ByteWidth: 64,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        MiscFlags: D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32,
        StructureByteStride: 16,
        ..Default::default()
    };
    let mut structured = None;
    let r = unsafe { device.CreateBuffer(&desc, None, Some(&mut structured)) };
    println!("CreateBuffer(structured) {:#010x}", code(r));

    let r = unsafe { device.CheckMultisampleQualityLevels(DXGI_FORMAT_R8G8B8A8_UNORM, 4) };
    let levels = r.as_ref().map_or(0, |l| *l);
    println!(
        "CheckMultisampleQualityLevels(4) {:#010x} {levels}",
        code(r.map(|_| ()))
    );
    let desc = D3D11_TEXTURE2D_DESC {
        Width: 16,
        Height: 16,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 4,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        ..Default::default()
    };
    let mut multisampled = None;
    let r = unsafe { device.CreateTexture2D(&desc, None, Some(&mut multisampled)) };
    println!("CreateTexture2D(4 samples) {:#010x}", code(r));

    let desc = D3D11_TEXTURE2D_DESC {
        Width: 16,
        Height: 16,
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
    let (mut target, mut rtv, mut vs, mut ps, mut blend) = (None, None, None, None, None);
    let mut blend_desc = D3D11_BLEND_DESC::default();
    blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        SrcBlend: D3D11_BLEND_ONE,
        DestBlend: D3D11_BLEND_SRC1_COLOR,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_ZERO,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    unsafe {
        device.CreateBlendState(&blend_desc, Some(&mut blend))?;
        context.OMSetBlendState(blend.as_ref(), None, 0xffff_ffff);
        device.CreateTexture2D(&desc, None, Some(&mut target))?;
        device.CreateRenderTargetView(target.as_ref().unwrap(), None, Some(&mut rtv))?;
        device.CreateVertexShader(VS, None, Some(&mut vs))?;
        device.CreatePixelShader(PS, None, Some(&mut ps))?;
        context.OMSetRenderTargets(Some(&[rtv.clone()]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: 16.0,
            Height: 16.0,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.VSSetShader(vs.as_ref(), None);
        context.PSSetShader(ps.as_ref(), None);
        context.Draw(3, 0);
        context.Flush();
    }
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
