//! The call census and frame capture, on when WC_OUT names a directory. Counts every call a program
//! makes to the methods of its Direct3D 11 devices, immediate and deferred contexts, swap chains and
//! DXGI factories, and writes the counts to <WC_OUT>\census.txt when the process ends. Devices,
//! swap chains and factories have their method tables patched with counting thunks the first time
//! an object using one is created through this library. Contexts are handed to the program wrapped
//! instead, since Direct3D 11 rewrites entries of a context's table as it runs: each wrapper method
//! counts and jumps through the context's own table as it is at that moment. WC_CAPTURE lists frame numbers (comma separated): the back buffer is
//! saved as <WC_OUT>\frame-<n>.png before those presents, and the process ends after the last.

use std::ffi::c_void;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Memory::{PAGE_PROTECTION_FLAGS, PAGE_READWRITE, VirtualProtect};
use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
use windows::core::{GUID, HRESULT, IUnknown, Interface};

include!(concat!(env!("OUT_DIR"), "/census.rs"));

pub static COUNTS: [AtomicU64; INSTANCES * SLOTS] =
    [const { AtomicU64::new(0) }; INSTANCES * SLOTS];
pub static ORIGINALS: [AtomicUsize; INSTANCES * SLOTS] =
    [const { AtomicUsize::new(0) }; INSTANCES * SLOTS];
/// The method table each instance patched, for typed hooks to find their original.
static VTABLES: [AtomicUsize; INSTANCES] = [const { AtomicUsize::new(0) }; INSTANCES];
static PATCHED: Mutex<Vec<(Table, &'static str)>> = Mutex::new(Vec::new());
/// Tables left uncounted because every instance was taken.
static UNCOUNTED: AtomicU64 = AtomicU64::new(0);
static FRAMES: AtomicU64 = AtomicU64::new(0);
static WRITTEN: AtomicBool = AtomicBool::new(false);
static NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Calls of each context method, immediate contexts first, then deferred ones.
pub static WRAPPED_COUNTS: [AtomicU64; 2 * CONTEXT_SLOTS] =
    [const { AtomicU64::new(0) }; 2 * CONTEXT_SLOTS];
/// The wrapper of each wrapped context, by the context's address.
static WRAPPERS: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

unsafe extern "C" {
    static wc_census_thunks: u8;
    static wc_wrapper_thunks: u8;
}

/// A context as the program sees it: its own method table and the context it stands for.
#[repr(C)]
struct Wrapper {
    vtable: *const usize,
    context: *mut c_void,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Table {
    Device,
    Immediate,
    Deferred,
    SwapChain,
    Factory,
}

impl Table {
    const ALL: [Table; 5] = [
        Table::Device,
        Table::Immediate,
        Table::Deferred,
        Table::SwapChain,
        Table::Factory,
    ];

    fn label(self) -> &'static str {
        match self {
            Table::Device => "device",
            Table::Immediate => "immediate-context",
            Table::Deferred => "deferred-context",
            Table::SwapChain => "swap-chain",
            Table::Factory => "factory",
        }
    }

    /// The table's interfaces, most derived first.
    fn chain(self) -> &'static [(GUID, &'static str)] {
        match self {
            Table::Device => &[
                (ID3D11Device5::IID, "ID3D11Device5"),
                (ID3D11Device4::IID, "ID3D11Device4"),
                (ID3D11Device3::IID, "ID3D11Device3"),
                (ID3D11Device2::IID, "ID3D11Device2"),
                (ID3D11Device1::IID, "ID3D11Device1"),
                (ID3D11Device::IID, "ID3D11Device"),
            ],
            Table::Immediate | Table::Deferred => &[
                (ID3D11DeviceContext4::IID, "ID3D11DeviceContext4"),
                (ID3D11DeviceContext3::IID, "ID3D11DeviceContext3"),
                (ID3D11DeviceContext2::IID, "ID3D11DeviceContext2"),
                (ID3D11DeviceContext1::IID, "ID3D11DeviceContext1"),
                (ID3D11DeviceContext::IID, "ID3D11DeviceContext"),
            ],
            Table::SwapChain => &[
                (IDXGISwapChain4::IID, "IDXGISwapChain4"),
                (IDXGISwapChain3::IID, "IDXGISwapChain3"),
                (IDXGISwapChain2::IID, "IDXGISwapChain2"),
                (IDXGISwapChain1::IID, "IDXGISwapChain1"),
                (IDXGISwapChain::IID, "IDXGISwapChain"),
            ],
            Table::Factory => &[
                (IDXGIFactory7::IID, "IDXGIFactory7"),
                (IDXGIFactory6::IID, "IDXGIFactory6"),
                (IDXGIFactory5::IID, "IDXGIFactory5"),
                (IDXGIFactory4::IID, "IDXGIFactory4"),
                (IDXGIFactory3::IID, "IDXGIFactory3"),
                (IDXGIFactory2::IID, "IDXGIFactory2"),
                (IDXGIFactory1::IID, "IDXGIFactory1"),
                (IDXGIFactory::IID, "IDXGIFactory"),
            ],
        }
    }
}

