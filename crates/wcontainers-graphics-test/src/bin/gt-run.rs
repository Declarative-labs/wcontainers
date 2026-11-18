//! Runs the Direct3D scene programs and checks or records their outputs.
//!
//! `gt-run check <out-dir> [--api d3d11|d3d12] [--driver hardware|warp] [--level <level>]
//! [--dll <name>=<path>]... [<scene>...]` copies the scene programs (gt-d3d11, gt-d3d12, from this program's directory) into
//! `<out-dir>\bin` with each `--dll` beside them under its name (`--dll d3d11.dll=<path>` puts a
//! replacement d3d11.dll there), runs every scene (or the named ones) in its own process, and checks
//! each output against the stored references (crates/wcontainers-graphics-test/references/<api>)
//! within its allowance (see check.rs). It prints one line per output, `pass`, `FAIL` or `absent`
//! (the implementation reports what the output needs as unsupported, which is not a failure), then
//! a count, and exits 1 if any fails. `--level` caps the Direct3D 11 feature level the scene
//! programs ask for, so real Direct3D shows which outputs a less capable device leaves absent. It also fails a scene whose program loaded a library of a
//! replacement's name from anywhere but `<out-dir>\bin`, and a scene name no program has, notes a
//! replacement no scene loaded (other libraries given are only copied), and reports whether the stored shader corpus matches this build.
//!
//! `gt-run frames <reference-dir> <run-dir>` checks a real program's frames (frame-<n>.png) against
//! its references within their allowances; `gt-run frames --record [--ignore <x>,<y>,<w>,<h>]...
//! <reference-dir> <run-dir>...` stores the first run's frames as the references, with allowances
//! from the other runs and the rectangles left out (see `Allowance::from_repeats`).
//!
//! `gt-run record <out-dir>` runs every scene with the debug layer on the GPU twice (the two runs
//! must agree exactly) and on WARP, and writes the GPU's outputs and their allowances to
//! `<out-dir>\references\<api>` and this build's compiled shaders to `<out-dir>\shaders`, to be
//! copied into the checkout. It prints how far WARP is from the GPU for each output.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use wcontainers_graphics_test::check::{self, Allowance, State, TOLERANCES};
use wcontainers_graphics_test::output::ABSENT;
use wcontainers_graphics_test::{LIBRARIES, compare, load};

const APIS: [&str; 2] = ["d3d11", "d3d12"];
/// How long one scene may run before it counts as hung.
const TIMEOUT: Duration = Duration::from_secs(120);

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("gt-run: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

struct Options {
    out: PathBuf,
    apis: Vec<String>,
    driver: String,
    level: Option<String>,
    dlls: Vec<(String, PathBuf)>,
    scenes: Vec<String>,
}

fn run() -> Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: gt-run check <out-dir> [--api d3d11|d3d12] [--driver hardware|warp] [--level <level>] [--dll <name>=<path>]... [<scene>...] | gt-run record <out-dir> | gt-run frames [--record] <reference-dir> <run-dir>...";
    let (mode, rest) = args.split_first().context(usage)?;
    if mode == "frames" {
        return frames(rest);
    }
    let (out, rest) = rest.split_first().context(usage)?;
    let mut options = Options {
        out: PathBuf::from(out),
        apis: APIS.iter().map(|a| a.to_string()).collect(),
        driver: "hardware".to_owned(),
        level: None,
        dlls: Vec::new(),
        scenes: Vec::new(),
    };
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--api" => options.apis = vec![rest.next().context(usage)?.clone()],
            "--driver" => options.driver = rest.next().context(usage)?.clone(),
            "--level" => options.level = Some(rest.next().context(usage)?.clone()),
            "--dll" => {
                let (name, path) = rest
                    .next()
                    .context(usage)?
                    .split_once('=')
                    .context("--dll takes <name>=<path>")?;
                options.dlls.push((name.to_owned(), PathBuf::from(path)));
            }
            scene => options.scenes.push(scene.to_owned()),
        }
    }
    anyhow::ensure!(
        options.apis.iter().all(|a| APIS.contains(&a.as_str())),
        "apis are d3d11 and d3d12"
    );
    anyhow::ensure!(
        options.level.is_none() || options.apis == ["d3d11"],
        "--level is for --api d3d11: Direct3D 12 devices are 11_0 and up, their features the adapter's"
    );
    match mode.as_str() {
        "check" => check_mode(&options),
        "record" => record_mode(&options),
        _ => bail!("{usage}"),
    }
}

