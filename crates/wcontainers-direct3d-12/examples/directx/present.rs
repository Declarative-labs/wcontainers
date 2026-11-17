//! DXGI's outputs and swap chains: the host's monitors as outputs, and swap chains on a Direct3D 12
//! queue over the infrastructure crate's, run on the device thread.

use std::ffi::c_void;
use std::sync::Mutex;

use wcontainers_direct3d_12::device::ChainId;
use wcontainers_directx_graphics_infrastructure::{
    COLOR_SPACE_SRGB, DisplayMode, Model, OutputDesc, SwapChainDesc, SwapChainError, presents_in,
    sdr,
};
use wcontainers_gpu::TextureUsage;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW,
    HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, Ref, Result, implement};

use crate::gpu;
use crate::objects::{Resource, give, hr};
use wcontainers_direct3d_12::Error;

/// `MONITORINFOF_PRIMARY`.
const PRIMARY: u32 = 1;

unsafe extern "system" fn collect(m: HMONITOR, _: HDC, _: *mut RECT, list: LPARAM) -> BOOL {
    // SAFETY: `outputs` passes its list.
    unsafe { (*(list.0 as *mut Vec<HMONITOR>)).push(m) };
    TRUE
}

/// The host's monitors, the primary first, with each one's handle, as Windows reports them.
pub fn outputs() -> Vec<(OutputDesc, isize)> {
    let mut monitors: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only pushes into the list, which outlives the call.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut monitors as *mut _ as isize),
        )
    };
    let mut out = Vec::new();
    for m in monitors {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: `info` is the extended structure its size says.
        if !unsafe { GetMonitorInfoW(m, &mut info as *mut _ as *mut MONITORINFO) }.as_bool() {
            continue;
        }
        let name_len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(32);
        let mut mode = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        // SAFETY: a device name Windows gave, ended by a zero, and a mode of the size it says.
        let hz = if unsafe {
            EnumDisplaySettingsW(
                PCWSTR(info.szDevice.as_ptr()),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
            )
        }
        .as_bool()
        {
            mode.dmDisplayFrequency
        } else {
            0
        };
        let r = info.monitorInfo.rcMonitor;
        let primary = info.monitorInfo.dwFlags & PRIMARY != 0;
        out.push((
            !primary,
            OutputDesc {
                name: String::from_utf16_lossy(&info.szDevice[..name_len]),
                desktop: [r.left, r.top, r.right, r.bottom],
                refresh_numerator: hz,
                refresh_denominator: 1,
            },
            m.0 as isize,
        ));
    }
    out.sort_by_key(|o| o.0);
    out.into_iter().map(|(_, d, m)| (d, m)).collect()
}

#[implement(IDXGIOutput6)]
pub struct Output {
    pub desc: OutputDesc,
    pub monitor: isize,
}

fn mode_desc(m: &DisplayMode) -> DXGI_MODE_DESC {
    DXGI_MODE_DESC {
        Width: m.width,
        Height: m.height,
        RefreshRate: DXGI_RATIONAL {
            Numerator: m.refresh_numerator,
            Denominator: m.refresh_denominator,
        },
        Format: DXGI_FORMAT(m.format as i32),
        ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_PROGRESSIVE,
        Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
    }
}

fn mode1(m: &DXGI_MODE_DESC) -> DXGI_MODE_DESC1 {
    DXGI_MODE_DESC1 {
        Width: m.Width,
        Height: m.Height,
        RefreshRate: m.RefreshRate,
        Format: m.Format,
        ScanlineOrdering: m.ScanlineOrdering,
        Scaling: m.Scaling,
        Stereo: FALSE,
    }
}

