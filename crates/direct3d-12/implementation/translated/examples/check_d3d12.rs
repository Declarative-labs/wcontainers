//! A small Direct3D 12 program of this track's own, for checking the front against real Direct3D 12:
//! an indexed quad from two vertex streams copied from an upload buffer into a default buffer, one
//! triangle of it culled, a whole clear and a rectangle clear (beside the scissor, which clears
//! ignore), a scissor, and a read back of a box; and every answer of `CheckFeatureSupport` the front
//! gives. Run it once beside our d3d12.dll, dxgi.dll and wcontainers_d3d12.dll and once without,
//! then compare:
//!   check_d3d12 draw <out.rgba>
//!   check_d3d12 compare <a.rgba> <b.rgba> <tolerance>
//!   check_d3d12 features <out.txt>
//!   check_d3d12 compare-features <real.txt> <ours.txt> [all]
//!   check_d3d12 outputs <out.txt>   (output 0's `GetDesc1` and swap chains' colour spaces, for
//!   compare-features)
//!   check_d3d12 fence-wait [rounds]   (time from a queue's signal to its fence event, waited for)
//!   check_d3d12 answers <out.txt>   (results of calls at the edges of the API, for compare-features)
//!   check_d3d12 cases <out.txt>   (the pass bar's cases, each in a process of its own; check/cases.rs)
//!   check_d3d12 compare-cases <real.txt> <ours.txt>
//!   check_d3d12 compare-case-pictures <real dir> <ours dir> [<case>]   (the pictures cases write)
//! It draws with the rig's triangle shaders.

#[cfg(windows)]
#[path = "check/cases.rs"]
mod cases;

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("draw") => {
            let pixels = draw()?;
            std::fs::write(&args[2], pixels).expect("writing the picture");
        }
        Some("compare") => {
            let a = std::fs::read(&args[2]).expect("the first picture");
            let b = std::fs::read(&args[3]).expect("the second picture");
            let tolerance: u8 = args[4].parse().expect("a tolerance");
            std::process::exit(i32::from(!compare_pictures(&a, &b, tolerance)));
        }
        Some("compare-case-pictures") => {
            let mut same = true;
            let named = args.get(4);
            for (name, tolerance) in cases::PICTURES
                .iter()
                .filter(|(name, _)| named.is_none_or(|n| n == name))
            {
                let file = format!("{name}.{}", cases::PICTURE_EXTENSION);
                let read = |dir: &String| std::fs::read(std::path::Path::new(dir).join(&file));
                print!("{name}: ");
                match (read(&args[2]), read(&args[3])) {
                    (Ok(a), Ok(b)) => same &= compare_pictures(&a, &b, *tolerance),
                    _ => {
                        println!("a picture is missing");
                        same = false;
                    }
                }
            }
            std::process::exit(i32::from(!same));
        }
        Some("features") => {
            std::fs::write(&args[2], features()?).expect("writing the answers");
        }
        Some("fence-wait") => {
            let rounds: u32 = args.get(2).map_or(1000, |n| n.parse().expect("a count"));
            fence_wait(rounds)?;
        }
        Some("outputs") => {
            std::fs::write(&args[2], outputs()?).expect("writing the answers");
        }
        Some("case") => match cases::run(&args[2]) {
            Ok(line) => println!("{line}"),
            Err(e) => println!("error {e}"),
        },
        Some("cases") => {
            let mut out = String::new();
            for name in cases::CASES {
                let run =
                    std::process::Command::new(std::env::current_exe().expect("this program"))
                        .args(["case", name])
                        .output()
                        .expect("running a case");
                for l in String::from_utf8_lossy(&run.stderr).lines() {
                    eprintln!("{name}: {l}");
                }
                let line = String::from_utf8_lossy(&run.stdout);
                let line = match line.lines().last() {
                    Some(l) if run.status.success() => l.to_string(),
                    _ => format!("failed: {}", run.status),
                };
                out.push_str(&format!("{name}: {line}\n"));
            }
            std::fs::write(&args[2], out).expect("writing the results");
        }
        Some("compare-cases") => {
            let real = std::fs::read_to_string(&args[2]).expect("real Direct3D 12's results");
            let ours = std::fs::read_to_string(&args[3]).expect("our results");
            let mut differ = 0;
            for (r, o) in real.lines().zip(ours.lines()) {
                if r == o {
                    println!("same     {r}");
                } else {
                    differ += 1;
                    println!("DIFFERS  real {r}\n         ours {o}");
                }
            }
            println!("{differ} of {} cases differ", real.lines().count());
            std::process::exit(i32::from(differ > 0));
        }
        Some("answers") => {
            std::fs::write(&args[2], answers()?).expect("writing the answers");
        }
        Some("compare-features") => {
            let real = std::fs::read_to_string(&args[2]).expect("real Direct3D 12's answers");
            let ours = std::fs::read_to_string(&args[3]).expect("our answers");
            let verbose = args.get(4).is_some_and(|a| a == "all");
            let listed = args
                .get(5)
                .map(|f| std::fs::read_to_string(f).expect("the listed differences"));
            let good = compare_features(&real, &ours, verbose, listed.as_deref());
            std::process::exit(i32::from(!good));
        }
        _ => eprintln!(
            "usage: check_d3d12 draw <out.rgba> | compare <a> <b> <tolerance> | features <out.txt> \
             | compare-features <real.txt> <ours.txt> [all|some] [listed.txt]"
        ),
    }
    Ok(())
}

