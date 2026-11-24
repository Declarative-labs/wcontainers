//! The call census and frame capture, on when WC_OUT names a directory. Counts every call a program
//! makes to the methods of its Direct3D 11 devices, immediate and deferred contexts, its Direct3D 12
//! devices, command queues and command lists, its swap chains and DXGI factories, and writes the
//! counts to <WC_OUT>\census.txt when the process ends. Devices, queues, command lists, swap chains
//! and factories have their method tables patched with counting thunks the first time an object
//! using one is created through this library (command lists: one of each kind the library makes
//! on each new Direct3D 12 device, and the copies of those tables the display driver gives lists at
//! their first Reset). Contexts are handed to the program wrapped
//! instead, since Direct3D 11 rewrites entries of a context's table as it runs: each wrapper method
//! counts and jumps through the context's own table as it is at that moment. WC_CAPTURE lists frame
//! numbers (comma separated): the back buffer is saved as <WC_OUT>\frame-<n>.png before those
//! presents, the process ends after the last, and the program runs on the clock of clock.rs.

use std::ffi::c_void;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D12::*;
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
/// The command queue each Direct3D 12 swap chain presents on, by the swap chain's identity (its
/// IUnknown), as given to the factory when it was made.
static QUEUES: Mutex<Vec<(usize, ID3D12CommandQueue)>> = Mutex::new(Vec::new());
/// The id of the thread making the census's own calls (saving a frame, patching command list
/// tables) while it does, 0 otherwise: its calls are not counted, while other threads' are.
pub static SAVING: AtomicU64 = AtomicU64::new(0);
static OWN_CALLS: Mutex<()> = Mutex::new(());

/// Runs `f` with the calls this thread makes left out of the census.
fn uncounted<T>(f: impl FnOnce() -> T) -> T {
    let _turn = OWN_CALLS.lock().unwrap_or_else(|e| e.into_inner());
    SAVING.store(thread_id(), Ordering::Release);
    let result = f();
    SAVING.store(0, Ordering::Release);
    result
}

fn thread_id() -> u64 {
    u64::from(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() })
}

/// Whether a call on this thread is counted: it is not one of the census's own.
fn counted() -> bool {
    SAVING.load(Ordering::Acquire) != thread_id()
}

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
    Device12,
    Queue,
    List,
}

