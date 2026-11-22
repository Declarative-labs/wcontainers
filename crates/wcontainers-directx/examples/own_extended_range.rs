//! Swap chains of the extended-range format (`R10G10B10_XR_BIAS_A2_UNORM`) beside one of
//! `R10G10B10A2_UNORM` holding the same codes: each made on devices of 11_0 and 10_0 and refused
//! in the flip model, its buffer's views of the formats of its group, then its buffer filled with
//! columns of known codes, presented to a window shown on the program's desktop, and each column's
//! colour read back from the window as the compositor holds it. Prints each answer, for comparing with real Direct3D; real Direct3D
//! composes nothing on a desktop not shown, so the colours are for comparing with the
//! specification's decoding.

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use windows::Win32::Foundation::{HMODULE, HWND};
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D11::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetPixel,
        ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::System::Threading::Sleep;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP, WS_VISIBLE,
    };
    use windows::core::{Interface, w};

    /// The window and its buffers: one column per code, each `COLUMN` pixels wide.
    const COLUMN: u32 = 16;
    const HEIGHT: u32 = 16;
    /// Codes of a 10-bit channel: the extended-range format's 0 is 384 and its 1 is 894; 1023 is
    /// its largest, 1.2529.
    const CODES: [u32; 8] = [0, 200, 384, 500, 639, 800, 894, 1023];
    const WIDTH: u32 = COLUMN * CODES.len() as u32;
    /// The bit offsets of green, blue and alpha in a texel of the 10-bit formats.
    const GREEN: u32 = 10;
    const BLUE: u32 = 20;
    const ALPHA: u32 = 30;
    /// Blue's code in every column (639, one half in the extended-range format) and alpha's.
    const BLUE_CODE: u32 = CODES[4];
    const OPAQUE: u32 = 3;
    /// `PW_RENDERFULLCONTENT`: the window's content as the compositor shows it.
    const RENDER_FULL_CONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(2);
    /// How long the compositor is given to take a presented frame.
    const SHOWN_WITHIN_MS: u32 = 300;

    let code = |r: windows::core::Result<()>| r.err().map_or(0, |e| e.code().0) as u32;
    let device_of = |level: D3D_FEATURE_LEVEL| -> windows::core::Result<_> {
        let (mut device, mut context) = (None, None);
        unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[level]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?
        };
        let (device, context): (ID3D11Device, ID3D11DeviceContext) =
            (device.unwrap(), context.unwrap());
        Ok((device, context))
    };
    let window: HWND = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("own_extended_range"),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            WIDTH as i32,
            HEIGHT as i32,
            None,
            None,
            None,
            None,
        )?
    };
    let chain_desc = |format| DXGI_SWAP_CHAIN_DESC {
        BufferDesc: DXGI_MODE_DESC {
            Width: WIDTH,
            Height: HEIGHT,
            Format: format,
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

    // Made at 10_0 and refused in the flip model.
    {
        let (device, _context) = device_of(D3D_FEATURE_LEVEL_10_0)?;
        let factory: IDXGIFactory =
            unsafe { device.cast::<IDXGIDevice>()?.GetAdapter()?.GetParent()? };
        let mut chain = None;
        let r = unsafe {
            factory.CreateSwapChain(
                &device,
                &chain_desc(DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM),
                &mut chain,
            )
        };
        println!("10_0 swap chain {:#010x}", r.0 as u32);
        drop(chain);
    }
    let (device, context) = device_of(D3D_FEATURE_LEVEL_11_0)?;
    let factory: IDXGIFactory = unsafe { device.cast::<IDXGIDevice>()?.GetAdapter()?.GetParent()? };
    let flip = DXGI_SWAP_CHAIN_DESC {
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        ..chain_desc(DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM)
    };
    let mut chain = None;
    let r = unsafe { factory.CreateSwapChain(&device, &flip, &mut chain) };
    println!("flip model swap chain {:#010x}", r.0 as u32);
    drop(chain);

    // Columns of codes: red rising, green falling, blue in the middle.
    let texels: Vec<u32> = (0..HEIGHT)
        .flat_map(|_| {
            CODES.iter().enumerate().flat_map(|(i, &c)| {
                let texel =
                    c | CODES[CODES.len() - 1 - i] << GREEN | BLUE_CODE << BLUE | OPAQUE << ALPHA;
                std::iter::repeat_n(texel, COLUMN as usize)
            })
        })
        .collect();
    for format in [
        DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM,
    ] {
        let mut chain = None;
        let r = unsafe { factory.CreateSwapChain(&device, &chain_desc(format), &mut chain) };
        println!("swap chain of {} {:#010x}", format.0, r.0 as u32);
        let Some(chain) = chain else { continue };
        let buffer: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { buffer.GetDesc(&mut desc) };
        println!("buffer format {} bind {:#x}", desc.Format.0, desc.BindFlags);
        for view in [
            DXGI_FORMAT_R10G10B10A2_UNORM,
            DXGI_FORMAT_R10G10B10A2_UINT,
            DXGI_FORMAT_R10G10B10_XR_BIAS_A2_UNORM,
        ] {
            let rtv = D3D11_RENDER_TARGET_VIEW_DESC {
                Format: view,
                ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                ..Default::default()
            };
            let r = unsafe { device.CreateRenderTargetView(&buffer, Some(&rtv), None) };
            println!("target view as {}: {:#010x}", view.0, code(r));
        }
        unsafe {
            context.UpdateSubresource(
                &buffer,
                0,
                None,
                texels.as_ptr().cast(),
                WIDTH * size_of::<u32>() as u32,
                0,
            )
        };
        let r = unsafe { chain.Present(0, DXGI_PRESENT(0)) };
        println!("Present {:#010x}", r.0 as u32);
        unsafe { Sleep(SHOWN_WITHIN_MS) };
        // The window as the compositor holds it.
        unsafe {
            let screen = GetDC(Some(window));
            let memory = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, WIDTH as i32, HEIGHT as i32);
            let old = SelectObject(memory, bitmap.into());
            let printed = PrintWindow(window, memory, RENDER_FULL_CONTENT).as_bool();
            println!("printed {printed}");
            for (i, &red) in CODES.iter().enumerate() {
                let (x, y) = ((i as u32 * COLUMN + COLUMN / 2) as i32, (HEIGHT / 2) as i32);
                let c = GetPixel(memory, x, y).0;
                println!(
                    "column of codes {red} {} {} shown {} {} {}",
                    CODES[CODES.len() - 1 - i],
                    BLUE_CODE,
                    c & 0xff,
                    c >> 8 & 0xff,
                    c >> 16 & 0xff
                );
            }
            SelectObject(memory, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memory);
            ReleaseDC(Some(window), screen);
        }
    }
    unsafe { DestroyWindow(window)? };
    let r = unsafe { device.GetDeviceRemovedReason() };
    println!("GetDeviceRemovedReason {:#010x}", code(r));
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
