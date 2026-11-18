//! The public surface: a pipeline in `alpine:3.22` from its OCI image (as `docker save` writes it) and
//! from a plain tar of its files (as `docker export` writes it) gives the reference's output; the
//! fields the status file lists as honoured behave so; a second run in a new process starts from the
//! cache's directory. Needs Docker with `alpine:3.22` for linux/arm64.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use wcontainers_arm64::Arm64;
use wcontainers_runtime::oci_spec::runtime::Spec;
use wcontainers_runtime::{Ending, Image, Machine, Runtime, RuntimeSettings, System, base_spec};
use wcontainers_wasmtime::{CpuSide, LookAhead, Optimise, Origin, Tuning};

const IMAGE: &str = "alpine:3.22";
const PIPELINE: &str = "cat /etc/os-release | grep ^ID= | tr a-z A-Z; ls /bin | sort -r | head -3; \
     echo one two three | awk '{print $3, $1}' | sed s/three/3/; seq 1 100 | grep 7 | wc -l; \
     echo to error >&2; exit 7";

/// A stream the test reads after the run.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

fn scratch() -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("wc-surface");
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn docker(arguments: &[&str]) -> std::process::Output {
    let output = Command::new("docker").args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "docker {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// The image in the OCI image layout and its files as a plain tar, made once per test process.
fn images() -> &'static (PathBuf, PathBuf) {
    static IMAGES: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
    IMAGES.get_or_init(|| {
        let directory = scratch().join(format!("images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let saved = directory.join("saved.tar");
        docker(&["pull", "-q", "--platform", "linux/arm64", IMAGE]);
        docker(&[
            "save",
            "--platform",
            "linux/arm64",
            "-o",
            saved.to_str().unwrap(),
            IMAGE,
        ]);
        let layout = directory.join("layout");
        tar::Archive::new(std::fs::File::open(&saved).unwrap())
            .unpack(&layout)
            .unwrap();
        let name = format!("wc-surface-{}", std::process::id());
        docker(&[
            "create",
            "--platform",
            "linux/arm64",
            "--name",
            &name,
            IMAGE,
        ]);
        let files = directory.join("files.tar");
        let exported = Command::new("docker")
            .args(["export", "-o", files.to_str().unwrap(), &name])
            .output()
            .unwrap();
        docker(&["rm", "-f", &name]);
        assert!(exported.status.success());
        (layout, files)
    })
}

fn reference(command: &str) -> Outcome {
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--platform",
            "linux/arm64",
            IMAGE,
            "sh",
            "-c",
        ])
        .arg(command)
        .output()
        .unwrap();
    Outcome {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap(),
    }
}

/// Runs `spec` on `image` through the public surface, with the cache in `cache`.
fn ours(spec: &Spec, image: &Image, cache: Option<PathBuf>) -> (Outcome, Vec<(bool, Origin)>) {
    let runtime = Runtime::<CpuSide<Arm64>>::new(&RuntimeSettings { cache });
    let (stdout, stderr) = (Captured::default(), Captured::default());
    let system = System::new(Box::new(stdout.clone()), Box::new(stderr.clone()));
    let tuning = Tuning {
        look_ahead: LookAhead::Full,
        optimise: Optimise::Speed,
    };
    let mut container = runtime
        .container(spec, image, &tuning, &Machine { offer: () }, system)
        .unwrap();
    let Ok(Ending::Finished { status }) = container.run();
    let origins = container
        .back_end()
        .translated()
        .iter()
        .map(|t| (t.at_run_time, t.origin))
        .collect();
    let exit_code = if status & 0x7f == 0 {
        (status >> 8) & 0xff
    } else {
        128 + (status & 0x7f)
    };
    let outcome = Outcome {
        stdout: stdout.text(),
        stderr: stderr.text(),
        exit_code,
    };
    (outcome, origins)
}

fn with_args(mut spec: Spec, args: &[&str]) -> Spec {
    let mut process = spec.process().clone().unwrap();
    process.set_args(Some(args.iter().map(|a| a.to_string()).collect()));
    spec.set_process(Some(process));
    spec
}

#[test]
fn a_pipeline_from_the_oci_image_matches_the_reference() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let spec = image.spec().unwrap();
    assert_eq!(
        spec.process().as_ref().unwrap().args().as_ref().unwrap(),
        &["/bin/sh"]
    );
    let spec = with_args(spec, &["sh", "-c", PIPELINE]);
    let (outcome, _) = ours(&spec, &image, None);
    assert_eq!(outcome, reference(PIPELINE));
}

#[test]
fn a_pipeline_from_a_plain_tar_matches_the_reference() {
    let (_, files) = images();
    let image = Image::tar(files).unwrap();
    let spec = with_args(base_spec(), &["sh", "-c", PIPELINE]);
    let (outcome, _) = ours(&spec, &image, None);
    assert_eq!(outcome, reference(PIPELINE));
}

