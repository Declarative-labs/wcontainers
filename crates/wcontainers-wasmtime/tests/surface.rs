//! The public surface: a pipeline in `alpine:3.22` from its OCI image (as `docker save` writes it) and
//! from a plain tar of its files (as `docker export` writes it) gives the reference's output; the
//! fields the status file lists as honoured behave so; a second run in a new process starts from the
//! cache's directory. Needs Docker with `alpine:3.22` for linux/arm64.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serde_json::json;

use wcontainers_arm64::Arm64;
use wcontainers_runtime::oci_spec::runtime::Spec;
use wcontainers_runtime::{
    Container, Ending, Image, Machine, Runtime, RuntimeSettings, Step, System, base_spec,
};
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
        std::fs::remove_file(&saved).unwrap();
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

/// The look-ahead `LOOK_AHEAD` names, as in the kernel tests: `full` (also when unset), `none`, or
/// `bounded`, whose N (1 to 64) is drawn from `LOOK_AHEAD_SEED` (0 if unset) and printed.
fn look_ahead() -> LookAhead {
    match std::env::var("LOOK_AHEAD").as_deref() {
        Err(_) | Ok("full") => LookAhead::Full,
        Ok("none") => LookAhead::None,
        Ok("bounded") => {
            let seed: u64 = std::env::var("LOOK_AHEAD_SEED").map_or(0, |seed| {
                seed.parse().expect("LOOK_AHEAD_SEED is an unsigned number")
            });
            let n = u32::from(blake3::hash(&seed.to_le_bytes()).as_bytes()[0]) % 64 + 1;
            eprintln!("look-ahead bounded {n} (LOOK_AHEAD_SEED {seed})");
            LookAhead::Bounded(n)
        }
        Ok(other) => panic!("LOOK_AHEAD is full, none or bounded, not {other}"),
    }
}

/// The command line program's `--look-ahead` for [`look_ahead`].
fn look_ahead_argument() -> String {
    match look_ahead() {
        LookAhead::Full => "full".to_string(),
        LookAhead::None => "none".to_string(),
        LookAhead::Bounded(n) => n.to_string(),
    }
}

/// A container of `spec` on `image` through the public surface, at [`look_ahead`], writing into the
/// two streams.
fn container(
    spec: &Spec,
    image: &Image,
    cache: Option<PathBuf>,
    stdout: &Captured,
    stderr: &Captured,
) -> Container<CpuSide<Arm64>, System> {
    let runtime = Runtime::<CpuSide<Arm64>>::new(&RuntimeSettings { cache });
    let system = System::new(Box::new(stdout.clone()), Box::new(stderr.clone()));
    let tuning = Tuning {
        look_ahead: look_ahead(),
        optimise: Optimise::Speed,
    };
    runtime
        .container(spec, image, &tuning, &Machine { offer: () }, system)
        .unwrap()
}

/// What a shell reports for a wait status.
fn outcome(status: i32, stdout: &Captured, stderr: &Captured) -> Outcome {
    let exit_code = if status & 0x7f == 0 {
        (status >> 8) & 0xff
    } else {
        128 + (status & 0x7f)
    };
    Outcome {
        stdout: stdout.text(),
        stderr: stderr.text(),
        exit_code,
    }
}

