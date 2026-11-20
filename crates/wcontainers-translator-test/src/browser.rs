//! The browser runner: a flat blob run in Chrome by `browser/runner.js`, with the translator built for wasm32
//! from `browser/` so that code is translated at run time there too.

use crate::layout::SIGNALLED;
use crate::programs::{Blob, Ran, placement};
use crate::reference::{release_build, target_folder};
use linux_raw_sys::general::{SIGBUS, SIGFPE, SIGILL, SIGSEGV, SIGTRAP};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
use wcontainers_abi::Permissions;
use wcontainers_arm64::INSTRUCTION_BYTES;
use wcontainers_translator::{LookAhead, Omit, Optimise};

/// The browser translator's package folder beside this crate's manifest, also its folder under `target`.
const PACKAGE: &str = "browser";
/// Rust's name for the target the browser translator is built for, and the module that build makes.
const MODULE_TRIPLE: &str = "wasm32-unknown-unknown";
const MODULE: &str = "wcontainers_browser.wasm";
/// The folder of the browser's target folder that holds a folder for each case.
const CASES: &str = "cases";
/// The files of a case's folder the runner loads: the translator module, the blob and its stack.
const TRANSLATOR_FILE: &str = "translator.wasm";
const BLOB_FILE: &str = "blob.bin";
const STACK_FILE: &str = "stack.bin";
/// The case's setup and its instruction cases, which `run.mjs` reads from the case's folder.
const SETUP_FILE: &str = "case.json";
const CASES_FILE: &str = "cases.bin";
/// The folder of instruction cases under [`CASES`].
const INSTRUCTION_CASES: &str = "instructions";
/// The program that runs the browser runner's script, and that script beside this crate's manifest.
const NODE: &str = "node";
const RUNNER_SCRIPT: &str = "run.mjs";
/// The variable through which the runner script gives V8 its flags.
const JS_FLAGS_VARIABLE: &str = "WC_CHROME_JS_FLAGS";
/// What Chrome's DevTools protocol reports when a page crashes.
const PAGE_CRASHED: &str = "Target crashed";

fn package() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PACKAGE)
}

fn target() -> PathBuf {
    target_folder(PACKAGE)
}

/// Build the translator for the browser, once per process, and return the module's path.
pub fn translator() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| release_build(&package(), MODULE_TRIPLE, &target()).join(MODULE))
        .clone()
}

/// The folder of the case `name`, holding the translator module, `blob` and `stack`.
fn case_folder(name: &str, blob: &[u8], stack: &[u8]) -> PathBuf {
    let directory = target().join(CASES).join(name);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::copy(translator(), directory.join(TRANSLATOR_FILE)).unwrap();
    std::fs::write(directory.join(BLOB_FILE), blob).unwrap();
    std::fs::write(directory.join(STACK_FILE), stack).unwrap();
    directory
}

/// What happened in Chrome: the program's output and status, and the runner's report.
pub struct Report {
    pub ran: Ran,
    /// How long the guest ran in the page each time it was run, translating and compiling the first module
    /// not included.
    pub runs: Vec<f64>,
    /// The slow paths taken, as `(reason, count, nanoseconds spent)`.
    pub counters: Vec<(String, u64, u64)>,
    pub user_agent: String,
}

impl Report {
    pub fn count(&self, reason: &str) -> u64 {
        self.counters
            .iter()
            .find(|(name, ..)| name == reason)
            .map_or(0, |c| c.1)
    }

    /// The time spent on a slow path, in nanoseconds.
    pub fn time(&self, reason: &str) -> u64 {
        self.counters
            .iter()
            .find(|(name, ..)| name == reason)
            .map_or(0, |c| c.2)
    }
}

/// Run a blob in Chrome at look-ahead none, starting at its first byte.
pub fn translated(blob: &Blob) -> Report {
    translated_with(blob, (LookAhead::None, Optimise::Speed), Omit::default())
}

/// Run a blob in Chrome at a look-ahead, translated for speed or size, leaving out some checks only to
/// measure them.
pub fn translated_with(blob: &Blob, how: (LookAhead, Optimise), omit: Omit) -> Report {
    repeated(blob, how, omit, 1)
}

/// [`translated_with`], running the program `repeat` times in the same instances, each run timed by itself.
/// The report holds the output of the last run.
pub fn repeated(blob: &Blob, how: (LookAhead, Optimise), omit: Omit, repeat: u32) -> Report {
    run_in_chrome(blob, how, omit, repeat, None, 0)
}

/// [`repeated`] at full look-ahead, the translation split into modules of at most `module_bytes` of code,
/// only to measure calls between modules.
pub fn repeated_split(blob: &Blob, optimise: Optimise, repeat: u32, module_bytes: u32) -> Report {
    let how = (LookAhead::Full, optimise);
    run_in_chrome(blob, how, Omit::default(), repeat, None, module_bytes)
}

/// [`translated_with`] with V8 given these flags, only for measuring what the engine does (such as compiling
/// every function when its module is made, not at its first call).
pub fn translated_with_engine_flags(blob: &Blob, look_ahead: LookAhead, flags: &str) -> Report {
    let how = (look_ahead, Optimise::Speed);
    run_in_chrome(blob, how, Omit::default(), 1, Some(flags), 0)
}

