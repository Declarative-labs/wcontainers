//! What the harness keeps once for the whole process, whichever guest's module asks for it.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use wasmtime::Engine;

/// Returns a file name no other build or run, in this process or another, uses.
pub fn unique(stem: &str) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{stem}.{}.{n}", std::process::id())
}

/// Returns the process's one engine, made by `make` the first time, so compiled modules can be shared.
pub fn engine(make: impl FnOnce() -> Engine) -> Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(make).clone()
}

/// Returns where Csmith's headers were copied, copied by `copy` the first time.
pub fn csmith_include(copy: impl FnOnce() -> PathBuf) -> PathBuf {
    static INCLUDE: OnceLock<PathBuf> = OnceLock::new();
    INCLUDE.get_or_init(copy).clone()
}