struct Config {
    out: PathBuf,
    capture: Vec<u64>,
}

/// Reads the census's settings, before the library calls go to is loaded.
pub fn configure() {
    config();
}

fn config() -> Option<&'static Config> {
    static CONFIG: OnceLock<Option<Config>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let out = PathBuf::from(std::env::var_os("WC_OUT")?);
            let mut capture: Vec<u64> = std::env::var("WC_CAPTURE")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().parse().expect("WC_CAPTURE holds frame numbers"))
                .collect();
            capture.sort_unstable();
            Some(Config { out, capture })
        })
        .as_ref()
}

fn methods(interface: &str) -> &'static [&'static str] {
    INTERFACES
        .iter()
        .find(|(name, _)| *name == interface)
        .expect("a counted interface")
        .1
}

fn note(text: String) {
    NOTES.lock().unwrap().push(text);
}

/// Called with each object of `table` the program gets through this library: patches the method
/// tables behind its interfaces, once per table.
///
/// # Safety
/// `object` is null or a live COM object of that kind.
pub unsafe fn patch(table: Table, object: *mut c_void) {
    if config().is_none() || object.is_null() {
        return;
    }
    let own = unsafe { *(object as *const usize) };
    if VTABLES.iter().any(|v| v.load(Ordering::Acquire) == own) {
        return;
    }
    let unknown = unsafe { IUnknown::from_raw_borrowed(&object) }.unwrap();
    // Every distinct table behind the chain's interfaces, found before any is changed.
    let mut found: Vec<(usize, &'static str)> = Vec::new();
    for (iid, name) in table.chain() {
        let mut interface = std::ptr::null_mut();
        if unsafe { unknown.query(iid, &mut interface) }.is_ok() {
            let vtable = unsafe { *(interface as *const usize) };
            drop(unsafe { IUnknown::from_raw(interface) });
            if !found.iter().any(|(v, _)| *v == vtable) {
                found.push((vtable, name));
            }
        }
    }
    for (vtable, interface) in found {
        install(table, vtable, interface);
    }
}

/// The wrappers' method tables, for immediate and for deferred contexts.
fn wrapper_tables() -> &'static [Vec<usize>; 2] {
    static TABLES: OnceLock<[Vec<usize>; 2]> = OnceLock::new();
    TABLES.get_or_init(|| {
        let thunks = (&raw const wc_wrapper_thunks) as usize;
        [0, 1].map(|t| {
            let mut table: Vec<usize> = (0..CONTEXT_SLOTS)
                .map(|k| thunks + 32 * (t * CONTEXT_SLOTS + k))
                .collect();
            table[0] = wrapper_query as *const () as usize;
            table
        })
    })
}

/// Hands the program a wrapper in place of the context `*out` (one wrapper per context).
///
/// # Safety
/// `out` is null or points to null or a live context.
pub unsafe fn wrap(table: Table, out: *mut *mut c_void) {
    if config().is_none() || out.is_null() || unsafe { *out }.is_null() {
        return;
    }
    let context = unsafe { *out } as usize;
    let tables = wrapper_tables();
    if tables
        .iter()
        .any(|t| unsafe { *(context as *const *const usize) } == t.as_ptr())
    {
        return;
    }
    let mut wrappers = WRAPPERS.lock().unwrap();
    let wrapper = match wrappers.iter().find(|(c, _)| *c == context) {
        Some((_, wrapper)) => *wrapper,
        None => {
            let index = usize::from(table == Table::Deferred);
            let wrapper = Box::into_raw(Box::new(Wrapper {
                vtable: tables[index].as_ptr(),
                context: context as *mut c_void,
            })) as usize;
            wrappers.push((context, wrapper));
            wrapper
        }
    };
    unsafe { *out = wrapper as *mut c_void };
}