/// Prints how many pixels of pictures `a` and `b` differ beyond `tolerance` in a channel, and where,
/// and returns whether none does.
#[cfg(windows)]
fn compare_pictures(a: &[u8], b: &[u8], tolerance: u8) -> bool {
    assert_eq!(a.len(), b.len(), "pictures of different sizes");
    let largest = a
        .iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0);
    let beyond = a
        .chunks(4)
        .zip(b.chunks(4))
        .filter(|(x, y)| {
            x.iter()
                .zip(y.iter())
                .any(|(p, q)| p.abs_diff(*q) > tolerance)
        })
        .count();
    println!(
        "{beyond} of {} pixels beyond {tolerance}, largest difference {largest}",
        a.len() / 4
    );
    if beyond > 0 {
        // Where they differ, one character per 4 by 4 block; then the first differing pixel.
        let side = ((a.len() / 4) as f64).sqrt() as usize;
        for by in 0..side / 4 {
            let line: String = (0..side / 4)
                .map(|bx| {
                    let differs = (0..16).any(|i| {
                        let p = ((by * 4 + i / 4) * side + bx * 4 + i % 4) * 4;
                        (0..4).any(|c| a[p + c].abs_diff(b[p + c]) > tolerance)
                    });
                    if differs { '#' } else { '.' }
                })
                .collect();
            println!("{line}");
        }
        let p = (0..a.len() / 4)
            .find(|p| (0..4).any(|c| a[p * 4 + c].abs_diff(b[p * 4 + c]) > tolerance))
            .unwrap();
        println!(
            "pixel {} {}: {:?} and {:?}",
            p % side,
            p / side,
            &a[p * 4..p * 4 + 4],
            &b[p * 4..p * 4 + 4]
        );
    }
    beyond == 0
}

/// Bits of `bits` answers above real Direct3D 12's on the RTX 3080 that the front serves:
/// `R32G32B32_FLOAT`'s `SHADER_SAMPLE` and `SHADER_GATHER`, read from `rgba32float` textures.
#[cfg(windows)]
const SERVED_BEYOND_REAL: &[(&str, u64)] = &[(
    "format_6.support1",
    (support1::SHADER_SAMPLE | support1::SHADER_GATHER) as u64,
)];
#[cfg(windows)]
use wcontainers_directx_graphics_infrastructure::support1;

/// Prints each answer of ours beyond real Direct3D 12's (every differing one with `all`) and returns whether none of ours promises more than real
/// Direct3D 12's on the same GPU: per line, `le` values may be lower, `ge` higher (or zero, not
/// reported), `bits` a subset (with `SERVED_BEYOND_REAL`'s bits),
/// and an answer real Direct3D 12 refuses must be refused for the same reason (an ability ours
/// lacks is refused as not done only where real answers). With `listed` (lines "key real ours"),
/// every differing answer must be listed and every listed one must still differ so.
#[cfg(windows)]
fn compare_features(real: &str, ours: &str, verbose: bool, listed: Option<&str>) -> bool {
    let parse = |text: &str| -> std::collections::BTreeMap<String, (String, String)> {
        text.lines()
            .filter_map(|l| {
                let mut w = l.split_whitespace();
                Some((
                    w.next()?.to_owned(),
                    (w.next()?.to_owned(), w.next()?.to_owned()),
                ))
            })
            .collect()
    };
    let (real, ours) = (parse(real), parse(ours));
    let number = |v: &str| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok();
    let mut good = true;
    let mut differ = 0;
    for (key, (r, rule)) in &real {
        let Some((o, _)) = ours.get(key) else {
            println!("{key}: real {r}, ours missing");
            good = false;
            continue;
        };
        if r == o {
            continue;
        }
        let within = match (number(r), number(o)) {
            // Refused for a missing ability only where real answers.
            _ if rule == "eq" => r == "ok" && o == "notimpl",
            (Some(r), Some(o)) => match rule.as_str() {
                "le" => o <= r,
                // A bound: ours may be wider, or zero where the ability is absent.
                "ge" => o >= r || o == 0,
                "bits" => {
                    let served = SERVED_BEYOND_REAL
                        .iter()
                        .find(|(k, _)| k == key)
                        .map_or(0, |(_, bits)| *bits);
                    o & !(r | served) == 0
                }
                _ => rule == "info",
            },
            _ if rule == "info" => true,
            // Ours answered what real refused.
            _ if o == "ok" => false,
            // Ours refused what real answered.
            (Some(_), None) => true,
            _ if o == "notimpl" => true,
            _ => false,
        };
        if !within || verbose {
            println!(
                "{key}: real {r}, ours {o}{}",
                if within { "" } else { "  BEYOND REAL" }
            );
        }
        differ += 1;
        good &= within;
        let line = format!("{key} {r} {o}");
        if listed.is_some_and(|l| !l.lines().any(|x| x == line)) {
            println!("{line}  NOT LISTED");
            good = false;
        }
    }
    for line in listed
        .into_iter()
        .flat_map(str::lines)
        .filter(|l| !l.starts_with('#'))
    {
        let mut w = line.split_whitespace();
        let (key, r, o) = (w.next(), w.next(), w.next());
        let holds = key.is_some_and(|k| {
            real.get(k).map(|v| v.0.as_str()) == r && ours.get(k).map(|v| v.0.as_str()) == o
        });
        if !holds {
            println!("{line}  LISTED, NO LONGER SO");
            good = false;
        }
    }
    println!(
        "{differ} of {} answers differ, {}",
        real.len(),
        if good {
            "none beyond real Direct3D 12"
        } else {
            "some beyond real Direct3D 12"
        }
    );
    good
}

