//! A small Direct3D 11 program sampling, loading and gathering a mip chain under
//! `SetResourceMinLOD` clamps (fractional, past the last level, through a view starting at level
//! one, with a sampler whose own minimum is higher), printing what `GetResourceMinLOD` answers; and
//! a draw reading both helper blocks (a start vertex the vertex shader's `SV_VertexID` leaves out,
//! and a sampler's `MipLODBias`). Prints the target's rows as hex, for comparing with real
//! Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::Interface;

    const VS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/common.vs_full.dxbc");
    const MIPS: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/textures.ps_mips.dxbc");
    const LOAD: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/textures.ps_load.dxbc");
    const GATHER: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/samplers.ps_gather.dxbc");
    const SAMPLE: &[u8] =
        include_bytes!("../../wcontainers-graphics-test/shaders/dxbc/samplers.ps_sample.dxbc");
    const SIZE: u32 = 16;
    const LEVELS: u32 = 5;

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

    // A 16x16 chain of five levels; texel (x, y) of level l is (l * 60, x * 16, y * 16, 255 - l * 40).
    let levels: Vec<Vec<u8>> = (0..LEVELS)
        .map(|l| {
            let n = SIZE >> l;
            (0..n * n)
                .flat_map(|i| {
                    let (x, y) = (i % n, i / n);
                    [
                        (l * 60) as u8,
                        (x * 16) as u8,
                        (y * 16) as u8,
                        (255 - l * 40) as u8,
                    ]
                })
                .collect()
        })
        .collect();
    let data: Vec<D3D11_SUBRESOURCE_DATA> = levels
        .iter()
        .enumerate()
        .map(|(l, t)| D3D11_SUBRESOURCE_DATA {
            pSysMem: t.as_ptr().cast(),
            SysMemPitch: (SIZE >> l) * 4,
            SysMemSlicePitch: 0,
        })
        .collect();
    let texture = |misc: u32| -> windows::core::Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: SIZE,
            Height: SIZE,
            MipLevels: LEVELS,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            MiscFlags: misc,
            ..Default::default()
        };
        let mut out = None;
        unsafe { device.CreateTexture2D(&desc, Some(data.as_ptr()), Some(&mut out))? };
        Ok(out.unwrap())
    };
    let clamped = texture(D3D11_RESOURCE_MISC_RESOURCE_CLAMP.0 as u32)?;
    let plain = texture(0)?;
    let view =
        |t: &ID3D11Texture2D, first: u32| -> windows::core::Result<ID3D11ShaderResourceView> {
            let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D,
                Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                    Texture2D: D3D11_TEX2D_SRV {
                        MostDetailedMip: first,
                        MipLevels: u32::MAX,
                    },
                },
            };
            let mut out = None;
            unsafe { device.CreateShaderResourceView(t, Some(&desc), Some(&mut out))? };
            Ok(out.unwrap())
        };
    let whole = view(&clamped, 0)?;
    let from_one = view(&clamped, 1)?;
    let plain_view = view(&plain, 0)?;

    let sampler = |min_lod: f32, bias: f32| -> windows::core::Result<ID3D11SamplerState> {
        let desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressV: D3D11_TEXTURE_ADDRESS_WRAP,
            AddressW: D3D11_TEXTURE_ADDRESS_WRAP,
            MipLODBias: bias,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: min_lod,
            MaxLOD: f32::MAX,
        };
        let mut out = None;
        unsafe { device.CreateSamplerState(&desc, Some(&mut out))? };
        Ok(out.unwrap())
    };
    let linear = sampler(0.0, 0.0)?;
    let from_two = sampler(2.0, 0.0)?;
    let biased = sampler(0.0, 1.5)?;

    // The cell the samplers' shaders read: the whole target.
    let rect: Vec<u8> = [0.0f32, 0.0, SIZE as f32, SIZE as f32]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let mut cell = None;
    unsafe {
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            },
            Some(&D3D11_SUBRESOURCE_DATA {
                pSysMem: rect.as_ptr().cast(),
                ..Default::default()
            }),
            Some(&mut cell),
        )?
    };

    let pixel = |code: &[u8]| -> windows::core::Result<ID3D11PixelShader> {
        let mut out = None;
        unsafe { device.CreatePixelShader(code, None, Some(&mut out))? };
        Ok(out.unwrap())
    };
    let mut vs = None;
    unsafe { device.CreateVertexShader(VS, None, Some(&mut vs))? };
    let shaders = [
        ("mips", pixel(MIPS)?),
        ("load", pixel(LOAD)?),
        ("gather", pixel(GATHER)?),
        ("sample", pixel(SAMPLE)?),
    ];

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
        context.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        context.VSSetShader(vs.as_ref(), None);
        context.PSSetConstantBuffers(0, Some(&[cell]));
    }

    let draw = |name: &str,
                ps: &ID3D11PixelShader,
                srv: &ID3D11ShaderResourceView,
                smp: &ID3D11SamplerState,
                start: u32| {
        let rtv = rtv.as_ref().unwrap();
        unsafe {
            context.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 0.0]);
            context.PSSetShader(ps, None);
            context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
            context.PSSetSamplers(0, Some(&[Some(smp.clone())]));
            context.Draw(3, start);
            context.CopyResource(&staging, &target);
        }
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
        unsafe { context.Unmap(&staging, 0) };
    };

    // A texture made without the clamp flag: Direct3D drops its clamp.
    unsafe { context.SetResourceMinLOD(&plain, 2.25) };
    draw(
        "mips plain clamp 2.25",
        &shaders[0].1,
        &plain_view,
        &linear,
        0,
    );
    draw("sample plain", &shaders[3].1, &plain_view, &linear, 0);
    unsafe { context.SetResourceMinLOD(&plain, 0.0) };
    for clamp in [0.0f32, 0.5, 1.5, 2.75, 4.0, 5.0] {
        unsafe { context.SetResourceMinLOD(&clamped, clamp) };
        for (name, ps) in &shaders {
            draw(&format!("{name} clamp {clamp}"), ps, &whole, &linear, 0);
        }
        draw(
            &format!("mips from one clamp {clamp}"),
            &shaders[0].1,
            &from_one,
            &linear,
            0,
        );
        draw(
            &format!("load from one clamp {clamp}"),
            &shaders[1].1,
            &from_one,
            &linear,
            0,
        );
    }
    unsafe { context.SetResourceMinLOD(&clamped, 1.5) };
    draw(
        "mips sampler from two clamp 1.5",
        &shaders[0].1,
        &whole,
        &from_two,
        0,
    );
    // Both helper blocks: the vertex shader's start vertex and the pixel shader's bias, alone and
    // with a clamp.
    draw(
        "sample biased start 5",
        &shaders[3].1,
        &plain_view,
        &biased,
        5,
    );
    draw(
        "sample biased start 5 clamp 1.5",
        &shaders[3].1,
        &whole,
        &biased,
        5,
    );
    draw(
        "mips biased start 2 clamp 1.5",
        &shaders[0].1,
        &whole,
        &biased,
        2,
    );
    // What the clamp answers, on textures made with and without the clamp flag and on a buffer.
    let mut buffer = None;
    unsafe {
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 64,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut buffer),
        )?
    };
    let buffer = buffer.unwrap();
    for (name, r) in [
        ("clamped", clamped.cast::<ID3D11Resource>()?),
        ("plain", plain.cast::<ID3D11Resource>()?),
        ("buffer", buffer.cast::<ID3D11Resource>()?),
    ] {
        let before = unsafe { context.GetResourceMinLOD(&r) };
        let mut answers = Vec::new();
        for lod in [1.25f32, 7.0, 0.0] {
            unsafe { context.SetResourceMinLOD(&r, lod) };
            answers.push(unsafe { context.GetResourceMinLOD(&r) });
        }
        println!("{name} min lod {before} then {answers:?}");
    }
    // A deferred context's call, seen from the immediate one before its list runs.
    let mut deferred = None;
    unsafe { device.CreateDeferredContext(0, Some(&mut deferred))? };
    let deferred = deferred.unwrap();
    unsafe { deferred.SetResourceMinLOD(&clamped, 3.5) };
    println!("deferred set, immediate reads {}", unsafe {
        context.GetResourceMinLOD(&clamped)
    });
    let mut list = None;
    unsafe { deferred.FinishCommandList(false, Some(&mut list))? };
    unsafe { context.ExecuteCommandList(list.as_ref(), false) };
    println!("list run, immediate reads {}", unsafe {
        context.GetResourceMinLOD(&clamped)
    });
    unsafe { context.SetResourceMinLOD(&clamped, 0.0) };

    let reason = unsafe { device.GetDeviceRemovedReason() };
    println!("removed {:#010x}", reason.err().map_or(0, |e| e.code().0));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
