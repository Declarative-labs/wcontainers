//! EGL through its C entry points: the library loaded from the folder the caller names, every handle a value of the
//! glue's own type that safe code cannot forge, and every entry point a safe method (calls.rs, generated). The
//! OpenGL ES glue reaches a context through `Binding`.
//!
//! What the marks rely on: the library at the path is EGL (its exports have egl.xml's signatures); EGL's calls are
//! thread-safe (EGL 1.5 §3.1); a handle EGL made stays valid until its destroy call, which the glue makes only while no
//! other call of this library is in flight (the gate), and never after; a context current on a thread is destroyed
//! only once it is released (EGL 1.5 §3.7.2).
#![cfg(not(target_arch = "wasm32"))]

#[path = "calls.rs"]
mod calls;

use core::ffi::{CStr, c_void};
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use wcontainers_egl::constants;

/// The file names an EGL library has, tried in order in the folder named.
const FILE_NAMES: &[&str] = &["libEGL.so.1", "libEGL.so", "libEGL.dll", "libEGL.dylib"];

/// An EGL handle as the address EGL gave, so that the glue's types stay `Send` and `Sync`.
type Address = usize;

mod runtime {
    use core::cell::Cell;
    use core::ffi::{CStr, c_void};

    use wcontainers_egl::constants;

    std::thread_local! {
        /// The error of the glue's last refusal on this thread, reported by the next `get_error` before EGL's.
        static REFUSED: Cell<i32> = const { Cell::new(constants::SUCCESS) };
        /// How many calls the glue itself refused on this thread.
        static REFUSALS: Cell<u64> = const { Cell::new(0) };
    }

    /// Records a refusal, as EGL records its own errors per thread.
    pub(crate) fn refuse(error: i32) {
        REFUSALS.with(|refusals| refusals.set(refusals.get().saturating_add(1)));
        REFUSED.with(|refused| refused.set(error));
    }

    /// How many calls the glue itself refused on this thread.
    pub(crate) fn refusals() -> u64 {
        REFUSALS.with(Cell::get)
    }

    /// Returns and clears the last refusal.
    pub(crate) fn take_refusal() -> Option<i32> {
        REFUSED.with(|refused| Some(refused.replace(constants::SUCCESS)).filter(|&error| error != constants::SUCCESS))
    }

    /// The count C is told for a slice: its length over `by`, refused when that is not exact or does not fit.
    pub(crate) fn counted<T: TryFrom<usize>>(length: usize, by: usize) -> Option<T> {
        if by == 0 || !length.is_multiple_of(by) {
            return None;
        }
        T::try_from(length / by).ok()
    }

    /// An attribute list as EGL reads it: pairs up to the first key `EGL_NONE`, each forced pair in place of the
    /// caller's, then `EGL_NONE`; None when a key has no value.
    pub(crate) fn attribute_list(list: &[i32], forced: &[(i32, i32)]) -> Option<Vec<i32>> {
        let mut pairs: Vec<(i32, i32)> = Vec::new();
        let mut rest = list;
        loop {
            match rest {
                [] => break,
                [key, ..] if *key == constants::NONE => break,
                [key, value, more @ ..] => {
                    pairs.push((*key, *value));
                    rest = more;
                }
                [_] => return None,
            }
        }
        pairs.retain(|(key, _)| !forced.iter().any(|(forced_key, _)| forced_key == key));
        pairs.extend_from_slice(forced);
        let mut out: Vec<i32> = pairs.into_iter().flat_map(|(key, value)| [key, value]).collect();
        out.push(constants::NONE);
        Some(out)
    }

    /// Copies a text the library keeps (eglQueryString's), or None for null.
    pub(crate) fn copied_text(text: *const u8) -> Option<Vec<u8>> {
        if text.is_null() {
            return None;
        }
        #[expect(unsafe_code, reason = "egl.xml: eglQueryString returns a zero-ended text the library keeps for the display's life, or null")]
        // SAFETY: not null, so the library's zero-ended text, which lives as long as the display, which is never terminated.
        let text = unsafe { CStr::from_ptr(text.cast()) };
        Some(text.to_bytes().to_vec())
    }

    /// The address of a pointer EGL gave, kept as a number.
    pub(crate) fn address_of(pointer: *mut c_void) -> usize {
        pointer.expose_provenance()
    }
}

/// The library loaded, its entry points, and the gate that keeps a destroy from running beside any other call.
struct Loaded {
    entries: calls::Entries,
    /// Every call holds this for reading while it runs; a destroy holds it for writing.
    gate: RwLock<()>,
    /// Kept loaded while any handle, binding or library value of it lives.
    _library: libloading::Library,
}

