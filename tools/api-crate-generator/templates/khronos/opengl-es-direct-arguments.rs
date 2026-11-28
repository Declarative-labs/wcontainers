//! Every argument of every OpenGL ES and EGL method reaches the library as the method was given it, and everything the
//! library writes and returns reaches the caller: the generated stand-in library (fake/fake_egl.c), built here with the
//! C compiler (WCONTAINERS_CC, else `cc`), writes each call's arguments to the file FAKE_LOG names (a number as C has
//! it, a float to 9 digits, a handle as `h` and its place, what each pointer reaches: values, a byte range's count and
//! hash, a text), then each value it wrote (a pattern of its own) and the value it returned (one of its own: a number
//! with its top bit set, a handle, a text). Each method is called with arguments of its own, its line compared with what
//! the registry says the call passes, and what it gave back compared with what the stand-in logged it wrote and
//! returned; each EGL method that may be refused is called again while the stand-in refuses. The calls run in a process
//! of their own (this test run again), which the log's variable is set for.
#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::Command;

use wcontainers_egl::Egl;
use wcontainers_egl::constants as egl;
use wcontainers_egl_direct::Library;
use wcontainers_opengl_es::constants;
use wcontainers_opengl_es::{CompressedPixelUnpack, OpenGlEs, PixelPack, PixelUnpack};
use wcontainers_opengl_es_direct::OpenGlEsContext;

/// The variable naming the stand-in's log, set for the process that makes the calls.
const LOG_VARIABLE: &str = "FAKE_LOG";
/// The variable naming the folder the stand-in was built into.
const FOLDER_VARIABLE: &str = "WCONTAINERS_FAKE_FOLDER";
/// The bytes every pixel transfer and optional byte range is given: more than any call here reaches.
const ROOM: usize = 8192;
/// The bytes a 2 by 2 (by 1) transfer of RGBA bytes reaches at the stand-in's unpack alignment of 4.
const PIXELS: usize = 16;
/// The bytes a compressed transfer is given.
const COMPRESSED: usize = 8;
/// A fence in a line the stand-in must log: the handle the last glFenceSync made.
const FENCE: &str = "FENCE";
/// What the stand-in logs as the first word of a line telling what a call gave back.
const GIVEN_BACK: &str = "  ";

/// The stand-in library built from fake/fake_egl.c, as libEGL.so.1 in a folder of this test's.
fn built() -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fake-egl");
    std::fs::create_dir_all(&folder).expect("the stand-in's folder made");
    let compiler = std::env::var_os("WCONTAINERS_CC").unwrap_or_else(|| "cc".into());
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("fake").join("fake_egl.c");
    let status = Command::new(compiler).args(["-O1", "-std=c11", "-shared", "-fPIC", "-o"]).arg(folder.join("libEGL.so.1")).arg(&source).arg("-ldl").status().expect("the C compiler runs");
    assert!(status.success(), "the stand-in builds from {}", source.display());
    folder
}

/// A byte range as the stand-in logs it: its count and FNV-1a hash.
fn bytes(given: &[u8]) -> String {
    let hash = given.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3));
    format!("bytes {} {hash:016x}", given.len())
}

/// `length` bytes of their own, none zero, told apart by `seed`.
fn patterned(length: usize, seed: usize) -> Vec<u8> {
    (0..length).map(|at| u8::try_from((at * 31 + seed * 7) % 251 + 1).unwrap_or(1)).collect()
}

/// A value as the stand-in logs it: a number as C prints it, a float as its bits, a flag as any number (true when not
/// zero).
trait Logged: core::fmt::Debug {
    fn is_logged(&self, text: &str) -> bool;
}

impl Logged for i32 {
    fn is_logged(&self, text: &str) -> bool {
        text.parse::<Self>() == Ok(*self)
    }
}

impl Logged for u32 {
    fn is_logged(&self, text: &str) -> bool {
        text.parse::<Self>() == Ok(*self)
    }
}

impl Logged for i64 {
    fn is_logged(&self, text: &str) -> bool {
        text.parse::<Self>() == Ok(*self)
    }
}

