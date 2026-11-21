//! A small Direct3D 11 program drawing through `DrawInstancedIndirect` and
//! `DrawIndexedInstancedIndirect` with a vertex shader that shades by `SV_VertexID` and
//! `SV_InstanceID`, so wrong start vertices, base vertices or start instances show; the same
//! arguments are also drawn directly. Prints the target's rows as hex, for comparing with real
//! Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::s;

    const VS: &[u8] = include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/draws.vs.dxbc");
    const PS: &[u8] = include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/draws.ps.dxbc");
    const SIZE: u32 = 32;

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

    let buffer = |bytes: &[u8], bind: u32, misc: u32| -> windows::core::Result<ID3D11Buffer> {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: bytes.len() as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind,
            MiscFlags: misc,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: bytes.as_ptr().cast(),
            ..Default::default()
        };
        let mut out = None;
        unsafe { device.CreateBuffer(&desc, Some(&data), Some(&mut out))? };
        Ok(out.unwrap())
    };
    let floats = |f: &[f32]| -> Vec<u8> { f.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let words = |w: &[u32]| -> Vec<u8> { w.iter().flat_map(|v| v.to_le_bytes()).collect() };

    // Eight small triangles in slot 0; each vertex id from 0 to 23 picks one.
    let mut corners = Vec::new();
    for t in 0..8 {
        let x = -0.9 + 0.22 * t as f32;
        corners.extend_from_slice(&[x, -0.2, x + 0.1, 0.2, x + 0.2, -0.2]);
    }
    let vertices = buffer(&floats(&corners), D3D11_BIND_VERTEX_BUFFER.0 as u32, 0)?;
    // Per instance: an offset and a colour.
    let mut per_instance = Vec::new();
    for i in 0..6 {
        let row = 0.6 - 0.3 * i as f32;
        per_instance.extend_from_slice(&[
            0.0,
            row,
            0.2 + 0.1 * i as f32,
            0.5,
            0.9 - 0.1 * i as f32,
            1.0,
        ]);
    }
    let instances = buffer(&floats(&per_instance), D3D11_BIND_VERTEX_BUFFER.0 as u32, 0)?;
    let indices = buffer(
        &words(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        D3D11_BIND_INDEX_BUFFER.0 as u32,
        0,
    )?;
    // Arguments at a word offset: vertices, instances, start vertex, start instance; then indexed:
    // indices, instances, start index, base vertex, start instance.
    let args = buffer(
        &words(&[0xdead, 3, 2, 6, 1, 3, 2, 3, 9, 3]),
        0,
        D3D11_RESOURCE_MISC_DRAWINDIRECT_ARGS.0 as u32,
    )?;

    let mut vs = None;
    let mut ps = None;
    let mut layout = None;
    unsafe {
        device.CreateVertexShader(VS, None, Some(&mut vs))?;
        device.CreatePixelShader(PS, None, Some(&mut ps))?;
        let element = |name, format, slot, offset, class, step| D3D11_INPUT_ELEMENT_DESC {
            SemanticName: name,
            SemanticIndex: 0,
            Format: format,
            InputSlot: slot,
            AlignedByteOffset: offset,
            InputSlotClass: class,
            InstanceDataStepRate: step,
        };
        let elements = [
            element(
                s!("POSITION"),
                DXGI_FORMAT_R32G32_FLOAT,
                0,
                0,
                D3D11_INPUT_PER_VERTEX_DATA,
                0,
            ),
            element(
                s!("OFFSET"),
                DXGI_FORMAT_R32G32_FLOAT,
                1,
                0,
                D3D11_INPUT_PER_INSTANCE_DATA,
                1,
            ),
            element(
                s!("COLOR"),
                DXGI_FORMAT_R32G32B32A32_FLOAT,
                1,
                8,
                D3D11_INPUT_PER_INSTANCE_DATA,
                1,
            ),
        ];
        device.CreateInputLayout(&elements, VS, Some(&mut layout))?;
    }

    let target_desc = |usage: D3D11_USAGE, bind: u32, cpu: u32| D3D11_TEXTURE2D_DESC {
        Width: SIZE,
        Height: SIZE,
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
        ..Default::default()
    };
    let (mut target, mut staging, mut rtv) = (None, None, None);
    unsafe {
        device.CreateTexture2D(
            &target_desc(D3D11_USAGE_DEFAULT, D3D11_BIND_RENDER_TARGET.0 as u32, 0),
            None,
            Some(&mut target),
        )?;
        device.CreateTexture2D(
            &target_desc(D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ.0 as u32),
            None,
            Some(&mut staging),
        )?;
    }
    let (target, staging) = (target.unwrap(), staging.unwrap());
    unsafe {
        device.CreateRenderTargetView(&target, None, Some(&mut rtv))?;
        context.OMSetRenderTargets(Some(&[rtv.clone()]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
        context.IASetInputLayout(layout.as_ref());
        context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.IASetVertexBuffers(
            0,
            2,
            Some([Some(vertices), Some(instances)].as_ptr()),
            Some([8, 24].as_ptr()),
            Some([0, 0].as_ptr()),
        );
        context.IASetIndexBuffer(&indices, DXGI_FORMAT_R32_UINT, 0);
        context.VSSetShader(vs.as_ref(), None);
        context.PSSetShader(ps.as_ref(), None);
    }

    let print = |name: &str| {
        let rtv = rtv.as_ref().unwrap();
        unsafe { context.CopyResource(&staging, &target) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        if let Err(e) = unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) } {
            println!("{name} Map {:#010x}", e.code().0);
            return;
        }
        for y in 0..SIZE {
            let row = unsafe {
                std::slice::from_raw_parts(
                    mapped
                        .pData
                        .cast::<u8>()
                        .add((y * mapped.RowPitch) as usize),
                    (SIZE * 4) as usize,
                )
            };
            let hex: String = row.iter().map(|b| format!("{b:02x}")).collect();
            println!("{name} {y} {hex}");
        }
        unsafe {
            context.Unmap(&staging, 0);
            context.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0]);
        }
    };

    unsafe { context.ClearRenderTargetView(rtv.as_ref().unwrap(), &[0.0, 0.0, 0.0, 1.0]) };
    unsafe { context.DrawInstancedIndirect(&args, 4) };
    print("indirect");
    unsafe { context.DrawInstanced(3, 2, 6, 1) };
    print("direct");
    unsafe { context.DrawIndexedInstancedIndirect(&args, 20) };
    print("indexed_indirect");
    unsafe { context.DrawIndexedInstanced(3, 2, 3, 9, 3) };
    print("indexed_direct");
    let reason = unsafe { device.GetDeviceRemovedReason() };
    println!("removed {:#010x}", reason.err().map_or(0, |e| e.code().0));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