impl Table {
    const ALL: [Table; 8] = [
        Table::Device,
        Table::Immediate,
        Table::Deferred,
        Table::Device12,
        Table::Queue,
        Table::List,
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
            Table::Device12 => "d3d12-device",
            Table::Queue => "command-queue",
            Table::List => "command-list",
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
            Table::Device12 => &[
                (ID3D12Device14::IID, "ID3D12Device14"),
                (ID3D12Device13::IID, "ID3D12Device13"),
                (ID3D12Device12::IID, "ID3D12Device12"),
                (ID3D12Device11::IID, "ID3D12Device11"),
                (ID3D12Device10::IID, "ID3D12Device10"),
                (ID3D12Device9::IID, "ID3D12Device9"),
                (ID3D12Device8::IID, "ID3D12Device8"),
                (ID3D12Device7::IID, "ID3D12Device7"),
                (ID3D12Device6::IID, "ID3D12Device6"),
                (ID3D12Device5::IID, "ID3D12Device5"),
                (ID3D12Device4::IID, "ID3D12Device4"),
                (ID3D12Device3::IID, "ID3D12Device3"),
                (ID3D12Device2::IID, "ID3D12Device2"),
                (ID3D12Device1::IID, "ID3D12Device1"),
                (ID3D12Device::IID, "ID3D12Device"),
            ],
            Table::Queue => &[(ID3D12CommandQueue::IID, "ID3D12CommandQueue")],
            Table::List => &[
                (
                    ID3D12GraphicsCommandList10::IID,
                    "ID3D12GraphicsCommandList10",
                ),
                (
                    ID3D12GraphicsCommandList9::IID,
                    "ID3D12GraphicsCommandList9",
                ),
                (
                    ID3D12GraphicsCommandList8::IID,
                    "ID3D12GraphicsCommandList8",
                ),
                (
                    ID3D12GraphicsCommandList7::IID,
                    "ID3D12GraphicsCommandList7",
                ),
                (
                    ID3D12GraphicsCommandList6::IID,
                    "ID3D12GraphicsCommandList6",
                ),
                (
                    ID3D12GraphicsCommandList5::IID,
                    "ID3D12GraphicsCommandList5",
                ),
                (
                    ID3D12GraphicsCommandList4::IID,
                    "ID3D12GraphicsCommandList4",
                ),
                (
                    ID3D12GraphicsCommandList3::IID,
                    "ID3D12GraphicsCommandList3",
                ),
                (
                    ID3D12GraphicsCommandList2::IID,
                    "ID3D12GraphicsCommandList2",
                ),
                (
                    ID3D12GraphicsCommandList1::IID,
                    "ID3D12GraphicsCommandList1",
                ),
                (ID3D12GraphicsCommandList::IID, "ID3D12GraphicsCommandList"),
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

/// The census's settings, read from the environment on first use (a test sets them directly).
static CONFIG: OnceLock<Option<Config>> = OnceLock::new();

fn config() -> Option<&'static Config> {
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

pub fn note(text: String) {
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
                .map(|k| thunks + 64 * (t * CONTEXT_SLOTS + k))
                .collect();
            table[0] = wrapper_query as *const () as usize;
            table[get_data_slot()] = wrapper_get_data as *const () as usize;
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
    if counted() {
        WRAPPED_COUNTS[index * CONTEXT_SLOTS].fetch_add(1, Ordering::Relaxed);
    }
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

/// Where GetData sits in a context's method table.
fn get_data_slot() -> usize {
    let (_, interface) = Table::Immediate.chain()[0];
    methods(interface)
        .iter()
        .position(|n| *n == "GetData")
        .expect("a context has GetData")
}

/// GetData of a wrapper: the context's own answer, except that while the frame clock runs a
/// timestamp query is answered at once from that clock (in 100-nanosecond ticks, never disjoint),
/// so the timings and query latency a program shows are the same on every run. The context is
/// still asked, until it answers (flushing after the first ask), so the implementation's queries
/// run and must complete; an error it gives, or no answer within `QUERY_WAIT`, is reported.
unsafe extern "system" fn wrapper_get_data(
    this: *mut c_void,
    query: *mut c_void,
    data: *mut c_void,
    size: u32,
    flags: u32,
) -> HRESULT {
    let wrapper = unsafe { &*(this as *const Wrapper) };
    let index = usize::from(wrapper.vtable == wrapper_tables()[1].as_ptr());
    let k = get_data_slot();
    if counted() {
        WRAPPED_COUNTS[index * CONTEXT_SLOTS + k].fetch_add(1, Ordering::Relaxed);
    }
    let own: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, u32, u32) -> HRESULT =
        unsafe { std::mem::transmute(*(*(wrapper.context as *const *const usize)).add(k)) };
    let kind = if crate::clock::running() {
        unsafe { IUnknown::from_raw_borrowed(&query) }
            .and_then(|q| q.cast::<ID3D11Query>().ok())
            .map(|q| unsafe { q.GetDesc() }.Query)
    } else {
        None
    };
    let mut answer = [0u64; 2];
    let mut ask = |size: usize| {
        let since = interrupt_time();
        let mut flags = flags;
        let failure = loop {
            let result = unsafe {
                own(
                    wrapper.context,
                    query,
                    answer.as_mut_ptr().cast(),
                    size as u32,
                    flags,
                )
            };
            if result.is_err() {
                break format!("{result:?}");
            }
            if result == windows::Win32::Foundation::S_OK {
                return;
            }
            if interrupt_time() - since > QUERY_WAIT {
                break format!("not answered within {} s", QUERY_WAIT / 10_000_000);
            }
            flags = 0;
            std::thread::yield_now();
        };
        if !QUERY_ERROR.swap(true, Ordering::Relaxed) {
            eprintln!("d3d11 forward: GetData of a timestamp query failed: {failure}");
        }
    };
    match kind {
        Some(D3D11_QUERY_TIMESTAMP) if data.is_null() || size as usize >= size_of::<u64>() => {
            ask(size_of::<u64>());
            if !data.is_null() {
                unsafe { *(data as *mut u64) = crate::clock::now() };
            }
            windows::Win32::Foundation::S_OK
        }
        Some(D3D11_QUERY_TIMESTAMP_DISJOINT)
            if data.is_null()
                || size as usize >= size_of::<D3D11_QUERY_DATA_TIMESTAMP_DISJOINT>() =>
        {
            ask(size_of::<D3D11_QUERY_DATA_TIMESTAMP_DISJOINT>());
            if !data.is_null() {
                unsafe {
                    *(data as *mut D3D11_QUERY_DATA_TIMESTAMP_DISJOINT) =
                        D3D11_QUERY_DATA_TIMESTAMP_DISJOINT {
                            Frequency: crate::clock::FREQUENCY,
                            Disjoint: false.into(),
                        }
                };
            }
            windows::Win32::Foundation::S_OK
        }
        _ => unsafe { own(wrapper.context, query, data, size, flags) },
    }
}

/// Whether the context gave an error, or no answer, for a timestamp query answered from the frame
/// clock.
static QUERY_ERROR: AtomicBool = AtomicBool::new(false);
/// How long a timestamp query may take to be answered, in 100-nanosecond units.
const QUERY_WAIT: u64 = 5 * 10_000_000;

/// Real time in 100-nanosecond units, from a clock the frame clock does not replace.
fn interrupt_time() -> u64 {
    let mut time = 0;
    unsafe {
        windows_sys::Win32::System::WindowsProgramming::QueryUnbiasedInterruptTime(&mut time)
    };
    time
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
    // A copy of a patched table holds thunks and typed hooks of ours; each entry gets the function
    // they call in the end, so a call is counted once.
    let source = copied_from(vtable);
    VTABLES[instance].store(vtable, Ordering::Release);
    for (k, name) in names.iter().enumerate() {
        unsafe {
            let entry = *entries.add(k);
            let real = match (thunk_index(entry), source) {
                (Some(n), _) => ORIGINALS[n].load(Ordering::Acquire),
                (None, Some(source)) if typed(table, name) == Some(entry) => {
                    ORIGINALS[source * SLOTS + k].load(Ordering::Acquire)
                }
                _ => entry,
            };
            ORIGINALS[base + k].store(real, Ordering::Release);
            *entries.add(k) = typed(table, name).unwrap_or(thunks + THUNK * (base + k));
        }
    }
    unsafe { VirtualProtect(entries.cast(), names.len() * 8, old, &mut old) }.ok();
    patched.push((table, interface));
}

/// Which counting thunk an address is, if it is one.
fn thunk_index(address: usize) -> Option<usize> {
    let thunks = (&raw const wc_census_thunks) as usize;
    (thunks..thunks + THUNK * INSTANCES * SLOTS)
        .contains(&address)
        .then(|| (address - thunks) / THUNK)
}

/// The instance a method table was copied from, when it is a copy of one patched here: its
/// QueryInterface, never a typed hook, is then a thunk of that instance.
fn copied_from(vtable: usize) -> Option<usize> {
    thunk_index(unsafe { *(vtable as *const usize) }).map(|n| n / SLOTS)
}

/// Patches the table of `object` when it is a copy of one patched here, made after it was patched:
/// the display driver gives a Direct3D 12 command list its own copy of the runtime's table, with
/// its own functions for the draws and the state it sets, and the runtime's (ours) for the rest.
///
/// # Safety
/// `object` is null or a live COM object of that kind.
unsafe fn adopt(table: Table, object: *mut c_void) {
    if config().is_none() || object.is_null() {
        return;
    }
    let vtable = unsafe { *(object as *const usize) };
    if VTABLES.iter().any(|v| v.load(Ordering::Acquire) == vtable) {
        return;
    }
    if let Some(source) = copied_from(vtable) {
        let interface = PATCHED.lock().unwrap()[source].1;
        install(table, vtable, interface);
    }
}

/// Counts a call that came through a typed hook and returns the function it replaced. A copy of a
/// patched table that found no room left is not counted.
fn enter(this: *mut c_void, name: &str) -> usize {
    let vtable = unsafe { *(this as *const usize) };
    let patched = VTABLES
        .iter()
        .position(|v| v.load(Ordering::Acquire) == vtable);
    let instance = patched
        .or_else(|| copied_from(vtable))
        .expect("a typed hook is reached only through a patched table or a copy of one");
    let interface = PATCHED.lock().unwrap()[instance].1;
    let k = methods(interface).iter().position(|n| *n == name).unwrap();
    if patched.is_some() && counted() {
        COUNTS[instance * SLOTS + k].fetch_add(1, Ordering::Relaxed);
    }
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
        (Table::Device12, "CreateCommandQueue") => create_command_queue as *const () as usize,
        (Table::Device12, "CreateCommandQueue1") => create_command_queue1 as *const () as usize,
        (Table::List, "Reset") => list_reset as *const () as usize,
        (Table::List, "Close") => list_close as *const () as usize,
        (Table::Queue, "ExecuteCommandLists") => execute_command_lists as *const () as usize,
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
        unsafe {
            patch(Table::SwapChain, *out);
            remember_queue(device, *out);
        }
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
        unsafe {
            patch(Table::SwapChain, *out);
            remember_queue(device, *out);
        }
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
        unsafe {
            patch(Table::SwapChain, *out);
            remember_queue(device, *out);
        }
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
        unsafe {
            patch(Table::SwapChain, *out);
            remember_queue(device, *out);
        }
    }
    result
}

unsafe extern "system" fn create_command_queue(
    this: *mut c_void,
    desc: *const c_void,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    type Original = unsafe extern "system" fn(
        *mut c_void,
        *const c_void,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT;
    let original: Original = unsafe { std::mem::transmute(enter(this, "CreateCommandQueue")) };
    let result = unsafe { original(this, desc, riid, out) };
    if result.is_ok() && !out.is_null() {
        unsafe { patch(Table::Queue, *out) };
    }
    result
}

unsafe extern "system" fn create_command_queue1(
    this: *mut c_void,
    desc: *const c_void,
    creator: *const GUID,
    riid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    type Original = unsafe extern "system" fn(
        *mut c_void,
        *const c_void,
        *const GUID,
        *const GUID,
        *mut *mut c_void,
    ) -> HRESULT;
    let original: Original = unsafe { std::mem::transmute(enter(this, "CreateCommandQueue1")) };
    let result = unsafe { original(this, desc, creator, riid, out) };
    if result.is_ok() && !out.is_null() {
        unsafe { patch(Table::Queue, *out) };
    }
    result
}

/// Calls of the typed command list and queue hooks that came from the graphics libraries themselves
/// rather than the program, by table and method: Microsoft's DXGI resets, closes and executes a
/// command list of its own on the program's queue at each present. They are counted with the
/// program's.
static BY_LIBRARIES: Mutex<Vec<(Table, &'static str, u64)>> = Mutex::new(Vec::new());

/// Notes a call of a typed hook made by the graphics libraries, when it came from one.
#[inline(never)]
fn by_libraries(table: Table, name: &'static str) {
    if !counted() || !from_libraries() {
        return;
    }
    let mut calls = BY_LIBRARIES.lock().unwrap();
    match calls.iter_mut().find(|(t, n, _)| *t == table && *n == name) {
        Some((_, _, count)) => *count += 1,
        None => calls.push((table, name, 1)),
    }
}

/// Whether the typed hook that calls `by_libraries` was called from a graphics library rather than
/// the program: a module in the system directory, or one named as a Direct3D or DXGI library
/// (d3d11.dll, d3d12.dll, d3d12core.dll, dxgi.dll) wherever it was loaded from, as a replacement
/// or the Agility SDK's runtime beside the program is; never this library.
#[inline(never)]
fn from_libraries() -> bool {
    use windows::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;
    use windows_sys::Win32::System::LibraryLoader::*;
    let mut frames = [std::ptr::null_mut(); 1];
    // Frame 0 is this function, frame 1 by_libraries and frame 2 the hook: frame 3 is where the
    // hook was called from.
    if unsafe { RtlCaptureStackBackTrace(3, &mut frames, None) } != 1 {
        return false;
    }
    let mut module = std::ptr::null_mut();
    let found = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            frames[0].cast(),
            &mut module,
        )
    };
    if found == 0 {
        return false;
    }
    static MODULES: Mutex<Vec<(usize, bool)>> = Mutex::new(Vec::new());
    let mut modules = MODULES.lock().unwrap();
    if let Some((_, library)) = modules.iter().find(|(m, _)| *m == module as usize) {
        return *library;
    }
    let mut own = std::ptr::null_mut();
    let found = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            (from_libraries as *const ()).cast(),
            &mut own,
        )
    };
    let path = |get: &dyn Fn(&mut [u16]) -> u32| {
        let mut buffer = [0u16; 1024];
        let len = get(&mut buffer) as usize;
        String::from_utf16_lossy(&buffer[..len.min(buffer.len())]).to_lowercase()
    };
    let file = path(&|b| unsafe { GetModuleFileNameW(module, b.as_mut_ptr(), b.len() as u32) });
    let system = path(&|b| unsafe {
        windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
            b.as_mut_ptr(),
            b.len() as u32,
        )
    });
    let name = file.rsplit('\\').next().unwrap_or_default();
    let library = found != 0
        && own != module
        && ((!system.is_empty() && file.starts_with(&format!("{system}\\")))
            || ["d3d11.dll", "d3d12.dll", "d3d12core.dll", "dxgi.dll"].contains(&name));
    modules.push((module as usize, library));
    library
}

