//! A small Direct3D 11 program on deferred contexts, context state objects and resolves: command
//! lists that clear, map a dynamic buffer and copy, run with the state kept and cleared; what a
//! deferred context refuses; state objects swapped in and out; a multisampled clear resolved. Prints
//! the answers and the bytes read back, for comparing with real Direct3D line by line.

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
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?
    };
    let (device, c): (ID3D11Device, ID3D11DeviceContext) = (device.unwrap(), context.unwrap());
    let device1: ID3D11Device1 = device.cast()?;
    let c1: ID3D11DeviceContext1 = c.cast()?;

    let texture = |samples: u32, usage: D3D11_USAGE, bind: D3D11_BIND_FLAG, cpu: u32| {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: 4,
            Height: 4,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: samples,
                Quality: 0,
            },
            Usage: usage,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: cpu,
            MiscFlags: 0,
        };
        let mut t = None;
        let r = unsafe { device.CreateTexture2D(&desc, None, Some(&mut t)) };
        (code(r), t)
    };
    let buffer = |usage: D3D11_USAGE, bind: D3D11_BIND_FLAG, cpu: u32| {
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: usage,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: cpu,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut b = None;
        unsafe { device.CreateBuffer(&desc, None, Some(&mut b)) }.map(|()| b.unwrap())
    };
    let read = D3D11_CPU_ACCESS_READ.0 as u32;
    let write = D3D11_CPU_ACCESS_WRITE.0 as u32;
    let target = texture(1, D3D11_USAGE_DEFAULT, D3D11_BIND_RENDER_TARGET, 0)
        .1
        .unwrap();
    let staging = texture(1, D3D11_USAGE_STAGING, D3D11_BIND_FLAG(0), read)
        .1
        .unwrap();
    let dynamic = buffer(D3D11_USAGE_DYNAMIC, D3D11_BIND_VERTEX_BUFFER, write)?;
    let plain = buffer(D3D11_USAGE_DEFAULT, D3D11_BIND_VERTEX_BUFFER, 0)?;
    let back = buffer(D3D11_USAGE_STAGING, D3D11_BIND_FLAG(0), read)?;
    let mut rtv = None;
    unsafe { device.CreateRenderTargetView(&target, None, Some(&mut rtv)) }?;
    let rtv = rtv.unwrap();

    // The first texel of `target` and the bytes of `plain`, read back on the immediate context.
    let texel = || -> windows::core::Result<u32> {
        unsafe {
            c.CopyResource(&staging, &target);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            c.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let v = *m.pData.cast::<u32>();
            c.Unmap(&staging, 0);
            Ok(v)
        }
    };
    let bytes = || -> windows::core::Result<Vec<u8>> {
        unsafe {
            c.CopyResource(&back, &plain);
            let mut m = D3D11_MAPPED_SUBRESOURCE::default();
            c.Map(&back, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
            let v = std::slice::from_raw_parts(m.pData.cast::<u8>(), 16).to_vec();
            c.Unmap(&back, 0);
            Ok(v)
        }
    };
    let bound = |ctx: &ID3D11DeviceContext| {
        let mut targets = [None];
        unsafe { ctx.OMGetRenderTargets(Some(&mut targets), None) };
        targets[0]
            .as_ref()
            .map_or(0, |t| u32::from(t.as_raw() == rtv.as_raw()) + 1)
    };

    let mut deferred = None;
    let r = unsafe { device.CreateDeferredContext(0, Some(&mut deferred)) };
    println!("CreateDeferredContext {:#010x}", code(r));
    let d = deferred.unwrap();
    println!("GetType {}", unsafe { d.GetType() }.0);
    let mut flagged = None;
    let r = unsafe { device.CreateDeferredContext(1, Some(&mut flagged)) };
    println!("CreateDeferredContext(flags 1) {:#010x}", code(r));

    // What a deferred context refuses.
    let mut m = D3D11_MAPPED_SUBRESOURCE::default();
    let r = unsafe { d.Map(&dynamic, 0, D3D11_MAP_WRITE_NO_OVERWRITE, 0, Some(&mut m)) };
    println!("Map(no overwrite, first) {:#010x}", code(r));
    let r = unsafe { d.Map(&plain, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut m)) };
    println!("Map(default usage) {:#010x}", code(r));
    let r = unsafe { d.Map(&back, 0, D3D11_MAP_READ, 0, Some(&mut m)) };
    println!("Map(read) {:#010x}", code(r));
    let mut event = None;
    let q = D3D11_QUERY_DESC {
        Query: D3D11_QUERY_EVENT,
        MiscFlags: 0,
    };
    unsafe { device.CreateQuery(&q, Some(&mut event)) }?;
    let event = event.unwrap();
    unsafe { d.End(&event) };
    let r = unsafe { d.GetData(&event, None, 0, 0) };
    println!("GetData(deferred) {:#010x}", code(r));
    let mut list = None;
    let r = unsafe { c.FinishCommandList(false, Some(&mut list)) };
    println!("FinishCommandList(immediate) {:#010x}", code(r));

    // A list that clears, writes the dynamic buffer twice and copies it, and binds a target.
    unsafe {
        d.ClearRenderTargetView(&rtv, &[0.0, 1.0, 0.0, 1.0]);
        d.Map(&dynamic, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut m))?;
        for i in 0..16u8 {
            *m.pData.cast::<u8>().add(i as usize) = i + 1;
        }
        d.Unmap(&dynamic, 0);
        d.Map(&dynamic, 0, D3D11_MAP_WRITE_NO_OVERWRITE, 0, Some(&mut m))?;
        *m.pData.cast::<u8>().add(15) = 0xee;
        d.Unmap(&dynamic, 0);
        d.CopyResource(&plain, &dynamic);
        d.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
    }
    let mut list = None;
    let r = unsafe { d.FinishCommandList(false, Some(&mut list)) };
    println!("FinishCommandList {:#010x}", code(r));
    let list = list.unwrap();
    println!("GetContextFlags {}", unsafe { list.GetContextFlags() });
    println!("deferred target after finish {}", bound(&d));
    println!("texel before {:#010x}", texel()?);
    unsafe { c.OMSetRenderTargets(None, None) };
    unsafe { c.ExecuteCommandList(&list, true) };
    println!("immediate target after kept {}", bound(&c));
    println!("texel after {:#010x}", texel()?);
    println!("bytes after {:02x?}", bytes()?);
    unsafe { c.ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 0.0]) };
    unsafe { c.ExecuteCommandList(&list, false) };
    println!("immediate target after cleared {}", bound(&c));
    println!("texel again {:#010x}", texel()?);

    // A list finished keeping the deferred context's state.
    unsafe { d.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None) };
    let mut kept = None;
    let r = unsafe { d.FinishCommandList(true, Some(&mut kept)) };
    println!("FinishCommandList(keep) {:#010x}", code(r));
    println!("deferred target after keep {}", bound(&d));
    unsafe { d.ClearRenderTargetView(&rtv, &[0.0, 0.0, 1.0, 1.0]) };
    let mut blue = None;
    unsafe { d.FinishCommandList(false, Some(&mut blue)) }?;
    unsafe { c.ExecuteCommandList(blue.as_ref(), false) };
    println!("texel blue {:#010x}", texel()?);

    // Context state objects.
    let mut chosen = D3D_FEATURE_LEVEL(0);
    let mut state = None;
    let r = unsafe {
        device1.CreateDeviceContextState(
            0,
            &[D3D_FEATURE_LEVEL_11_0],
            D3D11_SDK_VERSION,
            &ID3D11Device1::IID,
            Some(&mut chosen),
            Some(&mut state),
        )
    };
    println!("CreateDeviceContextState {:#010x} {:#x}", code(r), chosen.0);
    let r = unsafe {
        device1.CreateDeviceContextState(0, &[], D3D11_SDK_VERSION, &ID3D11Device1::IID, None, None)
    };
    println!("CreateDeviceContextState(no levels) {:#010x}", code(r));
    let state = state.unwrap();
    unsafe { c.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None) };
    let mut previous = None;
    unsafe { c1.SwapDeviceContextState(&state, Some(&mut previous)) };
    println!("first swap hands back a state {}", previous.is_some());
    println!("target in the new state {}", bound(&c));
    let mut again = None;
    unsafe { c1.SwapDeviceContextState(previous.as_ref(), Some(&mut again)) };
    println!(
        "swap back hands back the new one {}",
        again.as_ref().map(Interface::as_raw) == Some(state.as_raw())
    );
    println!("target in the first state {}", bound(&c));
    let d1: ID3D11DeviceContext1 = d.cast()?;
    let mut none = None;
    unsafe { d1.SwapDeviceContextState(&state, Some(&mut none)) };
    println!("deferred swap hands back a state {}", none.is_some());

    // A multisampled clear resolved.
    let levels = unsafe { device.CheckMultisampleQualityLevels(DXGI_FORMAT_R8G8B8A8_UNORM, 4) }?;
    println!("CheckMultisampleQualityLevels(4) {levels}");
    let (r, many) = texture(4, D3D11_USAGE_DEFAULT, D3D11_BIND_RENDER_TARGET, 0);
    println!("CreateTexture2D(4 samples) {r:#010x}");
    let (r, _) = texture(4, D3D11_USAGE_DEFAULT, D3D11_BIND_SHADER_RESOURCE, 0);
    println!("CreateTexture2D(4 samples, not drawn into) {r:#010x}");
    if let Some(many) = many {
        let mut view = None;
        unsafe { device.CreateRenderTargetView(&many, None, Some(&mut view)) }?;
        let view = view.unwrap();
        let mut desc = D3D11_RENDER_TARGET_VIEW_DESC::default();
        unsafe { view.GetDesc(&mut desc) };
        println!("view dimension {}", desc.ViewDimension.0);
        unsafe {
            c.ClearRenderTargetView(&view, &[1.0, 0.0, 0.0, 1.0]);
            c.ResolveSubresource(&target, 0, &many, 0, DXGI_FORMAT_R8G8B8A8_UNORM);
        }
        println!("texel resolved {:#010x}", texel()?);
    }
    let reason = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(reason));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