/// Fills a caller's mode list as DXGI does: the count alone without a list, `MORE_DATA` when the
/// list is too short.
///
/// # Safety
/// `count` is writable; `list`, when not null, holds `*count` entries.
unsafe fn fill<T>(modes: Vec<T>, count: *mut u32, list: *mut T) -> Result<()> {
    let count = unsafe { count.as_mut() }.ok_or(DXGI_ERROR_INVALID_CALL)?;
    if list.is_null() {
        *count = modes.len() as u32;
        return Ok(());
    }
    let modes_len = modes.len();
    let n = (*count as usize).min(modes_len);
    for (i, m) in modes.into_iter().take(n).enumerate() {
        unsafe { list.add(i).write(m) };
    }
    let all = modes_len;
    *count = n as u32;
    if n < all {
        return Err(DXGI_ERROR_MORE_DATA.into());
    }
    Ok(())
}

impl Output {
    fn name(&self) -> [u16; 32] {
        let mut name = [0u16; 32];
        for (d, c) in name.iter_mut().zip(self.desc.name.encode_utf16().take(31)) {
            *d = c;
        }
        name
    }

    fn rect(&self) -> RECT {
        let [left, top, right, bottom] = self.desc.desktop;
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn GetDesc(&self) -> Result<DXGI_OUTPUT_DESC> {
        Ok(DXGI_OUTPUT_DESC {
            DeviceName: self.name(),
            DesktopCoordinates: self.rect(),
            AttachedToDesktop: TRUE,
            Rotation: DXGI_MODE_ROTATION_IDENTITY,
            Monitor: HMONITOR(self.monitor as *mut c_void),
        })
    }

    /// An SDR output in sRGB, as plan 4.9 says.
    pub fn GetDesc1(&self) -> Result<DXGI_OUTPUT_DESC1> {
        let [red, green, blue, white] = sdr::PRIMARIES;
        Ok(DXGI_OUTPUT_DESC1 {
            DeviceName: self.name(),
            DesktopCoordinates: self.rect(),
            AttachedToDesktop: TRUE,
            Rotation: DXGI_MODE_ROTATION_IDENTITY,
            Monitor: HMONITOR(self.monitor as *mut c_void),
            BitsPerColor: sdr::BITS_PER_COLOR,
            ColorSpace: DXGI_COLOR_SPACE_TYPE(COLOR_SPACE_SRGB as i32),
            RedPrimary: red,
            GreenPrimary: green,
            BluePrimary: blue,
            WhitePoint: white,
            MinLuminance: sdr::MIN_LUMINANCE,
            MaxLuminance: sdr::MAX_LUMINANCE,
            MaxFullFrameLuminance: sdr::MAX_FULL_FRAME_LUMINANCE,
        })
    }

    pub fn GetDisplayModeList(
        &self,
        format: DXGI_FORMAT,
        _flags: DXGI_ENUM_MODES,
        count: *mut u32,
        list: *mut DXGI_MODE_DESC,
    ) -> Result<()> {
        let modes = self
            .desc
            .modes(format.0 as u32)
            .iter()
            .map(mode_desc)
            .collect();
        // SAFETY: as the API requires of the caller.
        unsafe { fill(modes, count, list) }
    }

    pub fn GetDisplayModeList1(
        &self,
        format: DXGI_FORMAT,
        _flags: DXGI_ENUM_MODES,
        count: *mut u32,
        list: *mut DXGI_MODE_DESC1,
    ) -> Result<()> {
        let modes = self
            .desc
            .modes(format.0 as u32)
            .iter()
            .map(|m| mode1(&mode_desc(m)))
            .collect();
        // SAFETY: as the API requires of the caller.
        unsafe { fill(modes, count, list) }
    }

    pub fn FindClosestMatchingMode(
        &self,
        wanted: *const DXGI_MODE_DESC,
        closest: *mut DXGI_MODE_DESC,
        _device: Ref<IUnknown>,
    ) -> Result<()> {
        // SAFETY: the API requires both.
        let (wanted, closest) = unsafe {
            (
                wanted.as_ref().ok_or(DXGI_ERROR_INVALID_CALL)?,
                closest.as_mut().ok_or(DXGI_ERROR_INVALID_CALL)?,
            )
        };
        *closest = mode_desc(&self.desc.closest_mode(&DisplayMode {
            width: wanted.Width,
            height: wanted.Height,
            refresh_numerator: wanted.RefreshRate.Numerator,
            refresh_denominator: wanted.RefreshRate.Denominator,
            format: wanted.Format.0 as u32,
        }));
        Ok(())
    }

    pub fn FindClosestMatchingMode1(
        &self,
        wanted: *const DXGI_MODE_DESC1,
        closest: *mut DXGI_MODE_DESC1,
        device: Ref<IUnknown>,
    ) -> Result<()> {
        // SAFETY: the API requires both.
        let (wanted, closest) = unsafe {
            (
                wanted.as_ref().ok_or(DXGI_ERROR_INVALID_CALL)?,
                closest.as_mut().ok_or(DXGI_ERROR_INVALID_CALL)?,
            )
        };
        let w = DXGI_MODE_DESC {
            Width: wanted.Width,
            Height: wanted.Height,
            RefreshRate: wanted.RefreshRate,
            Format: wanted.Format,
            ScanlineOrdering: wanted.ScanlineOrdering,
            Scaling: wanted.Scaling,
        };
        let mut c = DXGI_MODE_DESC::default();
        self.FindClosestMatchingMode(&w, &mut c, device)?;
        *closest = mode1(&c);
        Ok(())
    }

    /// No overlays: the host composes every picture.
    pub fn SupportsOverlays(&self) -> BOOL {
        FALSE
    }

    /// No hardware composition: the host composes every picture.
    pub fn CheckHardwareCompositionSupport(&self) -> Result<u32> {
        Ok(0)
    }
}

/// The front's error for a swap chain call, as DXGI reports it.
fn chain_error(e: SwapChainError) -> windows::core::Error {
    match e {
        SwapChainError::InvalidCall(s) => windows::core::Error::new(DXGI_ERROR_INVALID_CALL, s),
        SwapChainError::Gpu(e) => hr(Error::from(e)),
    }
}

/// A window's client size.
fn client_size(hwnd: isize) -> (u32, u32) {
    let mut r = RECT::default();
    // SAFETY: the program's window.
    let _ = unsafe { GetClientRect(HWND(hwnd as *mut c_void), &mut r) };
    (
        (r.right - r.left).max(0) as u32,
        (r.bottom - r.top).max(0) as u32,
    )
}

/// `DXGI_USAGE_SHADER_INPUT` and `DXGI_USAGE_UNORDERED_ACCESS`.
const USAGE_SHADER_INPUT: u32 = 0x10;
const USAGE_UNORDERED_ACCESS: u32 = 0x400;
/// Swap chain flags the swap chain honours: none changes what it does here
/// (`ALLOW_MODE_SWITCH`, `ALLOW_TEARING`).
const FLAGS_TAKEN: u32 = 0x2 | 0x800;

/// `CreateSwapChainForHwnd` (and `CreateSwapChain`) on a Direct3D 12 queue of this library.
pub fn create(
    device: Ref<IUnknown>,
    hwnd: HWND,
    desc: &DXGI_SWAP_CHAIN_DESC1,
    fullscreen: Option<&DXGI_SWAP_CHAIN_FULLSCREEN_DESC>,
) -> Result<IDXGISwapChain1> {
    let queue: ID3D12CommandQueue = device
        .ok()?
        .cast()
        .map_err(|_| windows::core::Error::new(E_INVALIDARG, "not a Direct3D 12 queue"))?;
    if fullscreen.is_some_and(|f| !f.Windowed.as_bool()) {
        return Err(DXGI_ERROR_NOT_CURRENTLY_AVAILABLE.into());
    }
    if desc.Flags & !FLAGS_TAKEN != 0 {
        return Err(windows::core::Error::new(
            E_NOTIMPL,
            "swap chain flags other than mode switches and tearing",
        ));
    }
    if desc.Stereo.as_bool() || desc.SampleDesc.Count != 1 {
        return Err(DXGI_ERROR_INVALID_CALL.into());
    }
    // A window's swap chain shows no alpha: DXGI refuses premultiplied and straight alpha for it.
    if desc.AlphaMode != DXGI_ALPHA_MODE_UNSPECIFIED && desc.AlphaMode != DXGI_ALPHA_MODE_IGNORE {
        return Err(DXGI_ERROR_INVALID_CALL.into());
    }
    if desc.Scaling != DXGI_SCALING_STRETCH {
        return Err(windows::core::Error::new(
            E_NOTIMPL,
            "scaling other than stretching",
        ));
    }
    let model = Model::from_dxgi(desc.SwapEffect.0 as u32).ok_or(DXGI_ERROR_INVALID_CALL)?;
    if !model.is_flip() {
        // Direct3D 12 takes flip models only.
        return Err(DXGI_ERROR_INVALID_CALL.into());
    }
    let hwnd = hwnd.0 as isize;
    let (mut width, mut height) = (desc.Width, desc.Height);
    if width == 0 || height == 0 {
        let (w, h) = client_size(hwnd);
        width = if width == 0 { w.max(8) } else { width };
        height = if height == 0 { h.max(8) } else { height };
    }
    let mut usage = TextureUsage::empty();
    if desc.BufferUsage.0 & USAGE_SHADER_INPUT != 0 {
        usage |= TextureUsage::SAMPLE;
    }
    if desc.BufferUsage.0 & USAGE_UNORDERED_ACCESS != 0 {
        usage |= TextureUsage::STORAGE;
    }
    let front = SwapChainDesc {
        width,
        height,
        format: desc.Format.0 as u32,
        buffers: desc.BufferCount,
        model,
        usage,
    };
    let id = gpu::call_windowed(move |s| {
        let window = s.add_window(hwnd).map_err(|e| {
            eprintln!("wcontainers: the window takes no surface: {e}");
            SwapChainError::InvalidCall("the window takes no surface")
        })?;
        s.front.create_swap_chain(window, &front)
    })
    .map_err(chain_error)?;
    let mut d = *desc;
    d.Width = width;
    d.Height = height;
    Ok(IDXGISwapChain4::from(SwapChain {
        id,
        hwnd,
        queue,
        state: Mutex::new(ChainState {
            desc: d,
            buffers: Vec::new(),
        }),
    })
    .into())
}

struct ChainState {
    desc: DXGI_SWAP_CHAIN_DESC1,
    /// The back buffers handed out, made at the first `GetBuffer` of each.
    buffers: Vec<Option<ID3D12Resource>>,
}

#[implement(IDXGISwapChain4)]
pub struct SwapChain {
    id: ChainId,
    hwnd: isize,
    queue: ID3D12CommandQueue,
    state: Mutex<ChainState>,
}

/// `DXGI_PRESENT_TEST`, `DXGI_PRESENT_RESTART`, `DXGI_PRESENT_DO_NOT_WAIT` and
/// `DXGI_PRESENT_ALLOW_TEARING`.
const PRESENT_TEST: u32 = 0x1;
const PRESENT_RESTART: u32 = 0x4;
const PRESENT_DO_NOT_WAIT: u32 = 0x8;
const PRESENT_ALLOW_TEARING: u32 = 0x200;

/// How many references `r` has besides the caller's own.
fn references(r: &ID3D12Resource) -> u32 {
    let u: &IUnknown = r.into();
    // SAFETY: a live COM object; the reference taken is given back at once.
    unsafe {
        let raw = u.as_raw();
        (u.vtable().AddRef)(raw);
        (u.vtable().Release)(raw) - 1
    }
}

impl SwapChain {
    pub fn Present(&self, sync_interval: u32, flags: DXGI_PRESENT) -> HRESULT {
        let flags = flags.0;
        if flags & !(PRESENT_TEST | PRESENT_RESTART | PRESENT_DO_NOT_WAIT | PRESENT_ALLOW_TEARING)
            != 0
        {
            eprintln!("wcontainers: Present with flags {flags:#x} is not implemented");
            return E_NOTIMPL;
        }
        if flags & PRESENT_TEST != 0 {
            return S_OK;
        }
        let id = self.id;
        let result = gpu::call_windowed(move |s| {
            let d = &mut s.front;
            if flags & PRESENT_DO_NOT_WAIT != 0 && d.would_wait(id) {
                return Err(None);
            }
            d.present(id, sync_interval).map_err(Some)
        });
        match result {
            Ok(()) => S_OK,
            Err(None) => DXGI_ERROR_WAS_STILL_DRAWING,
            Err(Some(SwapChainError::Gpu(wcontainers_gpu::GpuError::DeviceLost))) => {
                DXGI_ERROR_DEVICE_REMOVED
            }
            Err(Some(e)) => chain_error(e).code(),
        }
    }