/// Times rounds of: an empty queue signals a fence, the program asks for an event at that value
/// and waits for it. Prints the mean and largest wait.
#[cfg(windows)]
fn fence_wait(rounds: u32) -> windows::core::Result<()> {
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device)? };
    let device = device.unwrap();
    let queue: ID3D12CommandQueue =
        unsafe { device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())? };
    let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
    let event = unsafe { CreateEventW(None, false, false, None)? };
    let mut times = Vec::with_capacity(rounds as usize);
    for i in 1..=u64::from(rounds) {
        let start = std::time::Instant::now();
        unsafe {
            queue.Signal(&fence, i)?;
            fence.SetEventOnCompletion(i, event)?;
            WaitForSingleObject(event, INFINITE);
        }
        times.push(start.elapsed().as_micros());
    }
    let first = times[0];
    let mean = times.iter().sum::<u128>() / times.len() as u128;
    times.sort_unstable();
    let p99 = times[times.len() * 99 / 100];
    let largest = times[times.len() - 1];
    println!(
        "{rounds} rounds: first {first} us, mean {mean} us, 99th percentile {p99} us, largest {largest} us"
    );
    Ok(())
}

/// Output 0's description (`info`: listed, never beyond) and what swap chains of four back buffer
/// formats answer `CheckColorSpaceSupport` for each colour space (`bits`).
#[cfg(windows)]
fn outputs() -> windows::core::Result<String> {
    use std::fmt::Write;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };
    use windows::core::{Interface, w};
    let mut out = String::new();
    let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))? };
    let adapter = unsafe { factory.EnumAdapters1(0)? };
    match unsafe { adapter.EnumOutputs(0) }.and_then(|o| o.cast::<IDXGIOutput6>()) {
        Ok(output) => describe(&output, &mut out)?,
        Err(e) => writeln!(out, "output err_{:x} info", e.code().0).unwrap(),
    }
    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)? };
    let device = device.unwrap();
    let queue: ID3D12CommandQueue =
        unsafe { device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())? };
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("outputs"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            100,
            100,
            None,
            None,
            None,
            None,
        )?
    };
    for format in [
        DXGI_FORMAT_R8G8B8A8_UNORM,
        DXGI_FORMAT_B8G8R8A8_UNORM,
        DXGI_FORMAT_R10G10B10A2_UNORM,
        DXGI_FORMAT_R16G16B16A16_FLOAT,
    ] {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: 64,
            Height: 64,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            Scaling: DXGI_SCALING_STRETCH,
            ..Default::default()
        };
        let chain = unsafe { factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None) }
            .and_then(|c| c.cast::<IDXGISwapChain3>());
        let chain = match chain {
            Ok(c) => c,
            Err(e) => {
                writeln!(out, "swap_chain_{} err_{:x} info", format.0, e.code().0).unwrap();
                continue;
            }
        };
        for space in 0..=25 {
            let flags =
                unsafe { chain.CheckColorSpaceSupport(DXGI_COLOR_SPACE_TYPE(space)) }.unwrap_or(0);
            writeln!(out, "color_space_{}_{space} {flags:#x} bits", format.0).unwrap();
        }
    }
    unsafe { DestroyWindow(window)? };
    Ok(out)
}

