//! A small Direct3D 11 program the front serves: four quads from a dynamic vertex buffer written by
//! `Map(WRITE_DISCARD)`, drawn by indexed draws at different base vertices, each tinted by a constant
//! buffer rewritten with `UpdateSubresource` between draws, then read back and checked texel by texel
//! against what the arithmetic says. Prints `pass`, or the first wrong texel and exits with 1.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::{Interface, s};

    const VS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/common.vs_color.dxbc");
    const PS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/dynamic.ps_tint.dxbc");
    const SIZE: u32 = 64;
    const COLOR: [f32; 4] = [0.2, 0.4, 0.6, 1.0];
    const TINTS: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    ];

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

    let texture = |usage, bind: u32, cpu: u32| -> windows::core::Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
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
            MiscFlags: 0,
        };
        let mut out = None;
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut out))? };
        Ok(out.unwrap())
    };
    let target = texture(D3D11_USAGE_DEFAULT, D3D11_BIND_RENDER_TARGET.0 as u32, 0)?;
    let staging = texture(D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ.0 as u32)?;
    let mut rtv = None;
    unsafe { device.CreateRenderTargetView(&target, None, Some(&mut rtv))? };

    let (mut vs, mut ps, mut layout) = (None, None, None);
    let elements = [
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("POSITION"),
            Format: DXGI_FORMAT_R32G32B32_FLOAT,
            ..Default::default()
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("COLOR"),
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
            ..Default::default()
        },
    ];
    unsafe {
        device.CreateVertexShader(VS, None, Some(&mut vs))?;
        device.CreatePixelShader(PS, None, Some(&mut ps))?;
        device.CreateInputLayout(&elements, VS, Some(&mut layout))?;
    }

    let buffer =
        |desc: D3D11_BUFFER_DESC, init: Option<&[u8]>| -> windows::core::Result<ID3D11Buffer> {
            let data = init.map(|b| D3D11_SUBRESOURCE_DATA {
                pSysMem: b.as_ptr().cast(),
                ..Default::default()
            });
            let mut out = None;
            unsafe {
                device.CreateBuffer(&desc, data.as_ref().map(|d| d as *const _), Some(&mut out))?
            };
            Ok(out.unwrap())
        };
    let vertices = buffer(
        D3D11_BUFFER_DESC {
            ByteWidth: 16 * 28,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            ..Default::default()
        },
        None,
    )?;
    let index_bytes: Vec<u8> = [0u16, 1, 2, 2, 1, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    let indices = buffer(
        D3D11_BUFFER_DESC {
            ByteWidth: index_bytes.len() as u32,
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: D3D11_BIND_INDEX_BUFFER.0 as u32,
            ..Default::default()
        },
        Some(&index_bytes),
    )?;
    let tint = buffer(
        D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        },
        None,
    )?;

    // Quad k covers quadrant k of the target, its corners clockwise on screen.
    let mut floats = Vec::new();
    for k in 0..4 {
        let (left, top) = ((k % 2) as f32 - 1.0, 1.0 - (k / 2) as f32);
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (0.0, -1.0), (1.0, -1.0)] {
            floats.extend([left + x, top + y, 0.5]);
            floats.extend(COLOR);
        }
    }
    unsafe {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context.Map(&vertices, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(floats.as_ptr(), mapped.pData.cast(), floats.len());
        context.Unmap(&vertices, 0);

        context.ClearRenderTargetView(rtv.as_ref().unwrap(), &[0.0, 0.0, 0.0, 1.0]);
        context.OMSetRenderTargets(Some(&[rtv.clone()]), None);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
        context.IASetInputLayout(layout.as_ref());
        context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.IASetVertexBuffers(0, 1, Some(&Some(vertices.clone())), Some(&28), Some(&0));
        context.IASetIndexBuffer(&indices, DXGI_FORMAT_R16_UINT, 0);
        context.VSSetShader(vs.as_ref(), None);
        context.PSSetShader(ps.as_ref(), None);
        context.PSSetConstantBuffers(0, Some(&[Some(tint.clone())]));
        for (k, t) in TINTS.iter().enumerate() {
            context.UpdateSubresource(&tint, 0, None, t.as_ptr().cast(), 0, 0);
            context.DrawIndexed(6, 0, 4 * k as i32);
        }
        context.CopyResource(&staging, &target);
    }

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };
    let mut wrong = None;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let k = (y / 32 * 2 + x / 32) as usize;
            let want: [u8; 4] =
                std::array::from_fn(|c| (COLOR[c] * TINTS[k][c] * 255.0).round() as u8);
            let at = (y * mapped.RowPitch + x * 4) as usize;
            let got: [u8; 4] =
                std::array::from_fn(|c| unsafe { *mapped.pData.cast::<u8>().add(at + c) });
            if got != want && wrong.is_none() {
                wrong = Some(format!("texel {x},{y}: {got:?}, expected {want:?}"));
            }
        }
    }
    unsafe { context.Unmap(&staging, 0) };
    let adapter = unsafe {
        device
            .cast::<windows::Win32::Graphics::Dxgi::IDXGIDevice>()?
            .GetAdapter()?
            .GetDesc()?
    };
    let name = String::from_utf16_lossy(&adapter.Description);
    println!("adapter {}", name.trim_end_matches('\0'));
    // Which library the process's D3D11CreateDevice resolves into.
    unsafe {
        use windows::Win32::System::LibraryLoader::*;
        let d3d11 = GetModuleHandleW(windows::core::w!("d3d11.dll"))?;
        let entry = GetProcAddress(d3d11, s!("D3D11CreateDevice")).unwrap();
        let mut module = HMODULE::default();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            windows::core::PCWSTR(entry as *const u16),
            &mut module,
        )?;
        let mut path = [0u16; 512];
        let n = GetModuleFileNameW(Some(module), &mut path) as usize;
        println!(
            "D3D11CreateDevice in {}",
            String::from_utf16_lossy(&path[..n])
        );
    }
    match wrong {
        None => println!("pass"),
        Some(w) => {
            println!("FAIL {w}");
            std::process::exit(1);
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