/// Reset and Close of a command list: the display driver gives a list its own copy of the table at
/// its first Reset, which is patched before the program's next call.
unsafe extern "system" fn list_reset(
    this: *mut c_void,
    allocator: *mut c_void,
    state: *mut c_void,
) -> HRESULT {
    by_libraries(Table::List, "Reset");
    unsafe { adopt(Table::List, this) };
    let original: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HRESULT =
        unsafe { std::mem::transmute(enter(this, "Reset")) };
    let result = unsafe { original(this, allocator, state) };
    unsafe { adopt(Table::List, this) };
    result
}

unsafe extern "system" fn list_close(this: *mut c_void) -> HRESULT {
    by_libraries(Table::List, "Close");
    unsafe { adopt(Table::List, this) };
    let original: unsafe extern "system" fn(*mut c_void) -> HRESULT =
        unsafe { std::mem::transmute(enter(this, "Close")) };
    let result = unsafe { original(this) };
    unsafe { adopt(Table::List, this) };
    result
}

/// ExecuteCommandLists: counts the lists executed whose table was not counted then (made in a way
/// this library does not see), so the census says how many lists' calls it may be missing, and
/// patches their tables if they are copies of a patched one.
unsafe extern "system" fn execute_command_lists(
    this: *mut c_void,
    count: u32,
    lists: *const *mut c_void,
) {
    by_libraries(Table::Queue, "ExecuteCommandLists");
    let original: unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void) =
        unsafe { std::mem::transmute(enter(this, "ExecuteCommandLists")) };
    if !lists.is_null() {
        for k in 0..count as usize {
            let list = unsafe { *lists.add(k) };
            let table = if list.is_null() {
                0
            } else {
                unsafe { *(list as *const usize) }
            };
            if !VTABLES.iter().any(|v| v.load(Ordering::Acquire) == table) {
                if counted() {
                    OTHER_LISTS.fetch_add(1, Ordering::Relaxed);
                }
                unsafe { adopt(Table::List, list) };
            }
        }
    }
    unsafe { original(this, count, lists) }
}

