//! The browser runner: a flat blob run in Chrome by `browser/runner.js`, with the translator built for wasm32
//! from `browser/` so that code is translated at run time there too.

use crate::guest::GuestTest;
use crate::programs::{Blob, Ran, placement};
use linux_raw_sys::general::{SIGBUS, SIGFPE, SIGILL, SIGSEGV, SIGTRAP};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use wcontainers_guest_cpu::{MemoryFault, TrapKind};
use wcontainers_translator::{LookAhead, Omit, Optimise};

fn package() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("browser")
}

fn target() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/browser")
}

/// Where the page's runs keep their files, under the browser build.
const CASES: &str = "cases";
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
const RUNNER_KEY: &str = "runner";
const LOCK_CODE_KEY: &str = "lockCode";
const STOP_KINDS_KEY: &str = "stopKinds";
const MEMORY_FAULT_STOPS_KEY: &str = "memoryFaultStops";
pub(crate) const STOP_KIND_KEY: &str = "kind";
const LINK_ADDRESS_KEY: &str = "linkAddress";
const STOP_STATE_KEY: &str = "stateAtStop";

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

/// Build the translator for the browser for a guest, once per process and guest, and return the module's
/// path.
pub fn translator<G: GuestTest>() -> PathBuf {
    static BUILT: Mutex<Vec<(&str, PathBuf)>> = Mutex::new(Vec::new());
    let mut built = BUILT.lock().unwrap();
    if let Some((_, path)) = built.iter().find(|(name, _)| *name == G::NAME) {
        return path.clone();
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let directory = target().join(G::NAME);
    let status = Command::new(cargo)
        .args([
            "build",
            "--release",
            "--quiet",
            "--features",
            G::NAME,
            "--target",
            "wasm32-unknown-unknown",
            "--target-dir",
        ])
        .arg(&directory)
        .current_dir(package())
        .status()
        .expect("run cargo for the browser translator");
    assert!(status.success(), "the browser translator did not build");
    let path = directory.join("wasm32-unknown-unknown/release/wcontainers_browser.wasm");
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
    let directory = case_directory::<G>(&blob.name);
    std::fs::copy(translator::<G>(), directory.join("translator.wasm")).unwrap();
    std::fs::write(directory.join("blob.bin"), &blob.bytes).unwrap();
    std::fs::write(directory.join("stack.bin"), &place.stack).unwrap();
    let regions: Vec<Value> = place
        .regions
        .iter()
        .map(|(range, permissions)| serde_json::json!([range.start, range.end, permissions.bits()]))
        .collect();
    let mut setup = serde_json::json!({
        "codeBytes": blob.code_bytes,
        "load": blob.load,
        "memoryBytes": place.memory_bytes,
        "regions": regions,
        "sp": place.sp,
        "lookAhead": match look_ahead {
            LookAhead::None => 0,
            LookAhead::Full => 1,
            LookAhead::Bounded(_) => 2,
        },
        "bound": match look_ahead {
            LookAhead::Bounded(n) => n,
            _ => 0,
        },
        "repeat": repeat,
        "optimise": optimise as u8,
        "splitBytes": module_bytes,
        "standIn": stand_in::<G>(),
        "omit": u8::from(omit.stop)
            | (u8::from(omit.permissions) << 1)
            | (u8::from(omit.returns) << 2)
            | (u8::from(omit.depth) << 3),
    });
    setup[LINK_ADDRESS_KEY] = serde_json::json!(blob.link_address);
    let result = launch(&directory, &setup, flags);
    std::fs::remove_dir_all(&directory).unwrap();
    // An array of bytes, or none where the runner gives null (a stop with no thread state).
    let bytes = |name: &str| -> Vec<u8> {
        result[name]
            .as_array()
            .into_iter()
            .flatten()
            .map(|b| b.as_u64().unwrap() as u8)
            .collect()
    };
    let stopped = &result["stopped"];
    let kind = stopped[STOP_KIND_KEY].as_str().unwrap_or_default();
    let signal = match (kind, TrapKind::from_name(kind)) {
        (FINISHED, _) => None,
        (MEMORY_FAULT, _) => Some(SIGSEGV),
        (STACK_FAULT, _) => Some(SIGBUS),
        (SINGLE_STEP, _) => Some(SIGTRAP),
        (_, Some(trap)) => Some(trap_signal(trap)),
        _ => panic!(
            "the program stopped without exiting in Chrome: {stopped} at {}",
            result["pc"]
        ),
    };
    let status = match signal {
        // Linux reports the low 8 bits of the status.
        None => i32::from(stopped["status"].as_i64().unwrap() as u8),
        // A process killed by a signal is reported as a shell does, 128 plus Linux's number of the signal.
        Some(signal) => 128 + signal as i32,
    };
    let counters = result["counters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["reason"].as_str().unwrap().to_string(),
                c["count"].as_u64().unwrap(),
                c["nanoseconds"].as_u64().unwrap(),
            )
        })
        .collect();
    Report {
        ran: Ran {
            stdout: bytes("stdout"),
            stderr: bytes("stderr"),
            status,
        },
        stop: stopped[STOP_KIND_KEY]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        state: bytes(STOP_STATE_KEY),
        counters,
        runs: result["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|ms| ms.as_f64().unwrap() / 1000.0)
            .collect(),
        user_agent: result["userAgent"].as_str().unwrap_or_default().to_string(),
    }
}

