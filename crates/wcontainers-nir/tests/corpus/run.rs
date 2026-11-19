//! The parent, which samples the corpus and runs one child a shader, and the child, which takes one
//! shader through the routes.

use crate::compare::{self, CompareOptions, Verdict};
use crate::entry;
use crate::lavapipe;
use crate::needs;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use wcontainers_gpu::ReflectedBinding;
use wcontainers_nir::mesa::{Nir, Stage};

/// The corpus's shader files.
const SPIRV_EXTENSION: &str = "spv";
/// How long one shader's child may run before it is stopped and counted as hung.
const CHILD_TIME_LIMIT: Duration = Duration::from_secs(120);
const CHILD_POLL: Duration = Duration::from_millis(20);
/// How many of a crashed child's last lines of errors its reason keeps.
const CRASH_LINES: usize = 3;

/// The candidate routes of plan 15.3, by name in the results.
pub const ROUTE_ZINK: &str = "zink-spirv-naga";
pub const ROUTES: [&str; 1] = [ROUTE_ZINK];
/// A route's outcome while it runs, replaced by the crash's reason if the child dies in it.
const IN_ROUTE: &str = "in the route";
/// The same while the converted shader is held to the reference.
const IN_COMPARISON: &str = "in the comparison";

/// The knobs, by their environment variable.
const CORPUS: &str = "WC_NIR_CORPUS";
const RESULTS: &str = "WC_NIR_RESULTS";
const SHADERS: &str = "WC_NIR_SHADERS";
const SEED: &str = "WC_NIR_SEED";
const JOBS: &str = "WC_NIR_JOBS";
const INPUT_SETS: &str = "WC_NIR_INPUT_SETS";
/// The input sets of each run when `WC_NIR_INPUT_SETS` is unset.
const DEFAULT_INPUT_SETS: u32 = 64;
/// A folder for each route's intermediate form, for diagnosis.
const DUMP: &str = "WC_NIR_DUMP";
/// The seed of the sample when `WC_NIR_SEED` is unset.
const DEFAULT_SEED: u64 = 0x5eed;
/// Lavapipe's worker threads, none: every run in the child's own thread, so a program whose
/// invocations race gives the same result each time.
const DRIVER_THREADS_VARIABLE: &str = "LP_NUM_THREADS";
const NO_DRIVER_THREADS: &str = "0";
/// The argument that makes the process the child running one shader.
const ONE: &str = "--one";

fn knob<T: std::str::FromStr>(name: &str) -> Option<T> {
    let text = std::env::var(name).ok()?;
    Some(
        text.parse()
            .unwrap_or_else(|_| panic!("{name}={text} is not a number")),
    )
}

pub fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(at) = args.iter().position(|a| a == ONE) {
        let [path, route] = &args[at + 1..] else {
            panic!("--one takes a shader file and a route");
        };
        child(std::path::Path::new(path), route);
        return;
    }
    let Some(corpus) = std::env::var_os(CORPUS) else {
        eprintln!("corpus: {CORPUS} unset; nothing run");
        return;
    };
    let options = RunOptions {
        corpus: corpus.into(),
        results: std::env::var_os(RESULTS).map(Into::into),
        shaders: knob(SHADERS),
        seed: knob(SEED).unwrap_or(DEFAULT_SEED),
        jobs: knob(JOBS).unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        }),
    };
    parent(&options);
}

pub struct RunOptions {
    pub corpus: PathBuf,
    pub results: Option<PathBuf>,
    pub shaders: Option<usize>,
    pub seed: u64,
    pub jobs: usize,
}

/// One shader's result: one line of the results.
#[derive(Serialize, Deserialize)]
pub struct ShaderResult {
    pub shader: String,
    /// The entry point's stage, or None where the file has no single entry point the routes take.
    pub stage: Option<String>,
    /// Why the shader reached no route (it has no NIR), where it did not.
    pub refused: Option<String>,
    /// What the program needs that our Vulkan 1.1 does not report (counted apart; no route run).
    pub unreported: Option<String>,
    pub routes: Vec<RouteResult>,
}

#[derive(Serialize, Deserialize)]
pub struct RouteResult {
    pub route: String,
    pub outcome: Outcome,
    /// Nanoseconds from the NIR to the validated module, failed or not.
    pub convert_ns: u64,
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Outcome {
    /// Did not reach the common form, for the named reason.
    Failed(String),
    /// In the common form, held to the reference.
    Compared(Verdict),
}

fn shader_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("a directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            shader_files(&path, out);
        } else if path.extension().is_some_and(|e| e == SPIRV_EXTENSION) {
            out.push(path);
        }
    }
}