/// Command lists executed whose table was not counted then.
static OTHER_LISTS: AtomicU64 = AtomicU64::new(0);

/// Patches the command list tables of `device` from lists made here, of each kind, before the
/// program records any; the calls made doing so are not counted.
fn patch_lists(device: &ID3D12Device) -> windows::core::Result<()> {
    uncounted(|| {
        for kind in [
            D3D12_COMMAND_LIST_TYPE_DIRECT,
            D3D12_COMMAND_LIST_TYPE_BUNDLE,
            D3D12_COMMAND_LIST_TYPE_COMPUTE,
            D3D12_COMMAND_LIST_TYPE_COPY,
        ] {
            let allocator: ID3D12CommandAllocator = unsafe { device.CreateCommandAllocator(kind)? };
            let list: ID3D12GraphicsCommandList =
                unsafe { device.CreateCommandList(0, kind, &allocator, None)? };
            unsafe { patch(Table::List, list.as_raw()) };
        }
        Ok(())
    })
}

/// A COM object's identity: its IUnknown pointer.
fn identity(object: *mut c_void) -> Option<usize> {
    let unknown = unsafe { IUnknown::from_raw_borrowed(&object) }?;
    unknown.cast::<IUnknown>().ok().map(|u| u.as_raw() as usize)
}

