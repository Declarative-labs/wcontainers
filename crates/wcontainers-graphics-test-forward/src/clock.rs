//! The clock a program sees while its frames are captured, so that a program animated by the clock
//! draws the same frames on every run. It starts at the next whole second of the real clocks and
//! moves only when the program presents a frame (by 1/60 s) or sleeps (by the time asked for).
//! The functions programs read time through (the performance counter, tick counts, the system time
//! and winmm's timeGetTime if loaded) are detoured to it in the process's own system libraries, so
//! every caller sees it, whichever module calls and however it found the function.

use std::cell::Cell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use retour::RawDetour;
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::SystemInformation::{GetSystemTimeAsFileTime, GetTickCount64};

/// 100-nanosecond units in a second, Windows' unit of time.
const SECOND: u64 = 10_000_000;
/// How far each presented frame moves the clock.
const FRAME: u64 = SECOND / 60;

/// Time passed on this clock, in 100-nanosecond units.
static ELAPSED: AtomicU64 = AtomicU64::new(0);
static START: OnceLock<Start> = OnceLock::new();
/// The original SleepEx, called by the sleep detours.
static SLEEP_EX: OnceLock<usize> = OnceLock::new();

thread_local! {
    /// Set while a Sleep detour runs, so the SleepEx it may call does not count the time again.
    static SLEEPING: Cell<bool> = const { Cell::new(false) };
}

/// The real clocks' readings the clock starts from, each rounded up to a whole second.
struct Start {
    frequency: u64,
    counter: u64,
    ticks: u64,
    file_time: u64,
}

fn round_up(value: u64, unit: u64) -> u64 {
    value.div_ceil(unit) * unit
}

/// Starts the clock and detours the time functions to it. Returns what could not be detoured.
pub fn install() -> Vec<String> {
    let mut frequency = 0;
    let mut counter = 0;
    let mut file_time = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    unsafe {
        QueryPerformanceFrequency(&mut frequency);
        QueryPerformanceCounter(&mut counter);
        GetSystemTimeAsFileTime(&mut file_time);
    }
    let frequency = frequency as u64;
    let file_time =
        (u64::from(file_time.dwHighDateTime) << 32) | u64::from(file_time.dwLowDateTime);
    if START
        .set(Start {
            frequency,
            counter: round_up(counter as u64, frequency),
            ticks: round_up(unsafe { GetTickCount64() }, 1000),
            file_time: round_up(file_time, SECOND),
        })
        .is_err()
    {
        return Vec::new();
    }
    let detours: [(&[&str], &str, *const ()); 9] = [
        (
            &["ntdll.dll"],
            "RtlQueryPerformanceCounter",
            counter_now as *const (),
        ),
        (
            &["kernel32.dll", "kernelbase.dll"],
            "GetTickCount",
            ticks32 as *const (),
        ),
        (
            &["kernel32.dll", "kernelbase.dll"],
            "GetTickCount64",
            ticks64 as *const (),
        ),
        (
            &["winmm.dll", "winmmbase.dll"],
            "timeGetTime",
            ticks32 as *const (),
        ),
        (
            &["kernel32.dll", "kernelbase.dll"],
            "GetSystemTimeAsFileTime",
            system_time as *const (),
        ),
        (
            &["kernel32.dll", "kernelbase.dll"],
            "GetSystemTimePreciseAsFileTime",
            system_time as *const (),
        ),
        (&["kernelbase.dll"], "SleepEx", sleep_ex as *const ()),
        (&["kernelbase.dll"], "Sleep", sleep as *const ()),
        (&["kernel32.dll"], "Sleep", sleep as *const ()),
    ];
    let mut missed = Vec::new();
    let mut done: Vec<usize> = Vec::new();
    for (modules, name, detour) in detours {
        let mut found = false;
        for module in modules {
            let wide: Vec<u16> = module.encode_utf16().chain([0]).collect();
            let handle = unsafe { GetModuleHandleW(wide.as_ptr()) };
            if handle.is_null() {
                continue;
            }
            let symbol: Vec<u8> = name.bytes().chain([0]).collect();
            let Some(target) = (unsafe { GetProcAddress(handle, symbol.as_ptr()) }) else {
                continue;
            };
            found = true;
            let target = target as usize;
            // One function may sit behind several names (kernel32's export forwarded to kernelbase).
            if done.contains(&target) {
                continue;
            }
            match unsafe { RawDetour::new(target as *const (), detour) }
                .and_then(|d| unsafe { d.enable() }.map(|()| d))
            {
                Ok(d) => {
                    if name == "SleepEx" {
                        let _ = SLEEP_EX.set(d.trampoline() as *const () as usize);
                    }
                    done.push(target);
                    // Kept for the life of the process.
                    std::mem::forget(d);
                }
                Err(e) => missed.push(format!("{module} {name}: {e}")),
            }
        }
        if !found && name != "timeGetTime" {
            missed.push(format!("{name}: not found"));
        }
    }
    missed
}

/// Moves the clock on by one presented frame.
pub fn frame() {
    ELAPSED.fetch_add(FRAME, Ordering::Relaxed);
}

fn start() -> &'static Start {
    START
        .get()
        .expect("the clock is started before it is detoured to")
}

fn elapsed() -> u64 {
    ELAPSED.load(Ordering::Relaxed)
}

unsafe extern "system" fn counter_now(counter: *mut i64) -> u32 {
    let s = start();
    let ticks = u128::from(elapsed()) * u128::from(s.frequency) / u128::from(SECOND);
    unsafe { *counter = (s.counter + ticks as u64) as i64 };
    1
}

unsafe extern "system" fn ticks64() -> u64 {
    start().ticks + elapsed() / 10_000
}

unsafe extern "system" fn ticks32() -> u32 {
    unsafe { ticks64() as u32 }
}

unsafe extern "system" fn system_time(time: *mut FILETIME) {
    let t = start().file_time + elapsed();
    unsafe {
        *time = FILETIME {
            dwLowDateTime: t as u32,
            dwHighDateTime: (t >> 32) as u32,
        }
    };
}

/// Moves the clock on by a sleep of `ms` milliseconds (INFINITE moves it not at all).
fn slept(ms: u32) {
    if ms != u32::MAX {
        ELAPSED.fetch_add(u64::from(ms) * 10_000, Ordering::Relaxed);
    }
}

unsafe extern "system" fn sleep_ex(ms: u32, alertable: i32) -> u32 {
    if !SLEEPING.get() {
        slept(ms);
    }
    let original: unsafe extern "system" fn(u32, i32) -> u32 =
        unsafe { std::mem::transmute(*SLEEP_EX.get().expect("SleepEx is detoured before Sleep")) };
    unsafe { original(ms, alertable) }
}

unsafe extern "system" fn sleep(ms: u32) {
    slept(ms);
    SLEEPING.set(true);
    unsafe { sleep_ex(ms, 0) };
    SLEEPING.set(false);
}