/// The first library loaded from each folder, kept for the process: the system unloads a library with its last
/// handle, and Mesa's then unloads its driver too and loads it again for the next context (about 55 ms a context,
/// MEASURED by the WebGL back end). Every later load of the folder is a library of its own (its handles are its own)
/// that finds the code still loaded.
static KEPT: Mutex<Vec<(Option<std::path::PathBuf>, Arc<Loaded>)>> = Mutex::new(Vec::new());

/// Keeps the folder's first library loaded for the process.
fn keep(folder: Option<&std::path::Path>, loaded: &Arc<Loaded>) {
    let mut kept = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !kept.iter().any(|(kept, _)| kept.as_deref() == folder) {
        kept.push((folder.map(std::path::Path::to_path_buf), Arc::clone(loaded)));
    }
}

/// EGL's library, loaded once and shared by every clone.
#[derive(Clone)]
pub struct Library {
    loaded: Arc<Loaded>,
}

impl Library {
    fn open(folder: Option<&std::path::Path>) -> Option<Library> {
        let library = FILE_NAMES.iter().find_map(|name| {
            let path = folder.map_or_else(|| std::path::PathBuf::from(name), |folder| folder.join(name));
            #[expect(unsafe_code, reason = "loading the EGL library the caller names: its initializers run, and the library is EGL, whose exports have egl.xml's signatures")]
            // SAFETY: the caller names the folder of an EGL library (ANGLE's, or the system's when none); its initializers
            // are EGL's own.
            unsafe { libloading::Library::new(&path) }.ok()
        })?;
        let entries = calls::Entries::load(|name: &CStr| {
            #[expect(unsafe_code, reason = "looking up an export of the EGL library by name; the address is only read as the entry egl.xml declares")]
            // SAFETY: the symbol is read as a data address, never called here; calls.rs types it by egl.xml.
            let symbol = unsafe { library.get::<unsafe extern "system" fn()>(name.to_bytes_with_nul()) };
            symbol.map_or(core::ptr::null(), |symbol| *symbol as *const c_void)
        });
        let loaded = Arc::new(Loaded { entries, gate: RwLock::new(()), _library: library });
        keep(folder, &loaded);
        let library = Library { loaded };
        // Every client API entry point must come by name (EGL 1.5, or EGL_KHR_client_get_all_proc_addresses).
        let extensions = {
            let _gate = library.reading();
            let query = library.entries().query_string?;
            #[expect(unsafe_code, reason = "egl.xml: eglQueryString(EGL_NO_DISPLAY, EGL_EXTENSIONS) reads nothing of ours and returns the client extensions or null")]
            // SAFETY: no pointer of ours is passed.
            let text = unsafe { query(core::ptr::null_mut(), constants::EXTENSIONS) };
            runtime::copied_text(text).unwrap_or_default()
        };
        let names_all = extensions.split(|&byte| byte == b' ').any(|name| name == b"EGL_KHR_client_get_all_proc_addresses");
        (names_all && library.entries().get_proc_address.is_some() && library.entries().get_current_context.is_some()).then_some(library)
    }

    /// How many calls the glue itself refused on the calling thread, before reaching the library: a test tells the
    /// glue's refusal from the library's own error by it.
    pub fn refusals(&self) -> u64 {
        runtime::refusals()
    }

    fn entries(&self) -> &calls::Entries {
        &self.loaded.entries
    }

    fn identity(&self) -> Address {
        Arc::as_ptr(&self.loaded).expose_provenance()
    }

    fn reading(&self) -> RwLockReadGuard<'_, ()> {
        self.loaded.gate.read().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn writing(&self) -> RwLockWriteGuard<'_, ()> {
        self.loaded.gate.write().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn display_address(&self, display: &Display) -> Option<*mut c_void> {
        (display.library == self.identity()).then(|| core::ptr::with_exposed_provenance_mut(display.address))
    }

    fn config_address(&self, config: &Config, display: *mut c_void) -> Option<*mut c_void> {
        (config.library == self.identity() && config.display == display.expose_provenance()).then(|| core::ptr::with_exposed_provenance_mut(config.address))
    }

    fn held_address(&self, held: &Held, display: *mut c_void) -> Option<*mut c_void> {
        (held.library == self.identity() && held.display == display.expose_provenance() && held.alive.load(Ordering::Acquire))
            .then(|| core::ptr::with_exposed_provenance_mut(held.address))
    }

    /// The OpenGL ES glue's way to a context: None unless every handle is of this library and alive.
    pub fn binding(&self, display: &Display, context: &Context, draw: Option<&Surface>, read: Option<&Surface>) -> Option<Binding> {
        let display_address = self.display_address(display)?;
        self.held_address(&context.held, display_address)?;
        for surface in [draw, read].into_iter().flatten() {
            self.held_address(&surface.held, display_address)?;
        }
        Some(Binding {
            library: self.clone(),
            display: display.address,
            context: Arc::clone(&context.held),
            draw: draw.map(|surface| Arc::clone(&surface.held)),
            read: read.map(|surface| Arc::clone(&surface.held)),
        })
    }
}