/// Keeps the queue a swap chain was made on, when the factory was given a Direct3D 12 queue.
///
/// # Safety
/// `device` and `chain` are live COM objects.
unsafe fn remember_queue(device: *mut c_void, chain: *mut c_void) {
    if config().is_none() || device.is_null() || chain.is_null() {
        return;
    }
    let Some(queue) = unsafe { IUnknown::from_raw_borrowed(&device) }
        .and_then(|d| d.cast::<ID3D12CommandQueue>().ok())
    else {
        return;
    };
    if let Some(id) = identity(chain) {
        QUEUES.lock().unwrap().push((id, queue));
    }
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

/// Called for a Direct3D 12 device the program created through this library: patches the device and
/// the DXGI factory's table (one made here: every factory of the process shares its class's table),
/// so the swap chains the program makes on its queues are patched in turn.
///
/// # Safety
/// `device` is a live device.
pub unsafe fn created12(device: *mut c_void) {
    if config().is_none() || device.is_null() {
        return;
    }
    match unsafe { CreateDXGIFactory2::<IDXGIFactory2>(DXGI_CREATE_FACTORY_FLAGS(0)) } {
        Ok(factory) => unsafe { patch(Table::Factory, factory.as_raw()) },
        Err(error) => note(format!("factory not counted: {error}")),
    }
    let lists = unsafe { IUnknown::from_raw_borrowed(&device) }
        .unwrap()
        .cast::<ID3D12Device>()
        .and_then(|d| patch_lists(&d));
    if let Err(error) = lists {
        note(format!("command lists counted only once executed: {error}"));
    }
    unsafe { patch(Table::Device12, device) };
}

/// Counts a present and saves or ends at the frames WC_CAPTURE names.
fn frame(chain: *mut c_void) {
    let frame = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    crate::clock::frame();
    let Some(config) = config() else { return };
    if config.capture.binary_search(&frame).is_err() {
        return;
    }
    // Calls made while saving the frame are not the program's: they are not counted.
    if let Err(error) = uncounted(|| save(chain, frame, config)) {
        note(format!("frame {frame} not saved: {error:#}"));
    }
    if config.capture.last() == Some(&frame) {
        write();
        let _ = unsafe { TerminateProcess(GetCurrentProcess(), 0) };
    }
}

fn save(chain: *mut c_void, frame: u64, config: &Config) -> anyhow::Result<()> {
    let chain = unsafe { IDXGISwapChain::from_raw_borrowed(&chain) }.unwrap();
    let queue = identity(chain.as_raw()).and_then(|id| {
        let queues = QUEUES.lock().unwrap();
        queues
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, q)| q.clone())
    });
    let (width, height, format, mut pixels) = match queue {
        Some(queue) => read12(chain, &queue)?,
        None => read11(chain)?,
    };
    let (bgra, wide) = layout(format)?;
    let (pixels, depth) = if wide {
        // Bits 0-9 red, 10-19 green, 20-29 blue, 30-31 alpha; widened by repeating the top bits.
        let widen = |v: u32| ((v << 6) | (v >> 4)) as u16;
        let wide: Vec<u8> = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|texel| {
                let v = u32::from_le_bytes(*texel);
                [
                    widen(v & 0x3ff),
                    widen((v >> 10) & 0x3ff),
                    widen((v >> 20) & 0x3ff),
                    (v >> 30) as u16 * 0x5555,
                ]
            })
            .flat_map(u16::to_be_bytes)
            .collect();
        (wide, png::BitDepth::Sixteen)
    } else {
        for texel in pixels.as_chunks_mut::<4>().0 {
            if bgra {
                texel.swap(0, 2);
            }
            if format == DXGI_FORMAT_B8G8R8X8_UNORM {
                texel[3] = 255;
            }
        }
        (pixels, png::BitDepth::Eight)
    };
    std::fs::create_dir_all(&config.out)?;
    let file = std::fs::File::create(config.out.join(format!("frame-{frame}.png")))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(depth);
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}

