//! A small Direct3D 11 program with a swap chain on a window it never shows: the factory reached
//! through the device's adapter, `CreateSwapChain`, the back buffer (the same object each time)
//! cleared and read back, `Present`, and `ResizeBuffers` refused while the buffer is held and served
//! after; then a flip model swap chain's frame latency waitable object, waited on before and after
//! a present. Prints each answer, for comparing with real Direct3D line by line.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::{CloseHandle, HMODULE, HWND};
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::System::Threading::WaitForSingleObject;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };
    use windows::core::{Interface, w};

    /// How long a presented frame may take to retire.
    const RETIRED_WITHIN_MS: u32 = 5000;
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
    let window: HWND = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("own_swap_chain"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            200,
            200,
            None,
            None,
            None,
            None,
        )?
    };
    let adapter = unsafe { device.cast::<IDXGIDevice>()?.GetAdapter()? };
    let factory: IDXGIFactory = unsafe { adapter.GetParent()? };
    let desc = DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: 64,
            Height: 32,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            ..Default::default()
        },
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 1,
        OutputWindow: window,
        Windowed: true.into(),
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
        Flags: 0,
    };
    let mut chain = None;
    let r = unsafe { factory.CreateSwapChain(&device, &desc, &mut chain) };
    println!("CreateSwapChain {:#010x}", r.0 as u32);
    let chain = chain.unwrap();
    let buffer: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
    let again: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
    println!("GetBuffer twice the same {}", buffer == again);
    drop(again);
    let mut size = D3D11_TEXTURE2D_DESC::default();
    unsafe { buffer.GetDesc(&mut size) };
    println!(
        "buffer {}x{} format {} bind {:#x}",
        size.Width, size.Height, size.Format.0, size.BindFlags
    );

    let staging_desc = D3D11_TEXTURE2D_DESC {
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
        ..size
    };
    let (mut rtv, mut staging) = (None, None);
    unsafe {
        device.CreateRenderTargetView(&buffer, None, Some(&mut rtv))?;
        device.CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
        context.ClearRenderTargetView(rtv.as_ref().unwrap(), &[1.0, 0.5, 0.0, 1.0]);
        context.CopyResource(staging.as_ref().unwrap(), &buffer);
    }
    let staging = staging.unwrap();
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))? };
    let texel = unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), 4) };
    println!("cleared texel {:02x?}", texel);
    unsafe { context.Unmap(&staging, 0) };

    let r = unsafe { chain.Present(0, DXGI_PRESENT(0)) };
    println!("Present {:#010x}", r.0 as u32);
    let r = unsafe { chain.ResizeBuffers(1, 32, 16, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0)) };
    println!("ResizeBuffers while held {:#010x}", code(r));
    drop(rtv);
    drop(buffer);
    unsafe { context.ClearState() };
    let r = unsafe { chain.ResizeBuffers(1, 32, 16, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0)) };
    println!("ResizeBuffers after release {:#010x}", code(r));
    let d = unsafe { chain.GetDesc()? };
    println!("now {}x{}", d.BufferDesc.Width, d.BufferDesc.Height);
    let buffer: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
    unsafe { buffer.GetDesc(&mut size) };
    println!("buffer {}x{}", size.Width, size.Height);
    drop(buffer);
    drop(chain);

    // A flip model swap chain with a frame latency waitable object: the object lets the program
    // begin as many frames as the latency, and each frame presented and retired one more.
    let factory2: IDXGIFactory2 = factory.cast()?;
    let waitable = DXGI_SWAP_CHAIN_DESC1 {
        Width: 64,
        Height: 32,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
        ..Default::default()
    };
    let blit = DXGI_SWAP_CHAIN_DESC1 {
        BufferCount: 1,
        SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
        ..waitable
    };
    let r = unsafe { factory2.CreateSwapChainForHwnd(&device, window, &blit, None, None) };
    println!("waitable blit model {:#010x}", code(r.map(drop)));
    let chain: IDXGISwapChain2 =
        unsafe { factory2.CreateSwapChainForHwnd(&device, window, &waitable, None, None)? }
            .cast()?;
    println!("waitable latency {}", unsafe {
        chain.GetMaximumFrameLatency()?
    });
    let object = unsafe { chain.GetFrameLatencyWaitableObject() };
    println!("waitable object null {}", object.is_invalid());
    let wait = |ms| unsafe { WaitForSingleObject(object, ms) }.0;
    println!("first frame wait {:#x}", wait(0));
    println!("second frame wait before a present {:#x}", wait(0));
    let r = unsafe { chain.Present(0, DXGI_PRESENT(0)) };
    println!("waitable Present {:#010x}", r.0 as u32);
    println!(
        "second frame wait after a present {:#x}",
        wait(RETIRED_WITHIN_MS)
    );
    let r = unsafe { chain.SetMaximumFrameLatency(2) };
    println!("SetMaximumFrameLatency 2 {:#010x}", code(r));
    println!("wait after the latency grew {:#x}", wait(0));
    println!("wait past the latency {:#x}", wait(0));
    unsafe { CloseHandle(object)? };

    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