/// QueryInterface of a wrapper: asked for a context interface it answers with itself, so the
/// program stays on the wrapper; anything else is the context's own answer.
unsafe extern "system" fn wrapper_query(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    let wrapper = unsafe { &*(this as *const Wrapper) };
    let index = usize::from(wrapper.vtable == wrapper_tables()[1].as_ptr());
    WRAPPED_COUNTS[index * CONTEXT_SLOTS].fetch_add(1, Ordering::Relaxed);
    let context = unsafe { IUnknown::from_raw_borrowed(&wrapper.context) }.unwrap();
    let result = unsafe { context.query(iid, out) };
    let own = Table::Immediate
        .chain()
        .iter()
        .any(|(id, _)| unsafe { *iid } == *id);
    if result.is_ok() && own {
        // The reference the context gave is the wrapper's: they share one count.
        unsafe { *out = this };
    }
    result
}

fn install(table: Table, vtable: usize, interface: &'static str) {
    let mut patched = PATCHED.lock().unwrap();
    if VTABLES.iter().any(|v| v.load(Ordering::Acquire) == vtable) {
        return;
    }
    let instance = patched.len();
    if instance == INSTANCES {
        UNCOUNTED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let names = methods(interface);
    let base = instance * SLOTS;
    let thunks = (&raw const wc_census_thunks) as usize;
    let entries = vtable as *mut usize;
    let mut old = PAGE_PROTECTION_FLAGS(0);
    if unsafe { VirtualProtect(entries.cast(), names.len() * 8, PAGE_READWRITE, &mut old) }.is_err()
    {
        note(format!(
            "{} as {interface}: its method table could not be made writable",
            table.label()
        ));
        return;
    }
    VTABLES[instance].store(vtable, Ordering::Release);
    for (k, name) in names.iter().enumerate() {
        unsafe {
            ORIGINALS[base + k].store(*entries.add(k), Ordering::Release);
            *entries.add(k) = typed(table, name).unwrap_or(thunks + 16 * (base + k));
        }
    }
    unsafe { VirtualProtect(entries.cast(), names.len() * 8, old, &mut old) }.ok();
    patched.push((table, interface));
}

/// Counts a call that came through a typed hook and returns the function it replaced.
fn enter(this: *mut c_void, name: &str) -> usize {
    let vtable = unsafe { *(this as *const usize) };
    let instance = VTABLES
        .iter()
        .position(|v| v.load(Ordering::Acquire) == vtable)
        .expect("a typed hook is reached only through a patched table");
    let interface = PATCHED.lock().unwrap()[instance].1;
    let k = methods(interface).iter().position(|n| *n == name).unwrap();
    COUNTS[instance * SLOTS + k].fetch_add(1, Ordering::Relaxed);
    ORIGINALS[instance * SLOTS + k].load(Ordering::Acquire)
}

/// Methods that do more than count: those that make swap chains and deferred contexts (patched
/// in turn) and the presents (frame capture).
fn typed(table: Table, name: &str) -> Option<usize> {
    let hook: usize = match (table, name) {
        (Table::SwapChain, "Present") => present as *const () as usize,
        (Table::SwapChain, "Present1") => present1 as *const () as usize,
        (Table::Factory, "CreateSwapChain") => create_swap_chain as *const () as usize,
        (Table::Factory, "CreateSwapChainForHwnd") => {
            create_swap_chain_for_hwnd as *const () as usize
        }
        (Table::Factory, "CreateSwapChainForCoreWindow") => {
            create_swap_chain_for_core_window as *const () as usize
        }
        (Table::Factory, "CreateSwapChainForComposition") => {
            create_swap_chain_for_composition as *const () as usize
        }
        (Table::Device, "GetImmediateContext") => get_immediate_context::<0> as *const () as usize,
        (Table::Device, "GetImmediateContext1") => get_immediate_context::<1> as *const () as usize,
        (Table::Device, "GetImmediateContext2") => get_immediate_context::<2> as *const () as usize,
        (Table::Device, "GetImmediateContext3") => get_immediate_context::<3> as *const () as usize,
        (Table::Device, "CreateDeferredContext") => {
            create_deferred_context::<0> as *const () as usize
        }
        (Table::Device, "CreateDeferredContext1") => {
            create_deferred_context::<1> as *const () as usize
        }
        (Table::Device, "CreateDeferredContext2") => {
            create_deferred_context::<2> as *const () as usize
        }
        (Table::Device, "CreateDeferredContext3") => {
            create_deferred_context::<3> as *const () as usize
        }
        _ => return None,
    };
    Some(hook)
}

unsafe extern "system" fn present(this: *mut c_void, interval: u32, flags: u32) -> HRESULT {
    let original = enter(this, "Present");
    // A test present shows nothing, so it is no frame.
    if flags & DXGI_PRESENT_TEST.0 == 0 {
        frame(this);
    }
    let original: unsafe extern "system" fn(*mut c_void, u32, u32) -> HRESULT =
        unsafe { std::mem::transmute(original) };
    unsafe { original(this, interval, flags) }
}

unsafe extern "system" fn present1(
    this: *mut c_void,
    interval: u32,
    flags: u32,
    parameters: *const c_void,
) -> HRESULT {
    let original = enter(this, "Present1");
    // A test present shows nothing, so it is no frame.
    if flags & DXGI_PRESENT_TEST.0 == 0 {
        frame(this);
    }
    let original: unsafe extern "system" fn(*mut c_void, u32, u32, *const c_void) -> HRESULT =
        unsafe { std::mem::transmute(original) };
    unsafe { original(this, interval, flags, parameters) }
}

unsafe extern "system" fn create_swap_chain(
    this: *mut c_void,
    device: *mut c_void,
    desc: *const c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    let original: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *const c_void,
        *mut *mut c_void,
    ) -> HRESULT = unsafe { std::mem::transmute(enter(this, "CreateSwapChain")) };
    let result = unsafe { original(this, device, desc, out) };
    if result.is_ok() {
        unsafe { patch(Table::SwapChain, *out) };
    }
    result
}