pub fn parent(options: &RunOptions) {
    let mut files = Vec::new();
    shader_files(&options.corpus, &mut files);
    let found = files.len();
    if let Some(count) = options.shaders {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(options.seed);
        files.shuffle(&mut rng);
        files.truncate(count);
        files.sort();
    }
    eprintln!(
        "corpus: {} of {found} shaders (seed {:#x}), {} at once",
        files.len(),
        options.seed,
        options.jobs
    );
    let exe = std::env::current_exe().expect("the running test's path");
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<ShaderResult>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..options.jobs {
            s.spawn(|| {
                while let Some(file) = files.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let mut result = ShaderResult {
                        shader: String::new(),
                        stage: None,
                        refused: None,
                        unreported: None,
                        routes: Vec::new(),
                    };
                    for route in ROUTES {
                        let one = run_child(&exe, file, route);
                        result.stage = result.stage.or(one.stage);
                        result.refused = result.refused.or(one.refused);
                        result.unreported = result.unreported.or(one.unreported);
                        result.routes.extend(one.routes);
                    }
                    result.shader = file
                        .strip_prefix(&options.corpus)
                        .unwrap_or(file)
                        .display()
                        .to_string();
                    results
                        .lock()
                        .expect("no child thread panicked")
                        .push(result);
                }
            });
        }
    });
    let mut results = results.into_inner().expect("no child thread panicked");
    results.sort_by(|a, b| a.shader.cmp(&b.shader));
    if let Some(path) = &options.results {
        let mut out = std::fs::File::create(path).expect("the results file can be made");
        for r in &results {
            serde_json::to_writer(&mut out, r).expect("results are written");
            writeln!(out).expect("results are written");
        }
    }
    summary(&results);
}

/// Runs one shader through one route in a child, or names why the child gave no result.
fn run_child(exe: &Path, file: &Path, route: &str) -> ShaderResult {
    let failed = |why: String| ShaderResult {
        shader: String::new(),
        stage: None,
        refused: Some(why),
        unreported: None,
        routes: Vec::new(),
    };
    let mut child = Command::new(exe)
        .arg(ONE)
        .arg(file)
        .arg(route)
        .env(DRIVER_THREADS_VARIABLE, NO_DRIVER_THREADS)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the child starts");
    let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let mut bytes = Vec::new();
                let _ = pipe.read_to_end(&mut bytes);
                text = String::from_utf8_lossy(&bytes).into_owned();
            }
            text
        })
    };
    let stdout = drain(child.stdout.take().map(|p| Box::new(p) as _));
    let stderr = drain(child.stderr.take().map(|p| Box::new(p) as _));
    let started = Instant::now();
    let mut hung = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("the child can be waited on") {
            break status;
        }
        if started.elapsed() > CHILD_TIME_LIMIT {
            let _ = child.kill();
            hung = true;
            break child.wait().expect("the child can be waited on");
        }
        std::thread::sleep(CHILD_POLL);
    };
    let stdout = stdout.join().expect("the reader ends");
    let stderr = stderr.join().expect("the reader ends");
    let last: Option<ShaderResult> = stdout
        .lines()
        .last()
        .and_then(|l| serde_json::from_str(l).ok());
    if status.success()
        && let Some(result) = last
    {
        return result;
    }
    let lines: Vec<&str> = stderr.lines().collect();
    let why = if hung {
        format!("hung: no result in {CHILD_TIME_LIMIT:?}")
    } else {
        format!(
            "crashed: {status}: {}",
            lines[lines.len().saturating_sub(CRASH_LINES)..].join(" | ")
        )
    };
    match last {
        Some(mut provisional) => {
            for r in &mut provisional.routes {
                if r.outcome == Outcome::Failed(IN_ROUTE.into()) {
                    r.outcome = Outcome::Failed(why.clone());
                } else if r.outcome == Outcome::Compared(Verdict::NotRun(IN_COMPARISON.into())) {
                    r.outcome =
                        Outcome::Compared(Verdict::NotRun(format!("{IN_COMPARISON}: {why}")));
                }
            }
            provisional
        }
        None => failed(why),
    }
}

fn summary(results: &[ShaderResult]) {
    let refused = results.iter().filter(|r| r.refused.is_some()).count();
    let unreported = results.iter().filter(|r| r.unreported.is_some()).count();
    eprintln!(
        "corpus: {} shaders, {refused} reached no route, {unreported} need what our Vulkan 1.1 \
         does not report",
        results.len()
    );
    for route in ROUTES {
        let taken: Vec<&RouteResult> = results
            .iter()
            .flat_map(|r| &r.routes)
            .filter(|r| r.route == route)
            .collect();
        let converted = taken
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Compared(_)))
            .count();
        let matched = taken
            .iter()
            .filter(|r| r.outcome == Outcome::Compared(Verdict::Matched))
            .count();
        let differ = taken
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::Compared(Verdict::Differs(_))))
            .count();
        let total_ns: u64 = taken.iter().map(|r| r.convert_ns).sum();
        let mean = Duration::from_nanos(total_ns / taken.len().max(1) as u64);
        eprintln!(
            "corpus: {route}: {converted} of {} converted, {mean:?} a shader; {matched} match the \
             reference, {differ} differ, {} not compared",
            taken.len(),
            converted - matched - differ
        );
    }
}

