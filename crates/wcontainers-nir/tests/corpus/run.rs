//! The parent, which samples the corpus and runs one child a shader, and the child, which takes one
//! shader through the routes.

use crate::bgfx;
use crate::compare::{self, CompareOptions, DEFAULT_SEED, INPUT_SETS, SEED, Verdict};
use crate::deqp;
use crate::entry;
use crate::glsl;
use crate::godot;
use crate::lavapipe;
use crate::needs;
use naga::{ImageClass, StorageAccess, StorageFormat, TypeInner};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use wcontainers_gpu::{Format, ReflectedBinding, SlotKind, VariantKey};
use wcontainers_nir::direct::{Rewrites, Routed};
use wcontainers_nir::mesa::{Nir, Stage};

/// The corpus's shader files.
const SPIRV_EXTENSION: &str = "spv";
/// How long one shader's child may run before it is stopped and counted as hung.
const CHILD_TIME_LIMIT: Duration = Duration::from_secs(120);
const CHILD_POLL: Duration = Duration::from_millis(20);
/// The input seeds a route is tried on while the reference gives no result on them (a loop bound
/// or step from the seeded inputs that never ends the loop, or an access the inputs send out of
/// range that crashes the driver): the run's seed, then the next ones.
const REFERENCE_SEEDS: u64 = 4;
/// Why a child stopped in the comparison gave no verdict: the reference had not ended.
const REFERENCE_DID_NOT_END: &str = "the reference did not end";
/// Why a child that died in the comparison gave no verdict: the reference crashed.
const REFERENCE_CRASHED: &str = "the reference crashed";
/// How many of a crashed child's last lines of errors its reason keeps.
const CRASH_LINES: usize = 3;

/// The candidate routes of plan 15.3, by name in the results.
pub const ROUTE_ZINK: &str = "zink-spirv-naga";
pub const ROUTE_DIRECT: &str = "nir-naga";
pub const ROUTES: [&str; 2] = [ROUTE_ZINK, ROUTE_DIRECT];
/// A route's outcome while it runs, replaced by the crash's reason if the child dies in it.
const IN_ROUTE: &str = "in the route";
/// The same while the converted shader is held to the reference.
const IN_COMPARISON: &str = "in the comparison";
/// Between the stages of a GLSL program in its result.
const STAGE_JOINER: &str = "+";