unsafe extern "system" fn create_swap_chain_for_hwnd(
    this: *mut c_void,
    device: *mut c_void,
    window: HWND,
    desc: *const c_void,
    fullscreen: *const c_void,
    output: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    type Original = unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        HWND,
        *const c_void,
        *const c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT;
    let original: Original = unsafe { std::mem::transmute(enter(this, "CreateSwapChainForHwnd")) };
    let result = unsafe { original(this, device, window, desc, fullscreen, output, out) };
    if result.is_ok() {
        unsafe { patch(Table::SwapChain, *out) };
    }
    result
}

unsafe extern "system" fn create_swap_chain_for_core_window(
    this: *mut c_void,
    device: *mut c_void,
    window: *mut c_void,
    desc: *const c_void,
    output: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    type Original = unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *const c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT;
    let original: Original =
        unsafe { std::mem::transmute(enter(this, "CreateSwapChainForCoreWindow")) };
    let result = unsafe { original(this, device, window, desc, output, out) };
    if result.is_ok() {
        unsafe { patch(Table::SwapChain, *out) };
    }
    result
}

unsafe extern "system" fn create_swap_chain_for_composition(
    this: *mut c_void,
    device: *mut c_void,
    desc: *const c_void,
    output: *mut c_void,
    out: *mut *mut c_void,
) -> HRESULT {
    type Original = unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *const c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT;
    let original: Original =
        unsafe { std::mem::transmute(enter(this, "CreateSwapChainForComposition")) };
    let result = unsafe { original(this, device, desc, output, out) };
    if result.is_ok() {
        unsafe { patch(Table::SwapChain, *out) };
    }
    result
}