/// Runs `spec` on `image` through the public surface, with the cache in `cache`.
fn ours(spec: &Spec, image: &Image, cache: Option<PathBuf>) -> (Outcome, Vec<(bool, Origin)>) {
    let (stdout, stderr) = (Captured::default(), Captured::default());
    let mut container = container(spec, image, cache, &stdout, &stderr);
    let Ok(Ending::Finished { status }) = container.run();
    let origins = container
        .back_end()
        .translated()
        .iter()
        .map(|t| (t.at_run_time, t.origin))
        .collect();
    (outcome(status, &stdout, &stderr), origins)
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
    let spec = with_args(image.spec().unwrap(), &["sh", "-c", PIPELINE]);
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

/// With no user in the image, the user, groups and environment are Docker's, from either form.
#[test]
fn the_user_and_environment_match_the_reference() {
    let (layout, files) = images();
    let command = "id; echo \"home=$HOME\"; env | sort | grep -v ^HOSTNAME=";
    let expected = reference(command);
    for image in [
        Image::oci(layout, "arm64").unwrap(),
        Image::tar(files).unwrap(),
    ] {
        let spec = with_args(image.spec().unwrap(), &["sh", "-c", command]);
        assert_eq!(ours(&spec, &image, None).0, expected);
    }
}

/// Linux's capability numbers, by the names the configuration uses.
const CAPABILITY_NUMBERS: [&str; 41] = [
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_DAC_READ_SEARCH",
    "CAP_FOWNER",
    "CAP_FSETID",
    "CAP_KILL",
    "CAP_SETGID",
    "CAP_SETUID",
    "CAP_SETPCAP",
    "CAP_LINUX_IMMUTABLE",
    "CAP_NET_BIND_SERVICE",
    "CAP_NET_BROADCAST",
    "CAP_NET_ADMIN",
    "CAP_NET_RAW",
    "CAP_IPC_LOCK",
    "CAP_IPC_OWNER",
    "CAP_SYS_MODULE",
    "CAP_SYS_RAWIO",
    "CAP_SYS_CHROOT",
    "CAP_SYS_PTRACE",
    "CAP_SYS_PACCT",
    "CAP_SYS_ADMIN",
    "CAP_SYS_BOOT",
    "CAP_SYS_NICE",
    "CAP_SYS_RESOURCE",
    "CAP_SYS_TIME",
    "CAP_SYS_TTY_CONFIG",
    "CAP_MKNOD",
    "CAP_LEASE",
    "CAP_AUDIT_WRITE",
    "CAP_AUDIT_CONTROL",
    "CAP_SETFCAP",
    "CAP_MAC_OVERRIDE",
    "CAP_MAC_ADMIN",
    "CAP_SYSLOG",
    "CAP_WAKE_ALARM",
    "CAP_BLOCK_SUSPEND",
    "CAP_AUDIT_READ",
    "CAP_PERFMON",
    "CAP_BPF",
    "CAP_CHECKPOINT_RESTORE",
];

/// Each resource limit as the configuration names it and `/proc/self/limits` does.
const LIMIT_NAMES: [(&str, &str); 16] = [
    ("RLIMIT_CPU", "Max cpu time"),
    ("RLIMIT_FSIZE", "Max file size"),
    ("RLIMIT_DATA", "Max data size"),
    ("RLIMIT_STACK", "Max stack size"),
    ("RLIMIT_CORE", "Max core file size"),
    ("RLIMIT_RSS", "Max resident set"),
    ("RLIMIT_NPROC", "Max processes"),
    ("RLIMIT_NOFILE", "Max open files"),
    ("RLIMIT_MEMLOCK", "Max locked memory"),
    ("RLIMIT_AS", "Max address space"),
    ("RLIMIT_LOCKS", "Max file locks"),
    ("RLIMIT_SIGPENDING", "Max pending signals"),
    ("RLIMIT_MSGQUEUE", "Max msgqueue size"),
    ("RLIMIT_NICE", "Max nice priority"),
    ("RLIMIT_RTPRIO", "Max realtime priority"),
    ("RLIMIT_RTTIME", "Max realtime timeout"),
];

/// The values the runtime accepts as honoured (its base configuration, with the umask and every
/// limit written out) are what the guest sees: the umask, each limit, the capability sets, the
/// mounts and the read-only paths. Any other value of these is refused (tests/refused.rs).
#[test]
fn the_guest_sees_the_honoured_values() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let command = "umask; echo; cat /proc/self/limits; echo; grep ^Cap /proc/self/status; echo; \
                   cat /proc/mounts";
    let mut spec = with_args(image.spec().unwrap(), &["sh", "-c", command]);
    let unlimited = u64::MAX;
    let limits = [
        ("RLIMIT_STACK", 8 << 20, unlimited),
        ("RLIMIT_CORE", 0, unlimited),
        ("RLIMIT_NOFILE", 1 << 20, 1 << 20),
        ("RLIMIT_NICE", 0, 0),
        ("RLIMIT_RTPRIO", 0, 0),
        ("RLIMIT_SIGPENDING", 128_302, 128_302),
        ("RLIMIT_MSGQUEUE", 819_200, 819_200),
    ];
    let rlimits: Vec<_> = LIMIT_NAMES
        .iter()
        .map(|(name, _)| {
            let (soft, hard) = limits
                .iter()
                .find(|(limit, ..)| limit == name)
                .map_or((unlimited, unlimited), |&(_, soft, hard)| (soft, hard));
            json!({"type": name, "soft": soft, "hard": hard})
        })
        .collect();
    let mut value = serde_json::to_value(&spec).unwrap();
    value["process"]["user"]["umask"] = json!(0o022);
    value["process"]["rlimits"] = json!(rlimits);
    spec = serde_json::from_value(value.clone()).unwrap();
    let (outcome, _) = ours(&spec, &image, None);
    assert_eq!((outcome.exit_code, outcome.stderr.as_str()), (0, ""));
    let sections: Vec<&str> = outcome.stdout.split("\n\n").collect();
    let [umask, limits, capabilities, mounts] = sections[..] else {
        panic!("{}", outcome.stdout);
    };

    assert_eq!(
        umask,
        format!(
            "{:04o}",
            value["process"]["user"]["umask"].as_u64().unwrap()
        )
    );

    for line in limits.lines().skip(1) {
        let (title, rest) = line.split_at(26);
        let (name, _) = LIMIT_NAMES
            .iter()
            .find(|(_, shown)| *shown == title.trim())
            .unwrap_or_else(|| panic!("{line}"));
        let given = rlimits.iter().find(|limit| limit["type"] == *name).unwrap();
        let number = |text: &str| {
            if text == "unlimited" {
                u64::MAX
            } else {
                text.parse().unwrap()
            }
        };
        let seen: Vec<u64> = rest.split_whitespace().take(2).map(number).collect();
        assert_eq!(
            seen,
            [
                given["soft"].as_u64().unwrap(),
                given["hard"].as_u64().unwrap()
            ],
            "{line}"
        );
    }
    assert_eq!(limits.lines().count(), 1 + LIMIT_NAMES.len());

    let sets = &value["process"]["capabilities"];
    for (shown, set) in [
        ("CapInh", "inheritable"),
        ("CapPrm", "permitted"),
        ("CapEff", "effective"),
        ("CapBnd", "bounding"),
        ("CapAmb", "ambient"),
    ] {
        let mask = sets[set]
            .as_array()
            .into_iter()
            .flatten()
            .fold(0u64, |mask, name| {
                let number = CAPABILITY_NUMBERS
                    .iter()
                    .position(|known| *known == name.as_str().unwrap())
                    .unwrap();
                mask | 1 << number
            });
        let line = format!("{shown}:\t{mask:016x}");
        assert!(
            capabilities.lines().any(|seen| seen == line),
            "{line} in\n{capabilities}"
        );
    }

    // Each mount as the configuration gives it, then each read-only path, and nothing else but the root.
    let seen: Vec<Vec<&str>> = mounts
        .lines()
        .map(|line| line.split(' ').collect())
        .collect();
    let options =
        |line: &[&str]| -> Vec<String> { line[3].split(',').map(str::to_string).collect() };
    let trimmed = |option: &str| match option.split_once('=') {
        Some((name @ ("mode" | "ptmxmode"), number)) => {
            format!("{name}={}", number.trim_start_matches('0'))
        }
        _ => option.to_string(),
    };
    let given = value["mounts"].as_array().unwrap();
    for mount in given {
        let line = seen
            .iter()
            .find(|line| line[1] == mount["destination"])
            .unwrap_or_else(|| panic!("{mount} in\n{mounts}"));
        assert_eq!(
            (line[0], line[2]),
            (
                mount["source"].as_str().unwrap(),
                mount["type"].as_str().unwrap()
            )
        );
        let shown = options(line);
        for option in mount["options"].as_array().unwrap() {
            match option.as_str().unwrap() {
                "strictatime" => assert!(!shown.contains(&"relatime".to_string()), "{line:?}"),
                // Shown by neither Linux nor the kernel: no visible effect, or tmpfs's default.
                "newinstance" | "mode=1777" => {}
                option => assert!(shown.contains(&trimmed(option)), "{option} in {line:?}"),
            }
        }
        for flag in ["nosuid", "noexec", "nodev", "ro"] {
            let asked = mount["options"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o == flag);
            assert_eq!(
                shown.contains(&flag.to_string()),
                asked,
                "{flag} in {line:?}"
            );
        }
    }
    let readonly = value["linux"]["readonlyPaths"].as_array().unwrap();
    for path in readonly {
        let line = seen.iter().find(|line| line[1] == path).unwrap();
        assert!(options(line).contains(&"ro".to_string()), "{line:?}");
    }
    assert_eq!(seen[0][1], "/");
    assert_eq!(seen.len(), 1 + given.len() + readonly.len(), "{mounts}");
    // Made in the configuration's order.
    let destinations: Vec<&str> = seen[1..=given.len()].iter().map(|line| line[1]).collect();
    let listed: Vec<&str> = given
        .iter()
        .map(|mount| mount["destination"].as_str().unwrap())
        .collect();
    assert_eq!(destinations, listed, "{mounts}");
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
            .args(["--look-ahead", &look_ahead_argument()])
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

/// The shipped outside's timer ends a free turn at the kernel's slice: a process that never makes a
/// system call does not keep the shell from going on.
#[test]
fn a_busy_process_is_preempted_by_the_timer() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let command = "while :; do :; done & sleep 0.2; kill $!; echo done";
    let spec = with_args(image.spec().unwrap(), &["sh", "-c", command]);
    let (outcome, _) = ours(&spec, &image, None);
    assert_eq!(outcome, reference(command));
}