/// Whether a back buffer format `save` takes (each 4 bytes a texel) is blue first, and whether it
/// has 10-bit colour, saved at 16 bits a channel.
fn layout(format: DXGI_FORMAT) -> anyhow::Result<(bool, bool)> {
    Ok(match format {
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => (false, false),
        DXGI_FORMAT_B8G8R8A8_UNORM
        | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
        | DXGI_FORMAT_B8G8R8X8_UNORM => (true, false),
        DXGI_FORMAT_R10G10B10A2_UNORM => (false, true),
        other => anyhow::bail!("the back buffer's format {} is not one this saves", other.0),
    })
}

/// A Direct3D 12 swap chain's current back buffer, copied on its own queue after the program's
/// work and waited for.
fn read12(
    chain: &IDXGISwapChain,
    queue: &ID3D12CommandQueue,
) -> anyhow::Result<(u32, u32, DXGI_FORMAT, Vec<u8>)> {
    use std::mem::{ManuallyDrop, transmute_copy};
    let index = unsafe { chain.cast::<IDXGISwapChain3>()?.GetCurrentBackBufferIndex() };
    let back: ID3D12Resource = unsafe { chain.GetBuffer(index)? };
    let desc = unsafe { back.GetDesc() };
    layout(desc.Format)?;
    anyhow::ensure!(
        desc.SampleDesc.Count == 1,
        "the back buffer is multisampled"
    );
    let mut device: Option<ID3D12Device> = None;
    unsafe { queue.GetDevice(&mut device)? };
    let device = device.unwrap();
    let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
    let mut total = 0u64;
    unsafe {
        device.GetCopyableFootprints(
            &desc,
            0,
            1,
            0,
            Some(&mut footprint),
            None,
            None,
            Some(&mut total),
        )
    };
    let buffer = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Width: total,
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
    let heap = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_READBACK,
        ..Default::default()
    };
    let mut readback: Option<ID3D12Resource> = None;
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            &buffer,
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
            &mut readback,
        )?
    };
    let readback = readback.unwrap();
    let kind = unsafe { queue.GetDesc() }.Type;
    let allocator: ID3D12CommandAllocator = unsafe { device.CreateCommandAllocator(kind)? };
    let list: ID3D12GraphicsCommandList =
        unsafe { device.CreateCommandList(0, kind, &allocator, None)? };
    // The back buffer is in the present state when presented; it goes back to it after the copy.
    let barrier = |before, after| D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: unsafe { transmute_copy(&back) },
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    };
    /// A copy location holding a reference of its own to its resource (an AddRef), given back when
    /// it drops.
    struct Location(D3D12_TEXTURE_COPY_LOCATION);
    impl Drop for Location {
        fn drop(&mut self) {
            let held = std::mem::replace(&mut self.0.pResource, ManuallyDrop::new(None));
            drop(ManuallyDrop::into_inner(held));
        }
    }
    let to = Location(D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(readback.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            PlacedFootprint: footprint,
        },
    });
    let from = Location(D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(back.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
            SubresourceIndex: 0,
        },
    });
    let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
    unsafe {
        list.ResourceBarrier(&[barrier(
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        list.CopyTextureRegion(&to.0, 0, 0, 0, &from.0, None);
        list.ResourceBarrier(&[barrier(
            D3D12_RESOURCE_STATE_COPY_SOURCE,
            D3D12_RESOURCE_STATE_PRESENT,
        )]);
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast()?)]);
        queue.Signal(&fence, 1)?;
        // Without an event the call returns once the fence has reached the value.
        fence.SetEventOnCompletion(1, windows::Win32::Foundation::HANDLE::default())?;
    }
    let (width, height) = (desc.Width as u32, desc.Height);
    let row = width as usize * 4;
    let mut pixels = vec![0u8; row * height as usize];
    unsafe {
        let mut mapped = std::ptr::null_mut();
        readback.Map(
            0,
            Some(&D3D12_RANGE {
                Begin: 0,
                End: total as usize,
            }),
            Some(&mut mapped),
        )?;
        let pitch = footprint.Footprint.RowPitch as usize;
        for y in 0..height as usize {
            let source = std::slice::from_raw_parts(
                mapped
                    .cast::<u8>()
                    .add(footprint.Offset as usize + y * pitch),
                row,
            );
            pixels[y * row..(y + 1) * row].copy_from_slice(source);
        }
        readback.Unmap(0, Some(&D3D12_RANGE::default()));
    }
    Ok((width, height, desc.Format, pixels))
}