/// GetImmediateContext and its versions 1 to 3, which share one shape.
unsafe extern "system" fn get_immediate_context<const VERSION: u8>(
    this: *mut c_void,
    out: *mut *mut c_void,
) {
    let name = [
        "GetImmediateContext",
        "GetImmediateContext1",
        "GetImmediateContext2",
        "GetImmediateContext3",
    ][VERSION as usize];
    let original: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) =
        unsafe { std::mem::transmute(enter(this, name)) };
    unsafe {
        original(this, out);
        wrap(Table::Immediate, out);
    }
}

/// CreateDeferredContext and its versions 1 to 3, which share one shape.
unsafe extern "system" fn create_deferred_context<const VERSION: u8>(
    this: *mut c_void,
    flags: u32,
    out: *mut *mut c_void,
) -> HRESULT {
    let name = [
        "CreateDeferredContext",
        "CreateDeferredContext1",
        "CreateDeferredContext2",
        "CreateDeferredContext3",
    ][VERSION as usize];
    let original: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT =
        unsafe { std::mem::transmute(enter(this, name)) };
    let result = unsafe { original(this, flags, out) };
    if result.is_ok() {
        unsafe { wrap(Table::Deferred, out) };
    }
    result
}

/// Called for a device the program created through this library, with where its immediate context
/// and its swap chain were written (null if not asked for): patches the device, the swap chain and
/// the DXGI factory above the device, and puts a wrapper in place of the context.
///
/// # Safety
/// `device` is a live device; `context` and `chain` are null or point to null or live objects.
pub unsafe fn created(device: *mut c_void, context: *mut *mut c_void, chain: *mut c_void) {
    if config().is_none() || device.is_null() {
        return;
    }
    let factory = (|| -> windows::core::Result<IDXGIFactory> {
        let device = unsafe { IUnknown::from_raw_borrowed(&device) }
            .unwrap()
            .cast::<IDXGIDevice>()?;
        unsafe { device.GetAdapter()?.GetParent() }
    })();
    match &factory {
        Ok(factory) => unsafe { patch(Table::Factory, factory.as_raw()) },
        Err(error) => note(format!("factory not counted: {error}")),
    }
    unsafe {
        patch(Table::Device, device);
        patch(Table::SwapChain, chain);
        wrap(Table::Immediate, context);
    }
}

/// Counts a present and saves or ends at the frames WC_CAPTURE names.
fn frame(chain: *mut c_void) {
    let frame = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    let Some(config) = config() else { return };
    if config.capture.binary_search(&frame).is_err() {
        return;
    }
    // Calls made while saving the frame are not the program's: they are taken back out.
    let counts = || COUNTS.iter().chain(&WRAPPED_COUNTS);
    let before: Vec<u64> = counts().map(|c| c.load(Ordering::Relaxed)).collect();
    if let Err(error) = save(chain, frame, config) {
        note(format!("frame {frame} not saved: {error:#}"));
    }
    for (count, before) in counts().zip(before) {
        count.store(before, Ordering::Relaxed);
    }
    if config.capture.last() == Some(&frame) {
        write();
        let _ = unsafe { TerminateProcess(GetCurrentProcess(), 0) };
    }
}

fn save(chain: *mut c_void, frame: u64, config: &Config) -> anyhow::Result<()> {
    let chain = unsafe { IDXGISwapChain::from_raw_borrowed(&chain) }.unwrap();
    let back: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { back.GetDesc(&mut desc) };
    let bgra = match desc.Format {
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => false,
        DXGI_FORMAT_B8G8R8A8_UNORM
        | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        | DXGI_FORMAT_B8G8R8X8_UNORM => true,
        other => anyhow::bail!("the back buffer's format {} is not one this saves", other.0),
    };
    anyhow::ensure!(
        desc.SampleDesc.Count == 1,
        "the back buffer is multisampled"
    );
    let device = unsafe { back.GetDevice()? };
    let context = unsafe { device.GetImmediateContext()? };
    let staging_desc = D3D11_TEXTURE2D_DESC {
        MipLevels: 1,
        ArraySize: 1,
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
        ..desc
    };
    let mut staging = None;
    unsafe { device.CreateTexture2D(&staging_desc, None, Some(&mut staging))? };
    let staging = staging.unwrap();
    let mut pixels = vec![0u8; desc.Width as usize * desc.Height as usize * 4];
    unsafe {
        context.CopyResource(&staging, &back);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        let row = desc.Width as usize * 4;
        for y in 0..desc.Height as usize {
            let source = std::slice::from_raw_parts(
                mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                row,
            );
            pixels[y * row..(y + 1) * row].copy_from_slice(source);
        }
        context.Unmap(&staging, 0);
    }
    for texel in pixels.as_chunks_mut::<4>().0 {
        if bgra {
            texel.swap(0, 2);
        }
        if desc.Format == DXGI_FORMAT_B8G8R8X8_UNORM {
            texel[3] = 255;
        }
    }
    std::fs::create_dir_all(&config.out)?;
    let file = std::fs::File::create(config.out.join(format!("frame-{frame}.png")))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), desc.Width, desc.Height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}