fn run_in_chrome(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    repeat: u32,
    flags: Option<&str>,
    module_bytes: u32,
) -> Report {
    let place = placement(blob);
    let directory = case_folder(&blob.name, &blob.bytes, &place.stack);
    let (look_ahead, bound) = look_ahead.code();
    let setup = Setup {
        bound: Some(bound),
        code_bytes: blob.code_bytes,
        keep_float_flags: Some(blob.reads_float_status()),
        load: blob.load,
        look_ahead,
        memory_bytes: place.memory_bytes,
        omit: omit.bits(),
        optimise: Some(optimise as u8),
        regions: regions(&place.regions),
        repeat,
        sp: Some(place.sp),
        split_bytes: Some(module_bytes),
        ..Setup::default()
    };
    let result = launch(&directory, &setup, flags);
    let answer: RunAnswer = serde_json::from_value(result.clone()).expect("the runner's answer");
    let stop: Stop = serde_json::from_value(answer.stopped.clone()).expect("the runner's stop");
    let status = match stop.kind.as_str() {
        // Linux reports the low 8 bits of the status.
        FINISHED => i32::from(stop.status.expect("an exit status") as u8),
        kind => match STOP_SIGNALS.iter().find(|(name, _)| *name == kind) {
            Some((_, signal)) => SIGNALLED + *signal as i32,
            None => panic!(
                "the program stopped without exiting in Chrome: {} at {}",
                answer.stopped, answer.pc
            ),
        },
    };
    Report {
        ran: Ran {
            stdout: answer.stdout,
            stderr: answer.stderr,
            status,
        },
        counters: answer
            .counters
            .into_iter()
            .map(|c| (c.reason, c.count, c.nanoseconds))
            .collect(),
        runs: answer
            .runs
            .iter()
            .map(|ms| ms * Duration::from_millis(1).as_secs_f64())
            .collect(),
        user_agent: answer.user_agent.unwrap_or_default(),
    }
}

/// The runner's name for a program that exited, and its names for the other ways a program stops, with the
/// signal Linux gives for each.
const FINISHED: &str = "finished";
const STOP_SIGNALS: [(&str, u32); 10] = [
    ("memory fault", SIGSEGV),
    ("general protection", SIGSEGV),
    ("overflow", SIGSEGV),
    ("alignment", SIGBUS),
    ("stack fault", SIGBUS),
    ("undefined", SIGILL),
    ("unsupported", SIGILL),
    ("breakpoint", SIGTRAP),
    ("divide error", SIGFPE),
    ("float exception", SIGFPE),
];

/// What the runner is told about a case, written to the case's folder as JSON. The fields are in the
/// order of their names, as the JSON has always had them.
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Setup {
    /// Instruction cases' places: start and length of each.
    #[serde(skip_serializing_if = "Option::is_none")]
    areas: Option<Vec<(u64, u64)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bound: Option<u32>,
    /// How many instruction cases `cases.bin` holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    cases: Option<usize>,
    code_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_start: Option<u64>,
    /// Whether FPSR's flags are kept exact from the thread's start.
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_float_flags: Option<bool>,
    load: u64,
    look_ahead: u8,
    memory_bytes: u64,
    /// How many code modules translated ahead of time the folder holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    modules: Option<usize>,
    omit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    optimise: Option<u8>,
    /// Start, end and permissions of each region.
    regions: Vec<(u64, u64, u8)>,
    repeat: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    sp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    split_bytes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_at_syscall: Option<bool>,
}

fn regions(regions: &[(Range<u64>, Permissions)]) -> Vec<(u64, u64, u8)> {
    regions
        .iter()
        .map(|(range, permissions)| (range.start, range.end, permissions.bits()))
        .collect()
}

/// The runner's answer for a program run.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunAnswer {
    stopped: Value,
    pc: Value,
    counters: Vec<Counter>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    /// Milliseconds of each run.
    runs: Vec<f64>,
    user_agent: Option<String>,
}

#[derive(Deserialize)]
struct Stop {
    kind: String,
    status: Option<i64>,
}

#[derive(Deserialize)]
struct Counter {
    reason: String,
    count: u64,
    nanoseconds: u64,
}

/// The runner's answer when the page threw.
#[derive(Deserialize)]
struct Refusal {
    error: Option<Value>,
}

/// The runner's answer for loading modules.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadAnswer {
    compiling: f64,
    instantiating: f64,
    memory_before: i64,
    memory_after: i64,
}

/// The runner's answer for instruction cases.
#[derive(Deserialize)]
struct CasesAnswer {
    cases: Vec<StateRun>,
}

/// Run the browser runner on a case directory holding `translator.wasm`, `blob.bin`, `stack.bin` and, for
/// instruction cases, `cases.bin`, with this setup and V8 flags if given; give its answer.
fn launch(directory: &Path, setup: &Setup, flags: Option<&str>) -> Value {
    launch_or_refused(directory, setup, flags)
        .unwrap_or_else(|error| panic!("the browser runner failed: {error}"))
}