    pub fn Present1(
        &self,
        sync_interval: u32,
        flags: DXGI_PRESENT,
        parameters: *const DXGI_PRESENT_PARAMETERS,
    ) -> HRESULT {
        // Dirty and scroll rectangles are hints: the whole buffer is shown either way.
        let _ = parameters;
        self.Present(sync_interval, flags)
    }

    pub fn GetBuffer(&self, index: u32, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let id = self.id;
        if state.buffers.is_empty() {
            let n = gpu::call(move |d| {
                (0..)
                    .take_while(|&i| d.chain_buffer(id, i).is_some())
                    .count()
            });
            state.buffers = vec![None; n];
        }
        let desc = state.desc;
        let slot = state
            .buffers
            .get_mut(index as usize)
            .ok_or(DXGI_ERROR_INVALID_CALL)?;
        if slot.is_none() {
            let resource =
                gpu::call(move |d| d.chain_buffer(id, index)).ok_or(DXGI_ERROR_INVALID_CALL)?;
            let flags = if desc.BufferUsage.0 & USAGE_UNORDERED_ACCESS != 0 {
                D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET | D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS
            } else {
                D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET
            };
            *slot = Some(Resource::back_buffer(
                resource,
                D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Alignment: 0,
                    Width: u64::from(desc.Width),
                    Height: desc.Height,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: desc.Format,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                    Flags: flags,
                },
            ));
        }
        give(slot.clone().unwrap(), riid, out)
    }

