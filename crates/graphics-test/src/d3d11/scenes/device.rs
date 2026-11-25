//! Device creation: what D3D11CreateDevice and D3D11CreateDeviceAndSwapChain answer for each way of
//! calling them (out parameters left out, feature level lists, driver types with and without an
//! adapter, flags, SDK versions), and what the device made says of itself.

use std::ffi::c_void;
use std::fmt::Write;
use std::ptr::{null, null_mut};

use anyhow::Result;
use windows::Win32::Foundation::{E_INVALIDARG, HMODULE, S_FALSE, S_OK};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_UNSUPPORTED, DXGI_SWAP_CHAIN_DESC, IDXGIAdapter, IDXGIFactory4,
    IDXGISwapChain,
};
use windows::core::{HRESULT, Interface};

use crate::Driver;
use crate::d3d11::Gpu;
use crate::output::Output;

// Declared with raw parameters, so that success codes other than S_OK, absent out parameters and
// lists of length 0 can be passed and seen.
#[link(name = "d3d11")]
unsafe extern "system" {
    #[link_name = "D3D11CreateDevice"]
    fn create_device(
        adapter: *mut c_void,
        driver: D3D_DRIVER_TYPE,
        software: HMODULE,
        flags: u32,
        levels: *const D3D_FEATURE_LEVEL,
        count: u32,
        sdk: u32,
        device: *mut *mut c_void,
        level: *mut D3D_FEATURE_LEVEL,
        context: *mut *mut c_void,
    ) -> HRESULT;
    #[link_name = "D3D11CreateDeviceAndSwapChain"]
    fn create_device_and_swap_chain(
        adapter: *mut c_void,
        driver: D3D_DRIVER_TYPE,
        software: HMODULE,
        flags: u32,
        levels: *const D3D_FEATURE_LEVEL,
        count: u32,
        sdk: u32,
        description: *const DXGI_SWAP_CHAIN_DESC,
        swap_chain: *mut *mut c_void,
        device: *mut *mut c_void,
        level: *mut D3D_FEATURE_LEVEL,
        context: *mut *mut c_void,
    ) -> HRESULT;
}

/// What a level out parameter holds when the call did not write it.
const UNTOUCHED: D3D_FEATURE_LEVEL = D3D_FEATURE_LEVEL(0x7777);

/// One way of calling D3D11CreateDevice.
struct Call<'a> {
    adapter: Option<&'a IDXGIAdapter>,
    driver: D3D_DRIVER_TYPE,
    flags: D3D11_CREATE_DEVICE_FLAG,
    /// None: no list (a null pointer and 0).
    levels: Option<&'a [D3D_FEATURE_LEVEL]>,
    sdk: u32,
    device: bool,
    level: bool,
    context: bool,
}

/// What a call returned, and the device it made, if any.
struct Answer {
    result: HRESULT,
    device: Option<ID3D11Device>,
    level: D3D_FEATURE_LEVEL,
    context: Option<ID3D11DeviceContext>,
}

impl Call<'_> {
    fn new(driver: D3D_DRIVER_TYPE) -> Self {
        Call {
            adapter: None,
            driver,
            flags: D3D11_CREATE_DEVICE_FLAG(0),
            levels: None,
            sdk: D3D11_SDK_VERSION,
            device: true,
            level: true,
            context: true,
        }
    }

    fn make(&self, swap_chain: Option<bool>) -> Answer {
        let (mut device, mut level, mut context) = (null_mut(), UNTOUCHED, null_mut());
        let mut chain = null_mut();
        let (levels, count) = match self.levels {
            None => (null(), 0),
            // A list of length 0 still passes a pointer.
            Some([]) => (&D3D_FEATURE_LEVEL_11_0 as *const _, 0),
            Some(list) => (list.as_ptr(), list.len() as u32),
        };
        let adapter = self.adapter.map_or(null_mut(), |a| a.as_raw());
        let out = |wanted: bool, at: *mut *mut c_void| if wanted { at } else { null_mut() };
        let result = unsafe {
            match swap_chain {
                None => create_device(
                    adapter,
                    self.driver,
                    HMODULE::default(),
                    self.flags.0,
                    levels,
                    count,
                    self.sdk,
                    out(self.device, &mut device),
                    if self.level { &mut level } else { null_mut() },
                    out(self.context, &mut context),
                ),
                Some(asked) => create_device_and_swap_chain(
                    adapter,
                    self.driver,
                    HMODULE::default(),
                    self.flags.0,
                    levels,
                    count,
                    self.sdk,
                    null(),
                    out(asked, &mut chain),
                    out(self.device, &mut device),
                    if self.level { &mut level } else { null_mut() },
                    out(self.context, &mut context),
                ),
            }
        };
        if !chain.is_null() {
            drop(unsafe { IDXGISwapChain::from_raw(chain) });
        }
        Answer {
            result,
            device: (!device.is_null()).then(|| unsafe { ID3D11Device::from_raw(device) }),
            level,
            context: (!context.is_null())
                .then(|| unsafe { ID3D11DeviceContext::from_raw(context) }),
        }
    }
}

fn result_name(result: HRESULT) -> String {
    match result {
        S_OK => "S_OK".to_owned(),
        S_FALSE => "S_FALSE".to_owned(),
        E_INVALIDARG => "E_INVALIDARG".to_owned(),
        DXGI_ERROR_UNSUPPORTED => "DXGI_ERROR_UNSUPPORTED".to_owned(),
        other => format!("{:#010x}", other.0 as u32),
    }
}

fn level_name(level: D3D_FEATURE_LEVEL) -> String {
    match level {
        UNTOUCHED => "untouched".to_owned(),
        D3D_FEATURE_LEVEL(0) => "0".to_owned(),
        _ => format!("{}_{}", level.0 >> 12, (level.0 >> 8) & 0xf),
    }
}