/// The knobs, by their environment variable.
const CORPUS: &str = "WC_NIR_CORPUS";
const RESULTS: &str = "WC_NIR_RESULTS";
const SHADERS: &str = "WC_NIR_SHADERS";
const JOBS: &str = "WC_NIR_JOBS";
/// The input sets of each run when `WC_NIR_INPUT_SETS` is unset.
const DEFAULT_INPUT_SETS: u32 = 64;
/// A folder for each route's intermediate form, for diagnosis.
const DUMP: &str = "WC_NIR_DUMP";
/// Lavapipe's worker threads, none: every run in the child's own thread, so a program whose
/// invocations race gives the same result each time.
const DRIVER_THREADS_VARIABLE: &str = "LP_NUM_THREADS";
const NO_DRIVER_THREADS: &str = "0";
/// The folder of the Vulkan CTS's `alpha_to_one` programs (`corpus/cts-programs`): each fragment
/// program goes through `Nir::alpha_to_one` before the routes, and is held by G10's CTS cases, not
/// by lavapipe, which does not run the `demote_samples` the step makes.
const ALPHA_TO_ONE_FOLDER: &str = "alpha_to_one";
const HELD_BY_CTS: &str = "held by G10's CTS cases: lavapipe does not run demote_samples";
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
    /// The seed of the inputs it was held to, where the reference did not end on the run's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_seed: Option<u64>,
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
        } else if path.extension().is_some_and(|e| e == deqp::EXTENSION) {
            out.extend(deqp::items(&path));
        } else if godot::is_shader(&path) {
            out.extend(godot::items(&path));
        } else if path
            .extension()
            .is_some_and(|e| e == SPIRV_EXTENSION || e == glsl::SHADER_TEST_EXTENSION)
            || bgfx::is_desktop_shader(&path)
        {
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
                        let mut one = run_child(&exe, file, route, options.seed);
                        for retry in 1..REFERENCE_SEEDS {
                            if !reference_gave_no_result(&one) {
                                break;
                            }
                            let seed = options.seed.wrapping_add(retry);
                            one = run_child(&exe, file, route, seed);
                            for r in &mut one.routes {
                                r.inputs_seed = Some(seed);
                            }
                        }
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

fn reference_gave_no_result(result: &ShaderResult) -> bool {
    result.routes.iter().any(|r| {
        matches!(&r.outcome, Outcome::Compared(Verdict::NotRun(why))
            if why.contains(REFERENCE_DID_NOT_END) || why.contains(REFERENCE_CRASHED))
    })
}

/// Runs one shader through one route in a child, its inputs from `seed`, or names why the child
/// gave no result.
fn run_child(exe: &Path, file: &Path, route: &str, seed: u64) -> ShaderResult {
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
        .env(SEED, seed.to_string())
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
    let reference_ended = lines.contains(&compare::REFERENCE_ENDED);
    let why = if hung && !reference_ended {
        format!("hung: no result in {CHILD_TIME_LIMIT:?}: {REFERENCE_DID_NOT_END} on seed {seed}")
    } else if hung {
        format!("hung: no result in {CHILD_TIME_LIMIT:?}: the route's module did not end")
    } else {
        format!(
            "crashed: {status}: {}",
            lines[lines.len().saturating_sub(CRASH_LINES)..].join(" | ")
        )
    };
    // Killed by a signal, not ended by a panic of the harness.
    let signalled = !hung && status.code().is_none();
    match last {
        Some(mut provisional) => {
            for r in &mut provisional.routes {
                if r.outcome == Outcome::Failed(IN_ROUTE.into()) {
                    r.outcome = Outcome::Failed(why.clone());
                } else if r.outcome == Outcome::Compared(Verdict::NotRun(IN_COMPARISON.into())) {
                    r.outcome = Outcome::Compared(if reference_ended && (hung || signalled) {
                        // The route's module hung or crashed the driver the reference ran on.
                        Verdict::Differs(format!("{IN_COMPARISON}: {why}"))
                    } else if signalled {
                        Verdict::NotRun(format!(
                            "{IN_COMPARISON}: {REFERENCE_CRASHED} on seed {seed}: {why}"
                        ))
                    } else {
                        Verdict::NotRun(format!("{IN_COMPARISON}: {why}"))
                    });
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
    let shared: Vec<&ShaderResult> = results
        .iter()
        .filter(|r| {
            r.routes.len() == ROUTES.len()
                && r.routes
                    .iter()
                    .all(|route| route.outcome == Outcome::Compared(Verdict::Matched))
        })
        .collect();
    for route in ROUTES {
        let mut times: Vec<u64> = shared
            .iter()
            .flat_map(|r| &r.routes)
            .filter(|r| r.route == route)
            .map(|r| r.convert_ns)
            .collect();
        times.sort_unstable();
        let Some(&median) = times.get(times.len() / 2) else {
            continue;
        };
        let total: u64 = times.iter().sum();
        eprintln!(
            "corpus: {route} on the {} shaders every route matches: median {:?}, mean {:?} a shader",
            times.len(),
            Duration::from_nanos(median),
            Duration::from_nanos(total / times.len() as u64)
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
    if file
        .extension()
        .is_some_and(|e| e == glsl::SHADER_TEST_EXTENSION)
        || bgfx::is_desktop_shader(file)
        || deqp::sources(file).is_some()
        || godot::sources(file).is_some()
    {
        return glsl_program(file, route);
    }
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
    let alpha_to_one = stage == Stage::Fragment
        && file
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|f| f == ALPHA_TO_ONE_FOLDER);
    let nir = match Nir::from_spirv(&words, stage, &entry.name) {
        Ok(mut nir) => {
            if alpha_to_one {
                nir.alpha_to_one();
            }
            nir
        }
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
            inputs_seed: None,
        },
    );
    match route {
        ROUTE_ZINK | ROUTE_DIRECT => {
            let (mut route, converted) = if route == ROUTE_ZINK {
                zink_route(nir, stage, file)
            } else {
                direct_route(nir, stage)
            };
            if converted.is_some() && alpha_to_one {
                route.outcome = Outcome::Compared(Verdict::NotRun(HELD_BY_CTS.into()));
            } else if let Some((module, reflection)) = converted {
                provisional(
                    &result,
                    RouteResult {
                        outcome: Outcome::Compared(Verdict::NotRun(IN_COMPARISON.into())),
                        route: route.route.clone(),
                        convert_ns: route.convert_ns,
                        inputs_seed: None,
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

/// Candidate B: the common form's step, then NIR read straight into naga and validated; with the
/// shaders crate's rewrites (15.4) where the module needs them, each storage image without a
/// format bound as a view of its kind (`bound_views`), as a driver names the bound view. The cache
/// key, which both candidates take from the same NIR, is made outside the time measured, as
/// candidate A's is.
fn direct_route(
    mut nir: Nir,
    stage: Stage,
) -> (RouteResult, Option<(naga::Module, Vec<ReflectedBinding>)>) {
    let key = nir.key();
    let started = Instant::now();
    let converted = key.and_then(|key| {
        let reflection = nir.to_common_form(stage)?;
        let routed = wcontainers_nir::direct::route(&mut nir)?;
        let arrays = routed.module.global_variables.iter().any(|(_, g)| {
            matches!(
                routed.module.types[g.ty].inner,
                TypeInner::BindingArray { .. }
            )
        });
        if routed.rewrites == Rewrites::default() && !arrays {
            wcontainers_nir::spirv::validate(&routed.module)?;
            return Ok((routed.module, reflection));
        }
        let variant = VariantKey {
            storage_formats: bound_views(&routed),
            ..VariantKey::default()
        };
        let (module, reflection) =
            wcontainers_nir::direct::rewrite(routed, reflection, &variant, &key)?;
        Ok((module.module, reflection))
    });
    let convert_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    match converted {
        Ok(c) => (
            RouteResult {
                route: ROUTE_DIRECT.into(),
                outcome: Outcome::Failed(IN_ROUTE.into()),
                convert_ns,
                inputs_seed: None,
            },
            Some(c),
        ),
        Err(e) => (
            RouteResult {
                route: ROUTE_DIRECT.into(),
                outcome: Outcome::Failed(e.to_string()),
                convert_ns,
                inputs_seed: None,
            },
            None,
        ),
    }
}

/// The format of the view bound at each storage slot route B declared without one, of the kind its
/// placeholder has: one 32-bit component where the program reads and writes it (the only storage
/// formats the common form reads and writes), else four.
fn bound_views(routed: &Routed) -> Vec<(u32, Format)> {
    let module = &routed.module;
    let placeholder = |slot: u32| {
        module.global_variables.iter().find_map(|(_, g)| {
            let first = g
                .binding
                .as_ref()?
                .binding
                .checked_sub(SlotKind::Storage.first_binding())?;
            let (ty, count) = match module.types[g.ty].inner {
                TypeInner::BindingArray {
                    base,
                    size: naga::ArraySize::Constant(n),
                } => (base, n.get()),
                _ => (g.ty, 1),
            };
            let TypeInner::Image {
                class: ImageClass::Storage { format, access },
                ..
            } = module.types[ty].inner
            else {
                return None;
            };
            let read_write = access.contains(StorageAccess::LOAD | StorageAccess::STORE);
            (first..first + count)
                .contains(&slot)
                .then_some((format, read_write))
        })
    };
    routed
        .rewrites
        .formatless
        .iter()
        .filter_map(|&slot| {
            let view = match placeholder(slot)? {
                (StorageFormat::R32Sint, true) => Format::R32Sint,
                (StorageFormat::R32Uint, true) => Format::R32Uint,
                (_, true) => Format::R32Float,
                (StorageFormat::R32Sint, false) => Format::Rgba32Sint,
                (StorageFormat::R32Uint, false) => Format::Rgba32Uint,
                (_, false) => Format::Rgba32Float,
            };
            Some((slot, view))
        })
        .collect()
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
        inputs_seed: None,
    };
    match (converted, reflection) {
        (Ok((module, _)), Ok(reflection)) => (result, Some((module, reflection))),
        (Err(e), _) | (_, Err(e)) => {
            result.outcome = Outcome::Failed(e.to_string());
            (result, None)
        }
    }
}

/// A GLSL program (a piglit test, a bgfx shader, a dEQP case or a Godot variant): its stages through Mesa's compiler, each through the route; a
/// compute program dispatched, a vertex and fragment program drawn, held to llvmpipe.
fn glsl_program(file: &Path, route: &str) -> ShaderResult {
    let mut result = ShaderResult {
        shader: file.display().to_string(),
        stage: None,
        refused: None,
        unreported: None,
        routes: Vec::new(),
    };
    let sources = if let Some(sources) = deqp::sources(file) {
        sources
    } else if let Some(sources) = godot::sources(file) {
        sources
    } else if bgfx::is_desktop_shader(file) {
        bgfx::sources(&std::fs::read(file).expect("the file can be read"))
    } else {
        let bytes = std::fs::read(file).expect("the file can be read");
        glsl::shader_test_sources(&String::from_utf8_lossy(&bytes))
    };
    let sources = match sources {
        Ok(s) => glsl::at_full_precision(s),
        Err(e) => {
            result.refused = Some(e);
            return result;
        }
    };
    result.stage = Some(
        sources
            .iter()
            .map(|(s, _)| format!("{s:?}"))
            .collect::<Vec<_>>()
            .join(STAGE_JOINER),
    );
    let given: Vec<(Stage, &str)> = sources.iter().map(|(s, t)| (*s, t.as_str())).collect();
    let stages = match Nir::from_glsl(&given) {
        Ok(s) => s,
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
            inputs_seed: None,
        },
    );
    let mut taken = RouteResult {
        route: route.into(),
        outcome: Outcome::Failed(IN_ROUTE.into()),
        convert_ns: 0,
        inputs_seed: None,
    };
    let mut converted = Vec::new();
    for stage in stages {
        let (one, module) = match route {
            ROUTE_ZINK => zink_route(stage.nir, stage.stage, file),
            ROUTE_DIRECT => direct_route(stage.nir, stage.stage),
            other => panic!("no route named {other}"),
        };
        taken.convert_ns += one.convert_ns;
        match module {
            Some(m) => converted.push((stage.stage, stage.names, m)),
            None => {
                taken.outcome = one.outcome;
                result.routes.push(taken);
                return result;
            }
        }
    }
    provisional(
        &result,
        RouteResult {
            outcome: Outcome::Compared(Verdict::NotRun(IN_COMPARISON.into())),
            route: route.into(),
            convert_ns: taken.convert_ns,
            inputs_seed: None,
        },
    );
    let options = CompareOptions {
        seed: knob(SEED).unwrap_or(DEFAULT_SEED),
        input_sets: knob(INPUT_SETS).unwrap_or(DEFAULT_INPUT_SETS),
    };
    taken.outcome = Outcome::Compared(match (converted.as_slice(), sources.as_slice()) {
        ([(Stage::Compute, names, (module, reflection))], [(_, source)]) => {
            glsl::compare_compute(source, names, module, reflection, options)
        }
        _ => drawn_comparison(&sources, route, file, options),
    });
    result.routes.push(taken);
    result
}

/// A GLSL program's vertex and fragment stages drawn (`glsl::drawn_program`), the drawn program
/// through the same route, held to llvmpipe.
fn drawn_comparison(
    sources: &[(Stage, String)],
    route: &str,
    file: &Path,
    options: CompareOptions,
) -> Verdict {
    let drawn = match glsl::drawn_program(sources) {
        Ok(d) => d,
        Err(e) => return Verdict::NotRun(e),
    };
    let given: Vec<(Stage, &str)> = drawn.iter().map(|(s, t)| (*s, t.as_str())).collect();
    let stages = match Nir::from_glsl(&given) {
        Ok(s) => s,
        Err(e) => return Verdict::NotRun(format!("the drawn program: {e}")),
    };
    let mut routed = Vec::new();
    for stage in stages {
        let (one, module) = match route {
            ROUTE_ZINK => zink_route(stage.nir, stage.stage, file),
            _ => direct_route(stage.nir, stage.stage),
        };
        match module {
            Some(m) => routed.push((stage.stage, stage.names, m)),
            None => {
                let Outcome::Failed(why) = one.outcome else {
                    unreachable!("a route without a module failed")
                };
                return Verdict::NotRun(format!("the drawn program on the route: {why}"));
            }
        }
    }
    let [
        (Stage::Vertex, vertex_names, (vertex, vertex_reflection)),
        (Stage::Fragment, fragment_names, (fragment, fragment_reflection)),
    ] = routed.as_slice()
    else {
        return Verdict::NotRun("the drawn program is not a vertex and a fragment stage".into());
    };
    glsl::compare_draw(
        &drawn,
        &glsl::RoutedStage {
            names: vertex_names,
            module: vertex,
            reflection: vertex_reflection,
        },
        &glsl::RoutedStage {
            names: fragment_names,
            module: fragment,
            reflection: fragment_reflection,
        },
        options,
    )
}