impl Logged for u64 {
    fn is_logged(&self, text: &str) -> bool {
        text.parse::<Self>() == Ok(*self)
    }
}

impl Logged for u8 {
    fn is_logged(&self, text: &str) -> bool {
        text.parse::<Self>() == Ok(*self)
    }
}

impl Logged for f32 {
    fn is_logged(&self, text: &str) -> bool {
        text == format!("0x{:x}", self.to_bits())
    }
}

impl Logged for bool {
    fn is_logged(&self, text: &str) -> bool {
        text.parse::<u64>().is_ok_and(|value| (value != 0) == *self)
    }
}

/// The calls checked so far, the log's lines read, the last call's lines telling what it gave back, and what differed.
struct Calls {
    log: PathBuf,
    read: usize,
    checked: usize,
    given_back: usize,
    after: Vec<String>,
    failures: Vec<String>,
}

impl Calls {
    fn lines(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log).unwrap_or_default().lines().map(str::to_string).collect()
    }

    /// The last line `command` logged since the last check must be `expected` (FENCE the handle the last glFenceSync
    /// made); the lines after it tell what it gave back.
    fn expect(&mut self, command: &str, expected: &str) {
        let lines = self.lines();
        let new = lines.get(self.read..).unwrap_or_default();
        let spaced = format!("{command} ");
        let found = new.iter().rposition(|line| line == command || line.starts_with(&spaced));
        let fence = lines.iter().rposition(|line| line.starts_with("glFenceSync ")).and_then(|at| lines.get(at + 1)).and_then(|line| line.strip_prefix("  returned "));
        let expected = expected.replace(FENCE, fence.unwrap_or("no fence"));
        let received = found.map(|at| new[at].as_str());
        if received != Some(expected.as_str()) {
            self.failures.push(format!("{command}: the library received {received:?}, expected {expected:?}"));
        }
        self.after = found.map(|at| new[at + 1..].iter().take_while(|line| line.starts_with(GIVEN_BACK)).cloned().collect()).unwrap_or_default();
        self.read = lines.len();
        self.checked += 1;
    }

    /// What the last checked call logged after `prefix`.
    fn after(&self, prefix: &str) -> Option<String> {
        self.after.iter().find_map(|line| line.strip_prefix(prefix)).map(str::to_string)
    }

    fn given_back(&mut self, right: bool, what: impl FnOnce() -> String) {
        self.given_back += 1;
        if !right {
            self.failures.push(what());
        }
    }

    /// The values `given` back for parameter `at` must be the first the library wrote there.
    fn wrote<T: Logged>(&mut self, command: &str, at: usize, given: &[T]) {
        let logged = self.after(&format!("  wrote {at} "));
        let values: Vec<String> = logged.as_deref().and_then(|text| text.strip_prefix('[')).and_then(|text| text.strip_suffix(']')).map(|text| text.split(',').map(str::to_string).collect()).unwrap_or_default();
        let right = values.len() >= given.len() && given.iter().zip(&values).all(|(value, text)| value.is_logged(text));
        self.given_back(right, || format!("{command}: parameter {at} gave back {given:?}, the library wrote {logged:?}"));
    }

    /// The bytes `given` back for parameter `at` must begin with those the library wrote there.
    fn wrote_bytes(&mut self, command: &str, at: usize, given: &[u8]) {
        let logged = self.after(&format!("  wrote {at} "));
        let count = logged.as_deref().and_then(|text| text.split(' ').nth(1)).and_then(|count| count.parse::<usize>().ok());
        let right = count.and_then(|count| given.get(..count)).is_some_and(|written| Some(bytes(written)) == logged);
        self.given_back(right, || format!("{command}: parameter {at} gave back bytes other than the library wrote ({logged:?})"));
    }

    /// The value given back must be the one the library returned.
    fn returned<T: Logged>(&mut self, command: &str, value: &T) {
        let logged = self.after("  returned ");
        let right = logged.as_deref().is_some_and(|text| value.is_logged(text));
        self.given_back(right, || format!("{command}: gave back {value:?}, the library returned {logged:?}"));
    }

    /// The text given back must be the one the library returned (none for null).
    fn returned_text(&mut self, command: &str, value: Option<&[u8]>) {
        let logged = self.after("  returned ");
        let given = value.map_or_else(|| "null".to_string(), |text| format!("'{}'", String::from_utf8_lossy(text)));
        let right = logged.as_deref() == Some(given.as_str());
        self.given_back(right, || format!("{command}: gave back {given}, the library returned {logged:?}"));
    }

    /// A handle is given back exactly when the library returned one: its name in the log.
    fn returned_handle(&mut self, command: &str, made: bool) -> String {
        let logged = self.after("  returned ").unwrap_or_default();
        let right = made == logged.starts_with('h');
        self.given_back(right, || format!("{command}: gave back {}, the library returned {logged:?}", if made { "a handle" } else { "none" }));
        logged
    }

    /// The handles the library wrote for parameter `at`, by their names in the log.
    fn handles_wrote(&self, at: usize) -> Vec<String> {
        let logged = self.after(&format!("  wrote {at} ")).unwrap_or_default();
        logged.strip_prefix('[').and_then(|text| text.strip_suffix(']')).map(|text| text.split(',').filter(|name| !name.is_empty()).map(str::to_string).collect()).unwrap_or_default()
    }

    /// The answer given back must be success exactly when the library answered EGL_TRUE.
    fn answered(&mut self, command: &str, success: bool) {
        let logged = self.after("  returned ");
        let right = logged.as_deref().and_then(|text| text.parse::<i64>().ok()).is_some_and(|answer| (answer != 0) == success);
        self.given_back(right, || format!("{command}: gave back {success}, the library answered {logged:?}"));
    }

    /// No line of `command` since the last check: the glue refused before the library.
    fn not_reached(&mut self, command: &str) {
        let lines = self.lines();
        let spaced = format!("{command} ");
        if lines.get(self.read..).unwrap_or_default().iter().any(|line| line == command || line.starts_with(&spaced)) {
            self.failures.push(format!("{command} reached the library, which the glue should have refused"));
        }
        self.read = lines.len();
        self.checked += 1;
    }

    fn failed(&mut self, what: &str) {
        self.failures.push(what.to_string());
    }

    /// The file whose presence makes the stand-in refuse each EGL command answering EGL_TRUE or EGL_FALSE.
    fn refusing(&self, refuse: bool) {
        let file = PathBuf::from(format!("{}.refuse", self.log.display()));
        if refuse {
            std::fs::write(&file, b"").expect("the refusal file made");
        } else {
            let _ = std::fs::remove_file(&file);
        }
    }
}

