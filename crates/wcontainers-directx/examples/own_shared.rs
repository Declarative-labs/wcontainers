//! A small Direct3D 11 program sharing resources between two devices: a texture shared through a
//! KMT handle (opened on its own device and on another, written on each and read on the other
//! after a flush), one with a keyed mutex (acquired and released between the devices by key), a
//! buffer with a keyed mutex, and a texture shared through an NT handle. Prints each answer and
//! the texels each device reads, for comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::{CloseHandle, GENERIC_ALL, HANDLE, HMODULE};
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::{
        DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIAdapter, IDXGIKeyedMutex,
        IDXGIResource, IDXGIResource1,
    };
    use windows::core::{Interface, PCWSTR};

    const SIDE: u32 = 4;
    const TEXEL: u32 = 4;
    /// The bits DXGI's KMT handles carry (Wine's `is_kmt_handle`).
    const KMT_BITS: usize = 0xc000_0000;

    let code = |r: windows::core::Result<()>| r.err().map_or(0, |e| e.code().0) as u32;
    let device = || -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
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
        Ok((device.unwrap(), context.unwrap()))
    };
    let (a, ca) = device()?;
    let (b, cb) = device()?;

    let mut support = D3D11_FEATURE_DATA_FORMAT_SUPPORT2 {
        InFormat: DXGI_FORMAT_R8G8B8A8_UNORM,
        OutFormatSupport2: 0,
    };
    let r = unsafe {
        a.CheckFeatureSupport(
            D3D11_FEATURE_FORMAT_SUPPORT2,
            (&mut support as *mut D3D11_FEATURE_DATA_FORMAT_SUPPORT2).cast(),
            size_of::<D3D11_FEATURE_DATA_FORMAT_SUPPORT2>() as u32,
        )
    };
    println!(
        "FORMAT_SUPPORT2 {:#010x} shareable {}",
        code(r),
        support.OutFormatSupport2 & D3D11_FORMAT_SUPPORT2_SHAREABLE.0 as u32 != 0
    );

    let pattern = |seed: u8| -> Vec<u8> {
        (0..SIDE * SIDE * TEXEL)
            .map(|i| (i as u8).wrapping_mul(7).wrapping_add(seed))
            .collect()
    };
    let texture = |d: &ID3D11Device, misc: D3D11_RESOURCE_MISC_FLAG, init: &[u8]| {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: SIDE,
            Height: SIDE,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            MiscFlags: misc.0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: init.as_ptr().cast(),
            SysMemPitch: SIDE * TEXEL,
            SysMemSlicePitch: 0,
        };
        let mut t = None;
        let r = unsafe { d.CreateTexture2D(&desc, Some(&data), Some(&mut t)) };
        println!("CreateTexture2D({:#x}) {:#010x}", misc.0, code(r));
        t.unwrap()
    };
    // Waits for the device's work, as programs sharing resources do before the other device reads.
    let finish = |d: &ID3D11Device, c: &ID3D11DeviceContext| -> windows::core::Result<()> {
        let mut q = None;
        unsafe {
            d.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut q),
            )?
        };
        let q = q.unwrap();
        unsafe {
            c.End(&q);
            c.Flush();
        }
        let mut done = 0u32;
        while unsafe { c.GetData(&q, Some((&mut done as *mut u32).cast()), 4, 0) }.is_err()
            || done == 0
        {}
        Ok(())
    };
    let show = |what: &str,
                d: &ID3D11Device,
                c: &ID3D11DeviceContext,
                t: &ID3D11Texture2D|
     -> windows::core::Result<()> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { t.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.MiscFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let mut s = None;
        unsafe { d.CreateTexture2D(&desc, None, Some(&mut s))? };
        let s = s.unwrap();
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            c.CopyResource(&s, t);
            c.Map(&s, 0, D3D11_MAP_READ, 0, Some(&mut m))?;
        }
        let mut hex = String::new();
        for y in 0..SIDE {
            let row = unsafe {
                std::slice::from_raw_parts(
                    m.pData.cast::<u8>().add((y * m.RowPitch) as usize),
                    (SIDE * TEXEL) as usize,
                )
            };
            for byte in row {
                hex.push_str(&format!("{byte:02x}"));
            }
            hex.push(' ');
        }
        unsafe { c.Unmap(&s, 0) };
        println!("{what}: {}", hex.trim_end());
        Ok(())
    };
    let open = |d: &ID3D11Device, h: HANDLE| {
        let r: windows::core::Result<ID3D11Texture2D> = unsafe {
            let mut out = None;
            d.OpenSharedResource(h, &mut out).map(|_| out.unwrap())
        };
        println!(
            "OpenSharedResource {:#010x}",
            code(r.as_ref().map(|_| ()).map_err(|e| e.clone()))
        );
        r
    };
    let clear = |c: &ID3D11DeviceContext, d: &ID3D11Device, t: &ID3D11Texture2D, rgba: [f32; 4]| {
        let mut view = None;
        unsafe {
            d.CreateRenderTargetView(t, None, Some(&mut view))?;
            c.ClearRenderTargetView(&view.unwrap(), &rgba);
        }
        windows::core::Result::Ok(())
    };

    // Shared through a KMT handle.
    let t = texture(&a, D3D11_RESOURCE_MISC_SHARED, &pattern(1));
    finish(&a, &ca)?;
    let resource: IDXGIResource = t.cast()?;
    let h = unsafe { resource.GetSharedHandle() };
    let h = h.unwrap_or_default();
    println!("GetSharedHandle kmt {}", h.0 as usize & KMT_BITS != 0);
    let same = open(&a, h)?;
    println!("same object {}", same.as_raw() == t.as_raw());
    show("opened on its device", &a, &ca, &same)?;
    let other = open(&b, h)?;
    show("opened on the other device", &b, &cb, &other)?;
    clear(&ca, &a, &t, [1.0, 0.5, 0.0, 1.0])?;
    finish(&a, &ca)?;
    show("the other device after a clear and flush", &b, &cb, &other)?;
    let next = pattern(9);
    unsafe { cb.UpdateSubresource(&other, 0, None, next.as_ptr().cast(), SIDE * TEXEL, 0) };
    finish(&b, &cb)?;
    show("its device after the other's update and flush", &a, &ca, &t)?;

    // Shared with a keyed mutex. Their answers are read whole: `WAIT_TIMEOUT` is a success code.
    let acquire = |m: &IDXGIKeyedMutex, key: u64| unsafe {
        (Interface::vtable(m).AcquireSync)(m.as_raw(), key, 0).0 as u32
    };
    let release = |m: &IDXGIKeyedMutex, key: u64| unsafe {
        (Interface::vtable(m).ReleaseSync)(m.as_raw(), key).0 as u32
    };
    let k = texture(&a, D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX, &pattern(2));
    let mutex_a: IDXGIKeyedMutex = k.cast()?;
    println!("AcquireSync a 0 {:#010x}", acquire(&mutex_a, 0));
    clear(&ca, &a, &k, [0.0, 0.25, 1.0, 0.5])?;
    println!("ReleaseSync a 1 {:#010x}", release(&mutex_a, 1));
    let kh = unsafe { k.cast::<IDXGIResource>()?.GetSharedHandle() }.unwrap_or_default();
    let kb = open(&b, kh)?;
    let mutex_b: IDXGIKeyedMutex = kb.cast()?;
    println!("AcquireSync b 0 {:#010x}", acquire(&mutex_b, 0));
    println!("AcquireSync b 1 {:#010x}", acquire(&mutex_b, 1));
    show("the other device holding the mutex", &b, &cb, &kb)?;
    clear(&cb, &b, &kb, [0.75, 0.0, 0.0, 1.0])?;
    println!("ReleaseSync b 2 {:#010x}", release(&mutex_b, 2));
    println!("AcquireSync a 2 {:#010x}", acquire(&mutex_a, 2));
    show("its device holding the mutex again", &a, &ca, &k)?;
    println!("ReleaseSync a 0 {:#010x}", release(&mutex_a, 0));

    // A buffer with a keyed mutex.
    let mut buffer = None;
    let r = unsafe {
        a.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: SIDE * TEXEL,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut buffer),
        )
    };
    println!("CreateBuffer(keyed mutex) {:#010x}", code(r));
    if let Some(buffer) = buffer {
        let mutex: windows::core::Result<IDXGIKeyedMutex> = buffer.cast();
        println!("buffer keyed mutex {:#010x}", code(mutex.map(|_| ())));
    }

    // Shared through an NT handle.
    let n = texture(
        &a,
        D3D11_RESOURCE_MISC_FLAG(
            D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0,
        ),
        &pattern(3),
    );
    finish(&a, &ca)?;
    let r1: IDXGIResource1 = n.cast()?;
    println!(
        "GetSharedHandle(nt) {:#010x}",
        code(unsafe { n.cast::<IDXGIResource>()?.GetSharedHandle() }.map(|_| ()))
    );
    let nh = unsafe {
        r1.CreateSharedHandle(
            None,
            GENERIC_ALL.0 | DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0,
            PCWSTR::null(),
        )
    };
    println!(
        "CreateSharedHandle {:#010x}",
        code(nh.as_ref().map(|_| ()).map_err(|e| e.clone()))
    );
    let nh = nh?;
    let b1: ID3D11Device1 = b.cast()?;
    let nb: windows::core::Result<ID3D11Texture2D> = unsafe { b1.OpenSharedResource1(nh) };
    println!(
        "OpenSharedResource1 {:#010x}",
        code(nb.as_ref().map(|_| ()).map_err(|e| e.clone()))
    );
    show("opened through the NT handle", &b, &cb, &nb?)?;
    println!("CloseHandle {:#010x}", code(unsafe { CloseHandle(nh) }));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
