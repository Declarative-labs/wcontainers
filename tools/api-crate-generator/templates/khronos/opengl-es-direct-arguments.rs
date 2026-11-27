//! Every argument of every OpenGL ES method reaches the library as the method was given it: the generated stand-in
//! library (fake/fake_egl.c), built here with the C compiler (WCONTAINERS_CC, else `cc`), writes each call's arguments
//! to the file FAKE_LOG names (a number as C has it, a float to 9 digits, an address as `p` or `null`, an offset into a
//! bound buffer as its number), and each method is called once with arguments of its own, its line compared with what
//! the registry says the call passes (each value as given, each count its slice's length over its multiplier, each
//! pointer one of the caller's or ours). The calls run in a process of their own (this test run again), which the
//! log's variable is set for.
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

/// The calls checked so far, the log's lines read, and what differed.
struct Calls {
    log: PathBuf,
    read: usize,
    checked: usize,
    failures: Vec<String>,
}

impl Calls {
    /// The last line `command` logged since the last check must be `expected`.
    fn expect(&mut self, command: &str, expected: &str) {
        let text = std::fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let new = lines.get(self.read..).unwrap_or_default();
        let spaced = format!("{command} ");
        let received = new.iter().rev().find(|line| **line == command || line.starts_with(&spaced)).copied();
        if received != Some(expected) {
            self.failures.push(format!("{command}: the library received {received:?}, expected {expected:?}"));
        }
        self.read = lines.len();
        self.checked += 1;
    }
}

#[test]
fn every_argument_reaches_the_library() {
    let Some(log) = std::env::var_os(LOG_VARIABLE).map(PathBuf::from) else {
        let folder = built();
        let log = folder.join("calls.log");
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
    let mut calls = Calls { log, read: 0, checked: 0, failures: Vec::new() };
    every_method(&gl, &mut calls);
    println!("ARGUMENTS {} methods checked, {} received otherwise", calls.checked, calls.failures.len());
    assert!(calls.failures.is_empty(), "{}", calls.failures.join("\n"));
}

/// A fence this context made, for a method that takes one.
fn fence(gl: &OpenGlEsContext) -> <OpenGlEsContext as OpenGlEs>::Fence {
    gl.fence_sync(constants::SYNC_GPU_COMMANDS_COMPLETE, 0).expect("a fence")
}
// METHODS