/// Run the browser runner on a case directory holding `translator.wasm`, `blob.bin`, `stack.bin` and, for
/// instruction cases, `cases.bin`, with this setup and V8 flags if given; give its answer.
fn launch(directory: &Path, setup: &Value, flags: Option<&str>) -> Value {
    launch_or_refused(directory, setup, flags)
        .unwrap_or_else(|error| panic!("the browser runner failed: {error}"))
}

/// [`launch`], or what the page threw.
fn launch_or_refused(
    directory: &Path,
    setup: &Value,
    flags: Option<&str>,
) -> Result<Value, String> {
    let mut setup = setup.clone();
    setup[RUNNER_KEY] = runner_names();
    std::fs::write(directory.join("case.json"), setup.to_string()).unwrap();

    let mut command = Command::new("node");
    if let Some(flags) = flags {
        command.env("WC_CHROME_JS_FLAGS", flags);
    }
    let output = command
        .arg(package().join("run.mjs"))
        .arg(directory)
        .output()
        .expect("run node (from the Nix shell)");
    let errors = String::from_utf8_lossy(&output.stderr);
    eprint!("{errors}");
    // A page that crashes (the engine giving up on a module) is a refusal too.
    if errors.contains("Target crashed") {
        return Err("the page crashed".into());
    }
    assert!(output.status.success(), "the browser runner failed");
    let result: Value = serde_json::from_slice(&output.stdout).expect("the runner's JSON");
    match result.get("error") {
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
    let directory = case_directory::<G>(&format!("load-{name}"));
    std::fs::copy(translator::<G>(), directory.join("translator.wasm")).unwrap();
    std::fs::write(directory.join("blob.bin"), image).unwrap();
    std::fs::write(directory.join("stack.bin"), []).unwrap();
    for (i, module) in modules.iter().enumerate() {
        std::fs::write(directory.join(format!("module-{i}.wasm")), module).unwrap();
    }
    let memory = (load + image.len() as u64).next_multiple_of(1 << 16);
    let setup = serde_json::json!({
        "codeBytes": code.end,
        "codeStart": code.start,
        "load": load,
        "memoryBytes": memory,
        "regions": [],
        "sp": 0,
        "lookAhead": 1,
        "repeat": 0,
        "omit": 0,
        "modules": modules.len(),
    });
    let result = launch_or_refused(&directory, &setup, Some(flags));
    std::fs::remove_dir_all(&directory).unwrap();
    let result = result?;
    Ok(Loaded {
        compiling: result["compiling"].as_f64().expect("a time"),
        instantiating: result["instantiating"].as_f64().expect("a time"),
        memory: result["memoryAfter"].as_i64().expect("bytes")
            - result["memoryBefore"].as_i64().expect("bytes"),
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
    regions: &[(std::ops::Range<u64>, wcontainers_abi::Permissions)],
    code_area: std::ops::Range<u64>,
    places: &[std::ops::Range<u64>],
    cases: &[StateCase],
) -> Vec<StateRun> {
    let directory = case_directory::<G>("instructions");
    std::fs::copy(translator::<G>(), directory.join("translator.wasm")).unwrap();
    let code_bytes = G::MAX_LENGTH as usize;
    let padded = |code: &crate::guest::Encoded| {
        let mut bytes = code.code().to_vec();
        bytes.resize(code_bytes, G::FILLER);
        bytes
    };
    std::fs::write(directory.join("blob.bin"), padded(&cases[0].code)).unwrap();
    std::fs::write(directory.join("stack.bin"), []).unwrap();
    let mut bytes = Vec::new();
    for case in cases {
        bytes.push(case.code.length);
        bytes.extend(padded(&case.code));
        bytes.extend(&case.state);
        for place in &case.places {
            bytes.extend(place);
        }
    }
    std::fs::write(directory.join("cases.bin"), bytes).unwrap();
    let regions: Vec<Value> = regions
        .iter()
        .map(|(range, permissions)| serde_json::json!([range.start, range.end, permissions.bits()]))
        .collect();
    let places: Vec<Value> = places
        .iter()
        .map(|range| serde_json::json!([range.start, range.end - range.start]))
        .collect();
    let setup = serde_json::json!({
        "codeBytes": code_bytes,
        "load": code,
        "memoryBytes": memory_bytes,
        "regions": regions,
        "fill": [code_area.start, code_area.end, G::FILLER],
        "sp": 0,
        "lookAhead": 0,
        "repeat": 0,
        "omit": 0,
        "cases": cases.len(),
        "areas": places,
        "stopAtSyscall": true,
        "standIn": stand_in::<G>(),
    });
    let result = launch(&directory, &setup, None);
    std::fs::remove_dir_all(&directory).unwrap();
    let numbers = |value: &Value| -> Vec<u8> {
        value
            .as_array()
            .expect("bytes")
            .iter()
            .map(|b| b.as_u64().unwrap() as u8)
            .collect()
    };
    result["cases"]
        .as_array()
        .expect("the cases' results")
        .iter()
        .map(|run| StateRun {
            stopped: run["stopped"].clone(),
            state: numbers(&run["state"]),
            changed: run["changed"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    (
                        pair[0].as_u64().unwrap() as u32,
                        pair[1].as_u64().unwrap() as u8,
                    )
                })
                .collect(),
        })
        .collect()
}