/// A caller's own loop: [`Container::step`] until the container finishes, waiting with
/// [`Container::idle`] at each idle step, gives the reference's output.
#[test]
fn a_container_is_stepped_from_the_callers_loop() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let command = "echo a; sleep 0.3; echo b | tr b c; exit 4";
    let spec = with_args(image.spec().unwrap(), &["sh", "-c", command]);
    let (stdout, stderr) = (Captured::default(), Captured::default());
    let mut container = container(&spec, &image, None, &stdout, &stderr);
    let (mut steps, mut idles) = (0, 0);
    let status = loop {
        steps += 1;
        match container.step() {
            Ok(Step::Ran) => {}
            Ok(Step::Idle { until_nanoseconds }) => {
                idles += 1;
                container.idle(until_nanoseconds);
            }
            Ok(Step::Finished { status }) => break status,
            Ok(Step::Ended(never)) | Err(never) => match never {},
        }
    };
    println!("stepped: {steps} steps, {idles} idle");
    assert!(idles > 0, "the sleep waits at an idle step");
    assert_eq!(outcome(status, &stdout, &stderr), reference(command));
}

/// The threaded program tests/programs/threaded.c, built for linux/arm64 with zig's C compiler.
fn threaded_program() -> PathBuf {
    let directory = scratch().join(format!("threaded-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let program = directory.join("threaded");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/threaded.c");
    let built = Command::new("zig")
        .args([
            "cc",
            "-target",
            "aarch64-linux-musl",
            "-O2",
            "-static",
            "-s",
            "-o",
        ])
        .arg(&program)
        .arg(&source)
        .output()
        .expect("running zig cc; run the tests inside `nix develop`");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    program
}

/// The plain tar's files with `program` added as `/wc/<its name>`.
fn tar_with(files: &Path, program: &Path) -> PathBuf {
    let name = program.file_name().unwrap().to_str().unwrap();
    let combined = program.with_extension("tar");
    let mut builder = tar::Builder::new(std::fs::File::create(&combined).unwrap());
    let mut archive = tar::Archive::new(std::fs::File::open(files).unwrap());
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let header = entry.header().clone();
        builder.append(&header, &mut entry).unwrap();
    }
    let mut directory = tar::Header::new_gnu();
    directory.set_entry_type(tar::EntryType::Directory);
    directory.set_mode(0o755);
    directory.set_size(0);
    directory.set_uid(0);
    directory.set_gid(0);
    directory.set_mtime(0);
    builder
        .append_data(&mut directory, "wc/", std::io::empty())
        .unwrap();
    let bytes = std::fs::read(program).unwrap();
    let mut file = tar::Header::new_gnu();
    file.set_mode(0o755);
    file.set_size(bytes.len() as u64);
    file.set_uid(0);
    file.set_gid(0);
    file.set_mtime(0);
    builder
        .append_data(&mut file, format!("wc/{name}"), bytes.as_slice())
        .unwrap();
    builder.finish().unwrap();
    combined
}

