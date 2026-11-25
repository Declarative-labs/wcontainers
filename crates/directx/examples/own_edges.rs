//! A small Direct3D 11 program at the edges of what Direct3D defines: copies between formats of one
//! typeless group and of two groups, `Map(WRITE)` on a staging texture after a copy into it, shaders
//! refused at creation, a draw whose input layout reads a slot with no vertex buffer, and draws whose
//! shaders read empty texture, sampler and constant buffer slots (Direct3D reads zeros and samples
//! with its default sampler state). Prints each case's bytes as hex rows, its HRESULTs, and the
//! device's removal reason, for comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::s;

    const VS_COLOR: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/common.vs_color.dxbc");
    const PS_COLOR: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/common.ps_color.dxbc");
    const VS_FULL: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/common.vs_full.dxbc");
    const PS_LOAD: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/textures.ps_load.dxbc");
    const PS_CUBE: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/textures.ps_cube.dxbc");
    const PS_SAMPLE: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/samplers.ps_sample.dxbc");
    const PS_WINDOW: &[u8] =
        include_bytes!("../../graphics-test/shaders/dxbc/constants.ps_window.dxbc");
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
    let (device, context): (ID3D11Device, ID3D11DeviceContext) =
        (device.unwrap(), context.unwrap());

    // A texture of `format` and four bytes a texel, with `bytes` in it if given.
    let texture = |format: DXGI_FORMAT,
                   size: u32,
                   usage: D3D11_USAGE,
                   bind: D3D11_BIND_FLAG,
                   bytes: Option<&[u8]>|
     -> windows::core::Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: size,
            Height: size,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: usage,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: if usage == D3D11_USAGE_STAGING {
                (D3D11_CPU_ACCESS_READ.0 | D3D11_CPU_ACCESS_WRITE.0) as u32
            } else {
                0
            },
            ..Default::default()
        };
        let data = bytes.map(|b| D3D11_SUBRESOURCE_DATA {
            pSysMem: b.as_ptr().cast(),
            SysMemPitch: size * 4,
            ..Default::default()
        });
        let mut out = None;
        unsafe {
            device.CreateTexture2D(&desc, data.as_ref().map(|d| d as *const _), Some(&mut out))?
        };
        Ok(out.unwrap())
    };
    // Prints a staging texture's rows as hex, one line each.
    let print = |name: &str, staging: &ID3D11Texture2D, size: u32| {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let r = code(unsafe { context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) });
        println!("{name} Map {r:#010x}");
        if r != 0 {
            return;
        }
        for y in 0..size {
            let row = unsafe {
                std::slice::from_raw_parts(
                    mapped
                        .pData
                        .cast::<u8>()
                        .add((y * mapped.RowPitch) as usize),
                    (size * 4) as usize,
                )
            };
            let hex: String = row.iter().map(|b| format!("{b:02x}")).collect();
            println!("{name} {y} {hex}");
        }
        unsafe { context.Unmap(staging, 0) };
    };
    let pattern: Vec<u8> = (0..4 * 4 * 4).map(|i| (i * 37 + 11) as u8).collect();
    let old: Vec<u8> = vec![0x5a; 4 * 4 * 4];

    // Copies: into the sRGB twin and the unsigned integer format of one group (bits copied), and into
    // a format of another group of the same size (invalid: Direct3D drops it).
    let source = texture(
        DXGI_FORMAT_R8G8B8A8_UNORM,
        4,
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_SHADER_RESOURCE,
        Some(&pattern),
    )?;
    for (name, format, whole) in [
        ("copy_srgb", DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, true),
        ("copy_uint_region", DXGI_FORMAT_R8G8B8A8_UINT, false),
        ("copy_other_group", DXGI_FORMAT_R32_FLOAT, true),
        ("copy_other_group_region", DXGI_FORMAT_R16G16_FLOAT, false),
    ] {
        let staging = texture(
            format,
            4,
            D3D11_USAGE_STAGING,
            D3D11_BIND_FLAG(0),
            Some(&old),
        )?;
        unsafe {
            if whole {
                context.CopyResource(&staging, &source);
            } else {
                context.CopySubresourceRegion(&staging, 0, 0, 0, 0, &source, 0, None);
            }
        }
        print(name, &staging, 4);
    }

    // Map(WRITE) on a staging texture after a copy into it: the map holds the copied texels, and
    // what is written over some of them joins them.
    let staging = texture(
        DXGI_FORMAT_R8G8B8A8_UNORM,
        4,
        D3D11_USAGE_STAGING,
        D3D11_BIND_FLAG(0),
        Some(&old),
    )?;
    unsafe { context.CopyResource(&staging, &source) };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    let r = code(unsafe { context.Map(&staging, 0, D3D11_MAP_WRITE, 0, Some(&mut mapped)) });
    println!("map_write_after_copy Map(WRITE) {r:#010x}");
    if r == 0 {
        unsafe {
            std::ptr::write_bytes(
                mapped.pData.cast::<u8>().add(mapped.RowPitch as usize + 4),
                0xff,
                8,
            );
            context.Unmap(&staging, 0);
        }
    }
    print("map_write_after_copy", &staging, 4);

    // Shaders refused at creation: bytes that are no shader, and a vertex shader made as a pixel
    // shader.
    let mut refused = None;
    let r = code(unsafe { device.CreatePixelShader(&[0x5a; 64], None, Some(&mut refused)) });
    println!(
        "CreatePixelShader not_a_shader {r:#010x} {}",
        refused.is_some()
    );
    let r = code(unsafe { device.CreatePixelShader(VS_FULL, None, Some(&mut refused)) });
    println!(
        "CreatePixelShader vertex_shader {r:#010x} {}",
        refused.is_some()
    );

    // Draws into a target cleared to a colour that is not zero.
    let target = texture(
        DXGI_FORMAT_R8G8B8A8_UNORM,
        SIZE,
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_RENDER_TARGET,
        None,
    )?;
    let staging = texture(
        DXGI_FORMAT_R8G8B8A8_UNORM,
        SIZE,
        D3D11_USAGE_STAGING,
        D3D11_BIND_FLAG(0),
        None,
    )?;
    let mut rtv = None;
    unsafe {
        device.CreateRenderTargetView(&target, None, Some(&mut rtv))?;
        context.OMSetRenderTargets(Some(&[rtv.clone()]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
    }
    let draw = |name: &str, vs: &[u8], ps: &[u8], count: u32| -> windows::core::Result<()> {
        let (mut v, mut p) = (None, None);
        unsafe {
            device.CreateVertexShader(vs, None, Some(&mut v))?;
            device.CreatePixelShader(ps, None, Some(&mut p))?;
            context.ClearRenderTargetView(rtv.as_ref().unwrap(), &[0.25, 0.5, 0.75, 1.0]);
            context.VSSetShader(v.as_ref(), None);
            context.PSSetShader(p.as_ref(), None);
            context.Draw(count, 0);
            context.CopyResource(&staging, &target);
        }
        print(name, &staging, SIZE);
        Ok(())
    };

    // An input layout reading slot 1, which has no buffer: its colour reads as zeros.
    let elements = [
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("POSITION"),
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            ..Default::default()
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("COLOR"),
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            InputSlot: 1,
            ..Default::default()
        },
    ];
    let corners: [f32; 12] = [
        -1.0, -1.0, 0.0, -1.0, 1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0,
    ];
    let (mut layout, mut corners_buffer) = (None, None);
    unsafe {
        device.CreateInputLayout(&elements, VS_COLOR, Some(&mut layout))?;
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: 48,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: corners.as_ptr().cast(),
            ..Default::default()
        };
        device.CreateBuffer(&desc, Some(&data), Some(&mut corners_buffer))?;
        context.IASetInputLayout(layout.as_ref());
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
        context.IASetVertexBuffers(0, 1, Some(&corners_buffer), Some(&12), Some(&0));
    }
    draw("empty_vertex_slot", VS_COLOR, PS_COLOR, 4)?;

    // Full-target triangles whose pixel shaders read empty slots.
    unsafe {
        context.IASetInputLayout(None);
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    }
    draw("empty_texture_load", VS_FULL, PS_LOAD, 3)?;
    draw("empty_cube_and_sampler", VS_FULL, PS_CUBE, 3)?;
    draw("empty_constant_buffer", VS_FULL, PS_WINDOW, 3)?;

    // A texture sampled through an empty sampler slot: Direct3D's default state, linear and clamped.
    let (mut srv, mut cell) = (None, None);
    let rect: [f32; 4] = [0.0, 0.0, SIZE as f32, SIZE as f32];
    unsafe {
        device.CreateShaderResourceView(&source, None, Some(&mut srv))?;
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: rect.as_ptr().cast(),
            ..Default::default()
        };
        device.CreateBuffer(&desc, Some(&data), Some(&mut cell))?;
        context.PSSetShaderResources(0, Some(&[srv.clone()]));
        context.PSSetConstantBuffers(0, Some(&[cell.clone()]));
    }
    draw("empty_sampler", VS_FULL, PS_SAMPLE, 3)?;

    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