#[test]
fn every_argument_reaches_the_library() {
    let Some(log) = std::env::var_os(LOG_VARIABLE).map(PathBuf::from) else {
        let folder = built();
        let log = folder.join("calls.log");
        let _ = std::fs::remove_file(folder.join("calls.log.refuse"));
        let binary = std::env::current_exe().expect("the test binary");
        let output = Command::new(binary)
            .args(["--exact", "every_argument_reaches_the_library", "--nocapture", "--test-threads=1"])
            .env(LOG_VARIABLE, &log)
            .env(FOLDER_VARIABLE, &folder)
            .output()
            .expect("the calls' process runs");
        let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
        println!("{}", text.find("ARGUMENTS ").and_then(|at| text[at..].lines().next()).unwrap_or("ARGUMENTS: no count"));
        assert!(output.status.success(), "the calls' process failed: {text}");
        return;
    };
    let folder = PathBuf::from(std::env::var_os(FOLDER_VARIABLE).expect("the stand-in's folder named"));
    let library = Library::load(Some(&folder)).expect("the stand-in loads");
    let mut calls = Calls { log, read: 0, checked: 0, given_back: 0, after: Vec::new(), failures: Vec::new() };
    every_egl_method(&library, &mut calls);
    let display = library.get_display().expect("the default display");
    library.initialize(&display).expect("the display initializes");
    assert!(library.bind_api(egl::OPENGL_ES_API.cast_unsigned()));
    let mut configs = vec![None; 1];
    let wanted = [egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::RENDERABLE_TYPE, egl::OPENGL_ES3_BIT, egl::NONE];
    library.choose_config(&display, &wanted, &mut configs).expect("eglChooseConfig succeeds");
    let config = configs[0].clone().expect("a configuration");
    let context = library.create_context(&display, &config, None, &[egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE]).expect("a context");
    let surface = library.create_pbuffer_surface(&display, &config, &[egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE]).expect("a surface");
    let gl = OpenGlEsContext::owning(library, display, context, Some(surface.clone()), Some(surface)).expect("the context binds");
    // A buffer bound at each target a method takes an offset into (an offset with none bound is refused).
    let mut names = [0_u32; 2];
    gl.gen_buffers(&mut names);
    gl.bind_buffer(constants::ARRAY_BUFFER, names[0]);
    gl.bind_buffer(constants::ELEMENT_ARRAY_BUFFER, names[1]);
    calls.read = calls.lines().len();
    every_method(&gl, &mut calls);
    println!("ARGUMENTS {} methods checked, {} received otherwise; GIVEN BACK {} checked", calls.checked, calls.failures.len(), calls.given_back);
    assert!(calls.failures.is_empty(), "{}", calls.failures.join("\n"));
}