/// Results of calls at the edges of the API, a line each: `ok` for success, `notimpl` for a call
/// this front refuses as not done (accepted by compare-features as a refusal), else the HRESULT.
/// Swap chains with each alpha mode, presents of a 16-bit float swap chain, a fence waited for with
/// no event until another thread signals it, and committed resources with each heap flag.
#[cfg(windows)]
fn answers() -> windows::core::Result<String> {
    use std::fmt::Write;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };
    use windows::core::{HRESULT, w};
    let mut out = String::new();
    let mut line = |key: &str, r: HRESULT| {
        let v = match r.0 {
            0.. => "ok".to_owned(),
            -0x7fff_bfff => "notimpl".to_owned(),
            c => format!("{:#x}", c as u32),
        };
        writeln!(out, "{key} {v} eq").unwrap();
    };
    let code = |r: windows::core::Result<()>| r.map_or_else(|e| e.code(), |()| HRESULT(0));
    let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))? };
    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device)? };
    let device = device.unwrap();
    let queue: ID3D12CommandQueue =
        unsafe { device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC::default())? };
    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("answers"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            100,
            100,
            None,
            None,
            None,
            None,
        )?
    };
    let chain_desc = |format, alpha| DXGI_SWAP_CHAIN_DESC1 {
        Width: 64,
        Height: 64,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        Scaling: DXGI_SCALING_STRETCH,
        AlphaMode: alpha,
        ..Default::default()
    };
    for (name, alpha) in [
        ("unspecified", DXGI_ALPHA_MODE_UNSPECIFIED),
        ("premultiplied", DXGI_ALPHA_MODE_PREMULTIPLIED),
        ("straight", DXGI_ALPHA_MODE_STRAIGHT),
        ("ignore", DXGI_ALPHA_MODE_IGNORE),
    ] {
        let desc = chain_desc(DXGI_FORMAT_R8G8B8A8_UNORM, alpha);
        let r = unsafe { factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None) };
        line(&format!("alpha_{name}"), code(r.map(drop)));
    }
    // Each scaling, on a window whose client area differs from the buffers; the HDR metadata calls.
    for (name, scaling) in [
        ("none", DXGI_SCALING_NONE),
        ("aspect", DXGI_SCALING_ASPECT_RATIO_STRETCH),
        ("unknown", DXGI_SCALING(7)),
    ] {
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Scaling: scaling,
            ..chain_desc(DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_ALPHA_MODE_UNSPECIFIED)
        };
        match unsafe { factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None) } {
            Ok(chain) => {
                line(&format!("scaling_{name}"), HRESULT(0));
                line(&format!("scaling_{name}_present"), unsafe {
                    chain.Present(0, DXGI_PRESENT(0))
                });
            }
            Err(e) => line(&format!("scaling_{name}"), e.code()),
        }
    }
    let desc = chain_desc(DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_ALPHA_MODE_UNSPECIFIED);
    if let Ok(chain) = unsafe { factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None) }
        .and_then(|c| windows::core::Interface::cast::<IDXGISwapChain4>(&c))
    {
        let hdr10 = DXGI_HDR_METADATA_HDR10::default();
        let size = std::mem::size_of_val(&hdr10) as u32;
        let p: *const std::ffi::c_void = std::ptr::from_ref(&hdr10).cast();
        for (name, kind, size, data) in [
            ("none", DXGI_HDR_METADATA_TYPE_NONE, 0, None),
            ("hdr10", DXGI_HDR_METADATA_TYPE_HDR10, size, Some(p)),
            (
                "hdr10_short",
                DXGI_HDR_METADATA_TYPE_HDR10,
                size - 2,
                Some(p),
            ),
            ("hdr10_null", DXGI_HDR_METADATA_TYPE_HDR10, size, None),
            ("unknown", DXGI_HDR_METADATA_TYPE(9), 0, None),
        ] {
            line(
                &format!("hdr_metadata_{name}"),
                // The raw call, to pass sizes the wrapper's slice cannot.
                unsafe {
                    (windows::core::Interface::vtable(&chain).SetHDRMetaData)(
                        windows::core::Interface::as_raw(&chain),
                        kind,
                        size,
                        data.unwrap_or(std::ptr::null()),
                    )
                },
            );
        }
    }
    let desc = chain_desc(DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_ALPHA_MODE_UNSPECIFIED);
    match unsafe { factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None) } {
        Ok(chain) => {
            line("float_chain", HRESULT(0));
            for i in 0..3 {
                line(&format!("float_chain_present_{i}"), unsafe {
                    chain.Present(0, DXGI_PRESENT(0))
                });
            }
        }
        Err(e) => line("float_chain", e.code()),
    }
    unsafe { DestroyWindow(window)? };

    // A fence waited for with no event; another thread signals it 200 ms later.
    let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
    let signaller = {
        let fence = fence.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            unsafe { fence.Signal(7) }.unwrap();
        })
    };
    let start = std::time::Instant::now();
    line(
        "null_event_wait",
        code(unsafe { fence.SetEventOnCompletion(7, HANDLE::default()) }),
    );
    let waited = start.elapsed().as_millis();
    signaller.join().unwrap();
    // Ok when the call returned after the signal; 0x1 when it returned early.
    line("null_event_waited", HRESULT(i32::from(waited < 150)));
    line(
        "null_event_value",
        HRESULT((unsafe { fence.GetCompletedValue() } != 7) as i32),
    );

    for bit in 0..14 {
        let flags = D3D12_HEAP_FLAGS(1 << bit);
        let mut r: Option<ID3D12Resource> = None;
        let result = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                flags,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: 65536,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut r,
            )
        };
        line(&format!("heap_flag_{:#x}", 1 << bit), code(result));
    }
    // `ALLOW_SHADER_ATOMICS` on textures, plain and for unordered access, and on a buffer for
    // unordered access.
    for (name, dimension, flags) in [
        (
            "texture",
            D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            D3D12_RESOURCE_FLAG_NONE,
        ),
        (
            "uav_texture",
            D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        ),
        (
            "uav_buffer",
            D3D12_RESOURCE_DIMENSION_BUFFER,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        ),
    ] {
        let buffer = dimension == D3D12_RESOURCE_DIMENSION_BUFFER;
        let mut r: Option<ID3D12Resource> = None;
        let result = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_ALLOW_SHADER_ATOMICS,
                &D3D12_RESOURCE_DESC {
                    Dimension: dimension,
                    Width: if buffer { 65536 } else { 64 },
                    Height: if buffer { 1 } else { 64 },
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: if buffer {
                        DXGI_FORMAT_UNKNOWN
                    } else {
                        DXGI_FORMAT_R32_UINT
                    },
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: if buffer {
                        D3D12_TEXTURE_LAYOUT_ROW_MAJOR
                    } else {
                        D3D12_TEXTURE_LAYOUT_UNKNOWN
                    },
                    Flags: flags,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut r,
            )
        };
        line(&format!("heap_flag_atomics_{name}"), code(result));
    }
    // Buffers in custom heaps the CPU sees, plain and for unordered access.
    for (name, page, flags) in [
        (
            "write_back",
            D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
            D3D12_RESOURCE_FLAG_NONE,
        ),
        (
            "write_combine",
            D3D12_CPU_PAGE_PROPERTY_WRITE_COMBINE,
            D3D12_RESOURCE_FLAG_NONE,
        ),
        (
            "write_back_uav",
            D3D12_CPU_PAGE_PROPERTY_WRITE_BACK,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
        ),
    ] {
        let mut r: Option<ID3D12Resource> = None;
        let result = unsafe {
            device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_CUSTOM,
                    CPUPageProperty: page,
                    MemoryPoolPreference: D3D12_MEMORY_POOL_L0,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: 65536,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    Flags: flags,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut r,
            )
        };
        line(&format!("custom_buffer_{name}"), code(result));
    }
    Ok(out)
}

