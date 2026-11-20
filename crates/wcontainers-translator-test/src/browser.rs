//! The browser runner: a flat blob run in Chrome by `browser/runner.js`, with the translator built for wasm32
//! from `browser/` so that code is translated at run time there too.

use crate::programs::{Blob, Ran, placement};
use crate::reference::{release_build, target_folder};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
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

/// The setup's names for the thread's start, which runner.js reads: its stack pointer, and whether FPSR's
/// flags are kept exact from the start.
const START_SP: &str = "sp";
const KEEP_FLOAT_FLAGS: &str = "keepFloatFlags";

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
    let regions: Vec<Value> = place
        .regions
        .iter()
        .map(|(range, permissions)| serde_json::json!([range.start, range.end, permissions.bits()]))
        .collect();
    let setup = serde_json::json!({
        "codeBytes": blob.code_bytes,
        "load": blob.load,
        "memoryBytes": place.memory_bytes,
        "regions": regions,
        "lookAhead": look_ahead.code().0,
        "bound": look_ahead.code().1,
        "repeat": repeat,
        "optimise": optimise as u8,
        "splitBytes": module_bytes,
        "omit": u8::from(omit.stop)
            | (u8::from(omit.permissions) << 1)
            | (u8::from(omit.returns) << 2)
            | (u8::from(omit.depth) << 3),
    });
    let mut setup = setup;
    let fields = setup.as_object_mut().expect("an object");
    fields.insert(START_SP.into(), place.sp.into());
    fields.insert(KEEP_FLOAT_FLAGS.into(), blob.reads_float_status().into());
    let result = launch(&directory, &setup, flags);
    let bytes = |name: &str| -> Vec<u8> {
        result[name]
            .as_array()
            .expect("bytes")
            .iter()
            .map(|b| b.as_u64().unwrap() as u8)
            .collect()
    };
    let stopped = &result["stopped"];
    // Docker reports a process killed by a signal as 128 plus the signal's number.
    let status = match stopped["kind"].as_str() {
        // Linux reports the low 8 bits of the status.
        Some("finished") => stopped["status"].as_i64().unwrap() as i32 & 0xff,
        Some("memory fault") => 128 + 11,
        Some("alignment") => 128 + 7,
        Some("undefined" | "unsupported") => 128 + 4,
        Some("breakpoint") => 128 + 5,
        Some("divide error" | "float exception") => 128 + 8,
        Some("general protection" | "overflow") => 128 + 11,
        Some("stack fault") => 128 + 7,
        _ => panic!(
            "the program stopped without exiting in Chrome: {stopped} at {}",
            result["pc"]
        ),
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
    let setup = serde_json::json!({
        "codeBytes": code.end,
        "codeStart": code.start,
        "load": load,
        "memoryBytes": memory,
        "regions": [],
        "lookAhead": LookAhead::Full.code().0,
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

/// One instruction case: its code word, the thread state to start from (the contract's
/// `THREAD_STATE_SIZE` bytes) and the bytes to put at each place.
pub struct StateCase {
    pub word: u32,
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

/// Run instruction cases one after another in one Chrome page at look-ahead none, each word at `code` in a
/// memory of `memory_bytes` with these regions, ending at a system call or where control lands.
pub fn cases(
    code: u64,
    memory_bytes: u64,
    regions: &[(std::ops::Range<u64>, wcontainers_abi::Permissions)],
    places: &[std::ops::Range<u64>],
    cases: &[StateCase],
) -> Vec<StateRun> {
    let directory = case_folder("instructions", &cases[0].word.to_le_bytes(), &[]);
    let mut bytes = Vec::new();
    for case in cases {
        bytes.extend(case.word.to_le_bytes());
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
        "codeBytes": 4,
        "load": code,
        "memoryBytes": memory_bytes,
        "regions": regions,
        "lookAhead": LookAhead::None.code().0,
        "repeat": 0,
        "omit": 0,
        "cases": cases.len(),
        "areas": places,
        "stopAtSyscall": true,
    });
    let result = launch(&directory, &setup, None);
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