/// A Direct3D 11 swap chain's back buffer, copied and read on its device's immediate context.
fn read11(chain: &IDXGISwapChain) -> anyhow::Result<(u32, u32, DXGI_FORMAT, Vec<u8>)> {
    let back: ID3D11Texture2D = unsafe { chain.GetBuffer(0)? };
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { back.GetDesc(&mut desc) };
    layout(desc.Format)?;
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
    Ok((desc.Width, desc.Height, desc.Format, pixels))
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
    let other = OTHER_LISTS.load(Ordering::Relaxed);
    if other > 0 {
        let _ = writeln!(
            text,
            "# {other} command lists executed with a table not counted then (made in a way not seen): their calls before are not counted"
        );
    }
    let mut libraries: Vec<String> = BY_LIBRARIES
        .lock()
        .unwrap()
        .iter()
        .map(|(table, name, count)| format!("{} {name} {count}", table.label()))
        .collect();
    libraries.sort();
    if !libraries.is_empty() {
        let _ = writeln!(
            text,
            "# of the counts below, made by the graphics libraries, not the program: {}",
            libraries.join(", ")
        );
    }
    for note in NOTES.lock().unwrap().iter() {
        let _ = writeln!(text, "# {note}");
    }
    for export in crate::UNFORWARDED {
        let _ = writeln!(
            text,
            "# not forwarded, so absent from this library: {export}"
        );
    }
    // Entries the runtime changed after they were patched: calls through them went uncounted.
    let thunks = (&raw const wc_census_thunks) as usize;
    for (instance, (table, interface)) in patched.iter().enumerate() {
        let entries = VTABLES[instance].load(Ordering::Acquire) as *const usize;
        let changed: Vec<&str> = methods(interface)
            .iter()
            .enumerate()
            .filter(|(k, name)| {
                let ours = typed(*table, name).unwrap_or(thunks + THUNK * (instance * SLOTS + k));
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
        let _ = writeln!(text, "{name} {count}");
    }
    let _ = std::fs::create_dir_all(&config.out)
        .and_then(|()| std::fs::write(config.out.join("census.txt"), text));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A method table of distinct stand-in functions, never called, and an object using it.
    fn table(interface: &str, base: usize) -> &'static mut [usize] {
        let entries: Vec<usize> = (0..methods(interface).len())
            .map(|k| base + 16 * k)
            .collect();
        Box::leak(entries.into_boxed_slice())
    }

    fn object(table: &[usize]) -> *mut c_void {
        Box::leak(Box::new(table.as_ptr() as usize)) as *mut usize as *mut c_void
    }

    fn instance(table: &[usize]) -> usize {
        let vtable = table.as_ptr() as usize;
        VTABLES
            .iter()
            .position(|v| v.load(Ordering::Acquire) == vtable)
            .expect("patched")
    }

    /// Every counting and wrapper thunk has the length the census steps by: each starts with the
    /// read of the calling thread's id.
    #[test]
    fn thunks_are_evenly_spaced() {
        let start = [0x65, 0x4c, 0x8b, 0x1c, 0x25, 0x48, 0, 0, 0];
        let census = (&raw const wc_census_thunks) as usize;
        for n in [0, 1, INSTANCES * SLOTS - 1] {
            let bytes = unsafe { std::slice::from_raw_parts((census + THUNK * n) as *const u8, 9) };
            assert_eq!(bytes, start, "census thunk {n}");
        }
        for (t, table) in wrapper_tables().iter().enumerate() {
            // Entry 0 and GetData are methods of this library, not thunks.
            for k in [1, CONTEXT_SLOTS - 1] {
                let bytes = unsafe { std::slice::from_raw_parts(table[k] as *const u8, 9) };
                assert_eq!(bytes, start, "wrapper thunk {t} {k}");
            }
        }
    }

    /// A call through a counting thunk is counted on every thread but the one making the census's
    /// own calls, while it makes them.
    #[test]
    fn the_census_own_calls_are_not_counted() {
        static CALLED: AtomicU64 = AtomicU64::new(0);
        extern "system" fn target(x: u64) -> u64 {
            CALLED.fetch_add(1, Ordering::Relaxed);
            x + 1
        }
        // The last thunk, which no other test patches.
        let n = INSTANCES * SLOTS - 1;
        ORIGINALS[n].store(target as *const () as usize, Ordering::Release);
        let address = (&raw const wc_census_thunks) as usize + THUNK * n;
        let thunk: extern "system" fn(u64) -> u64 = unsafe { std::mem::transmute(address) };
        assert_eq!(thunk(1), 2);
        uncounted(|| {
            assert_eq!(thunk(2), 3);
            assert_eq!(std::thread::spawn(move || thunk(3)).join().unwrap(), 4);
        });
        assert_eq!(thunk(4), 5);
        assert_eq!(CALLED.load(Ordering::Relaxed), 4);
        assert_eq!(COUNTS[n].load(Ordering::Relaxed), 3);
    }

    /// The display driver's copy of a patched command list table: its own functions for some
    /// entries, the patched ones (thunks and typed hooks) for the rest. Patching the copy gives each
    /// entry the function it calls in the end, so every call is counted once, on the copy.
    #[test]
    fn a_copy_of_a_patched_table_counts_each_call_once() {
        // The census is on, as with WC_OUT set: set before any census call reads it, in the one
        // test that uses it.
        let _ = CONFIG.set(Some(Config {
            out: std::env::temp_dir(),
            capture: Vec::new(),
        }));
        let interface = "ID3D12GraphicsCommandList";
        let names = methods(interface);
        let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
        let runtime = table(interface, 0x10_0000);
        let real = runtime.to_vec();
        install(Table::List, runtime.as_ptr() as usize, interface);
        let source = instance(runtime);
        assert_eq!(runtime[at("Reset")], list_reset as *const () as usize);
        assert!(thunk_index(runtime[at("DrawInstanced")]).is_some());

        let copy: &'static mut [usize] = Box::leak(runtime.to_vec().into_boxed_slice());
        copy[at("DrawInstanced")] = 0x20_0000;
        let list = object(copy);
        // Not yet patched: a typed hook reached through the copy calls the runtime's function
        // and counts nothing.
        assert_eq!(enter(list, "Reset"), real[at("Reset")]);
        assert_eq!(
            COUNTS[source * SLOTS + at("Reset")].load(Ordering::Relaxed),
            0
        );

        unsafe { adopt(Table::List, list) };
        let patched = instance(copy);
        assert_ne!(patched, source);
        for (k, name) in names.iter().enumerate() {
            let want = if *name == "DrawInstanced" {
                0x20_0000
            } else {
                real[k]
            };
            assert_eq!(
                ORIGINALS[patched * SLOTS + k].load(Ordering::Acquire),
                want,
                "{name}"
            );
            let entry = copy[k];
            assert!(
                typed(Table::List, name) == Some(entry)
                    || thunk_index(entry) == Some(patched * SLOTS + k),
                "{name}"
            );
        }
        assert_eq!(enter(list, "Reset"), real[at("Reset")]);
        assert_eq!(
            COUNTS[patched * SLOTS + at("Reset")].load(Ordering::Relaxed),
            1
        );
        assert_eq!(
            COUNTS[source * SLOTS + at("Reset")].load(Ordering::Relaxed),
            0
        );
        // A table that is no copy of a patched one is left alone.
        let other = table(interface, 0x30_0000);
        unsafe { adopt(Table::List, object(other)) };
        assert_eq!(other[0], 0x30_0000);
    }
}