/// Every EGL method with handles of its own (two contexts, one sharing the other's objects; a surface to draw and
/// another to read), its line and what it gave back checked; then each refused by the library, and each handle
/// destroyed then refused by the glue before the library.
fn every_egl_method(library: &Library, calls: &mut Calls) {
    let display = library.get_display();
    calls.expect("eglGetDisplay", "eglGetDisplay null");
    let shown = calls.returned_handle("eglGetDisplay", display.is_some());
    let display = display.expect("the default display");
    let version = library.initialize(&display);
    calls.expect("eglInitialize", &format!("eglInitialize {shown} p p"));
    calls.answered("eglInitialize", version.is_some());
    let (major, minor) = version.unwrap_or((-1, -1));
    calls.wrote("eglInitialize", 1, &[major]);
    calls.wrote("eglInitialize", 2, &[minor]);
    let bound = library.bind_api(egl::OPENGL_ES_API.cast_unsigned());
    calls.expect("eglBindAPI", &format!("eglBindAPI {}", egl::OPENGL_ES_API));
    calls.answered("eglBindAPI", bound);
    let mut configs = vec![None; 3];
    let found = library.choose_config(&display, &[egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::NONE], &mut configs);
    calls.expect("eglChooseConfig", &format!("eglChooseConfig {shown} [{},{},{}] p 3 p", egl::SURFACE_TYPE, egl::PBUFFER_BIT, egl::NONE));
    calls.answered("eglChooseConfig", found.is_some());
    calls.wrote("eglChooseConfig", 4, &[found.unwrap_or(-1)]);
    let config_names = calls.handles_wrote(2);
    if configs.iter().flatten().count() != config_names.len() || config_names.len() != configs.len() {
        calls.failed(&format!("eglChooseConfig: {} configurations given back, the library wrote {config_names:?}", configs.iter().flatten().count()));
    }
    // The last configuration: one given back in the wrong place is told apart.
    let config = configs[2].clone().expect("a third configuration");
    let config_shown = config_names.get(2).cloned().unwrap_or_default();
    let value = library.get_config_attrib(&display, &config, egl::RED_SIZE);
    calls.expect("eglGetConfigAttrib", &format!("eglGetConfigAttrib {shown} {config_shown} {} p", egl::RED_SIZE));
    calls.answered("eglGetConfigAttrib", value.is_some());
    calls.wrote("eglGetConfigAttrib", 3, &[value.unwrap_or(-1)]);
    let first = library.create_context(&display, &config, None, &[egl::CONTEXT_MAJOR_VERSION, 3, egl::NONE]);
    let robust = format!("{},{}", egl::CONTEXT_OPENGL_ROBUST_ACCESS, egl::TRUE);
    calls.expect("eglCreateContext", &format!("eglCreateContext {shown} {config_shown} null [{},3,{robust},{}]", egl::CONTEXT_MAJOR_VERSION, egl::NONE));
    let first_shown = calls.returned_handle("eglCreateContext", first.is_some());
    let first = first.expect("a context");
    let shared = library.create_context(&display, &config, Some(&first), &[egl::NONE]);
    calls.expect("eglCreateContext", &format!("eglCreateContext {shown} {config_shown} {first_shown} [{robust},{}]", egl::NONE));
    let shared_shown = calls.returned_handle("eglCreateContext", shared.is_some());
    let shared = shared.expect("a context sharing the first's objects");
    let mut surfaces = Vec::new();
    for (width, height) in [(2, 3), (4, 5)] {
        let surface = library.create_pbuffer_surface(&display, &config, &[egl::WIDTH, width, egl::HEIGHT, height, egl::NONE]);
        calls.expect("eglCreatePbufferSurface", &format!("eglCreatePbufferSurface {shown} {config_shown} [{},{width},{},{height},{}]", egl::WIDTH, egl::HEIGHT, egl::NONE));
        let surface_shown = calls.returned_handle("eglCreatePbufferSurface", surface.is_some());
        surfaces.push((surface.expect("a surface"), surface_shown));
    }
    let [(draw, draw_shown), (read, read_shown)] = <[_; 2]>::try_from(surfaces).expect("two surfaces");
    let current = library.make_current(&display, Some(&draw), Some(&read), Some(&shared));
    calls.expect("eglMakeCurrent", &format!("eglMakeCurrent {shown} {draw_shown} {read_shown} {shared_shown}"));
    calls.answered("eglMakeCurrent", current);
    let released = library.make_current(&display, None, None, None);
    calls.expect("eglMakeCurrent", &format!("eglMakeCurrent {shown} null null null"));
    calls.answered("eglMakeCurrent", released);
    let vendor = library.query_string(Some(&display), egl::VENDOR);
    calls.expect("eglQueryString", &format!("eglQueryString {shown} {}", egl::VENDOR));
    calls.returned_text("eglQueryString", vendor.as_deref());
    let client = library.query_string(None, egl::EXTENSIONS);
    calls.expect("eglQueryString", &format!("eglQueryString null {}", egl::EXTENSIONS));
    calls.returned_text("eglQueryString", client.as_deref());
    let width = library.query_surface(&display, &read, egl::WIDTH);
    calls.expect("eglQuerySurface", &format!("eglQuerySurface {shown} {read_shown} {} p", egl::WIDTH));
    calls.answered("eglQuerySurface", width.is_some());
    calls.wrote("eglQuerySurface", 3, &[width.unwrap_or(-1)]);
    let swapped = library.swap_buffers(&display, &draw);
    calls.expect("eglSwapBuffers", &format!("eglSwapBuffers {shown} {draw_shown}"));
    calls.answered("eglSwapBuffers", swapped);
    let interval = library.swap_interval(&display, 7);
    calls.expect("eglSwapInterval", &format!("eglSwapInterval {shown} 7"));
    calls.answered("eglSwapInterval", interval);
    // A refusal of the glue's own, pending, is answered first: the second reaches the library.
    let _ = library.get_error();
    let error = library.get_error();
    calls.expect("eglGetError", "eglGetError");
    calls.returned("eglGetError", &error);

    // Each refused by the library.
    calls.refusing(true);
    let refused_display = library.get_display();
    calls.expect("eglGetDisplay", "eglGetDisplay null");
    calls.returned_handle("eglGetDisplay", refused_display.is_some());
    let version = library.initialize(&display);
    calls.expect("eglInitialize", &format!("eglInitialize {shown} p p"));
    calls.answered("eglInitialize", version.is_some());
    let bound = library.bind_api(egl::OPENGL_ES_API.cast_unsigned());
    calls.expect("eglBindAPI", &format!("eglBindAPI {}", egl::OPENGL_ES_API));
    calls.answered("eglBindAPI", bound);
    let mut refused_configs = vec![None; 3];
    let found = library.choose_config(&display, &[egl::NONE], &mut refused_configs);
    calls.expect("eglChooseConfig", &format!("eglChooseConfig {shown} [{}] p 3 p", egl::NONE));
    calls.answered("eglChooseConfig", found.is_some());
    if refused_configs.iter().any(Option::is_some) {
        calls.failed("eglChooseConfig: configurations given back from a refused call");
    }
    let value = library.get_config_attrib(&display, &config, egl::RED_SIZE);
    calls.expect("eglGetConfigAttrib", &format!("eglGetConfigAttrib {shown} {config_shown} {} p", egl::RED_SIZE));
    calls.answered("eglGetConfigAttrib", value.is_some());
    let context = library.create_context(&display, &config, None, &[egl::NONE]);
    calls.expect("eglCreateContext", &format!("eglCreateContext {shown} {config_shown} null [{robust},{}]", egl::NONE));
    calls.returned_handle("eglCreateContext", context.is_some());
    let surface = library.create_pbuffer_surface(&display, &config, &[egl::NONE]);
    calls.expect("eglCreatePbufferSurface", &format!("eglCreatePbufferSurface {shown} {config_shown} [{}]", egl::NONE));
    calls.returned_handle("eglCreatePbufferSurface", surface.is_some());
    let current = library.make_current(&display, Some(&read), Some(&draw), Some(&first));
    calls.expect("eglMakeCurrent", &format!("eglMakeCurrent {shown} {read_shown} {draw_shown} {first_shown}"));
    calls.answered("eglMakeCurrent", current);
    let vendor = library.query_string(Some(&display), egl::VENDOR);
    calls.expect("eglQueryString", &format!("eglQueryString {shown} {}", egl::VENDOR));
    calls.returned_text("eglQueryString", vendor.as_deref());
    let width = library.query_surface(&display, &draw, egl::HEIGHT);
    calls.expect("eglQuerySurface", &format!("eglQuerySurface {shown} {draw_shown} {} p", egl::HEIGHT));
    calls.answered("eglQuerySurface", width.is_some());
    let swapped = library.swap_buffers(&display, &read);
    calls.expect("eglSwapBuffers", &format!("eglSwapBuffers {shown} {read_shown}"));
    calls.answered("eglSwapBuffers", swapped);
    let interval = library.swap_interval(&display, 9);
    calls.expect("eglSwapInterval", &format!("eglSwapInterval {shown} 9"));
    calls.answered("eglSwapInterval", interval);
    // A destruction the library refused leaves the handle the glue's: still passed on.
    let destroyed = library.destroy_surface(&display, &draw);
    calls.expect("eglDestroySurface", &format!("eglDestroySurface {shown} {draw_shown}"));
    calls.answered("eglDestroySurface", destroyed);
    let _ = library.swap_buffers(&display, &draw);
    calls.expect("eglSwapBuffers", &format!("eglSwapBuffers {shown} {draw_shown}"));
    let destroyed = library.destroy_context(&display, &shared);
    calls.expect("eglDestroyContext", &format!("eglDestroyContext {shown} {shared_shown}"));
    calls.answered("eglDestroyContext", destroyed);
    let _ = library.make_current(&display, None, None, Some(&shared));
    calls.expect("eglMakeCurrent", &format!("eglMakeCurrent {shown} null null {shared_shown}"));
    calls.refusing(false);

    // Each destroyed, then refused by the glue before the library.
    for (surface, surface_shown) in [(&draw, &draw_shown), (&read, &read_shown)] {
        let destroyed = library.destroy_surface(&display, surface);
        calls.expect("eglDestroySurface", &format!("eglDestroySurface {shown} {surface_shown}"));
        calls.answered("eglDestroySurface", destroyed);
        let _ = library.swap_buffers(&display, surface);
        calls.not_reached("eglSwapBuffers");
    }
    for (context, context_shown) in [(&shared, &shared_shown), (&first, &first_shown)] {
        let destroyed = library.destroy_context(&display, context);
        calls.expect("eglDestroyContext", &format!("eglDestroyContext {shown} {context_shown}"));
        calls.answered("eglDestroyContext", destroyed);
        let _ = library.make_current(&display, None, None, Some(context));
        calls.not_reached("eglMakeCurrent");
    }
}

/// A fence this context made, for a method that takes one.
fn fence(gl: &OpenGlEsContext) -> <OpenGlEsContext as OpenGlEs>::Fence {
    gl.fence_sync(constants::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence")
}
// METHODS