/// The child: one shader through one route, its result as one JSON line on standard output. Before
/// the route runs it writes the result so far, the route marked as in progress, so that a crash in
/// the route is the route's failure.
pub fn child(file: &Path, route: &str) {
    let result = shader(file, route);
    println!(
        "{}",
        serde_json::to_string(&result).expect("a result is JSON")
    );
}

fn provisional(result: &ShaderResult, route: RouteResult) {
    let mut so_far = ShaderResult {
        shader: result.shader.clone(),
        stage: result.stage.clone(),
        refused: None,
        unreported: None,
        routes: vec![route],
    };
    so_far.refused.clone_from(&result.refused);
    println!(
        "{}",
        serde_json::to_string(&so_far).expect("a result is JSON")
    );
    std::io::stdout()
        .flush()
        .expect("standard output takes the line");
}

fn shader(file: &Path, route: &str) -> ShaderResult {
    let mut result = ShaderResult {
        shader: file.display().to_string(),
        stage: None,
        refused: None,
        unreported: None,
        routes: Vec::new(),
    };
    let bytes = std::fs::read(file).expect("the shader file can be read");
    let Some(words) = entry::words(&bytes) else {
        result.refused = Some("not whole SPIR-V words".into());
        return result;
    };
    let entries = entry::entry_points(&words);
    let [entry] = entries.as_slice() else {
        result.refused = Some(format!(
            "{} entry points; the corpus takes one a file",
            entries.len()
        ));
        return result;
    };
    result.stage = Some(format!("{:?}", entry.model));
    let stage = match entry.model {
        spirv::ExecutionModel::Vertex => Stage::Vertex,
        spirv::ExecutionModel::Fragment => Stage::Fragment,
        spirv::ExecutionModel::GLCompute => Stage::Compute,
        other => {
            result.refused = Some(format!("stage {other:?} is not in the common form"));
            return result;
        }
    };
    result.unreported = needs::unreported(&words);
    if result.unreported.is_some() {
        return result;
    }
    let nir = match Nir::from_spirv(&words, stage, &entry.name) {
        Ok(nir) => nir,
        Err(e) => {
            result.refused = Some(e.to_string());
            return result;
        }
    };
    provisional(
        &result,
        RouteResult {
            route: route.into(),
            outcome: Outcome::Failed(IN_ROUTE.into()),
            convert_ns: 0,
        },
    );
    match route {
        ROUTE_ZINK => {
            let (mut route, converted) = zink_route(nir, stage, file);
            if let Some((module, reflection)) = converted {
                provisional(
                    &result,
                    RouteResult {
                        outcome: Outcome::Compared(Verdict::NotRun(IN_COMPARISON.into())),
                        route: route.route.clone(),
                        convert_ns: route.convert_ns,
                    },
                );
                let options = CompareOptions {
                    seed: knob(SEED).unwrap_or(DEFAULT_SEED),
                    input_sets: knob(INPUT_SETS).unwrap_or(DEFAULT_INPUT_SETS),
                };
                let stage = match stage {
                    Stage::Vertex => lavapipe::Stage::Vertex,
                    Stage::Fragment => lavapipe::Stage::Fragment,
                    Stage::Compute => lavapipe::Stage::Compute,
                };
                route.outcome = Outcome::Compared(compare::compare(
                    &words,
                    &entry.name,
                    stage,
                    &module,
                    &reflection,
                    options,
                ));
            }
            result.routes.push(route);
        }
        other => panic!("no route named {other}"),
    }
    result
}

/// Candidate A: the binding rule, Zink's SPIR-V, naga's reader and validation. With `WC_NIR_DUMP` set, Zink's SPIR-V
/// is also written there (outside the time measured), under the shader's own path.
fn zink_route(
    mut nir: Nir,
    stage: Stage,
    file: &Path,
) -> (RouteResult, Option<(naga::Module, Vec<ReflectedBinding>)>) {
    let started = Instant::now();
    let reflection = nir.to_common_form(stage);
    let words = reflection.clone().and_then(|_| nir.into_spirv());
    let mut elapsed = started.elapsed();
    if let (Some(dir), Ok(words)) = (std::env::var_os(DUMP), &words) {
        let relative: PathBuf = file
            .components()
            .filter(|c| matches!(c, std::path::Component::Normal(_)))
            .collect();
        let path = Path::new(&dir).join(relative);
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        std::fs::create_dir_all(path.parent().expect("a file has a folder"))
            .expect("the dump folder takes the file");
        std::fs::write(&path, bytes).expect("the dump folder takes the file");
    }
    let started = Instant::now();
    let converted = words.and_then(|words| wcontainers_nir::spirv::read(&words));
    elapsed += started.elapsed();
    let convert_ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
    let mut result = RouteResult {
        route: ROUTE_ZINK.into(),
        outcome: Outcome::Failed(IN_ROUTE.into()),
        convert_ns,
    };
    match (converted, reflection) {
        (Ok((module, _)), Ok(reflection)) => (result, Some((module, reflection))),
        (Err(e), _) | (_, Err(e)) => {
            result.outcome = Outcome::Failed(e.to_string());
            (result, None)
        }
    }
}