/// Output `output`'s `GetDesc1`, a line per value.
#[cfg(windows)]
fn describe(
    output: &windows::Win32::Graphics::Dxgi::IDXGIOutput6,
    out: &mut String,
) -> windows::core::Result<()> {
    use std::fmt::Write;
    let d = unsafe { output.GetDesc1()? };
    let name = String::from_utf16_lossy(
        &d.DeviceName[..d.DeviceName.iter().position(|&c| c == 0).unwrap_or(32)],
    );
    let r = d.DesktopCoordinates;
    for (key, value) in [
        ("output_name", name),
        (
            "output_desktop",
            format!("{},{},{},{}", r.left, r.top, r.right, r.bottom),
        ),
        ("output_bits_per_color", d.BitsPerColor.to_string()),
        ("output_color_space", d.ColorSpace.0.to_string()),
        (
            "output_primaries",
            format!(
                "{:?},{:?},{:?},{:?}",
                d.RedPrimary, d.GreenPrimary, d.BluePrimary, d.WhitePoint
            )
            .replace(' ', ""),
        ),
        (
            "output_luminance",
            format!(
                "{},{},{}",
                d.MinLuminance, d.MaxLuminance, d.MaxFullFrameLuminance
            ),
        ),
    ] {
        writeln!(out, "{key} {value} info").unwrap();
    }
    Ok(())
}