    pub fn ResizeBuffers(
        &self,
        count: u32,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        flags: &DXGI_SWAP_CHAIN_FLAG,
    ) -> Result<()> {
        if flags.0 as u32 & !FLAGS_TAKEN != 0 {
            return Err(windows::core::Error::new(
                E_NOTIMPL,
                "swap chain flags other than mode switches and tearing",
            ));
        }
        let mut state = self.state.lock().unwrap();
        // DXGI refuses while the program holds a back buffer.
        if state.buffers.iter().flatten().any(|b| references(b) > 1) {
            return Err(windows::core::Error::new(
                DXGI_ERROR_INVALID_CALL,
                "a back buffer is still referenced",
            ));
        }
        state.buffers.clear();
        let (mut w, mut h) = (width, height);
        if w == 0 || h == 0 {
            let (cw, ch) = client_size(self.hwnd);
            w = if w == 0 { cw.max(8) } else { w };
            h = if h == 0 { ch.max(8) } else { h };
        }
        let id = self.id;
        let f = format.0 as u32;
        gpu::call_windowed(move |s| s.front.resize_swap_chain(id, count, w, h, f))
            .map_err(chain_error)?;
        let d = &mut state.desc;
        if count != 0 {
            d.BufferCount = count;
        }
        d.Width = w;
        d.Height = h;
        if f != 0 {
            d.Format = format;
        }
        d.Flags = flags.0 as u32;
        Ok(())
    }