/// Threads in free turns through the public surface, as on the reference: turns taken in order
/// through a mutex and a condition variable, a contended counter, a thread that spins without system
/// calls until another sets a flag (only the outside's timer ends its turn), a timer signal sent to
/// one chosen thread, and the process ended by one thread while another waits in `read` and the first
/// in `pthread_join`.
#[test]
fn a_threaded_program_matches_the_reference() {
    let (_, files) = images();
    let program = threaded_program();
    let directory = program.parent().unwrap().to_str().unwrap();
    let output = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "-v"])
        .arg(format!("{directory}:/wc:ro"))
        .args([IMAGE, "/wc/threaded"])
        .output()
        .unwrap();
    let theirs = Outcome {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap(),
    };
    assert_eq!(theirs.exit_code, 3, "{theirs:?}");
    let image = Image::tar(&tar_with(files, &program)).unwrap();
    let spec = with_args(image.spec().unwrap(), &["/wc/threaded"]);
    let (ours, _) = ours(&spec, &image, None);
    assert_eq!(ours, theirs);
}

/// An empty host name is no host name, as runc sets none then: the guest sees `localhost`.
#[test]
fn an_empty_host_name_is_localhost() {
    let (_, files) = images();
    let image = Image::tar(files).unwrap();
    let mut spec = with_args(
        image.spec().unwrap(),
        &["sh", "-c", "uname -n; cat /etc/hostname"],
    );
    spec.set_hostname(Some(String::new()));
    let (outcome, _) = ours(&spec, &image, None);
    assert_eq!(outcome.stdout, "localhost\nlocalhost\n");
}