/// The fields honoured beyond the program: host name, environment, working directory, additional
/// groups and annotations.
#[test]
fn honoured_fields_behave_so() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let mut spec = with_args(
        image.spec().unwrap(),
        &["sh", "-c", "uname -n; pwd; echo $GREETING; id -G"],
    );
    spec.set_hostname(Some("surface".to_string()));
    let mut process = spec.process().clone().unwrap();
    let mut env = process.env().clone().unwrap();
    env.push("GREETING=hello".to_string());
    process.set_env(Some(env));
    process.set_cwd("/etc".into());
    let mut user = process.user().clone();
    user.set_additional_gids(Some(vec![10, 20]));
    process.set_user(user);
    spec.set_process(Some(process));
    spec.set_annotations(Some([("a".to_string(), "b".to_string())].into()));
    let (outcome, _) = ours(&spec, &image, None);
    assert_eq!(
        outcome,
        Outcome {
            stdout: "surface\n/etc\nhello\n0 10 20\n".to_string(),
            stderr: String::new(),
            exit_code: 0,
        }
    );
}

/// The child of [`a_second_process_starts_from_the_cache`]: runs the pipeline with the cache in
/// `WC_SURFACE_CACHE` and prints how each translation was had and the run's time.
#[test]
#[ignore = "run by a_second_process_starts_from_the_cache"]
fn run_with_the_cache() {
    let Some(cache) = std::env::var_os("WC_SURFACE_CACHE") else {
        return;
    };
    let layout = PathBuf::from(std::env::var_os("WC_SURFACE_LAYOUT").unwrap());
    let started = Instant::now();
    let image = Image::oci(&layout, "arm64").unwrap();
    let spec = with_args(image.spec().unwrap(), &["sh", "-c", PIPELINE]);
    let (outcome, origins) = ours(&spec, &image, Some(cache.into()));
    let seconds = started.elapsed().as_secs_f64();
    let count = |run_time: bool, made: bool| {
        origins
            .iter()
            .filter(|(at_run_time, origin)| {
                *at_run_time == run_time
                    && matches!(origin, Origin::Made { .. }) == made
                    && !matches!(origin, Origin::Memory)
            })
            .count()
    };
    println!(
        "surface run: exit {}; ranges made {}, from disk {}; run-time pieces made {}, from disk {}; \
         {} in all; {seconds:.3} s",
        outcome.exit_code,
        count(false, true),
        count(false, false),
        count(true, true),
        count(true, false),
        origins.len()
    );
}

/// Runs the child in a new process: the first fills an empty cache directory, the second finds every
/// range's translation there. Pieces translated at run time where a free turn stopped mid-way differ
/// between runs, so some of them may be made again. Prints both times.
#[test]
fn a_second_process_starts_from_the_cache() {
    let (layout, _) = images();
    let cache = scratch().join(format!("cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    let run = || {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "run_with_the_cache", "--ignored", "--nocapture"])
            .env("WC_SURFACE_CACHE", &cache)
            .env("WC_SURFACE_LAYOUT", layout)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        let line = text
            .lines()
            .find(|line| line.starts_with("surface run:"))
            .unwrap_or_else(|| panic!("{text}{}", String::from_utf8_lossy(&output.stderr)))
            .to_string();
        println!("{line}");
        line
    };
    let first = run();
    let second = run();
    assert!(
        first.contains("exit 7; ranges made") && !first.contains("ranges made 0,"),
        "{first}"
    );
    assert!(
        second.contains("exit 7; ranges made 0, from disk"),
        "{second}"
    );
    assert!(!second.contains("ranges made 0, from disk 0;"), "{second}");
    let _ = std::fs::remove_dir_all(&cache);
}

/// The command line program runs the pipeline from the OCI image and from the plain tar as the
/// reference does, and fails naming a refused field.
#[test]
fn the_command_line_program_runs_a_container() {
    let (layout, files) = images();
    let program = env!("CARGO_BIN_EXE_wcontainers");
    let expected = reference(PIPELINE);
    for source in [
        ["--oci", layout.to_str().unwrap()],
        ["--tar", files.to_str().unwrap()],
    ] {
        let output = Command::new(program)
            .args(source)
            .args(["--", "sh", "-c", PIPELINE])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with(&expected.stderr), "{stderr}");
        assert_eq!(output.status.code(), Some(expected.exit_code));
    }
    let config = scratch().join(format!("config-{}.json", std::process::id()));
    let mut spec = with_args(base_spec(), &["sh"]);
    let mut process = spec.process().clone().unwrap();
    process.set_terminal(Some(true));
    spec.set_process(Some(process));
    spec.save(&config).unwrap();
    let output = Command::new(program)
        .args([
            "--tar",
            files.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("process.terminal is not supported"),
        "{stderr}"
    );
}