/// Writes <WC_OUT>\census.txt once: frames presented, the tables counted, then one line per
/// method called, "<table> <method> <calls>", and per export of the library, most called first.
pub fn write() {
    let Some(config) = config() else { return };
    if WRITTEN.swap(true, Ordering::AcqRel) {
        return;
    }
    let mut text = String::new();
    let program = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
    let _ = writeln!(text, "# program {}", program.unwrap_or_default());
    let _ = writeln!(text, "# frames {}", FRAMES.load(Ordering::Relaxed));
    let patched = PATCHED.lock().unwrap().clone();
    for (table, interface) in &patched {
        let _ = writeln!(text, "# counted {} as {interface}", table.label());
    }
    let _ = writeln!(
        text,
        "# counted immediate-context and deferred-context through {} wrapped contexts",
        WRAPPERS.lock().unwrap().len()
    );
    let uncounted = UNCOUNTED.load(Ordering::Relaxed);
    if uncounted > 0 {
        let _ = writeln!(
            text,
            "# {uncounted} method tables not counted: no room left"
        );
    }
    for note in NOTES.lock().unwrap().iter() {
        let _ = writeln!(text, "# {note}");
    }
    // Entries the runtime changed after they were patched: calls through them went uncounted.
    let thunks = (&raw const wc_census_thunks) as usize;
    for (instance, (table, interface)) in patched.iter().enumerate() {
        let entries = VTABLES[instance].load(Ordering::Acquire) as *const usize;
        let changed: Vec<&str> = methods(interface)
            .iter()
            .enumerate()
            .filter(|(k, name)| {
                let ours = typed(*table, name).unwrap_or(thunks + 16 * (instance * SLOTS + k));
                (unsafe { *entries.add(*k) }) != ours
            })
            .map(|(_, name)| *name)
            .collect();
        if !changed.is_empty() {
            let _ = writeln!(
                text,
                "# {} as {interface}: entries changed after patching, calls through them not counted: {}",
                table.label(),
                changed.join(" ")
            );
        }
    }
    for table in Table::ALL {
        let mut calls: Vec<(&str, u64)> = Vec::new();
        if matches!(table, Table::Immediate | Table::Deferred) {
            let base = usize::from(table == Table::Deferred) * CONTEXT_SLOTS;
            for (k, name) in methods("ID3D11DeviceContext4").iter().enumerate() {
                calls.push((name, WRAPPED_COUNTS[base + k].load(Ordering::Relaxed)));
            }
        }
        for (instance, (t, interface)) in patched.iter().enumerate() {
            if *t != table {
                continue;
            }
            for (k, name) in methods(interface).iter().enumerate() {
                let count = COUNTS[instance * SLOTS + k].load(Ordering::Relaxed);
                match calls.iter_mut().find(|(n, _)| n == name) {
                    Some((_, total)) => *total += count,
                    None => calls.push((name, count)),
                }
            }
        }
        calls.retain(|(_, count)| *count > 0);
        calls.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        for (name, count) in calls {
            let _ = writeln!(text, "{} {name} {count}", table.label());
        }
    }
    let mut exports: Vec<(String, u64)> = crate::export_counts();
    exports.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (name, count) in exports {
        let _ = writeln!(text, "{}.dll {name} {count}", crate::LIBRARY);
    }
    let _ = std::fs::create_dir_all(&config.out)
        .and_then(|()| std::fs::write(config.out.join("census.txt"), text));
}