/// The files Docker writes in every container are there from either form of image, as on the
/// reference: the host name, a hosts line for it, and a name server.
#[test]
fn dockers_files_are_written_from_either_image() {
    let (layout, files) = images();
    let command = "for f in hostname hosts resolv.conf; do ls -l /etc/$f | cut -c1-10; done; \
                   [ \"$(cat /etc/hostname)\" = \"$(uname -n)\" ] && echo named; \
                   tail -n 1 /etc/hosts | grep -c \"[[:space:]]$(uname -n)\\$\"; \
                   grep -q ^nameserver /etc/resolv.conf && echo resolver";
    let expected = reference(command);
    assert_eq!(
        expected.stdout,
        "-rw-r--r--\n-rw-r--r--\n-rw-r--r--\nnamed\n1\nresolver\n"
    );
    for image in [
        Image::oci(layout, "arm64").unwrap(),
        Image::tar(files).unwrap(),
    ] {
        let spec = with_args(image.spec().unwrap(), &["sh", "-c", command]);
        assert_eq!(ours(&spec, &image, None).0, expected);
    }
}

/// A program name with no slash that is not in `PATH` makes no container, as runc refuses it, even
/// when the working directory holds a file of that name.
#[test]
fn a_program_not_in_the_path_is_not_found() {
    let (_, files) = images();
    let image = Image::tar(files).unwrap();
    let mut spec = with_args(image.spec().unwrap(), &["ls", "/etc/alpine-release"]);
    let mut process = spec.process().clone().unwrap();
    process.set_env(Some(vec!["PATH=/nonexistent".to_string()]));
    process.set_cwd("/bin".into());
    spec.set_process(Some(process));
    let runtime = Runtime::<CpuSide<Arm64>>::new(&RuntimeSettings { cache: None });
    let system = System::new(Box::new(std::io::sink()), Box::new(std::io::sink()));
    let tuning = Tuning {
        look_ahead: look_ahead(),
        optimise: Optimise::Speed,
    };
    match runtime.container(&spec, &image, &tuning, &Machine { offer: () }, system) {
        Err(wcontainers_runtime::Error::NotFound(name)) => assert_eq!(name, "ls"),
        Err(other) => panic!("{other}"),
        Ok(_) => panic!("ls was run from the working directory"),
    }
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--platform",
            "linux/arm64",
            "-e",
            "PATH=/nonexistent",
        ])
        .args(["-w", "/bin", IMAGE, "ls", "/etc/alpine-release"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(127));
}