/// Copies the scene programs and the replacement libraries into `<out>\bin`.
fn stage(options: &Options) -> Result<PathBuf> {
    let here = std::env::current_exe()?.parent().unwrap().to_owned();
    let bin = options.out.join("bin");
    if bin.exists() {
        std::fs::remove_dir_all(&bin).with_context(|| format!("removing {}", bin.display()))?;
    }
    std::fs::create_dir_all(&bin)?;
    for api in APIS {
        let exe = format!("gt-{api}.exe");
        std::fs::copy(here.join(&exe), bin.join(&exe)).with_context(|| format!("copying {exe}"))?;
    }
    for (name, path) in &options.dlls {
        std::fs::copy(path, bin.join(name))
            .with_context(|| format!("copying {} as {name}", path.display()))?;
    }
    Ok(bin)
}

/// The scene names a program lists.
fn list(bin: &Path, api: &str) -> Result<Vec<String>> {
    let output = Command::new(bin.join(format!("gt-{api}")))
        .arg("list")
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "gt-{api} list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Runs one scene in its own process, its output and errors into `<dir>\<scene>.log`. Returns the
/// failure, if any, and the program's standard output.
fn run_scene(
    bin: &Path,
    api: &str,
    scene: &str,
    driver: &str,
    level: Option<&str>,
    dir: &Path,
    debug: bool,
) -> Result<(Option<String>, String)> {
    std::fs::create_dir_all(dir)?;
    let log = dir.join(format!("{scene}.log"));
    let mut command = Command::new(bin.join(format!("gt-{api}")));
    command.args([scene, driver]).arg(dir);
    if debug {
        command.arg("--debug");
    }
    if let Some(level) = level {
        command.args(["--level", level]);
    }
    let stdout = dir.join(format!("{scene}.stdout"));
    command
        .stdout(std::fs::File::create(&stdout)?)
        .stderr(std::fs::File::create(&log)?)
        .stdin(Stdio::null());
    let mut child = command.spawn()?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if start.elapsed() > TIMEOUT {
            child.kill()?;
            child.wait()?;
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let printed = std::fs::read_to_string(&stdout).unwrap_or_default();
    let failure = match status {
        None => Some(format!("hung for {} s and was stopped", TIMEOUT.as_secs())),
        Some(s) if s.success() => None,
        Some(s) => {
            let errors = std::fs::read_to_string(&log).unwrap_or_default();
            let last = errors
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .to_owned();
            Some(format!(
                "exit {:?}: {last} (log {})",
                s.code(),
                log.display()
            ))
        }
    };
    Ok((failure, printed))
}

/// Whether the outputs of `file` belong to `scene`: scene names hold no '-' or '.'.
fn of_scene(file: &str, scene: &str) -> bool {
    file.split(['-', '.']).next() == Some(scene)
}

fn check_mode(options: &Options) -> Result<bool> {
    let bin = stage(options)?;
    let (mut passed, mut failed, mut absent) = (0, 0, 0);
    let mut report = |state: State, line: String| {
        let (word, count) = match state {
            State::Pass => ("pass", &mut passed),
            State::Fail => ("FAIL", &mut failed),
            State::Absent => ("absent", &mut absent),
        };
        *count += 1;
        println!("{word} {line}");
    };
    let lists: Vec<Vec<String>> = options
        .apis
        .iter()
        .map(|api| list(&bin, api))
        .collect::<Result<_>>()?;
    for scene in &options.scenes {
        if !lists.iter().any(|l| l.contains(scene)) {
            report(
                State::Fail,
                format!(
                    "{scene}: no such scene in {}; see gt-d3d11 list and gt-d3d12 list",
                    options.apis.join(" or ")
                ),
            );
        }
    }
    let mut loaded = vec![false; options.dlls.len()];
    for (api, all) in options.apis.iter().zip(&lists) {
        let dir = options.out.join(api);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        let scenes: Vec<String> = if options.scenes.is_empty() {
            all.clone()
        } else {
            options
                .scenes
                .iter()
                .filter(|s| all.contains(s))
                .cloned()
                .collect()
        };
        let references = crate_dir().join("references").join(api);
        for scene in &scenes {
            let (failure, printed) = run_scene(
                &bin,
                api,
                scene,
                &options.driver,
                options.level.as_deref(),
                &dir,
                false,
            )?;
            // Where the program loaded each replacement from, as it printed before drawing (a
            // scene that failed may have printed it too).
            let printed = printed.to_lowercase();
            for ((name, _), loaded) in options.dlls.iter().zip(&mut loaded) {
                let name = name.to_lowercase();
                let prefix = format!("{name} from ");
                let Some(from) = printed
                    .split([',', '\n'])
                    .find_map(|part| part.trim().strip_prefix(&prefix))
                else {
                    continue;
                };
                if !LIBRARIES.contains(&name.as_str()) || from == "not loaded" {
                    continue;
                }
                *loaded = true;
                if from != bin.join(&name).display().to_string().to_lowercase() {
                    report(
                        State::Fail,
                        format!(
                            "{api} {scene}: did not load the replacement {name} from {}: {from}",
                            bin.display()
                        ),
                    );
                }
            }
            if let Some(failure) = failure {
                report(State::Fail, format!("{api} {scene}: {failure}"));
            }
        }
        if !references.join(TOLERANCES).exists() {
            report(
                State::Fail,
                format!("{api}: no references in {}", references.display()),
            );
            continue;
        }
        let verdicts = check::check_dir(&references, &dir, |f| {
            [".png", ".exr", ".txt", ".caps"]
                .iter()
                .any(|e| f.ends_with(e))
        })?;
        for v in verdicts
            .iter()
            .filter(|v| scenes.iter().any(|s| of_scene(&v.file, s)))
        {
            report(v.state, format!("{api} {}: {}", v.file, v.detail));
        }
    }
    for ((name, _), loaded) in options.dlls.iter().zip(&loaded) {
        if !loaded && LIBRARIES.contains(&name.to_lowercase().as_str()) {
            println!(
                "note: no scene run loaded a library named {name}, so its replacement was not used"
            );
        }
    }
    match corpus_differences() {
        Ok(differences) if differences.is_empty() => {
            println!("shader corpus: the stored bytecode matches this build")
        }
        Ok(differences) => println!(
            "shader corpus: differs from this build in {} (record again to store it): {}",
            differences.len(),
            differences.join(", ")
        ),
        Err(e) => println!("shader corpus: not compared: {e:#}"),
    }
    println!("{passed} passed, {failed} failed, {absent} absent");
    Ok(failed == 0)
}

fn frames(args: &[String]) -> Result<bool> {
    let usage = "usage: gt-run frames <reference-dir> <run-dir> | gt-run frames --record [--ignore <x>,<y>,<w>,<h>]... <reference-dir> <run-dir>...";
    match args {
        [record, rest @ ..] if record == "--record" => {
            let mut rest = rest;
            let mut ignored = Vec::new();
            while let [flag, rect, more @ ..] = rest
                && flag == "--ignore"
            {
                let numbers: Vec<u32> = rect
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()
                    .context("--ignore takes <x>,<y>,<w>,<h>")?;
                ignored.push(
                    <[u32; 4]>::try_from(numbers)
                        .ok()
                        .context("--ignore takes four numbers")?,
                );
                rest = more;
            }
            let [reference, runs @ ..] = rest else {
                bail!("{usage}")
            };
            anyhow::ensure!(!runs.is_empty(), "{usage}");
            let reference = Path::new(reference);
            std::fs::create_dir_all(reference)?;
            let mut names: Vec<String> = std::fs::read_dir(&runs[0])?
                .map(|e| Ok(e?.file_name().to_string_lossy().into_owned()))
                .collect::<Result<_>>()?;
            names.retain(|n| n.starts_with("frame-") && n.ends_with(".png"));
            names.sort();
            anyhow::ensure!(!names.is_empty(), "no frames in {}", runs[0]);
            let mut tolerances = String::from(
                "# <frame> <channel tolerance> <pixels allowed beyond it> warp 0 0 [tiles <tile>:<pixels>,...], from repeated runs\n",
            );
            let mut frames = Vec::new();
            for name in &names {
                let first = load(&Path::new(&runs[0]).join(name))?;
                let others = runs[1..]
                    .iter()
                    .map(|r| load(&Path::new(r).join(name)))
                    .collect::<Result<Vec<_>>>()?;
                let allowance = Allowance::from_repeats(&first, &others, &ignored)?;
                let none = compare::compare(&first, &first, 0.0)?;
                println!(
                    "{name}: {} of {} tiles drew differently from run to run",
                    allowance.tiles.len(),
                    none.tiles.len()
                );
                let unrepeatable = check::unrepeatable(&allowance, none.tiles.len());
                frames.push((name, allowance, none, unrepeatable));
                std::fs::copy(Path::new(&runs[0]).join(name), reference.join(name))?;
            }
            // A program that draws one frame differently on every run drifts from its start, so
            // none of its frames can be trusted to repeat.
            let drifts = frames.iter().any(|(_, _, _, unrepeatable)| *unrepeatable);
            if drifts {
                println!("so no frame is compared: the program draws differently on every run");
            }
            for (name, allowance, none, _) in &frames {
                tolerances.push_str(&if drifts {
                    check::unrepeatable_line(name)
                } else {
                    check::tolerance_line(name, allowance, none)
                });
            }
            std::fs::write(reference.join(TOLERANCES), tolerances)?;
            Ok(true)
        }
        [reference, run] => {
            let mut ok = true;
            for v in check::check_dir(Path::new(reference), Path::new(run), |f| {
                f.starts_with("frame-") && f.ends_with(".png")
            })? {
                println!(
                    "{} {}: {}",
                    if v.state == State::Pass {
                        "pass"
                    } else {
                        "FAIL"
                    },
                    v.file,
                    v.detail
                );
                ok &= v.state == State::Pass;
            }
            Ok(ok)
        }
        _ => bail!("{usage}"),
    }
}

/// The compiled shaders that differ between this build and the stored corpus, either way.
fn corpus_differences() -> Result<Vec<String>> {
    let built = PathBuf::from(env!("OUT_DIR"));
    let stored = crate_dir().join("shaders");
    let mut differences = Vec::new();
    for kind in ["dxbc", "dxil"] {
        let names = |dir: &Path| -> Result<Vec<String>> {
            let mut names: Vec<String> = match std::fs::read_dir(dir) {
                Ok(entries) => entries
                    .map(|e| Ok(e?.file_name().to_string_lossy().into_owned()))
                    .collect::<Result<_>>()?,
                Err(_) => Vec::new(),
            };
            names.sort();
            Ok(names)
        };
        let (b, s) = (names(&built.join(kind))?, names(&stored.join(kind))?);
        for name in b.iter().chain(s.iter().filter(|n| !b.contains(n))) {
            let a = std::fs::read(built.join(kind).join(name)).ok();
            let c = std::fs::read(stored.join(kind).join(name)).ok();
            if a != c {
                differences.push(format!("{kind}/{name}"));
            }
        }
    }
    Ok(differences)
}

fn record_mode(options: &Options) -> Result<bool> {
    anyhow::ensure!(
        options.dlls.is_empty() && options.scenes.is_empty(),
        "record runs every scene with the system's libraries"
    );
    let bin = stage(options)?;
    let mut ok = true;
    for api in &options.apis {
        let runs = ["hardware", "again", "warp"].map(|r| options.out.join(r).join(api));
        for dir in &runs {
            if dir.exists() {
                std::fs::remove_dir_all(dir)?;
            }
        }
        let scenes = list(&bin, api)?;
        for scene in &scenes {
            for (dir, driver) in runs.iter().zip(["hardware", "hardware", "warp"]) {
                if let (Some(failure), _) = run_scene(&bin, api, scene, driver, None, dir, true)? {
                    bail!("{api} {scene} on {driver}: {failure}");
                }
            }
        }
        let references = options.out.join("references").join(api);
        if references.exists() {
            std::fs::remove_dir_all(&references)?;
        }
        std::fs::create_dir_all(&references)?;
        let mut tolerances = String::from(
            "# <output> <channel tolerance> <pixels allowed beyond it> warp <pixels differing at all> <largest difference>\n",
        );
        let mut files: Vec<String> = std::fs::read_dir(&runs[0])?
            .map(|e| Ok(e?.file_name().to_string_lossy().into_owned()))
            .collect::<Result<_>>()?;
        files.retain(|f| !f.ends_with(".log") && !f.ends_with(".stdout"));
        files.sort();
        for dir in &runs {
            for entry in std::fs::read_dir(dir)? {
                let file = entry?.file_name().to_string_lossy().into_owned();
                if file.ends_with(&format!(".{ABSENT}")) {
                    let why = std::fs::read_to_string(dir.join(&file))?;
                    bail!("{api} {file} in {}: {}", dir.display(), why.trim());
                }
            }
        }
        for file in &files {
            let [gpu, again, warp] = runs.each_ref().map(|d| d.join(file));
            if file.ends_with(".txt") || file.ends_with(".caps") {
                let text = std::fs::read_to_string(&gpu)?;
                if std::fs::read_to_string(&again)? != text {
                    println!("{api} {file}: the two GPU runs differ");
                    ok = false;
                }
                let same = std::fs::read_to_string(&warp).ok().as_deref() == Some(text.as_str());
                println!(
                    "{api} {file}: warp {}",
                    if same { "same" } else { "differs" }
                );
                tolerances.push_str(&check::text_line(file, same));
            } else {
                let (g, a, w) = (load(&gpu)?, load(&again)?, load(&warp)?);
                let repeat = compare::compare(&g, &a, 0.0)?;
                if repeat.differing > 0 {
                    println!(
                        "{api} {file}: the two GPU runs differ in {} pixels",
                        repeat.differing
                    );
                    ok = false;
                }
                let (allowance, exact): (Allowance, _) = Allowance::from_warp(&g, &w)?;
                println!(
                    "{api} {file}: warp differs in {} of {} pixels, largest {}; allowance {} beyond {}",
                    exact.differing,
                    exact.pixels,
                    exact.largest,
                    allowance.pixels,
                    allowance.tolerance
                );
                tolerances.push_str(&check::tolerance_line(file, &allowance, &exact));
            }
            std::fs::copy(&gpu, references.join(file))?;
        }
        std::fs::write(references.join(TOLERANCES), tolerances)?;
    }
    let shaders = options.out.join("shaders");
    for kind in ["dxbc", "dxil"] {
        let to = shaders.join(kind);
        if to.exists() {
            std::fs::remove_dir_all(&to)?;
        }
        std::fs::create_dir_all(&to)?;
        for entry in std::fs::read_dir(PathBuf::from(env!("OUT_DIR")).join(kind))? {
            let entry = entry?;
            std::fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(ok)
}