/// An EGL display; made only by `get_display`, of one library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Display {
    address: Address,
    library: Address,
}

impl Display {
    fn new(address: *mut c_void, library: &Library) -> Display {
        Display { address: runtime::address_of(address), library: library.identity() }
    }
}

/// An EGL frame buffer configuration of one display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    address: Address,
    display: Address,
    library: Address,
}

impl Config {
    fn new(address: *mut c_void, display: *mut c_void, library: &Library) -> Config {
        Config { address: runtime::address_of(address), display: display.expose_provenance(), library: library.identity() }
    }
}

/// A handle EGL destroys: alive until its destroy call succeeds, shared by every clone.
#[derive(Debug)]
struct Held {
    address: Address,
    display: Address,
    library: Address,
    alive: AtomicBool,
}

impl Held {
    fn new(address: *mut c_void, display: *mut c_void, library: &Library) -> Arc<Held> {
        Arc::new(Held { address: runtime::address_of(address), display: display.expose_provenance(), library: library.identity(), alive: AtomicBool::new(true) })
    }

    /// Marks the handle destroyed: no call of the glue passes it again.
    fn end(&self) {
        self.alive.store(false, Ordering::Release);
    }
}

/// An EGL rendering context.
#[derive(Clone, Debug)]
pub struct Context {
    held: Arc<Held>,
}

impl Context {
    fn new(address: *mut c_void, display: *mut c_void, library: &Library) -> Context {
        Context { held: Held::new(address, display, library) }
    }
}

impl PartialEq for Context {
    fn eq(&self, other: &Context) -> bool {
        Arc::ptr_eq(&self.held, &other.held)
    }
}

/// An EGL surface.
#[derive(Clone, Debug)]
pub struct Surface {
    held: Arc<Held>,
}

impl Surface {
    fn new(address: *mut c_void, display: *mut c_void, library: &Library) -> Surface {
        Surface { held: Held::new(address, display, library) }
    }
}

impl PartialEq for Surface {
    fn eq(&self, other: &Surface) -> bool {
        Arc::ptr_eq(&self.held, &other.held)
    }
}

/// A context with the surfaces it draws to and reads from, as the OpenGL ES glue holds it.
pub struct Binding {
    library: Library,
    display: Address,
    context: Arc<Held>,
    draw: Option<Arc<Held>>,
    read: Option<Arc<Held>>,
}

impl Binding {
    /// Makes the context current on this thread when it is not; false when it cannot be (a handle destroyed, or EGL
    /// refusing), and then no OpenGL ES call may be made.
    pub fn enter(&self) -> bool {
        if !self.context.alive.load(Ordering::Acquire) {
            return false;
        }
        let entries = self.library.entries();
        let Some(current) = entries.get_current_context else { return false };
        #[expect(unsafe_code, reason = "egl.xml: eglGetCurrentContext takes nothing and returns the thread's current context or EGL_NO_CONTEXT")]
        // SAFETY: no pointer is passed.
        let current = unsafe { current() };
        if current.expose_provenance() == self.context.address {
            return true;
        }
        let _gate = self.library.reading();
        let alive = |held: &Option<Arc<Held>>| held.as_ref().is_none_or(|held| held.alive.load(Ordering::Acquire));
        if !self.context.alive.load(Ordering::Acquire) || !alive(&self.draw) || !alive(&self.read) {
            return false;
        }
        let Some(make_current) = entries.make_current else { return false };
        let address = |held: &Option<Arc<Held>>| held.as_ref().map_or(core::ptr::null_mut(), |held| core::ptr::with_exposed_provenance_mut(held.address));
        #[expect(unsafe_code, reason = "egl.xml: eglMakeCurrent takes a display, two surfaces and a context, here all alive handles of this library, read under the gate no destroy passes")]
        // SAFETY: every handle is one EGL made and has not destroyed (alive, checked under the gate).
        let made = unsafe {
            make_current(
                core::ptr::with_exposed_provenance_mut(self.display),
                address(&self.draw),
                address(&self.read),
                core::ptr::with_exposed_provenance_mut(self.context.address),
            )
        };
        made != 0
    }

    /// The address of a client API entry point, or null; only ever typed by the registry's declaration.
    pub fn entry_address(&self, name: &CStr) -> *const c_void {
        let Some(get_proc_address) = self.library.entries().get_proc_address else { return core::ptr::null() };
        #[expect(unsafe_code, reason = "egl.xml: eglGetProcAddress reads a zero-ended name and returns an address or null")]
        // SAFETY: the name is zero-ended.
        let address = unsafe { get_proc_address(name.as_ptr().cast()) };
        address.cast_const()
    }

    /// Whether this binding is of that library and context (the identity the OpenGL ES glue checks).
    pub fn context_identity(&self) -> Address {
        Arc::as_ptr(&self.context).expose_provenance()
    }
}
