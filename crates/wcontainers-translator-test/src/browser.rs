//! The browser runner: a flat blob run in Chrome by `browser/runner.js`, with the translator built for wasm32
//! from `browser/` so that code is translated at run time there too.

use crate::guest::GuestTest;
use crate::layout::SIGNALLED;
use crate::programs::{Blob, Ran, placement};
use crate::reference::{release_build_with, target_folder};
use linux_raw_sys::general::{SIGBUS, SIGFPE, SIGILL, SIGSEGV, SIGTRAP};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use wcontainers_abi::Permissions;
use wcontainers_guest_cpu::{MemoryFault, TrapKind};
use wcontainers_translator::{LookAhead, Omit, Optimise};

/// The browser translator's package folder beside this crate's manifest, also its folder under `target`.
const PACKAGE: &str = "browser";
/// Rust's name for the target the browser translator is built for, and the module that build makes.
const MODULE_TRIPLE: &str = "wasm32-unknown-unknown";
const MODULE: &str = "wcontainers_browser.wasm";
/// Cargo's flag that turns on a package's features; the browser translator's name its guest.
const FEATURES_FLAG: &str = "--features";
/// Where the page's runs keep their files, a folder for each, under the browser build.
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
/// The kinds of stop the browser runner reports beside a trap's (`TrapKind::name`).
pub const FINISHED: &str = "finished";
pub const SYSCALL: &str = "system call";
pub const SINGLE_STEP: &str = "single step";
pub const MEMORY_FAULT: &str = "memory fault";
pub const STACK_FAULT: &str = "stack fault";
pub const NO_CODE: &str = "no code";
const NO_EXACT_REGISTERS: &str = "fault without exact registers";
const STOP_KINDS: &[&str] = &[
    FINISHED,
    SYSCALL,
    SINGLE_STEP,
    MEMORY_FAULT,
    STACK_FAULT,
    NO_CODE,
    NO_EXACT_REGISTERS,
];

/// The stop a memory fault of each kind gives.
pub fn memory_fault_stop(fault: MemoryFault) -> &'static str {
    match fault {
        MemoryFault::Page => MEMORY_FAULT,
        MemoryFault::GeneralProtection => TrapKind::GeneralProtection.name(),
        MemoryFault::Stack => STACK_FAULT,
    }
}

/// The signal Linux sends for a trap of each kind.
pub fn trap_signal(trap: TrapKind) -> u32 {
    match trap {
        TrapKind::Undefined | TrapKind::Unsupported => SIGILL,
        TrapKind::Breakpoint => SIGTRAP,
        TrapKind::Alignment => SIGBUS,
        TrapKind::DivideError | TrapKind::FloatException => SIGFPE,
        TrapKind::GeneralProtection | TrapKind::Overflow => SIGSEGV,
    }
}

/// What parts the words of a stop kind's name.
const WORD_SEPARATOR: char = ' ';

/// A stop kind as the runner's script names its property: in lower camel case.
fn script_name(kind: &str) -> String {
    let mut words = kind.split(WORD_SEPARATOR);
    let mut name = words.next().unwrap_or_default().to_string();
    for word in words {
        let mut letters = word.chars();
        name.extend(letters.next().map(|first| first.to_ascii_uppercase()));
        name.extend(letters);
    }
    name
}

/// Keys of the page's settings and answer.
const LOCK_CODE_KEY: &str = "lockCode";
const STOP_KINDS_KEY: &str = "stopKinds";
const MEMORY_FAULT_STOPS_KEY: &str = "memoryFaultStops";
pub(crate) const STOP_KIND_KEY: &str = "kind";

/// The names and numbers the browser runner shares with the wasmtime runner.
fn runner_names() -> Value {
    let mut names = serde_json::Map::new();
    names.insert(LOCK_CODE_KEY.into(), crate::runner::LOCK_CODE.into());
    names.insert(
        STOP_KINDS_KEY.into(),
        STOP_KINDS
            .iter()
            .copied()
            .chain(TrapKind::ALL.iter().map(|trap| trap.name()))
            .map(|kind| (script_name(kind), Value::from(kind)))
            .collect::<serde_json::Map<_, _>>()
            .into(),
    );
    names.insert(
        MEMORY_FAULT_STOPS_KEY.into(),
        MemoryFault::ALL
            .iter()
            .map(|fault| memory_fault_stop(*fault))
            .collect::<Vec<_>>()
            .into(),
    );
    names.into()
}