impl Answer {
    fn describe(&self) -> String {
        let made = |m: bool| if m { "made" } else { "none" };
        format!(
            "{}, device {}, level {}, context {}",
            result_name(self.result),
            made(self.device.is_some()),
            level_name(self.level),
            made(self.context.is_some())
        )
    }
}

pub fn run(gpu: &Gpu) -> Result<Vec<Output>> {
    let driver = match gpu.driver {
        Driver::Hardware => D3D_DRIVER_TYPE_HARDWARE,
        Driver::Warp => D3D_DRIVER_TYPE_WARP,
    };
    let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory1()? };
    let adapter: IDXGIAdapter = match gpu.driver {
        Driver::Hardware => unsafe { factory.EnumAdapters(0)? },
        Driver::Warp => unsafe { factory.EnumWarpAdapter()? },
    };
    let mut text = String::new();
    let mut ask = |label: &str, call: Call, swap_chain: Option<bool>| -> Result<Answer> {
        let answer = call.make(swap_chain);
        writeln!(text, "{label}: {}", answer.describe())?;
        Ok(answer)
    };

    // Out parameters.
    let all = ask("every out parameter", Call::new(driver), None)?;
    let none = Call {
        device: false,
        level: false,
        context: false,
        ..Call::new(driver)
    };
    ask("no out parameter", none, None)?;
    let level_only = Call {
        device: false,
        context: false,
        ..Call::new(driver)
    };
    ask("only the level", level_only, None)?;
    let device_only = Call {
        level: false,
        context: false,
        ..Call::new(driver)
    };
    ask("only the device", device_only, None)?;
    let context_only = Call {
        device: false,
        level: false,
        ..Call::new(driver)
    };
    ask("only the context", context_only, None)?;

    // Feature level lists.
    let lists: [(&str, &[D3D_FEATURE_LEVEL]); 5] = [
        ("an empty list", &[]),
        (
            "11_0 then an unknown level",
            &[D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL(0x1234)],
        ),
        ("an unknown level alone", &[D3D_FEATURE_LEVEL(0x1234)]),
        (
            "10_0 twice",
            &[D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_0],
        ),
        (
            "9_3 then 11_0",
            &[D3D_FEATURE_LEVEL_9_3, D3D_FEATURE_LEVEL_11_0],
        ),
    ];
    for (label, levels) in lists {
        let call = Call {
            levels: Some(levels),
            ..Call::new(driver)
        };
        ask(&format!("levels: {label}"), call, None)?;
    }

    // Driver types and adapters.
    let unknown = Call {
        driver: D3D_DRIVER_TYPE_UNKNOWN,
        ..Call::new(driver)
    };
    ask("driver unknown without an adapter", unknown, None)?;
    let both = Call {
        adapter: Some(&adapter),
        ..Call::new(driver)
    };
    ask("the driver type and an adapter", both, None)?;
    let by_adapter = Call {
        adapter: Some(&adapter),
        driver: D3D_DRIVER_TYPE_UNKNOWN,
        ..Call::new(driver)
    };
    let on_adapter = ask("driver unknown with an adapter", by_adapter, None)?;
    let software = Call {
        driver: D3D_DRIVER_TYPE_SOFTWARE,
        ..Call::new(driver)
    };
    ask("driver software without a module", software, None)?;

    // Flags and SDK versions.
    let undefined = Call {
        flags: D3D11_CREATE_DEVICE_FLAG(0x4000_0000),
        ..Call::new(driver)
    };
    ask("an undefined flag", undefined, None)?;
    let old_sdk = Call {
        sdk: 0,
        ..Call::new(driver)
    };
    ask("SDK version 0", old_sdk, None)?;
    let flags = D3D11_CREATE_DEVICE_SINGLETHREADED | D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    let flagged = ask(
        "single-threaded and BGRA",
        Call {
            flags,
            ..Call::new(driver)
        },
        None,
    )?;

    // D3D11CreateDeviceAndSwapChain without a description.
    ask(
        "and swap chain: no description, no swap chain",
        Call::new(driver),
        Some(false),
    )?;
    ask(
        "and swap chain: no description, a swap chain asked for",
        Call::new(driver),
        Some(true),
    )?;

    // What the devices made say of themselves.
    if let (Some(device), Some(context)) = (&all.device, &all.context) {
        let own = unsafe { device.GetImmediateContext()? };
        writeln!(
            text,
            "level from the device equals the one returned: {}",
            unsafe { device.GetFeatureLevel() } == all.level
        )?;
        writeln!(
            text,
            "immediate context is the one returned: {}",
            own.as_raw() == context.as_raw()
        )?;
        writeln!(
            text,
            "context's device is the device: {}",
            unsafe { context.GetDevice()? }.as_raw() == device.as_raw()
        )?;
        writeln!(text, "creation flags without flags: {:#x}", unsafe {
            device.GetCreationFlags()
        })?;
    }
    if let Some(device) = &flagged.device {
        writeln!(
            text,
            "creation flags single-threaded and BGRA: {:#x}",
            unsafe { device.GetCreationFlags() }
        )?;
    }
    if let Some(device) = &on_adapter.device {
        let dxgi: windows::Win32::Graphics::Dxgi::IDXGIDevice = device.cast()?;
        let theirs = unsafe { dxgi.GetAdapter()?.GetDesc()? };
        let ours = unsafe { adapter.GetDesc()? };
        writeln!(
            text,
            "device on an adapter is on that adapter: {}",
            theirs.AdapterLuid.LowPart == ours.AdapterLuid.LowPart
                && theirs.AdapterLuid.HighPart == ours.AdapterLuid.HighPart
        )?;
    }
    Ok(vec![Output::text("results", text)])
}