/// What `CheckFeatureSupport` answers, a line per value: name, value in hexadecimal (or the
/// failing HRESULT as `err_` and its value), and how ours may differ (see `compare_features`).
#[cfg(windows)]
fn features() -> windows::core::Result<String> {
    use std::ffi::c_void;
    use std::fmt::Write;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;

    let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))? };
    let adapter: IDXGIAdapter1 = unsafe { factory.EnumAdapters1(0)? };
    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)? };
    let device = device.unwrap();
    let mut out = String::new();
    let mut line = |key: &str, value: Result<u64, windows::core::Error>, rule: &str| {
        match value {
            Ok(v) => writeln!(out, "{key} {v:#x} {rule}"),
            Err(e) => writeln!(out, "{key} err_{:08x} {rule}", e.code().0 as u32),
        }
        .unwrap();
    };
    fn ask<T>(
        device: &ID3D12Device,
        feature: D3D12_FEATURE,
        data: &mut T,
    ) -> windows::core::Result<()> {
        unsafe {
            device.CheckFeatureSupport(
                feature,
                (data as *mut T).cast::<c_void>(),
                size_of::<T>() as u32,
            )
        }
    }
    let flag = |b: windows::core::BOOL| u64::from(b.as_bool());

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "options.double_precision",
        v(flag(o.DoublePrecisionFloatShaderOps)),
        "le",
    );
    line("options.logic_op", v(flag(o.OutputMergerLogicOp)), "le");
    line(
        "options.min_precision",
        v(o.MinPrecisionSupport.0 as u64),
        "bits",
    );
    line(
        "options.tiled_resources_tier",
        v(o.TiledResourcesTier.0 as u64),
        "le",
    );
    line(
        "options.resource_binding_tier",
        v(o.ResourceBindingTier.0 as u64),
        "le",
    );
    line(
        "options.ps_stencil_ref",
        v(flag(o.PSSpecifiedStencilRefSupported)),
        "le",
    );
    line(
        "options.typed_uav_load_additional",
        v(flag(o.TypedUAVLoadAdditionalFormats)),
        "le",
    );
    line("options.rovs", v(flag(o.ROVsSupported)), "le");
    line(
        "options.conservative_raster_tier",
        v(o.ConservativeRasterizationTier.0 as u64),
        "le",
    );
    line(
        "options.va_bits_per_resource",
        v(o.MaxGPUVirtualAddressBitsPerResource as u64),
        "le",
    );
    line(
        "options.standard_swizzle",
        v(flag(o.StandardSwizzle64KBSupported)),
        "le",
    );
    line(
        "options.cross_node_sharing_tier",
        v(o.CrossNodeSharingTier.0 as u64),
        "le",
    );
    line(
        "options.cross_adapter_row_major",
        v(flag(o.CrossAdapterRowMajorTextureSupported)),
        "le",
    );
    line(
        "options.vp_rt_index_any_shader",
        v(flag(
            o.VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation,
        )),
        "le",
    );
    line(
        "options.resource_heap_tier",
        v(o.ResourceHeapTier.0 as u64),
        "le",
    );

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS1::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS1, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line("options1.wave_ops", v(flag(o.WaveOps)), "le");
    line(
        "options1.wave_lane_count_min",
        v(o.WaveLaneCountMin as u64),
        "le",
    );
    line(
        "options1.wave_lane_count_max",
        v(o.WaveLaneCountMax as u64),
        "ge",
    );
    line(
        "options1.total_lane_count",
        v(o.TotalLaneCount as u64),
        "le",
    );
    line(
        "options1.expanded_compute_states",
        v(flag(o.ExpandedComputeResourceStates)),
        "le",
    );
    line("options1.int64_shader_ops", v(flag(o.Int64ShaderOps)), "le");

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS2::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS2, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "options2.depth_bounds",
        v(flag(o.DepthBoundsTestSupported)),
        "le",
    );
    line(
        "options2.sample_positions_tier",
        v(o.ProgrammableSamplePositionsTier.0 as u64),
        "le",
    );

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS3::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS3, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "options3.copy_queue_timestamps",
        v(flag(o.CopyQueueTimestampQueriesSupported)),
        "le",
    );
    line(
        "options3.casting_fully_typed",
        v(flag(o.CastingFullyTypedFormatSupported)),
        "le",
    );
    line(
        "options3.write_buffer_immediate",
        v(o.WriteBufferImmediateSupportFlags.0 as u64),
        "bits",
    );
    line(
        "options3.view_instancing_tier",
        v(o.ViewInstancingTier.0 as u64),
        "le",
    );
    line(
        "options3.barycentrics",
        v(flag(o.BarycentricsSupported)),
        "le",
    );

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS4::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS4, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "options4.msaa_64kb_aligned",
        v(flag(o.MSAA64KBAlignedTextureSupported)),
        "le",
    );
    line(
        "options4.shared_resource_tier",
        v(o.SharedResourceCompatibilityTier.0 as u64),
        "le",
    );
    line(
        "options4.native_16bit_ops",
        v(flag(o.Native16BitShaderOpsSupported)),
        "le",
    );

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS5::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS5, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "options5.srv_only_tiled_tier3",
        v(flag(o.SRVOnlyTiledResourceTier3)),
        "le",
    );
    line(
        "options5.render_passes_tier",
        v(o.RenderPassesTier.0 as u64),
        "le",
    );
    line(
        "options5.raytracing_tier",
        v(o.RaytracingTier.0 as u64),
        "le",
    );

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS6::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS6, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "options6.additional_shading_rates",
        v(flag(o.AdditionalShadingRatesSupported)),
        "le",
    );
    line(
        "options6.per_primitive_rate_viewport_index",
        v(flag(o.PerPrimitiveShadingRateSupportedWithViewportIndexing)),
        "le",
    );
    line(
        "options6.vrs_tier",
        v(o.VariableShadingRateTier.0 as u64),
        "le",
    );
    line(
        "options6.shading_rate_tile",
        v(o.ShadingRateImageTileSize as u64),
        "le",
    );
    line(
        "options6.background_processing",
        v(flag(o.BackgroundProcessingSupported)),
        "le",
    );

    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS7::default();
    let r = ask(&device, D3D12_FEATURE_D3D12_OPTIONS7, &mut o);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "options7.mesh_shader_tier",
        v(o.MeshShaderTier.0 as u64),
        "le",
    );
    line(
        "options7.sampler_feedback_tier",
        v(o.SamplerFeedbackTier.0 as u64),
        "le",
    );

    let mut a = D3D12_FEATURE_DATA_ARCHITECTURE1::default();
    let r = ask(&device, D3D12_FEATURE_ARCHITECTURE1, &mut a);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "architecture1.tile_based",
        v(flag(a.TileBasedRenderer)),
        "eq",
    );
    line("architecture1.uma", v(flag(a.UMA)), "eq");
    line(
        "architecture1.cache_coherent_uma",
        v(flag(a.CacheCoherentUMA)),
        "eq",
    );
    line("architecture1.isolated_mmu", v(flag(a.IsolatedMMU)), "le");
    let mut a = D3D12_FEATURE_DATA_ARCHITECTURE::default();
    let r = ask(&device, D3D12_FEATURE_ARCHITECTURE, &mut a);
    line("architecture.uma", r.map(|()| flag(a.UMA)), "eq");
    let mut a = D3D12_FEATURE_DATA_ARCHITECTURE {
        NodeIndex: 1,
        ..Default::default()
    };
    line(
        "architecture.node1",
        ask(&device, D3D12_FEATURE_ARCHITECTURE, &mut a).map(|()| 0),
        "eq",
    );

    let levels = [
        D3D_FEATURE_LEVEL_9_1,
        D3D_FEATURE_LEVEL_10_0,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_12_0,
        D3D_FEATURE_LEVEL_12_1,
        D3D_FEATURE_LEVEL_12_2,
    ];
    let mut l = D3D12_FEATURE_DATA_FEATURE_LEVELS {
        NumFeatureLevels: levels.len() as u32,
        pFeatureLevelsRequested: levels.as_ptr(),
        ..Default::default()
    };
    let r = ask(&device, D3D12_FEATURE_FEATURE_LEVELS, &mut l);
    line(
        "feature_levels.max",
        r.map(|()| l.MaxSupportedFeatureLevel.0 as u64),
        "le",
    );
    let mut l = D3D12_FEATURE_DATA_FEATURE_LEVELS {
        NumFeatureLevels: 2,
        pFeatureLevelsRequested: levels.as_ptr(),
        ..Default::default()
    };
    let r = ask(&device, D3D12_FEATURE_FEATURE_LEVELS, &mut l);
    line(
        "feature_levels.max_of_9_1_10_0",
        r.map(|()| l.MaxSupportedFeatureLevel.0 as u64),
        "le",
    );
    let mut l = D3D12_FEATURE_DATA_FEATURE_LEVELS {
        NumFeatureLevels: 1,
        pFeatureLevelsRequested: &D3D_FEATURE_LEVEL_12_2,
        ..Default::default()
    };
    let r = ask(&device, D3D12_FEATURE_FEATURE_LEVELS, &mut l);
    line(
        "feature_levels.only_12_2",
        r.map(|()| l.MaxSupportedFeatureLevel.0 as u64),
        "le",
    );

    for asked in [0x51, 0x60, 0x62, 0x65, 0x66] {
        let mut s = D3D12_FEATURE_DATA_SHADER_MODEL {
            HighestShaderModel: D3D_SHADER_MODEL(asked),
        };
        let r = ask(&device, D3D12_FEATURE_SHADER_MODEL, &mut s);
        line(
            &format!("shader_model.asked_{asked:x}"),
            r.map(|()| s.HighestShaderModel.0 as u64),
            "le",
        );
    }
    for asked in [1, 2] {
        let mut s = D3D12_FEATURE_DATA_ROOT_SIGNATURE {
            HighestVersion: D3D_ROOT_SIGNATURE_VERSION(asked),
        };
        let r = ask(&device, D3D12_FEATURE_ROOT_SIGNATURE, &mut s);
        line(
            &format!("root_signature.asked_{asked}"),
            r.map(|()| s.HighestVersion.0 as u64),
            "le",
        );
    }

    let mut g = D3D12_FEATURE_DATA_GPU_VIRTUAL_ADDRESS_SUPPORT::default();
    let r = ask(&device, D3D12_FEATURE_GPU_VIRTUAL_ADDRESS_SUPPORT, &mut g);
    let v = |f: u64| r.clone().map(|()| f);
    line(
        "va.bits_per_resource",
        v(g.MaxGPUVirtualAddressBitsPerResource as u64),
        "le",
    );
    line(
        "va.bits_per_process",
        v(g.MaxGPUVirtualAddressBitsPerProcess as u64),
        "le",
    );

    // Every DXGI format up to B4G4R4A4_UNORM, and a size the API refuses.
    for format in 0..=115 {
        let format = DXGI_FORMAT(format);
        let mut s = D3D12_FEATURE_DATA_FORMAT_SUPPORT {
            Format: format,
            ..Default::default()
        };
        let r = ask(&device, D3D12_FEATURE_FORMAT_SUPPORT, &mut s);
        let v = |f: u64| r.clone().map(|()| f);
        line(
            &format!("format_{}.support1", format.0),
            v(s.Support1.0 as u32 as u64),
            "bits",
        );
        line(
            &format!("format_{}.support2", format.0),
            v(s.Support2.0 as u32 as u64),
            "bits",
        );
        let mut i = D3D12_FEATURE_DATA_FORMAT_INFO {
            Format: format,
            ..Default::default()
        };
        let r = ask(&device, D3D12_FEATURE_FORMAT_INFO, &mut i);
        line(
            &format!("format_{}.planes", format.0),
            r.map(|()| i.PlaneCount as u64),
            "eq",
        );
        for samples in [1, 2, 4, 8] {
            let mut q = D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS {
                Format: format,
                SampleCount: samples,
                ..Default::default()
            };
            let r = ask(&device, D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS, &mut q);
            line(
                &format!("format_{}.quality_levels_{samples}", format.0),
                r.map(|()| u64::from(q.NumQualityLevels)),
                "le",
            );
        }
    }
    let mut o = D3D12_FEATURE_DATA_D3D12_OPTIONS::default();
    let r = unsafe {
        device.CheckFeatureSupport(
            D3D12_FEATURE_D3D12_OPTIONS,
            (&mut o as *mut D3D12_FEATURE_DATA_D3D12_OPTIONS).cast(),
            4,
        )
    };
    line("options.short_size", r.map(|()| 0), "eq");
    Ok(out)
}

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn draw() -> windows::core::Result<Vec<u8>> {
    use std::mem::{ManuallyDrop, transmute_copy};
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;
    use windows::Win32::Graphics::Dxgi::*;
    use windows::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};
    use windows::core::{Interface, s};

    const VS: &[u8] =
        include_bytes!("../../../../graphics-test/shaders/dxil/triangle.vs.dxil");
    const PS: &[u8] =
        include_bytes!("../../../../graphics-test/shaders/dxil/triangle.ps.dxil");
    const SIZE: u32 = 64;
    let buffer = |size: u64| D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        ..Default::default()
    };
    unsafe {
        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
        let adapter: IDXGIAdapter1 = factory.EnumAdapters1(0)?;
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
        let device = device.unwrap();
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let allocator: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList =
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;

        let mut blob = None;
        D3D12SerializeVersionedRootSignature(
            &D3D12_VERSIONED_ROOT_SIGNATURE_DESC {
                Version: D3D_ROOT_SIGNATURE_VERSION_1_0,
                Anonymous: D3D12_VERSIONED_ROOT_SIGNATURE_DESC_0 {
                    Desc_1_0: D3D12_ROOT_SIGNATURE_DESC {
                        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
                        ..Default::default()
                    },
                },
            },
            &mut blob,
            None,
        )?;
        let blob = blob.unwrap();
        let root: ID3D12RootSignature = device.CreateRootSignature(
            0,
            std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize()),
        )?;

        // Positions in stream 0, colours in stream 1.
        let layout = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("POSITION"),
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default()
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("COLOR"),
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlot: 1,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                ..Default::default()
            },
        ];
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: transmute_copy(&root),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: VS.as_ptr().cast(),
                BytecodeLength: VS.len(),
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: PS.as_ptr().cast(),
                BytecodeLength: PS.len(),
            },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_BACK,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: layout.as_ptr(),
                NumElements: 2,
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        let pso: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc)?;

        let committed =
            |heap, desc: &D3D12_RESOURCE_DESC, state| -> windows::core::Result<ID3D12Resource> {
                let mut r = None;
                device.CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: heap,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    desc,
                    state,
                    None,
                    &mut r,
                )?;
                Ok(r.unwrap())
            };
        let target = committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: SIZE.into(),
                Height: SIZE,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )?;
        let heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let rtv = heap.GetCPUDescriptorHandleForHeapStart();
        device.CreateRenderTargetView(&target, None, rtv);

        // A quad: positions at 0, colours at 256, 16-bit indices at 512; counter-clockwise ones are
        // back faces and culled.
        let mut bytes = vec![0u8; 1024];
        let positions: [f32; 8] = [-0.9, 0.9, 0.9, 0.9, 0.9, -0.9, -0.9, -0.9];
        let colors: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0];
        let indices: [u16; 9] = [0, 1, 2, 0, 2, 3, 0, 3, 2];
        bytes[..32].copy_from_slice(&positions.map(f32::to_le_bytes).concat());
        bytes[256..304].copy_from_slice(&colors.map(f32::to_le_bytes).concat());
        bytes[512..530].copy_from_slice(&indices.map(u16::to_le_bytes).concat());
        let upload = committed(
            D3D12_HEAP_TYPE_UPLOAD,
            &buffer(1024),
            D3D12_RESOURCE_STATE_GENERIC_READ,
        )?;
        let mut mapped = std::ptr::null_mut();
        upload.Map(0, Some(&D3D12_RANGE::default()), Some(&mut mapped))?;
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.cast(), bytes.len());
        upload.Unmap(0, None);
        let vertices = committed(
            D3D12_HEAP_TYPE_DEFAULT,
            &buffer(1024),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        let transition = |r: &ID3D12Resource, before, after| D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: transmute_copy(r),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
        };
        list.CopyBufferRegion(&vertices, 0, &upload, 0, 1024);
        list.ResourceBarrier(&[transition(
            &vertices,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER | D3D12_RESOURCE_STATE_INDEX_BUFFER,
        )]);

        list.ClearRenderTargetView(rtv, &[0.1, 0.1, 0.15, 1.0], None);
        list.ClearRenderTargetView(
            rtv,
            &[0.0, 0.5, 0.5, 1.0],
            Some(&[RECT {
                left: 50,
                top: 4,
                right: 62,
                bottom: 30,
            }]),
        );
        list.SetPipelineState(&pso);
        list.SetGraphicsRootSignature(&root);
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.RSSetViewports(&[D3D12_VIEWPORT {
            Width: SIZE as f32,
            Height: SIZE as f32,
            MaxDepth: 1.0,
            ..Default::default()
        }]);
        list.RSSetScissorRects(&[RECT {
            left: 0,
            top: 0,
            right: 48,
            bottom: SIZE as i32,
        }]);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        let base = vertices.GetGPUVirtualAddress();
        list.IASetVertexBuffers(
            0,
            Some(&[
                D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: base,
                    SizeInBytes: 32,
                    StrideInBytes: 8,
                },
                D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: base + 256,
                    SizeInBytes: 48,
                    StrideInBytes: 12,
                },
            ]),
        );
        list.IASetIndexBuffer(Some(&D3D12_INDEX_BUFFER_VIEW {
            BufferLocation: base + 512,
            SizeInBytes: 18,
            Format: DXGI_FORMAT_R16_UINT,
        }));
        list.DrawIndexedInstanced(9, 1, 0, 0, 0);

        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0;
        device.GetCopyableFootprints(
            &target.GetDesc(),
            0,
            1,
            0,
            Some(&mut footprint),
            None,
            None,
            Some(&mut total),
        );
        let readback = committed(
            D3D12_HEAP_TYPE_READBACK,
            &buffer(total),
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        list.ResourceBarrier(&[transition(
            &target,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        list.CopyTextureRegion(
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: transmute_copy(&readback),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: footprint,
                },
            },
            0,
            0,
            0,
            &D3D12_TEXTURE_COPY_LOCATION {
                pResource: transmute_copy(&target),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            },
            Some(&D3D12_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: SIZE,
                bottom: SIZE,
                back: 1,
            }),
        );
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast()?)]);
        queue.Signal(&fence, 1)?;
        let event = CreateEventW(None, false, false, None)?;
        fence.SetEventOnCompletion(1, event)?;
        WaitForSingleObject(event, INFINITE);
        let mut mapped = std::ptr::null_mut();
        readback.Map(
            0,
            Some(&D3D12_RANGE {
                Begin: 0,
                End: total as usize,
            }),
            Some(&mut mapped),
        )?;
        let rows = std::slice::from_raw_parts(mapped.cast::<u8>(), total as usize);
        let pitch = footprint.Footprint.RowPitch as usize;
        let pixels = (0..SIZE as usize)
            .flat_map(|y| rows[y * pitch..y * pitch + SIZE as usize * 4].to_vec())
            .collect();
        readback.Unmap(0, None);
        Ok(pixels)
    }
}