/// A new directory for one run of the page, under this process's and this call's numbers so that runs at the
/// same time never share files; the caller removes it when the run has ended.
fn case_directory<G: GuestTest>(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let call = NEXT.fetch_add(1, Ordering::Relaxed);
    let directory = target()
        .join(CASES)
        .join(G::NAME)
        .join(name)
        .join(std::process::id().to_string())
        .join(call.to_string());
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// A new folder for a run of the case `name` ([`case_directory`]), holding the translator module, `blob` and
/// `stack`.
fn case_folder<G: GuestTest>(name: &str, blob: &[u8], stack: &[u8]) -> PathBuf {
    let directory = case_directory::<G>(name);
    std::fs::copy(translator::<G>(), directory.join(TRANSLATOR_FILE)).unwrap();
    std::fs::write(directory.join(BLOB_FILE), blob).unwrap();
    std::fs::write(directory.join(STACK_FILE), stack).unwrap();
    directory
}

/// Build the translator for the browser for a guest, once per process and guest, and return the module's
/// path.
pub fn translator<G: GuestTest>() -> PathBuf {
    static BUILT: Mutex<Vec<(&str, PathBuf)>> = Mutex::new(Vec::new());
    let mut built = BUILT.lock().unwrap();
    if let Some((_, path)) = built.iter().find(|(name, _)| *name == G::NAME) {
        return path.clone();
    }
    let features = [FEATURES_FLAG, G::NAME];
    let path = release_build_with(
        &package(),
        MODULE_TRIPLE,
        &target().join(G::NAME),
        &features,
    )
    .join(MODULE);
    built.push((G::NAME, path.clone()));
    path
}

/// The stand-in kernel's numbers, for the page's settings.
fn stand_in<G: GuestTest>() -> Value {
    let numbers = G::STAND_IN;
    serde_json::json!({
        "write": numbers.write,
        "writev": numbers.writev,
        "exit": numbers.exit,
        "exitGroup": numbers.exit_group,
        "brk": numbers.brk,
        "setBase": numbers.set_base.as_ref().map(|set| serde_json::json!({
            "number": set.number,
            "bases": set.bases.iter().map(|&(code, at)| [code, at as u64]).collect::<Vec<_>>(),
            "limit": set.limit.to_string(),
        })),
    })
}

/// What happened in Chrome: the program's output and status, and the runner's report.
pub struct Report {
    pub ran: Ran,
    /// The kind of stop the runner reported, and the thread state then (the contract's thread state size, or
    /// empty when the stop left none).
    pub stop: String,
    pub state: Vec<u8>,
    /// How long the guest ran in the page each time it was run, translating and compiling the first module
    /// not included.
    pub runs: Vec<f64>,
    /// The slow paths taken, as `(reason, count, nanoseconds spent)`.
    pub counters: Vec<(String, u64, u64)>,
    pub user_agent: String,
}

impl Report {
    /// The register block of the thread state at the stop, if the stop left one.
    pub fn block<G: GuestTest>(&self) -> Option<G::Block> {
        use zerocopy::FromBytes;
        let bytes = self.state.get(..size_of::<G::Block>())?;
        Some(G::Block::read_from_bytes(bytes).expect("a thread block"))
    }

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
pub fn translated<G: GuestTest>(blob: &Blob) -> Report {
    translated_with::<G>(blob, (LookAhead::None, Optimise::Speed), Omit::default())
}

/// Run a blob in Chrome at a look-ahead, translated for speed or size, leaving out some checks only to
/// measure them.
pub fn translated_with<G: GuestTest>(
    blob: &Blob,
    how: (LookAhead, Optimise),
    omit: Omit,
) -> Report {
    repeated::<G>(blob, how, omit, 1)
}

/// [`translated_with`], running the program `repeat` times in the same instances, each run timed by itself.
/// The report holds the output of the last run.
pub fn repeated<G: GuestTest>(
    blob: &Blob,
    how: (LookAhead, Optimise),
    omit: Omit,
    repeat: u32,
) -> Report {
    run_in_chrome::<G>(blob, how, omit, repeat, None, 0)
}

/// [`repeated`] at full look-ahead, the translation split into modules of at most `module_bytes` of code,
/// only to measure calls between modules.
pub fn repeated_split<G: GuestTest>(
    blob: &Blob,
    optimise: Optimise,
    repeat: u32,
    module_bytes: u32,
) -> Report {
    let how = (LookAhead::Full, optimise);
    run_in_chrome::<G>(blob, how, Omit::default(), repeat, None, module_bytes)
}

/// [`translated_with`] with V8 given these flags, only for measuring what the engine does (such as compiling
/// every function when its module is made, not at its first call).
pub fn translated_with_engine_flags<G: GuestTest>(
    blob: &Blob,
    look_ahead: LookAhead,
    flags: &str,
) -> Report {
    let how = (look_ahead, Optimise::Speed);
    run_in_chrome::<G>(blob, how, Omit::default(), 1, Some(flags), 0)
}

fn run_in_chrome<G: GuestTest>(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
    omit: Omit,
    repeat: u32,
    flags: Option<&str>,
    module_bytes: u32,
) -> Report {
    let place = placement::<G>(blob);
    let directory = case_folder::<G>(&blob.name, &blob.bytes, &place.stack);
    let (look_ahead, bound) = look_ahead.code();
    let setup = Setup {
        bound: Some(bound),
        code_bytes: blob.code_bytes,
        keep_float_flags: Some(blob.reads_float_status::<G>()),
        link_address: Some(blob.link_address),
        load: blob.load,
        look_ahead,
        memory_bytes: place.memory_bytes,
        omit: omit.bits(),
        optimise: Some(optimise as u8),
        regions: regions(&place.regions),
        repeat,
        sp: Some(place.sp),
        split_bytes: Some(module_bytes),
        stand_in: Some(stand_in::<G>()),
        ..Setup::default()
    };
    let result = launch(&directory, &setup, flags);
    std::fs::remove_dir_all(&directory).unwrap();
    let answer: RunAnswer = serde_json::from_value(result).expect("the runner's answer");
    let stop: Stop = serde_json::from_value(answer.stopped.clone()).expect("the runner's stop");
    let signal = match (stop.kind.as_str(), TrapKind::from_name(&stop.kind)) {
        (FINISHED, _) => None,
        (MEMORY_FAULT, _) => Some(SIGSEGV),
        (STACK_FAULT, _) => Some(SIGBUS),
        (SINGLE_STEP, _) => Some(SIGTRAP),
        (_, Some(trap)) => Some(trap_signal(trap)),
        _ => panic!(
            "the program stopped without exiting in Chrome: {} at {}",
            answer.stopped, answer.pc
        ),
    };
    let status = match signal {
        // Linux reports the low 8 bits of the status.
        None => i32::from(stop.status.expect("an exit status") as u8),
        // A process killed by a signal is reported as a shell does, `SIGNALLED` plus Linux's number of the
        // signal.
        Some(signal) => SIGNALLED + signal as i32,
    };
    Report {
        ran: Ran {
            stdout: answer.stdout,
            stderr: answer.stderr,
            status,
        },
        stop: stop.kind,
        // None where the runner gives null (a stop with no thread state).
        state: answer.state_at_stop.unwrap_or_default(),
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

/// What the runner is told about a case, written to the case's folder as JSON. The fields are in the
/// order of their names.
#[derive(Clone, Default, Serialize)]
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
    /// Start and end of the code area, and the guest's filler byte it holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<(u64, u64, u8)>,
    /// Whether the float flags are kept exact from the thread's start.
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_float_flags: Option<bool>,
    /// Where the blob is linked, null for code that may be loaded anywhere (see `translate`).
    #[serde(skip_serializing_if = "Option::is_none")]
    link_address: Option<Option<u64>>,
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
    /// The names and numbers the browser runner shares with the wasmtime runner, added at launch.
    #[serde(skip_serializing_if = "Option::is_none")]
    runner: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    split_bytes: Option<u32>,
    /// The stand-in kernel's numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    stand_in: Option<Value>,
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
    /// The thread state when it stopped, null when the stop left none.
    state_at_stop: Option<Vec<u8>>,
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
    let setup = Setup {
        runner: Some(runner_names()),
        ..setup.clone()
    };
    std::fs::write(
        directory.join(SETUP_FILE),
        serde_json::to_string(&setup).unwrap(),
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
pub fn load_modules<G: GuestTest>(
    name: &str,
    image: &[u8],
    load: u64,
    code: std::ops::Range<u64>,
    modules: &[&[u8]],
    flags: &str,
) -> Result<Loaded, String> {
    let directory = case_folder::<G>(&format!("load-{name}"), image, &[]);
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

/// One instruction case: its code, the thread state to start from (the contract's `THREAD_STATE_SIZE` bytes)
/// and the bytes to put at each place.
pub struct StateCase {
    pub code: crate::guest::Encoded,
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

/// Run instruction cases one after another in one Chrome page at look-ahead none, each instruction at `code`
/// in a memory of `memory_bytes` with these regions and `code_area` holding the guest's filler, ending at a
/// system call or where control lands. The lookup covers the instruction's address only, and the translator
/// is given the instruction's own bytes, as in wasmtime. Each instruction goes as its length, then as the
/// longest the guest has, the rest of it the filler.
pub fn cases<G: crate::guest::GuestTest>(
    code: u64,
    memory_bytes: u64,
    regions_given: &[(Range<u64>, Permissions)],
    code_area: Range<u64>,
    places: &[Range<u64>],
    cases: &[StateCase],
) -> Vec<StateRun> {
    let code_bytes = G::MAX_LENGTH as usize;
    let padded = |code: &crate::guest::Encoded| {
        let mut bytes = code.code().to_vec();
        bytes.resize(code_bytes, G::FILLER);
        bytes
    };
    let directory = case_folder::<G>(INSTRUCTION_CASES, &padded(&cases[0].code), &[]);
    let mut bytes = Vec::new();
    for case in cases {
        bytes.push(case.code.length);
        bytes.extend(padded(&case.code));
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
        code_bytes: code_bytes as u64,
        fill: Some((code_area.start, code_area.end, G::FILLER)),
        load: code,
        look_ahead: LookAhead::None.code().0,
        memory_bytes,
        regions: regions(regions_given),
        stand_in: Some(stand_in::<G>()),
        stop_at_syscall: Some(true),
        ..Setup::default()
    };
    let result = launch(&directory, &setup, None);
    std::fs::remove_dir_all(&directory).unwrap();
    let answer: CasesAnswer = serde_json::from_value(result).expect("the runner's answer");
    answer.cases
}