    pub fn GetDesc1(&self) -> Result<DXGI_SWAP_CHAIN_DESC1> {
        Ok(self.state.lock().unwrap().desc)
    }

    pub fn GetDesc(&self) -> Result<DXGI_SWAP_CHAIN_DESC> {
        let d = self.state.lock().unwrap().desc;
        Ok(DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: d.Width,
                Height: d.Height,
                RefreshRate: DXGI_RATIONAL::default(),
                Format: d.Format,
                ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
                Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
            },
            SampleDesc: d.SampleDesc,
            BufferUsage: d.BufferUsage,
            BufferCount: d.BufferCount,
            OutputWindow: HWND(self.hwnd as *mut c_void),
            Windowed: TRUE,
            SwapEffect: d.SwapEffect,
            Flags: d.Flags,
        })
    }

    pub fn GetHwnd(&self) -> Result<HWND> {
        Ok(HWND(self.hwnd as *mut c_void))
    }

    pub fn GetDevice(&self, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        give(self.queue.clone(), riid, out)
    }

    pub fn GetCurrentBackBufferIndex(&self) -> u32 {
        let id = self.id;
        gpu::call(move |d| d.chain(id).map_or(0, |c| c.current_index()))
    }

    pub fn GetLastPresentCount(&self) -> Result<u32> {
        let id = self.id;
        Ok(gpu::call(move |d| {
            d.chain(id).map_or(0, |c| c.present_count() as u32)
        }))
    }

    pub fn SetMaximumFrameLatency(&self, frames: u32) -> Result<()> {
        let id = self.id;
        gpu::call(move |d| match d.chain_mut(id) {
            Some(c) => c.set_max_latency(frames),
            None => Err(SwapChainError::InvalidCall("no such swap chain")),
        })
        .map_err(chain_error)
    }

    pub fn GetMaximumFrameLatency(&self) -> Result<u32> {
        let id = self.id;
        Ok(gpu::call(move |d| {
            d.chain(id).map_or(0, |c| c.max_latency())
        }))
    }

    /// Null: swap chains are made without `FRAME_LATENCY_WAITABLE_OBJECT`, as DXGI answers then.
    pub fn GetFrameLatencyWaitableObject(&self) -> HANDLE {
        HANDLE::default()
    }

    /// Windowed only: fullscreen is the host's to give.
    pub fn GetFullscreenState(
        &self,
        fullscreen: *mut BOOL,
        target: windows::core::OutRef<IDXGIOutput>,
    ) -> Result<()> {
        if !fullscreen.is_null() {
            // SAFETY: the caller's out pointer.
            unsafe { *fullscreen = FALSE };
        }
        let _ = target.write(None);
        Ok(())
    }

    pub fn SetFullscreenState(&self, fullscreen: BOOL, _target: Ref<IDXGIOutput>) -> Result<()> {
        if fullscreen.as_bool() {
            Err(DXGI_ERROR_NOT_CURRENTLY_AVAILABLE.into())
        } else {
            Ok(())
        }
    }

    pub fn IsTemporaryMonoSupported(&self) -> BOOL {
        FALSE
    }

    pub fn CheckColorSpaceSupport(&self, space: DXGI_COLOR_SPACE_TYPE) -> Result<u32> {
        let format = self.state.lock().unwrap().desc.Format.0 as u32;
        Ok(if presents_in(format, space.0 as u32) {
            DXGI_SWAP_CHAIN_COLOR_SPACE_SUPPORT_FLAG_PRESENT.0 as u32
        } else {
            0
        })
    }

    /// Each format presents in one colour space, so there is nothing to change.
    pub fn SetColorSpace1(&self, space: DXGI_COLOR_SPACE_TYPE) -> Result<()> {
        let format = self.state.lock().unwrap().desc.Format.0 as u32;
        if presents_in(format, space.0 as u32) {
            Ok(())
        } else {
            Err(E_INVALIDARG.into())
        }
    }
}

impl Drop for SwapChain {
    fn drop(&mut self) {
        self.state.get_mut().unwrap().buffers.clear();
        let id = self.id;
        gpu::call_windowed(move |s| s.front.destroy_swap_chain(id));
    }
}
