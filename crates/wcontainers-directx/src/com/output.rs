//! The outputs an adapter enumerates (`IDXGIAdapter::EnumOutputs`): the host's monitors, described
//! by the shared infrastructure crate, whose one mode is the current one.

use std::ffi::c_void;

use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW,
    HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::core::{BOOL, GUID, IUnknown, Interface, PCWSTR, Ref, Result, implement};

use wcontainers_directx_graphics_infrastructure::{DisplayMode, OutputDesc};

use super::*;

unsafe extern "system" fn collect(m: HMONITOR, _: HDC, _: *mut RECT, list: LPARAM) -> BOOL {
    // SAFETY: `monitors` passes its list.
    unsafe { (*(list.0 as *mut Vec<HMONITOR>)).push(m) };
    true.into()
}

/// The host's monitors, the primary first, each with its handle, as Windows reports them.
pub fn monitors() -> Vec<(OutputDesc, HMONITOR)> {
    let mut found: Vec<HMONITOR> = Vec::new();
    // SAFETY: the callback only pushes into the list, which outlives the call.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut found as *mut _ as isize),
        )
    };
    let mut out = Vec::new();
    for m in found {
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
        out.push((
            info.monitorInfo.dwFlags
                & windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY
                == 0,
            OutputDesc {
                name: String::from_utf16_lossy(&info.szDevice[..name_len]),
                desktop: [r.left, r.top, r.right, r.bottom],
                refresh_numerator: hz,
                refresh_denominator: 1,
            },
            m,
        ));
    }
    out.sort_by_key(|o| o.0);
    out.into_iter().map(|(_, d, m)| (d, m)).collect()
}

#[implement(IDXGIOutput)]
pub struct Output {
    pub(super) private: Private,
    desc: OutputDesc,
    monitor: HMONITOR,
    adapter: IDXGIAdapter1,
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

impl Output {
    pub fn new(desc: OutputDesc, monitor: HMONITOR, adapter: IDXGIAdapter1) -> Self {
        Output {
            private: Private::default(),
            desc,
            monitor,
            adapter,
        }
    }

    /// The adapter it belongs to.
    pub(super) fn get_parent(&self, riid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        // SAFETY: the caller's pointers, as COM passes them.
        unsafe { self.adapter.query(riid, out).ok() }
    }

    pub(super) fn get_desc(&self) -> Result<DXGI_OUTPUT_DESC> {
        let mut name = [0u16; 32];
        for (d, c) in name.iter_mut().zip(self.desc.name.encode_utf16().take(31)) {
            *d = c;
        }
        let [left, top, right, bottom] = self.desc.desktop;
        Ok(DXGI_OUTPUT_DESC {
            DeviceName: name,
            DesktopCoordinates: RECT {
                left,
                top,
                right,
                bottom,
            },
            AttachedToDesktop: true.into(),
            Rotation: DXGI_MODE_ROTATION_IDENTITY,
            Monitor: self.monitor,
        })
    }

    /// The modes of `format`, as DXGI fills a list: the count alone without one, and
    /// `DXGI_ERROR_MORE_DATA` when it is too short.
    pub(super) fn get_display_mode_list(
        &self,
        format: DXGI_FORMAT,
        _flags: DXGI_ENUM_MODES,
        count: *mut u32,
        list: *mut DXGI_MODE_DESC,
    ) -> Result<()> {
        // SAFETY: the API requires a count.
        let count = unsafe { count.as_mut() }.ok_or(DXGI_ERROR_INVALID_CALL)?;
        let modes = self.desc.modes(format.0 as u32);
        if list.is_null() {
            *count = modes.len() as u32;
            return Ok(());
        }
        let n = (*count as usize).min(modes.len());
        for (i, m) in modes.iter().take(n).enumerate() {
            // SAFETY: the list holds `*count` entries.
            unsafe { list.add(i).write(mode_desc(m)) };
        }
        *count = n as u32;
        if n < modes.len() {
            return Err(DXGI_ERROR_MORE_DATA.into());
        }
        Ok(())
    }

    pub(super) fn find_closest_matching_mode(
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
}
