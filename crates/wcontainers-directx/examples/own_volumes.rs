//! A small Direct3D 11 program on 1D and 3D textures: initial data at slice pitches that are and are
//! not a whole number of rows, `UpdateSubresource` and `CopySubresourceRegion` with 3D boxes, staging
//! 3D maps, a clear of two slices of a 3D render target, a copy between levels and layers of 1D
//! arrays, and the descriptions of views made without one. Prints bytes as hex rows and HRESULTs, for
//! comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::IDXGIAdapter;
    use windows::core::Interface;

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
    let rgba = DXGI_FORMAT_R8G8B8A8_UNORM;
    let rw = (D3D11_CPU_ACCESS_READ.0 | D3D11_CPU_ACCESS_WRITE.0) as u32;

    // A 4x4x4 texture of two levels, made from `init` if given.
    let volume = |usage: D3D11_USAGE,
                  bind: u32,
                  init: Option<&[D3D11_SUBRESOURCE_DATA]>|
     -> windows::core::Result<ID3D11Texture3D> {
        let desc = D3D11_TEXTURE3D_DESC {
            Width: 4,
            Height: 4,
            Depth: 4,
            MipLevels: 2,
            Format: rgba,
            Usage: usage,
            BindFlags: bind,
            CPUAccessFlags: if usage == D3D11_USAGE_STAGING { rw } else { 0 },
            MiscFlags: 0,
        };
        let mut out = None;
        unsafe {
            device.CreateTexture3D(&desc, init.map(|i| i.as_ptr()), Some(&mut out))?;
        }
        Ok(out.unwrap())
    };
    // Prints every slice's rows of subresource `sub` of a staging resource `w` texels wide.
    let print = |name: &str, staging: &ID3D11Resource, sub: u32, w: u32, h: u32, d: u32| {
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        let r = code(unsafe { context.Map(staging, sub, D3D11_MAP_READ, 0, Some(&mut m)) });
        println!("{name} Map {r:#010x}");
        if r != 0 {
            return;
        }
        for z in 0..d {
            for y in 0..h {
                let at = (z * m.DepthPitch + y * m.RowPitch) as usize;
                let row = unsafe {
                    std::slice::from_raw_parts(m.pData.cast::<u8>().add(at), 4 * w as usize)
                };
                let hex: String = row.iter().map(|b| format!("{b:02x}")).collect();
                println!("{name} {z} {y} {hex}");
            }
        }
        unsafe { context.Unmap(staging, sub) };
    };
    let pattern = |n: usize, seed: usize| -> Vec<u8> {
        (0..n).map(|i| (i * 37 + seed * 11 + 5) as u8).collect()
    };

    // Initial data: level 0 at a row pitch of 20 and a slice pitch of 90 (not a whole number of
    // rows), level 1 at 8 and 24 (three rows).
    let top = pattern(90 * 4, 1);
    let low = pattern(24 * 2, 2);
    let init = [
        D3D11_SUBRESOURCE_DATA {
            pSysMem: top.as_ptr().cast(),
            SysMemPitch: 20,
            SysMemSlicePitch: 90,
        },
        D3D11_SUBRESOURCE_DATA {
            pSysMem: low.as_ptr().cast(),
            SysMemPitch: 8,
            SysMemSlicePitch: 24,
        },
    ];
    let source = volume(
        D3D11_USAGE_DEFAULT,
        D3D11_BIND_SHADER_RESOURCE.0 as u32,
        Some(&init),
    )?;
    let staging = volume(D3D11_USAGE_STAGING, 0, None)?;
    unsafe { context.CopyResource(&staging, &source) };
    print("initial 0", &staging.cast()?, 0, 4, 4, 4);
    print("initial 1", &staging.cast()?, 1, 2, 2, 2);

    // A box of 2x2x2 in the middle of level 0, at a row pitch of 8 and a depth pitch of 20.
    let update = pattern(36, 3);
    let middle = D3D11_BOX {
        left: 1,
        top: 1,
        front: 1,
        right: 3,
        bottom: 3,
        back: 3,
    };
    unsafe {
        context.UpdateSubresource(&source, 0, Some(&middle), update.as_ptr().cast(), 8, 20);
        context.CopyResource(&staging, &source);
    }
    print("updated 0", &staging.cast()?, 0, 4, 4, 4);

    // Slices 1 and 2 of level 0 copied to slices 2 and 3 of a fresh staging volume, shifted by one.
    let moved = volume(D3D11_USAGE_STAGING, 0, None)?;
    let slab = D3D11_BOX {
        left: 0,
        top: 0,
        front: 1,
        right: 3,
        bottom: 4,
        back: 3,
    };
    unsafe { context.CopySubresourceRegion(&moved, 0, 1, 0, 2, &source, 0, Some(&slab)) };
    print("copied 0", &moved.cast()?, 0, 4, 4, 4);

    // A render target view of slices 1 and 2 of level 0, cleared, and one of level 1's slice 0.
    let target = volume(
        D3D11_USAGE_DEFAULT,
        (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        None,
    )?;
    let rtv = |mip: u32, first: u32, size: u32| -> windows::core::Result<ID3D11RenderTargetView> {
        let mut d = D3D11_RENDER_TARGET_VIEW_DESC {
            Format: rgba,
            ViewDimension: D3D11_RTV_DIMENSION_TEXTURE3D,
            ..Default::default()
        };
        d.Anonymous.Texture3D = D3D11_TEX3D_RTV {
            MipSlice: mip,
            FirstWSlice: first,
            WSize: size,
        };
        let mut out = None;
        unsafe { device.CreateRenderTargetView(&target, Some(&d), Some(&mut out))? };
        Ok(out.unwrap())
    };
    let (slices, below) = (rtv(0, 1, 2)?, rtv(1, 0, 1)?);
    unsafe {
        context.ClearRenderTargetView(&slices, &[0.25, 0.5, 0.75, 1.0]);
        context.ClearRenderTargetView(&below, &[1.0, 0.0, 0.5, 0.0]);
        context.CopyResource(&staging, &target);
    }
    print("cleared 0", &staging.cast()?, 0, 4, 4, 4);
    print("cleared 1", &staging.cast()?, 1, 2, 2, 2);
    let mut whole = None;
    let r = unsafe { device.CreateRenderTargetView(&target, None, Some(&mut whole)) };
    let mut d = D3D11_RENDER_TARGET_VIEW_DESC::default();
    if let Some(v) = &whole {
        unsafe { v.GetDesc(&mut d) };
    }
    let t = unsafe { d.Anonymous.Texture3D };
    println!(
        "rtv default {:#010x} dim {} mip {} first {} size {}",
        code(r),
        d.ViewDimension.0,
        t.MipSlice,
        t.FirstWSlice,
        t.WSize
    );
    let past = rtv(1, 1, 2);
    println!("rtv past the level {:#010x}", code(past.map(|_| ())));

    // A 1D array of 8 texels, 3 layers and all 4 levels; level 1 of layer 2 copied to level 0 of
    // layer 0 of a staging array at x 3.
    let line = |usage: D3D11_USAGE,
                bind: u32,
                init: Option<&[D3D11_SUBRESOURCE_DATA]>|
     -> windows::core::Result<ID3D11Texture1D> {
        let desc = D3D11_TEXTURE1D_DESC {
            Width: 8,
            MipLevels: 0,
            ArraySize: 3,
            Format: rgba,
            Usage: usage,
            BindFlags: bind,
            CPUAccessFlags: if usage == D3D11_USAGE_STAGING { rw } else { 0 },
            MiscFlags: 0,
        };
        let mut out = None;
        unsafe { device.CreateTexture1D(&desc, init.map(|i| i.as_ptr()), Some(&mut out))? };
        Ok(out.unwrap())
    };
    let bytes: Vec<Vec<u8>> = (0..12).map(|i| pattern(32 >> (i % 4), 10 + i)).collect();
    let init: Vec<_> = bytes
        .iter()
        .map(|b| D3D11_SUBRESOURCE_DATA {
            pSysMem: b.as_ptr().cast(),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        })
        .collect();
    let lines = line(
        D3D11_USAGE_IMMUTABLE,
        D3D11_BIND_SHADER_RESOURCE.0 as u32,
        Some(&init),
    )?;
    let read = line(D3D11_USAGE_STAGING, 0, None)?;
    let mut desc = D3D11_TEXTURE1D_DESC::default();
    unsafe { lines.GetDesc(&mut desc) };
    println!("1d desc mips {} layers {}", desc.MipLevels, desc.ArraySize);
    unsafe { context.CopySubresourceRegion(&read, 0, 3, 0, 0, &lines, 2 * 4 + 1, None) };
    print("1d copied", &read.cast()?, 0, 8, 1, 1);
    unsafe { context.CopyResource(&read, &lines) };
    for sub in [5, 11] {
        print(
            &format!("1d sub {sub}"),
            &read.cast()?,
            sub,
            8 >> (sub % 4),
            1,
            1,
        );
    }

    // Views made without a description: their dimension and fields.
    let srv = |r: &ID3D11Resource| {
        let mut out = None;
        let c = code(unsafe { device.CreateShaderResourceView(r, None, Some(&mut out)) });
        let mut d = D3D11_SHADER_RESOURCE_VIEW_DESC::default();
        if let Some(v) = &out {
            unsafe { v.GetDesc(&mut d) };
        }
        // The fields the dimension defines: two for 3D, four for the arrays.
        let raw: [u32; 4] = unsafe { std::mem::transmute(d.Anonymous.Texture2DArray) };
        let defined = if d.ViewDimension == D3D_SRV_DIMENSION_TEXTURE3D {
            2
        } else {
            4
        };
        println!(
            "srv default {c:#010x} dim {} fields {:?}",
            d.ViewDimension.0,
            &raw[..defined]
        );
    };
    srv(&lines.cast()?);
    srv(&source.cast()?);
    let cubes = D3D11_TEXTURE2D_DESC {
        Width: 4,
        Height: 4,
        MipLevels: 1,
        ArraySize: 12,
        Format: rgba,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: D3D11_RESOURCE_MISC_TEXTURECUBE.0 as u32,
    };
    let mut cube = None;
    unsafe { device.CreateTexture2D(&cubes, None, Some(&mut cube))? };
    srv(&cube.unwrap().cast()?);

    // A 3D texture cannot hold depth; a 3D view of a 2D texture is invalid.
    let desc = D3D11_TEXTURE3D_DESC {
        Width: 4,
        Height: 4,
        Depth: 4,
        MipLevels: 1,
        Format: DXGI_FORMAT_D32_FLOAT,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_DEPTH_STENCIL.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut depth = None;
    let r = unsafe { device.CreateTexture3D(&desc, None, Some(&mut depth)) };
    println!("CreateTexture3D(depth) {:#010x}", code(r));
    let mut d = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: rgba,
        ViewDimension: D3D_SRV_DIMENSION_TEXTURE3D,
        ..Default::default()
    };
    d.Anonymous.Texture3D = D3D11_TEX3D_SRV {
        MostDetailedMip: 0,
        MipLevels: 1,
    };
    let flat = D3D11_TEXTURE2D_DESC {
        ArraySize: 1,
        MiscFlags: 0,
        ..cubes
    };
    let mut plane = None;
    unsafe { device.CreateTexture2D(&flat, None, Some(&mut plane))? };
    let mut view = None;
    let r = unsafe { device.CreateShaderResourceView(&plane.unwrap(), Some(&d), Some(&mut view)) };
    println!("3D view of a 2D texture {:#010x}", code(r));

    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
