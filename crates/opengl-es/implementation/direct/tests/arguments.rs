// Written by tools/api-crate-generator from Khronos's gl.xml (and ANGLE's gl_angle_ext.xml for ANGLE's extensions), pinned in src/khronos/write.rs. Do not edit.
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

/// Each method called once with arguments of its own: the line the library logged for it checked, and what it gave
/// back.
fn every_method(gl: &OpenGlEsContext, calls: &mut Calls) {
    {
        gl.active_texture(1001);
        calls.expect("glActiveTexture", "glActiveTexture 1001");
    }
    {
        gl.attach_shader(1101, 1102);
        calls.expect("glAttachShader", "glAttachShader 1101 1102");
    }
    {
        gl.begin_query(1201, 1202);
        calls.expect("glBeginQuery", "glBeginQuery 1201 1202");
    }
    {
        gl.begin_transform_feedback(1301);
        calls.expect("glBeginTransformFeedback", "glBeginTransformFeedback 1301");
    }
    {
        gl.bind_buffer(1401, 1402);
        calls.expect("glBindBuffer", "glBindBuffer 1401 1402");
    }
    {
        gl.bind_buffer_base(1501, 1502, 1503);
        calls.expect("glBindBufferBase", "glBindBufferBase 1501 1502 1503");
    }
    {
        gl.bind_buffer_range(1601, 1602, 1603, 1604, 1605);
        calls.expect("glBindBufferRange", "glBindBufferRange 1601 1602 1603 1604 1605");
    }
    {
        gl.bind_framebuffer(1701, 1702);
        calls.expect("glBindFramebuffer", "glBindFramebuffer 1701 1702");
    }
    {
        gl.bind_renderbuffer(1801, 1802);
        calls.expect("glBindRenderbuffer", "glBindRenderbuffer 1801 1802");
    }
    {
        gl.bind_sampler(1901, 1902);
        calls.expect("glBindSampler", "glBindSampler 1901 1902");
    }
    {
        gl.bind_texture(2001, 2002);
        calls.expect("glBindTexture", "glBindTexture 2001 2002");
    }
    {
        gl.bind_vertex_array(2101);
        calls.expect("glBindVertexArray", "glBindVertexArray 2101");
    }
    {
        gl.blend_color(2201.5, 2202.5, 2203.5, 2204.5);
        calls.expect("glBlendColor", "glBlendColor 2201.5 2202.5 2203.5 2204.5");
    }
    {
        gl.blend_equation_separate(2301, 2302);
        calls.expect("glBlendEquationSeparate", "glBlendEquationSeparate 2301 2302");
    }
    {
        gl.blend_func_separate(2401, 2402, 2403, 2404);
        calls.expect("glBlendFuncSeparate", "glBlendFuncSeparate 2401 2402 2403 2404");
    }
    {
        gl.blit_framebuffer(2501, 2502, 2503, 2504, 2505, 2506, 2507, 2508, 2509, 2510);
        calls.expect("glBlitFramebuffer", "glBlitFramebuffer 2501 2502 2503 2504 2505 2506 2507 2508 2509 2510");
    }
    {
        let data = patterned(ROOM, 258);
        gl.buffer_data(2601, 2602, Some(data.as_slice()), 2604);
        calls.expect("glBufferData", &format!("glBufferData 2601 2602 {} 2604", bytes(&data[..2602])));
    }
    {
        let data: Vec<u8> = vec![150_u8, 151_u8];
        gl.buffer_sub_data(2701, 2702, &data);
        calls.expect("glBufferSubData", &format!("glBufferSubData 2701 2702 2 {}", bytes(&data)));
    }
    {
        let answer = gl.check_framebuffer_status(2801);
        calls.expect("glCheckFramebufferStatus", "glCheckFramebufferStatus 2801");
        let returned = answer;
        calls.returned("glCheckFramebufferStatus", &returned);
    }
    {
        gl.clear_bufferfi(2901, 2902, 2903.5, 2904);
        calls.expect("glClearBufferfi", "glClearBufferfi 2901 2902 2903.5 2904");
    }
    {
        let value = vec![755.5_f32, 756.5_f32];
        gl.clear_bufferfv(3001, 3002, &value);
        calls.expect("glClearBufferfv", "glClearBufferfv 3001 3002 [0x443ce000,0x443d2000,0x0,0x0]");
    }
    {
        let value = vec![792_i32, 793_i32];
        gl.clear_bufferiv(3101, 3102, &value);
        calls.expect("glClearBufferiv", "glClearBufferiv 3101 3102 [792,793,0,0]");
    }
    {
        let value = vec![829_u32, 830_u32];
        gl.clear_bufferuiv(3201, 3202, &value);
        calls.expect("glClearBufferuiv", "glClearBufferuiv 3201 3202 [829,830,0,0]");
    }
    {
        let sync = fence(gl);
        let answer = gl.client_wait_sync(&sync, 3302, 3303);
        calls.expect("glClientWaitSync", "glClientWaitSync FENCE 3302 3303");
        let returned = answer;
        calls.returned("glClientWaitSync", &returned);
    }
    {
        gl.color_mask(true, true, true, true);
        calls.expect("glColorMask", "glColorMask 1 1 1 1");
    }
    {
        gl.compile_shader(3501);
        calls.expect("glCompileShader", "glCompileShader 3501");
    }
    {
        let data = patterned(COMPRESSED, 424);
        gl.compressed_tex_sub_image_2d(3601, 3602, 3603, 3604, 3605, 3606, 3607, CompressedPixelUnpack::Bytes(&data));
        calls.expect("glCompressedTexSubImage2D", &format!("glCompressedTexSubImage2D 3601 3602 3603 3604 3605 3606 3607 8 {}", bytes(&data)));
    }
    {
        let data = patterned(COMPRESSED, 442);
        gl.compressed_tex_sub_image_3d(3701, 3702, 3703, 3704, 3705, 3706, 3707, 3708, 3709, CompressedPixelUnpack::Bytes(&data));
        calls.expect("glCompressedTexSubImage3D", &format!("glCompressedTexSubImage3D 3701 3702 3703 3704 3705 3706 3707 3708 3709 8 {}", bytes(&data)));
    }
    {
        gl.copy_buffer_sub_data(3801, 3802, 3803, 3804, 3805);
        calls.expect("glCopyBufferSubData", "glCopyBufferSubData 3801 3802 3803 3804 3805");
    }
    {
        gl.copy_tex_sub_image_2d(3901, 3902, 3903, 3904, 3905, 3906, 3907, 3908);
        calls.expect("glCopyTexSubImage2D", "glCopyTexSubImage2D 3901 3902 3903 3904 3905 3906 3907 3908");
    }
    {
        gl.copy_tex_sub_image_3d(4001, 4002, 4003, 4004, 4005, 4006, 4007, 4008, 4009);
        calls.expect("glCopyTexSubImage3D", "glCopyTexSubImage3D 4001 4002 4003 4004 4005 4006 4007 4008 4009");
    }
    {
        let answer = gl.create_program();
        calls.expect("glCreateProgram", "glCreateProgram");
        let returned = answer;
        calls.returned("glCreateProgram", &returned);
    }
    {
        let answer = gl.create_shader(4201);
        calls.expect("glCreateShader", "glCreateShader 4201");
        let returned = answer;
        calls.returned("glCreateShader", &returned);
    }
    {
        gl.cull_face(4301);
        calls.expect("glCullFace", "glCullFace 4301");
    }
    {
        let buffers = vec![1266_u32, 1267_u32];
        gl.delete_buffers(&buffers);
        calls.expect("glDeleteBuffers", "glDeleteBuffers 2 [1266,1267]");
    }
    {
        let framebuffers = vec![1303_u32, 1304_u32];
        gl.delete_framebuffers(&framebuffers);
        calls.expect("glDeleteFramebuffers", "glDeleteFramebuffers 2 [1303,1304]");
    }
    {
        gl.delete_program(4601);
        calls.expect("glDeleteProgram", "glDeleteProgram 4601");
    }
    {
        let ids = vec![1377_u32, 1378_u32];
        gl.delete_queries(&ids);
        calls.expect("glDeleteQueries", "glDeleteQueries 2 [1377,1378]");
    }
    {
        let renderbuffers = vec![1414_u32, 1415_u32];
        gl.delete_renderbuffers(&renderbuffers);
        calls.expect("glDeleteRenderbuffers", "glDeleteRenderbuffers 2 [1414,1415]");
    }
    {
        let samplers = vec![1451_u32, 1452_u32];
        gl.delete_samplers(&samplers);
        calls.expect("glDeleteSamplers", "glDeleteSamplers 2 [1451,1452]");
    }
    {
        gl.delete_shader(5001);
        calls.expect("glDeleteShader", "glDeleteShader 5001");
    }
    {
        let sync = fence(gl);
        gl.delete_sync(&sync);
        calls.expect("glDeleteSync", "glDeleteSync FENCE");
        gl.delete_sync(&sync);
        calls.not_reached("glDeleteSync");
    }
    {
        let textures = vec![1562_u32, 1563_u32];
        gl.delete_textures(&textures);
        calls.expect("glDeleteTextures", "glDeleteTextures 2 [1562,1563]");
    }
    {
        let arrays = vec![1599_u32, 1600_u32];
        gl.delete_vertex_arrays(&arrays);
        calls.expect("glDeleteVertexArrays", "glDeleteVertexArrays 2 [1599,1600]");
    }
    {
        gl.depth_func(5401);
        calls.expect("glDepthFunc", "glDepthFunc 5401");
    }
    {
        gl.depth_mask(true);
        calls.expect("glDepthMask", "glDepthMask 1");
    }
    {
        gl.depth_rangef(5601.5, 5602.5);
        calls.expect("glDepthRangef", "glDepthRangef 5601.5 5602.5");
    }
    {
        gl.detach_shader(5701, 5702);
        calls.expect("glDetachShader", "glDetachShader 5701 5702");
    }
    {
        gl.disable(5801);
        calls.expect("glDisable", "glDisable 5801");
    }
    {
        gl.disable_vertex_attrib_array(5901);
        calls.expect("glDisableVertexAttribArray", "glDisableVertexAttribArray 5901");
    }
    {
        gl.draw_arrays(6001, 6002, 6003);
        calls.expect("glDrawArrays", "glDrawArrays 6001 6002 6003");
    }
    {
        gl.draw_arrays_instanced(6101, 6102, 6103, 6104);
        calls.expect("glDrawArraysInstanced", "glDrawArraysInstanced 6101 6102 6103 6104");
    }
    {
        let bufs = vec![1932_u32, 1933_u32];
        gl.draw_buffers(&bufs);
        calls.expect("glDrawBuffers", "glDrawBuffers 2 [1932,1933]");
    }
    {
        gl.draw_elements_instanced(6301, 6302, 6303, 16, 6305);
        calls.expect("glDrawElementsInstanced", "glDrawElementsInstanced 6301 6302 6303 16 6305");
    }
    {
        gl.enable(6401);
        calls.expect("glEnable", "glEnable 6401");
    }
    {
        gl.enable_vertex_attrib_array(6501);
        calls.expect("glEnableVertexAttribArray", "glEnableVertexAttribArray 6501");
    }
    {
        gl.end_query(6601);
        calls.expect("glEndQuery", "glEndQuery 6601");
    }
    {
        gl.end_transform_feedback();
        calls.expect("glEndTransformFeedback", "glEndTransformFeedback");
    }
    {
        let answer = gl.fence_sync(6801, 6802);
        calls.expect("glFenceSync", "glFenceSync 6801 6802");
        let returned = answer;
        calls.returned_handle("glFenceSync", returned.is_some());
    }
    {
        gl.finish();
        calls.expect("glFinish", "glFinish");
    }
    {
        gl.flush();
        calls.expect("glFlush", "glFlush");
    }
    {
        gl.framebuffer_renderbuffer(1101, 1102, 1103, 1104);
        calls.expect("glFramebufferRenderbuffer", "glFramebufferRenderbuffer 1101 1102 1103 1104");
    }
    {
        gl.framebuffer_texture_2d(1201, 1202, 1203, 1204, 1205);
        calls.expect("glFramebufferTexture2D", "glFramebufferTexture2D 1201 1202 1203 1204 1205");
    }
    {
        gl.framebuffer_texture_layer(1301, 1302, 1303, 1304, 1305);
        calls.expect("glFramebufferTextureLayer", "glFramebufferTextureLayer 1301 1302 1303 1304 1305");
    }
    {
        gl.front_face(1401);
        calls.expect("glFrontFace", "glFrontFace 1401");
    }
    {
        let mut buffers = vec![0_u32; 2];
        gl.gen_buffers(&mut buffers);
        calls.expect("glGenBuffers", "glGenBuffers 2 p");
        calls.wrote("glGenBuffers", 1, &buffers);
    }
    {
        let mut framebuffers = vec![0_u32; 2];
        gl.gen_framebuffers(&mut framebuffers);
        calls.expect("glGenFramebuffers", "glGenFramebuffers 2 p");
        calls.wrote("glGenFramebuffers", 1, &framebuffers);
    }
    {
        let mut ids = vec![0_u32; 2];
        gl.gen_queries(&mut ids);
        calls.expect("glGenQueries", "glGenQueries 2 p");
        calls.wrote("glGenQueries", 1, &ids);
    }
    {
        let mut renderbuffers = vec![0_u32; 2];
        gl.gen_renderbuffers(&mut renderbuffers);
        calls.expect("glGenRenderbuffers", "glGenRenderbuffers 2 p");
        calls.wrote("glGenRenderbuffers", 1, &renderbuffers);
    }
    {
        let mut samplers = vec![0_u32; 2];
        gl.gen_samplers(&mut samplers);
        calls.expect("glGenSamplers", "glGenSamplers 2 p");
        calls.wrote("glGenSamplers", 1, &samplers);
    }
    {
        let mut textures = vec![0_u32; 2];
        gl.gen_textures(&mut textures);
        calls.expect("glGenTextures", "glGenTextures 2 p");
        calls.wrote("glGenTextures", 1, &textures);
    }
    {
        let mut arrays = vec![0_u32; 2];
        gl.gen_vertex_arrays(&mut arrays);
        calls.expect("glGenVertexArrays", "glGenVertexArrays 2 p");
        calls.wrote("glGenVertexArrays", 1, &arrays);
    }
    {
        gl.generate_mipmap(2201);
        calls.expect("glGenerateMipmap", "glGenerateMipmap 2201");
    }
    {
        let mut data = vec![false; 4];
        gl.get_booleanv(2301, &mut data);
        calls.expect("glGetBooleanv", "glGetBooleanv 2301 p");
        calls.wrote("glGetBooleanv", 1, &data);
    }
    {
        let _ = gl.get_error();
        let answer = gl.get_error();
        calls.expect("glGetError", "glGetError");
        let returned = answer;
        calls.returned("glGetError", &returned);
    }
    {
        let mut data = vec![0.0_f32; 4];
        gl.get_floatv(2501, &mut data);
        calls.expect("glGetFloatv", "glGetFloatv 2501 p");
        calls.wrote("glGetFloatv", 1, &data);
    }
    {
        let mut data = vec![0_i64; 4];
        gl.get_integer64v(2601, &mut data);
        calls.expect("glGetInteger64v", "glGetInteger64v 2601 p");
        calls.wrote("glGetInteger64v", 1, &data);
    }
    {
        let mut data = vec![0_i32; 4];
        gl.get_integerv(2701, &mut data);
        calls.expect("glGetIntegerv", "glGetIntegerv 2701 p");
        calls.wrote("glGetIntegerv", 1, &data);
    }
    {
        let mut params = vec![0_i32; 2];
        gl.get_internalformativ(2801, 2802, 2803, &mut params);
        calls.expect("glGetInternalformativ", "glGetInternalformativ 2801 2802 2803 2 p");
        calls.wrote("glGetInternalformativ", 4, &params);
    }
    {
        let mut info_log = vec![0_u8; 2];
        let answer = gl.get_program_info_log(2901, &mut info_log);
        calls.expect("glGetProgramInfoLog", "glGetProgramInfoLog 2901 2 p p");
        let out_2 = answer;
        calls.wrote("glGetProgramInfoLog", 2, &[out_2]);
        calls.wrote("glGetProgramInfoLog", 3, &info_log);
    }
    {
        let mut params = vec![0_i32; 4];
        gl.get_programiv(3001, 3002, &mut params);
        calls.expect("glGetProgramiv", "glGetProgramiv 3001 3002 p");
        calls.wrote("glGetProgramiv", 2, &params);
    }
    {
        let mut params = vec![0_u32; 4];
        gl.get_query_objectuiv(3101, 3102, &mut params);
        calls.expect("glGetQueryObjectuiv", "glGetQueryObjectuiv 3101 3102 p");
        calls.wrote("glGetQueryObjectuiv", 2, &params);
    }
    {
        let mut info_log = vec![0_u8; 2];
        let answer = gl.get_shader_info_log(3201, &mut info_log);
        calls.expect("glGetShaderInfoLog", "glGetShaderInfoLog 3201 2 p p");
        let out_2 = answer;
        calls.wrote("glGetShaderInfoLog", 2, &[out_2]);
        calls.wrote("glGetShaderInfoLog", 3, &info_log);
    }
    {
        let mut params = vec![0_i32; 4];
        gl.get_shaderiv(3301, 3302, &mut params);
        calls.expect("glGetShaderiv", "glGetShaderiv 3301 3302 p");
        calls.wrote("glGetShaderiv", 2, &params);
    }
    {
        let answer = gl.get_string(3401);
        calls.expect("glGetString", "glGetString 3401");
        let returned = answer;
        calls.returned_text("glGetString", returned.as_deref());
    }
    {
        let answer = gl.get_stringi(3501, 3502);
        calls.expect("glGetStringi", "glGetStringi 3501 3502");
        let returned = answer;
        calls.returned_text("glGetStringi", returned.as_deref());
    }
    {
        let sync = fence(gl);
        let mut values = vec![0_i32; 2];
        let answer = gl.get_synciv(&sync, 3602, &mut values);
        calls.expect("glGetSynciv", "glGetSynciv FENCE 3602 2 p p");
        let out_3 = answer;
        calls.wrote("glGetSynciv", 3, &[out_3]);
        calls.wrote("glGetSynciv", 4, &values);
    }
    {
        let answer = gl.get_uniform_block_index(3701, c"first text of 87");
        calls.expect("glGetUniformBlockIndex", "glGetUniformBlockIndex 3701 'first text of 87'");
        let returned = answer;
        calls.returned("glGetUniformBlockIndex", &returned);
    }
    {
        let answer = gl.get_uniform_location(3801, c"first text of 88");
        calls.expect("glGetUniformLocation", "glGetUniformLocation 3801 'first text of 88'");
        let returned = answer;
        calls.returned("glGetUniformLocation", &returned);
    }
    {
        let mut params = vec![0_i32; 4];
        gl.get_vertex_attribiv(3901, 3902, &mut params);
        calls.expect("glGetVertexAttribiv", "glGetVertexAttribiv 3901 3902 p");
        calls.wrote("glGetVertexAttribiv", 2, &params);
    }
    {
        let attachments = vec![3345_u32, 3346_u32];
        gl.invalidate_framebuffer(4001, &attachments);
        calls.expect("glInvalidateFramebuffer", "glInvalidateFramebuffer 4001 2 [3345,3346]");
    }
    {
        gl.link_program(4101);
        calls.expect("glLinkProgram", "glLinkProgram 4101");
    }
    {
        gl.pixel_storei(4201, 4202);
        calls.expect("glPixelStorei", "glPixelStorei 4201 4202");
    }
    {
        gl.polygon_offset(4301.5, 4302.5);
        calls.expect("glPolygonOffset", "glPolygonOffset 4301.5 4302.5");
    }
    {
        let mut pixels = vec![0_u8; ROOM];
        gl.read_pixels(4401, 4402, 2, 2, constants::RGBA, constants::UNSIGNED_BYTE, PixelPack::Bytes(&mut pixels));
        calls.expect("glReadPixels", "glReadPixels 4401 4402 2 2 6408 5121 p");
        calls.wrote_bytes("glReadPixels", 6, &pixels);
    }
    {
        gl.renderbuffer_storage(4501, 4502, 4503, 4504);
        calls.expect("glRenderbufferStorage", "glRenderbufferStorage 4501 4502 4503 4504");
    }
    {
        gl.renderbuffer_storage_multisample(4601, 4602, 4603, 4604, 4605);
        calls.expect("glRenderbufferStorageMultisample", "glRenderbufferStorageMultisample 4601 4602 4603 4604 4605");
    }
    {
        gl.sampler_parameterf(4701, 4702, 4703.5);
        calls.expect("glSamplerParameterf", "glSamplerParameterf 4701 4702 4703.5");
    }
    {
        gl.sampler_parameteri(4801, 4802, 4803);
        calls.expect("glSamplerParameteri", "glSamplerParameteri 4801 4802 4803");
    }
    {
        gl.scissor(4901, 4902, 4903, 4904);
        calls.expect("glScissor", "glScissor 4901 4902 4903 4904");
    }
    {
        gl.shader_source(5001, &[b"first text of 100".as_slice(), b"the second text of 100".as_slice()]);
        calls.expect("glShaderSource", "glShaderSource 5001 2 ['first text of 100','the second text of 100'] [17,22]");
    }
    {
        gl.stencil_func(5101, 5102, 5103);
        calls.expect("glStencilFunc", "glStencilFunc 5101 5102 5103");
    }
    {
        gl.stencil_func_separate(5201, 5202, 5203, 5204);
        calls.expect("glStencilFuncSeparate", "glStencilFuncSeparate 5201 5202 5203 5204");
    }
    {
        gl.stencil_mask(5301);
        calls.expect("glStencilMask", "glStencilMask 5301");
    }
    {
        gl.stencil_op(5401, 5402, 5403);
        calls.expect("glStencilOp", "glStencilOp 5401 5402 5403");
    }
    {
        gl.stencil_op_separate(5501, 5502, 5503, 5504);
        calls.expect("glStencilOpSeparate", "glStencilOpSeparate 5501 5502 5503 5504");
    }
    {
        gl.tex_parameteri(5601, 5602, 5603);
        calls.expect("glTexParameteri", "glTexParameteri 5601 5602 5603");
    }
    {
        gl.tex_storage_2d(5701, 5702, 5703, 5704, 5705);
        calls.expect("glTexStorage2D", "glTexStorage2D 5701 5702 5703 5704 5705");
    }
    {
        gl.tex_storage_3d(5801, 5802, 5803, 5804, 5805, 5806);
        calls.expect("glTexStorage3D", "glTexStorage3D 5801 5802 5803 5804 5805 5806");
    }
    {
        let pixels = patterned(ROOM, 1752);
        gl.tex_sub_image_2d(5901, 5902, 5903, 5904, 2, 2, constants::RGBA, constants::UNSIGNED_BYTE, PixelUnpack::Bytes(&pixels));
        calls.expect("glTexSubImage2D", &format!("glTexSubImage2D 5901 5902 5903 5904 2 2 6408 5121 {}", bytes(&pixels[..PIXELS])));
    }
    {
        let pixels = patterned(ROOM, 1770);
        gl.tex_sub_image_3d(6001, 6002, 6003, 6004, 6005, 2, 2, 1, constants::RGBA, constants::UNSIGNED_BYTE, PixelUnpack::Bytes(&pixels));
        calls.expect("glTexSubImage3D", &format!("glTexSubImage3D 6001 6002 6003 6004 6005 2 2 1 6408 5121 {}", bytes(&pixels[..PIXELS])));
    }
    {
        gl.transform_feedback_varyings(6101, &[c"first text of 111", c"the second text of 111"], 6104);
        calls.expect("glTransformFeedbackVaryings", "glTransformFeedbackVaryings 6101 2 ['first text of 111','the second text of 111'] 6104");
    }
    {
        gl.uniform1f(6201, 6202.5);
        calls.expect("glUniform1f", "glUniform1f 6201 6202.5");
    }
    {
        let value = vec![4196.5_f32, 4197.5_f32];
        gl.uniform1fv(6301, &value);
        calls.expect("glUniform1fv", "glUniform1fv 6301 2 [0x45832400,0x45832c00]");
    }
    {
        gl.uniform1i(6401, 6402);
        calls.expect("glUniform1i", "glUniform1i 6401 6402");
    }
    {
        let value = vec![4270_i32, 4271_i32];
        gl.uniform1iv(6501, &value);
        calls.expect("glUniform1iv", "glUniform1iv 6501 2 [4270,4271]");
    }
    {
        gl.uniform1ui(6601, 6602);
        calls.expect("glUniform1ui", "glUniform1ui 6601 6602");
    }
    {
        let value = vec![4344_u32, 4345_u32];
        gl.uniform1uiv(6701, &value);
        calls.expect("glUniform1uiv", "glUniform1uiv 6701 2 [4344,4345]");
    }
    {
        gl.uniform2f(6801, 6802.5, 6803.5);
        calls.expect("glUniform2f", "glUniform2f 6801 6802.5 6803.5");
    }
    {
        let value = vec![4418.5_f32, 4419.5_f32, 4420.5_f32, 4421.5_f32];
        gl.uniform2fv(6901, &value);
        calls.expect("glUniform2fv", "glUniform2fv 6901 2 [0x458a1400,0x458a1c00,0x458a2400,0x458a2c00]");
    }
    {
        let value = vec![4455_i32, 4456_i32, 4457_i32, 4458_i32];
        gl.uniform2iv(1001, &value);
        calls.expect("glUniform2iv", "glUniform2iv 1001 2 [4455,4456,4457,4458]");
    }
    {
        let value = vec![4492_u32, 4493_u32, 4494_u32, 4495_u32];
        gl.uniform2uiv(1101, &value);
        calls.expect("glUniform2uiv", "glUniform2uiv 1101 2 [4492,4493,4494,4495]");
    }
    {
        let value = vec![4529.5_f32, 4530.5_f32, 4531.5_f32, 4532.5_f32, 4533.5_f32, 4534.5_f32];
        gl.uniform3fv(1201, &value);
        calls.expect("glUniform3fv", "glUniform3fv 1201 2 [0x458d8c00,0x458d9400,0x458d9c00,0x458da400,0x458dac00,0x458db400]");
    }
    {
        gl.uniform3i(1301, 1302, 1303, 1304);
        calls.expect("glUniform3i", "glUniform3i 1301 1302 1303 1304");
    }
    {
        let value = vec![4603_i32, 4604_i32, 4605_i32, 4606_i32, 4607_i32, 4608_i32];
        gl.uniform3iv(1401, &value);
        calls.expect("glUniform3iv", "glUniform3iv 1401 2 [4603,4604,4605,4606,4607,4608]");
    }
    {
        let value = vec![4640_u32, 4641_u32, 4642_u32, 4643_u32, 4644_u32, 4645_u32];
        gl.uniform3uiv(1501, &value);
        calls.expect("glUniform3uiv", "glUniform3uiv 1501 2 [4640,4641,4642,4643,4644,4645]");
    }
    {
        let value = vec![4677.5_f32, 4678.5_f32, 4679.5_f32, 4680.5_f32, 4681.5_f32, 4682.5_f32, 4683.5_f32, 4684.5_f32];
        gl.uniform4fv(1601, &value);
        calls.expect("glUniform4fv", "glUniform4fv 1601 2 [0x45922c00,0x45923400,0x45923c00,0x45924400,0x45924c00,0x45925400,0x45925c00,0x45926400]");
    }
    {
        let value = vec![4714_i32, 4715_i32, 4716_i32, 4717_i32, 4718_i32, 4719_i32, 4720_i32, 4721_i32];
        gl.uniform4iv(1701, &value);
        calls.expect("glUniform4iv", "glUniform4iv 1701 2 [4714,4715,4716,4717,4718,4719,4720,4721]");
    }
    {
        let value = vec![4751_u32, 4752_u32, 4753_u32, 4754_u32, 4755_u32, 4756_u32, 4757_u32, 4758_u32];
        gl.uniform4uiv(1801, &value);
        calls.expect("glUniform4uiv", "glUniform4uiv 1801 2 [4751,4752,4753,4754,4755,4756,4757,4758]");
    }
    {
        gl.uniform_block_binding(1901, 1902, 1903);
        calls.expect("glUniformBlockBinding", "glUniformBlockBinding 1901 1902 1903");
    }
    {
        let value = vec![4832.5_f32, 4833.5_f32, 4834.5_f32, 4835.5_f32, 4836.5_f32, 4837.5_f32, 4838.5_f32, 4839.5_f32];
        gl.uniform_matrix2fv(2001, true, &value);
        calls.expect("glUniformMatrix2fv", "glUniformMatrix2fv 2001 2 1 [0x45970400,0x45970c00,0x45971400,0x45971c00,0x45972400,0x45972c00,0x45973400,0x45973c00]");
    }
    {
        let value = vec![4869.5_f32, 4870.5_f32, 4871.5_f32, 4872.5_f32, 4873.5_f32, 4874.5_f32, 4875.5_f32, 4876.5_f32, 4877.5_f32, 4878.5_f32, 4879.5_f32, 4880.5_f32];
        gl.uniform_matrix2x3fv(2101, true, &value);
        calls.expect("glUniformMatrix2x3fv", "glUniformMatrix2x3fv 2101 2 1 [0x45982c00,0x45983400,0x45983c00,0x45984400,0x45984c00,0x45985400,0x45985c00,0x45986400,0x45986c00,0x45987400,0x45987c00,0x45988400]");
    }
    {
        let value = vec![4906.5_f32, 4907.5_f32, 4908.5_f32, 4909.5_f32, 4910.5_f32, 4911.5_f32, 4912.5_f32, 4913.5_f32, 4914.5_f32, 4915.5_f32, 4916.5_f32, 4917.5_f32, 4918.5_f32, 4919.5_f32, 4920.5_f32, 4921.5_f32];
        gl.uniform_matrix2x4fv(2201, true, &value);
        calls.expect("glUniformMatrix2x4fv", "glUniformMatrix2x4fv 2201 2 1 [0x45995400,0x45995c00,0x45996400,0x45996c00,0x45997400,0x45997c00,0x45998400,0x45998c00,0x45999400,0x45999c00,0x4599a400,0x4599ac00,0x4599b400,0x4599bc00,0x4599c400,0x4599cc00]");
    }
    {
        let value = vec![4943.5_f32, 4944.5_f32, 4945.5_f32, 4946.5_f32, 4947.5_f32, 4948.5_f32, 4949.5_f32, 4950.5_f32, 4951.5_f32, 4952.5_f32, 4953.5_f32, 4954.5_f32, 4955.5_f32, 4956.5_f32, 4957.5_f32, 4958.5_f32, 4959.5_f32, 4960.5_f32];
        gl.uniform_matrix3fv(2301, true, &value);
        calls.expect("glUniformMatrix3fv", "glUniformMatrix3fv 2301 2 1 [0x459a7c00,0x459a8400,0x459a8c00,0x459a9400,0x459a9c00,0x459aa400,0x459aac00,0x459ab400,0x459abc00,0x459ac400,0x459acc00,0x459ad400,0x459adc00,0x459ae400,0x459aec00,0x459af400,0x459afc00,0x459b0400]");
    }
    {
        let value = vec![4980.5_f32, 4981.5_f32, 4982.5_f32, 4983.5_f32, 4984.5_f32, 4985.5_f32, 4986.5_f32, 4987.5_f32, 4988.5_f32, 4989.5_f32, 4990.5_f32, 4991.5_f32];
        gl.uniform_matrix3x2fv(2401, true, &value);
        calls.expect("glUniformMatrix3x2fv", "glUniformMatrix3x2fv 2401 2 1 [0x459ba400,0x459bac00,0x459bb400,0x459bbc00,0x459bc400,0x459bcc00,0x459bd400,0x459bdc00,0x459be400,0x459bec00,0x459bf400,0x459bfc00]");
    }
    {
        let value = vec![5017.5_f32, 5018.5_f32, 5019.5_f32, 5020.5_f32, 5021.5_f32, 5022.5_f32, 5023.5_f32, 5024.5_f32, 5025.5_f32, 5026.5_f32, 5027.5_f32, 5028.5_f32, 5029.5_f32, 5030.5_f32, 5031.5_f32, 5032.5_f32, 5033.5_f32, 5034.5_f32, 5035.5_f32, 5036.5_f32, 5037.5_f32, 5038.5_f32, 5039.5_f32, 5040.5_f32];
        gl.uniform_matrix3x4fv(2501, true, &value);
        calls.expect("glUniformMatrix3x4fv", "glUniformMatrix3x4fv 2501 2 1 [0x459ccc00,0x459cd400,0x459cdc00,0x459ce400,0x459cec00,0x459cf400,0x459cfc00,0x459d0400,0x459d0c00,0x459d1400,0x459d1c00,0x459d2400,0x459d2c00,0x459d3400,0x459d3c00,0x459d4400,0x459d4c00,0x459d5400,0x459d5c00,0x459d6400,0x459d6c00,0x459d7400,0x459d7c00,0x459d8400]");
    }
    {
        let value = vec![5054.5_f32, 5055.5_f32, 5056.5_f32, 5057.5_f32, 5058.5_f32, 5059.5_f32, 5060.5_f32, 5061.5_f32, 5062.5_f32, 5063.5_f32, 5064.5_f32, 5065.5_f32, 5066.5_f32, 5067.5_f32, 5068.5_f32, 5069.5_f32, 5070.5_f32, 5071.5_f32, 5072.5_f32, 5073.5_f32, 5074.5_f32, 5075.5_f32, 5076.5_f32, 5077.5_f32, 5078.5_f32, 5079.5_f32, 5080.5_f32, 5081.5_f32, 5082.5_f32, 5083.5_f32, 5084.5_f32, 5085.5_f32];
        gl.uniform_matrix4fv(2601, true, &value);
        calls.expect("glUniformMatrix4fv", "glUniformMatrix4fv 2601 2 1 [0x459df400,0x459dfc00,0x459e0400,0x459e0c00,0x459e1400,0x459e1c00,0x459e2400,0x459e2c00,0x459e3400,0x459e3c00,0x459e4400,0x459e4c00,0x459e5400,0x459e5c00,0x459e6400,0x459e6c00,0x459e7400,0x459e7c00,0x459e8400,0x459e8c00,0x459e9400,0x459e9c00,0x459ea400,0x459eac00,0x459eb400,0x459ebc00,0x459ec400,0x459ecc00,0x459ed400,0x459edc00,0x459ee400,0x459eec00]");
    }
    {
        let value = vec![5091.5_f32, 5092.5_f32, 5093.5_f32, 5094.5_f32, 5095.5_f32, 5096.5_f32, 5097.5_f32, 5098.5_f32, 5099.5_f32, 5100.5_f32, 5101.5_f32, 5102.5_f32, 5103.5_f32, 5104.5_f32, 5105.5_f32, 5106.5_f32];
        gl.uniform_matrix4x2fv(2701, true, &value);
        calls.expect("glUniformMatrix4x2fv", "glUniformMatrix4x2fv 2701 2 1 [0x459f1c00,0x459f2400,0x459f2c00,0x459f3400,0x459f3c00,0x459f4400,0x459f4c00,0x459f5400,0x459f5c00,0x459f6400,0x459f6c00,0x459f7400,0x459f7c00,0x459f8400,0x459f8c00,0x459f9400]");
    }
    {
        let value = vec![5128.5_f32, 5129.5_f32, 5130.5_f32, 5131.5_f32, 5132.5_f32, 5133.5_f32, 5134.5_f32, 5135.5_f32, 5136.5_f32, 5137.5_f32, 5138.5_f32, 5139.5_f32, 5140.5_f32, 5141.5_f32, 5142.5_f32, 5143.5_f32, 5144.5_f32, 5145.5_f32, 5146.5_f32, 5147.5_f32, 5148.5_f32, 5149.5_f32, 5150.5_f32, 5151.5_f32];
        gl.uniform_matrix4x3fv(2801, true, &value);
        calls.expect("glUniformMatrix4x3fv", "glUniformMatrix4x3fv 2801 2 1 [0x45a04400,0x45a04c00,0x45a05400,0x45a05c00,0x45a06400,0x45a06c00,0x45a07400,0x45a07c00,0x45a08400,0x45a08c00,0x45a09400,0x45a09c00,0x45a0a400,0x45a0ac00,0x45a0b400,0x45a0bc00,0x45a0c400,0x45a0cc00,0x45a0d400,0x45a0dc00,0x45a0e400,0x45a0ec00,0x45a0f400,0x45a0fc00]");
    }
    {
        gl.use_program(2901);
        calls.expect("glUseProgram", "glUseProgram 2901");
    }
    {
        gl.vertex_attrib_divisor(3001, 3002);
        calls.expect("glVertexAttribDivisor", "glVertexAttribDivisor 3001 3002");
    }
    {
        gl.vertex_attrib_i_pointer(3101, 3102, 3103, 3104, 16);
        calls.expect("glVertexAttribIPointer", "glVertexAttribIPointer 3101 3102 3103 3104 16");
    }
    {
        gl.vertex_attrib_pointer(3201, 3202, 3203, true, 3205, 16);
        calls.expect("glVertexAttribPointer", "glVertexAttribPointer 3201 3202 3203 1 3205 16");
    }
    {
        gl.viewport(3301, 3302, 3303, 3304);
        calls.expect("glViewport", "glViewport 3301 3302 3303 3304");
    }
    {
        let answer = gl.blend_equation_separatei_oes(3401, 3402, 3403);
        calls.expect("glBlendEquationSeparateiOES", "glBlendEquationSeparateiOES 3401 3402 3403");
        if answer.is_none() { calls.failed("glBlendEquationSeparateiOES answered None with its extensions named"); }
        
    }
    {
        let answer = gl.blend_func_separatei_oes(3501, 3502, 3503, 3504, 3505);
        calls.expect("glBlendFuncSeparateiOES", "glBlendFuncSeparateiOES 3501 3502 3503 3504 3505");
        if answer.is_none() { calls.failed("glBlendFuncSeparateiOES answered None with its extensions named"); }
        
    }
    {
        let answer = gl.clip_control_ext(3601, 3602);
        calls.expect("glClipControlEXT", "glClipControlEXT 3601 3602");
        if answer.is_none() { calls.failed("glClipControlEXT answered None with its extensions named"); }
        
    }
    {
        let answer = gl.color_maski_oes(3701, true, true, true, true);
        calls.expect("glColorMaskiOES", "glColorMaskiOES 3701 1 1 1 1");
        if answer.is_none() { calls.failed("glColorMaskiOES answered None with its extensions named"); }
        
    }
    {
        let answer = gl.disablei_oes(3801, 3802);
        calls.expect("glDisableiOES", "glDisableiOES 3801 3802");
        if answer.is_none() { calls.failed("glDisableiOES answered None with its extensions named"); }
        
    }
    {
        let answer = gl.enablei_oes(3901, 3902);
        calls.expect("glEnableiOES", "glEnableiOES 3901 3902");
        if answer.is_none() { calls.failed("glEnableiOES answered None with its extensions named"); }
        
    }
    {
        let mut params = vec![0_u64; 4];
        let answer = gl.get_query_objectui64v_ext(4001, 4002, &mut params);
        calls.expect("glGetQueryObjectui64vEXT", "glGetQueryObjectui64vEXT 4001 4002 p");
        if answer.is_none() { calls.failed("glGetQueryObjectui64vEXT answered None with its extensions named"); }
        calls.wrote("glGetQueryObjectui64vEXT", 2, &params);
    }
    {
        let mut params = vec![0_i32; 4];
        let answer = gl.get_queryiv_ext(4101, 4102, &mut params);
        calls.expect("glGetQueryivEXT", "glGetQueryivEXT 4101 4102 p");
        if answer.is_none() { calls.failed("glGetQueryivEXT answered None with its extensions named"); }
        calls.wrote("glGetQueryivEXT", 2, &params);
    }
    {
        let answer = gl.polygon_mode_angle(4201, 4202);
        calls.expect("glPolygonModeANGLE", "glPolygonModeANGLE 4201 4202");
        if answer.is_none() { calls.failed("glPolygonModeANGLE answered None with its extensions named"); }
        
    }
    {
        let answer = gl.polygon_offset_clamp_ext(4301.5, 4302.5, 4303.5);
        calls.expect("glPolygonOffsetClampEXT", "glPolygonOffsetClampEXT 4301.5 4302.5 4303.5");
        if answer.is_none() { calls.failed("glPolygonOffsetClampEXT answered None with its extensions named"); }
        
    }
    {
        let answer = gl.provoking_vertex_angle(4401);
        calls.expect("glProvokingVertexANGLE", "glProvokingVertexANGLE 4401");
        if answer.is_none() { calls.failed("glProvokingVertexANGLE answered None with its extensions named"); }
        
    }
    {
        let answer = gl.query_counter_ext(4501, 4502);
        calls.expect("glQueryCounterEXT", "glQueryCounterEXT 4501 4502");
        if answer.is_none() { calls.failed("glQueryCounterEXT answered None with its extensions named"); }
        
    }
}