/// [`launch`], or what the page threw.
fn launch_or_refused(
    directory: &Path,
    setup: &Setup,
    flags: Option<&str>,
) -> Result<Value, String> {
    std::fs::write(
        directory.join(SETUP_FILE),
        serde_json::to_string(setup).unwrap(),
    )
    .unwrap();

    let mut command = Command::new(NODE);
    if let Some(flags) = flags {
        command.env(JS_FLAGS_VARIABLE, flags);
    }
    let output = command
        .arg(package().join(RUNNER_SCRIPT))
        .arg(directory)
        .output()
        .expect("run node (from the Nix shell)");
    let errors = String::from_utf8_lossy(&output.stderr);
    eprint!("{errors}");
    // A page that crashes (the engine giving up on a module) is a refusal too.
    if errors.contains(PAGE_CRASHED) {
        return Err("the page crashed".into());
    }
    assert!(output.status.success(), "the browser runner failed");
    let result: Value = serde_json::from_slice(&output.stdout).expect("the runner's JSON");
    let refusal: Refusal = serde_json::from_value(result.clone()).expect("the runner's answer");
    match refusal.error {
        Some(error) => Err(error.as_str().unwrap_or_default().to_string()),
        None => Ok(result),
    }
}

/// What loading code modules in Chrome took.
pub struct Loaded {
    /// Milliseconds making the modules took, and making their instances.
    pub compiling: f64,
    pub instantiating: f64,
    /// Bytes the resident memory of Chrome's processes grew by from before the worker started to after.
    pub memory: i64,
}

/// Load code modules translated ahead of time from an image placed at `load`, its code at `code`, in Chrome,
/// V8 given these flags, without running anything; or what the page threw, such as an engine refusing a
/// limit. The lookup table covers the code alone.
pub fn load_modules(
    name: &str,
    image: &[u8],
    load: u64,
    code: std::ops::Range<u64>,
    modules: &[&[u8]],
    flags: &str,
) -> Result<Loaded, String> {
    let directory = case_folder(&format!("load-{name}"), image, &[]);
    for (i, module) in modules.iter().enumerate() {
        std::fs::write(directory.join(format!("module-{i}.wasm")), module).unwrap();
    }
    let memory = (load + image.len() as u64)
        .next_multiple_of(wcontainers_translator::contract::WASM_PAGE_BYTES);
    let setup = Setup {
        code_bytes: code.end,
        code_start: Some(code.start),
        load,
        look_ahead: LookAhead::Full.code().0,
        memory_bytes: memory,
        modules: Some(modules.len()),
        ..Setup::default()
    };
    let result = launch_or_refused(&directory, &setup, Some(flags));
    std::fs::remove_dir_all(&directory).unwrap();
    let answer: LoadAnswer = serde_json::from_value(result?).expect("the runner's answer");
    Ok(Loaded {
        compiling: answer.compiling,
        instantiating: answer.instantiating,
        memory: answer.memory_after - answer.memory_before,
    })
}

/// One instruction case: its code word, the thread state to start from (the contract's
/// `THREAD_STATE_SIZE` bytes) and the bytes to put at each place.
pub struct StateCase {
    pub word: u32,
    pub state: Vec<u8>,
    pub places: Vec<Vec<u8>>,
}

/// How an instruction case ended in Chrome: the runner's stop (its `kind` and details), the thread state
/// after, and the bytes of the places that changed, as offsets into the places taken one after another.
#[derive(Deserialize)]
pub struct StateRun {
    pub stopped: Value,
    pub state: Vec<u8>,
    pub changed: Vec<(u32, u8)>,
}

/// Run instruction cases one after another in one Chrome page at look-ahead none, each word at `code` in a
/// memory of `memory_bytes` with these regions, ending at a system call or where control lands.
pub fn cases(
    code: u64,
    memory_bytes: u64,
    regions_given: &[(Range<u64>, Permissions)],
    places: &[Range<u64>],
    cases: &[StateCase],
) -> Vec<StateRun> {
    let directory = case_folder(INSTRUCTION_CASES, &cases[0].word.to_le_bytes(), &[]);
    let mut bytes = Vec::new();
    for case in cases {
        bytes.extend(case.word.to_le_bytes());
        bytes.extend(&case.state);
        for place in &case.places {
            bytes.extend(place);
        }
    }
    std::fs::write(directory.join(CASES_FILE), bytes).unwrap();
    let setup = Setup {
        areas: Some(
            places
                .iter()
                .map(|range| (range.start, range.end - range.start))
                .collect(),
        ),
        cases: Some(cases.len()),
        code_bytes: INSTRUCTION_BYTES as u64,
        load: code,
        look_ahead: LookAhead::None.code().0,
        memory_bytes,
        regions: regions(regions_given),
        stop_at_syscall: Some(true),
        ..Setup::default()
    };
    let answer: CasesAnswer =
        serde_json::from_value(launch(&directory, &setup, None)).expect("the runner's answer");
    answer.cases
}
