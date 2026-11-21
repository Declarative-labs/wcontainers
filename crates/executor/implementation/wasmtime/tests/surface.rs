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
use wcontainers_executor_wasmtime::{
    BOUND, CpuSide, LookAhead, Optimise, Origin, TranslatorConfig, cli,
};
use wcontainers_runtime_single_kernel::oci_spec::runtime::Spec;
use wcontainers_runtime_single_kernel::{
    Container, Ending, GuestMachine, Image, LocalSystem, MASKED_PATHS_FIELD, READONLY_PATHS_FIELD,
    Runtime, RuntimeConfig, Step, base_spec,
};

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
    let output = Command::new(wcontainers_docker::CLIENT)
        .args(arguments)
        .output()
        .unwrap();
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
        docker(&[
            "pull",
            "-q",
            wcontainers_docker::PLATFORM_OPTION,
            "linux/arm64",
            IMAGE,
        ]);
        docker(&[
            "save",
            wcontainers_docker::PLATFORM_OPTION,
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
            wcontainers_docker::PLATFORM_OPTION,
            "linux/arm64",
            wcontainers_docker::NAME_OPTION,
            &name,
            IMAGE,
        ]);
        let files = directory.join("files.tar");
        let exported = Command::new(wcontainers_docker::CLIENT)
            .args([
                wcontainers_docker::EXPORT,
                "-o",
                files.to_str().unwrap(),
                &name,
            ])
            .output()
            .unwrap();
        docker(&[wcontainers_docker::REMOVE, "-f", &name]);
        assert!(exported.status.success());
        (layout, files)
    })
}

fn reference(command: &str) -> Outcome {
    reference_with(&[], None, command)
}

/// As [`reference`], with more options for `docker run` and, when given, `input` as its standard
/// input (none at all otherwise).
fn reference_with(options: &[&str], input: Option<&[u8]>, command: &str) -> Outcome {
    let mut child = Command::new(wcontainers_docker::CLIENT)
        .args([
            "run",
            wcontainers_docker::REMOVE_ON_EXIT,
            wcontainers_docker::PLATFORM_OPTION,
            "linux/arm64",
        ])
        .args(options)
        .args([IMAGE, "sh", "-c"])
        .arg(command)
        .stdin(match input {
            Some(_) => std::process::Stdio::piped(),
            None => std::process::Stdio::null(),
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        let mut stdin = child.stdin.take().unwrap();
        let input = input.to_vec();
        std::thread::spawn(move || stdin.write_all(&input));
    }
    let output = child.wait_with_output().unwrap();
    Outcome {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap(),
    }
}

/// The look-ahead `LOOK_AHEAD` names, as in the kernel tests: `full` (also when unset), `none`, or
/// `bounded`, whose N (1 to the translator's default bound) is drawn from `LOOK_AHEAD_SEED` (0 if
/// unset) and printed.
fn look_ahead() -> LookAhead {
    match std::env::var("LOOK_AHEAD").as_deref() {
        Err(_) | Ok("full") => LookAhead::Full,
        Ok("none") => LookAhead::None,
        Ok("bounded") => {
            let seed: u64 = std::env::var("LOOK_AHEAD_SEED").map_or(0, |seed| {
                seed.parse().expect("LOOK_AHEAD_SEED is an unsigned number")
            });
            let drawn = blake3::hash(&seed.to_le_bytes()).as_bytes()[..size_of::<u32>()]
                .try_into()
                .map(u32::from_le_bytes)
                .expect("a hash is longer than a word");
            let n = drawn % BOUND + 1;
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
) -> Container<CpuSide<Arm64>, LocalSystem> {
    let system = LocalSystem::new(Box::new(stdout.clone()), Box::new(stderr.clone()));
    container_on(spec, image, cache, system)
}

/// As [`container`], with a system the caller made.
fn container_on(
    spec: &Spec,
    image: &Image,
    cache: Option<PathBuf>,
    system: LocalSystem,
) -> Container<CpuSide<Arm64>, LocalSystem> {
    let runtime = Runtime::<CpuSide<Arm64>>::new(&RuntimeConfig { cache });
    let translator_config = TranslatorConfig {
        look_ahead: look_ahead(),
        optimise: Optimise::Speed,
    };
    runtime
        .container(
            spec,
            image,
            &translator_config,
            &GuestMachine { offer: () },
            system,
        )
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
        .executor()
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

/// The twin of the image in `layout` whose layers are zstd: an image layout holding its linux/arm64
/// manifest and configuration, and its layers' tars as libzstd compresses them.
fn zstd_twin(layout: &Path) -> PathBuf {
    use sha2::{Digest as _, Sha256};
    use std::io::Read;
    use wcontainers_runtime_single_kernel::image::{BLOBS_DIRECTORY, INDEX_FILE, LAYOUT_FILE};
    use wcontainers_runtime_single_kernel::oci_spec::image::{
        Descriptor, Digest, ImageIndex, ImageManifest, MediaType, Os,
    };
    let twin = layout.with_file_name("zstd-layout");
    let _ = std::fs::remove_dir_all(&twin);
    let blobs = twin.join(BLOBS_DIRECTORY).join("sha256");
    std::fs::create_dir_all(&blobs).unwrap();
    let read_blob = |descriptor: &Descriptor| {
        let digest = descriptor.digest();
        std::fs::read(
            layout
                .join(BLOBS_DIRECTORY)
                .join("sha256")
                .join(digest.digest()),
        )
        .unwrap()
    };
    let write_blob = |media_type: MediaType, bytes: &[u8]| {
        let digest = format!("{:x}", Sha256::digest(bytes));
        std::fs::write(blobs.join(&digest), bytes).unwrap();
        let digest: Digest = format!("sha256:{digest}").parse().unwrap();
        Descriptor::new(media_type, bytes.len() as u64, digest)
    };
    let mut index = ImageIndex::from_file(layout.join(INDEX_FILE)).unwrap();
    let found = index
        .manifests()
        .iter()
        .find(|descriptor| {
            descriptor.platform().as_ref().is_some_and(|platform| {
                *platform.os() == Os::Linux && platform.architecture().to_string() == "arm64"
            })
        })
        .unwrap()
        .clone();
    let mut manifest = ImageManifest::from_reader(read_blob(&found).as_slice()).unwrap();
    let config = manifest.config();
    write_blob(config.media_type().clone(), &read_blob(config));
    let layers = manifest
        .layers()
        .iter()
        .map(|layer| {
            assert_eq!(*layer.media_type(), MediaType::ImageLayerGzip);
            let mut tar = Vec::new();
            flate2::read::GzDecoder::new(read_blob(layer).as_slice())
                .read_to_end(&mut tar)
                .unwrap();
            let mut encoder = zstd::stream::Encoder::new(Vec::new(), 3).unwrap();
            encoder.include_checksum(true).unwrap();
            std::io::Write::write_all(&mut encoder, &tar).unwrap();
            write_blob(MediaType::ImageLayerZstd, &encoder.finish().unwrap())
        })
        .collect();
    manifest.set_layers(layers);
    let mut descriptor = write_blob(
        MediaType::ImageManifest,
        manifest.to_string().unwrap().as_bytes(),
    );
    descriptor.set_platform(found.platform().clone());
    index.set_manifests(vec![descriptor]);
    index.to_file(twin.join(INDEX_FILE)).unwrap();
    std::fs::copy(layout.join(LAYOUT_FILE), twin.join(LAYOUT_FILE)).unwrap();
    twin
}

/// Every file of the image's directories: its name, kind, mode, owner, size and time, then the
/// contents of every regular file.
const EVERY_FILE: &str = "cd / && D='bin etc home lib media mnt opt root run sbin srv tmp usr var'; \
     find $D -exec stat -c '%n %F %a %u %g %s %Y %N' {} + | sort; \
     find $D -type f -exec md5sum {} + | sort";

/// The image's zstd twin runs as the image does: the pipeline, and what every file holds.
#[test]
fn the_zstd_twin_of_the_oci_image_runs_as_the_image() {
    let (layout, _) = images();
    let gzip = Image::oci(layout, "arm64").unwrap();
    let zstd = Image::oci(&zstd_twin(layout), "arm64").unwrap();
    let run = |image: &Image, command| {
        let spec = with_args(image.spec().unwrap(), &["sh", "-c", command]);
        ours(&spec, image, None).0
    };
    let files = run(&gzip, EVERY_FILE);
    assert_eq!((files.exit_code, files.stderr.as_str()), (0, ""));
    assert!(files.stdout.contains("bin/busybox"));
    assert_eq!(run(&zstd, EVERY_FILE), files);
    assert_eq!(run(&zstd, PIPELINE), reference(PIPELINE));
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
        for flag in ["nosuid", "noexec", "nodev", wcontainers_docker::READ_ONLY] {
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
    let readonly = value["linux"][READONLY_PATHS_FIELD].as_array().unwrap();
    for path in readonly {
        let line = seen.iter().find(|line| line[1] == path).unwrap();
        assert!(
            options(line).contains(&wcontainers_docker::READ_ONLY.to_string()),
            "{line:?}"
        );
    }
    let masked = value["linux"][MASKED_PATHS_FIELD].as_array().unwrap();
    for path in masked {
        assert!(
            seen.iter().any(|line| line[1] == path),
            "{path} in\n{mounts}"
        );
    }
    assert_eq!(seen[0][1], "/");
    assert_eq!(
        seen.len(),
        1 + given.len() + readonly.len() + masked.len(),
        "{mounts}"
    );
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

/// A user the kernel cannot start the first process as: any but the one the base configuration
/// starts it as (root).
fn unhonoured_uid() -> u32 {
    base_spec()
        .process()
        .as_ref()
        .expect("a process")
        .user()
        .uid()
        + 1
}

/// The command line program runs the pipeline from the OCI image and from the plain tar as the
/// reference does, and fails naming a refused field, having warned of a property it ignored.
#[test]
fn the_command_line_program_runs_a_container() {
    let (layout, files) = images();
    let program = env!("CARGO_BIN_EXE_wcon");
    let expected = reference(PIPELINE);
    for source in [
        [cli::OCI, layout.to_str().unwrap()],
        [cli::TAR, files.to_str().unwrap()],
    ] {
        let output = Command::new(program)
            .args(source)
            .args([cli::LOOK_AHEAD, &look_ahead_argument()])
            .args([cli::END_OF_OPTIONS, "sh", "-c", PIPELINE])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with(&expected.stderr), "{stderr}");
        assert_eq!(output.status.code(), Some(expected.exit_code));
    }
    let config = scratch().join(format!("config-{}.json", std::process::id()));
    let spec = with_args(base_spec(), &["sh"]);
    let mut json = serde_json::to_value(&spec).unwrap();
    json["process"]["user"]["uid"] = json!(unhonoured_uid());
    json["futureField"] = json!(true);
    std::fs::write(&config, serde_json::to_vec(&json).unwrap()).unwrap();
    let output = Command::new(program)
        .args([
            cli::TAR,
            files.to_str().unwrap(),
            cli::CONFIG,
            config.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("process.user.uid is not supported"),
        "{stderr}"
    );
    assert!(stderr.contains("ignored: futureField: 1"), "{stderr}");
}

/// The local system's timer ends a free turn at the kernel's slice: a process that never makes a
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

/// Where the container mounts a host directory in the tests of the file backend.
const VOLUME: &str = "/data";

/// The commands of the bind mount test, run in [`VOLUME`]: reading what the host put there, then
/// making, writing, renaming, linking and removing files and directories, and listing what is left.
const VOLUME_COMMAND: &str = "cat given.txt && wc -c < sub/inner.txt && \
    echo written > made.txt && mkdir -p new/deeper && echo deep > new/deeper/f && \
    mv made.txt moved.txt && ln -s moved.txt link && ln moved.txt hard && readlink link && \
    cat link hard && chmod 640 moved.txt && rm sub/inner.txt && rmdir sub && \
    dd if=/dev/zero of=big bs=4096 count=300 2>/dev/null && truncate -s 1000 big && \
    printf 'x%.0s' $(seq 1 70000) > long && \
    ls -a . new new/deeper; stat -c '%n %s %a %F %h' given.txt moved.txt link hard big long new; \
    cat missing; ls -d nothing/; echo end";

/// A host directory for the bind mount test, with the files it starts with.
fn volume_directory(name: &str) -> PathBuf {
    let directory = scratch().join(format!("volume-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(directory.join("sub")).unwrap();
    std::fs::write(directory.join("given.txt"), "given by the host\n").unwrap();
    std::fs::write(directory.join("sub/inner.txt"), "inner\n").unwrap();
    directory
}

/// A directory's files as the host sees them: path, kind, size, permissions, contents and link
/// target, sorted.
fn tree(directory: &Path) -> Vec<String> {
    use std::os::unix::fs::PermissionsExt;
    let mut found = Vec::new();
    let mut walk = vec![directory.to_path_buf()];
    while let Some(at) = walk.pop() {
        for entry in std::fs::read_dir(&at).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let name = path.strip_prefix(directory).unwrap().display().to_string();
            let mode = meta.permissions().mode() & 0o7777;
            if meta.file_type().is_symlink() {
                found.push(format!(
                    "{name} link {:?}",
                    std::fs::read_link(&path).unwrap()
                ));
            } else if meta.is_dir() {
                found.push(format!("{name} directory {mode:o}"));
                walk.push(path);
            } else {
                let contents = blake3::hash(&std::fs::read(&path).unwrap());
                found.push(format!("{name} file {} {mode:o} {contents}", meta.len()));
            }
        }
    }
    found.sort();
    found
}

/// A host directory bind-mounted through the configuration is read and written through the local
/// system's file backend as Docker's bind mount is: the same output, and the same files left.
#[test]
fn a_host_directory_is_served_through_a_bind_mount() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let ours_directory = volume_directory("ours");
    let reference_directory = volume_directory("reference");
    let command = format!("cd {VOLUME} && {VOLUME_COMMAND}");
    let mut spec = with_args(image.spec().unwrap(), &["sh", "-c", &command]);
    let mut mounts = spec.mounts().clone().unwrap();
    mounts.push(
        serde_json::from_value(json!({
            "destination": VOLUME,
            "type": "bind",
            "source": ours_directory,
            "options": ["rbind", "rw"],
        }))
        .unwrap(),
    );
    spec.set_mounts(Some(mounts));
    let (ours, _) = ours(&spec, &image, None);
    // Docker on Linux leaves the files its root made owned by root: the reference hands them to the
    // test's user after the commands, so that the test reads them.
    use std::os::unix::fs::MetadataExt;
    let ours_owner = std::fs::metadata(&ours_directory).unwrap();
    let owner = format!("{}:{}", ours_owner.uid(), ours_owner.gid());
    let reference_command =
        format!("{command}; ended=$?; chown -R -h {owner} {VOLUME}; exit $ended");
    let output = Command::new(wcontainers_docker::CLIENT)
        .args([
            "run",
            wcontainers_docker::REMOVE_ON_EXIT,
            wcontainers_docker::PLATFORM_OPTION,
            "linux/arm64",
            "--volume",
        ])
        .arg(format!("{}:{VOLUME}", reference_directory.display()))
        .args([IMAGE, "sh", "-c", &reference_command])
        .output()
        .unwrap();
    let reference = Outcome {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap(),
    };
    println!("ours:\n{}{}", ours.stdout, ours.stderr);
    assert_eq!(ours, reference);
    assert_eq!(tree(&ours_directory), tree(&reference_directory));
    let _ = std::fs::remove_dir_all(&ours_directory);
    let _ = std::fs::remove_dir_all(&reference_directory);
}

/// The container's port as the TCP connection tests publish it.
const SERVED_PORT: u16 = 8080;

/// The command that serves one TCP connection in the container: it answers and prints what came.
fn serve_command() -> String {
    format!("echo pong | nc -l -p {SERVED_PORT}; echo served")
}

/// Sends "ping" to `at` and reads to the end, again until the server answers: a forwarder (Docker's on
/// Linux) may take a connection before the server listens and then reset or close it.
fn client(at: std::net::SocketAddr) -> String {
    use std::io::Read;
    let started = Instant::now();
    loop {
        let answered = std::net::TcpStream::connect(at).and_then(|mut stream| {
            stream.write_all(b"ping\n")?;
            stream.shutdown(std::net::Shutdown::Write)?;
            let mut answer = String::new();
            stream.read_to_string(&mut answer)?;
            Ok(answer)
        });
        match answered {
            Ok(answer) if !answer.is_empty() => return answer,
            outcome => {
                assert!(
                    started.elapsed().as_secs() < 120,
                    "never answered: {outcome:?}"
                );
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}

/// A free port of the host's loopback.
fn free_port() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap()
}

/// A program in the container serves a TCP connection from the host on a published port, as with
/// `docker run --publish`.
#[test]
fn a_tcp_connection_from_the_host_reaches_a_published_port() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let spec = with_args(image.spec().unwrap(), &["sh", "-c", &serve_command()]);
    let at = free_port();
    let (stdout, stderr) = (Captured::default(), Captured::default());
    let mut system = LocalSystem::new(Box::new(stdout.clone()), Box::new(stderr.clone()));
    system.publish(SERVED_PORT, at);
    let client_thread = std::thread::spawn(move || client(at));
    let mut container = container_on(&spec, &image, None, system);
    let Ok(Ending::Finished { status }) = container.run();
    let ours = (
        outcome(status, &stdout, &stderr),
        client_thread.join().unwrap(),
    );
    let reference_at = free_port();
    let reference_client = std::thread::spawn(move || client(reference_at));
    let output = Command::new(wcontainers_docker::CLIENT)
        .args([
            "run",
            wcontainers_docker::REMOVE_ON_EXIT,
            wcontainers_docker::PLATFORM_OPTION,
            "linux/arm64",
            "--publish",
        ])
        .arg(format!(
            "{}:{}:{SERVED_PORT}",
            reference_at.ip(),
            reference_at.port()
        ))
        .args([IMAGE, "sh", "-c", &serve_command()])
        .output()
        .unwrap();
    let reference = (
        Outcome {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap(),
        },
        reference_client.join().unwrap(),
    );
    assert_eq!(ours, reference);
}

/// Docker Desktop's name for the machine it runs on, as the reference's container reaches it.
const DOCKER_HOST_NAME: &str = "host.docker.internal";

/// This machine's address on its outward network: the source address of a route out (a datagram
/// socket connected to a documentation address sends nothing).
fn outward_address() -> std::net::IpAddr {
    let socket = std::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    socket
        .connect((std::net::Ipv4Addr::new(192, 0, 2, 1), SERVED_PORT))
        .unwrap();
    socket.local_addr().unwrap().ip()
}

/// A server on this machine answering one connection: what it read, after "echo: ".
fn echo_server() -> (u16, std::thread::JoinHandle<()>) {
    use std::io::{BufRead, BufReader};
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let thread = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        (&stream)
            .write_all(format!("echo: {line}").as_bytes())
            .unwrap();
    });
    (port, thread)
}

/// A program in the container connects to a server on this machine through the local system's
/// network, as the reference's reaches one through Docker Desktop.
#[test]
fn a_connection_from_the_container_reaches_a_host_server() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let command = |host: &str, port: u16| format!("echo hello | nc {host} {port}; echo status $?");
    let (port, server) = echo_server();
    let ours_command = command(&outward_address().to_string(), port);
    let spec = with_args(image.spec().unwrap(), &["sh", "-c", &ours_command]);
    let (ours, _) = ours(&spec, &image, None);
    server.join().unwrap();
    let (port, server) = echo_server();
    let reference = reference(&command(DOCKER_HOST_NAME, port));
    server.join().unwrap();
    assert_eq!(ours, reference);
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
/// calls until another sets a flag (only the system's timer ends its turn), a timer signal sent to
/// one chosen thread, and the process ended by one thread while another waits in `read` and the first
/// in `pthread_join`.
#[test]
fn a_threaded_program_matches_the_reference() {
    let (_, files) = images();
    let program = threaded_program();
    let directory = program.parent().unwrap().to_str().unwrap();
    let output = Command::new(wcontainers_docker::CLIENT)
        .args([
            "run",
            wcontainers_docker::REMOVE_ON_EXIT,
            wcontainers_docker::PLATFORM_OPTION,
            "linux/arm64",
            "-v",
        ])
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
    let runtime = Runtime::<CpuSide<Arm64>>::new(&RuntimeConfig { cache: None });
    let system = LocalSystem::new(Box::new(std::io::sink()), Box::new(std::io::sink()));
    let translator_config = TranslatorConfig {
        look_ahead: look_ahead(),
        optimise: Optimise::Speed,
    };
    match runtime.container(
        &spec,
        &image,
        &translator_config,
        &GuestMachine { offer: () },
        system,
    ) {
        Err(wcontainers_runtime_single_kernel::Error::NotFound(name)) => assert_eq!(name, "ls"),
        Err(other) => panic!("{other}"),
        Ok(_) => panic!("ls was run from the working directory"),
    }
    let output = Command::new(wcontainers_docker::CLIENT)
        .args([
            "run",
            wcontainers_docker::REMOVE_ON_EXIT,
            wcontainers_docker::PLATFORM_OPTION,
            "linux/arm64",
            "-e",
            "PATH=/nonexistent",
        ])
        .args(["-w", "/bin", IMAGE, "ls", "/etc/alpine-release"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(127));
}

/// What a program that reads its standard input is given: more than a pipe holds, so it comes in
/// as the kernel makes room.
fn input_lines() -> Vec<u8> {
    const LINES: u32 = 100_000;
    (1..=LINES)
        .map(|n| format!("{n}\n"))
        .collect::<String>()
        .into_bytes()
}

/// Reads its input with the shell's `read`, which shows nothing of how the input was cut, and
/// with programs that read all of it.
const INPUT_COMMAND: &str = "read first; echo first $first; stat -L -c %F /proc/self/fd/0; \
     sort -n -r | head -2; read after || echo end of input";

/// Standard input from the system reaches the first process as `docker run --interactive` gives
/// it: a pipe, read to its end.
#[test]
fn standard_input_matches_docker_interactive() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let spec = with_args(image.spec().unwrap(), &["sh", "-c", INPUT_COMMAND]);
    let input = input_lines();
    let (stdout, stderr) = (Captured::default(), Captured::default());
    let mut system = LocalSystem::new(Box::new(stdout.clone()), Box::new(stderr.clone()));
    system.input(Box::new(std::io::Cursor::new(input.clone())));
    let mut container = container_on(&spec, &image, None, system);
    let Ok(Ending::Finished { status }) = container.run();
    assert_eq!(
        outcome(status, &stdout, &stderr),
        reference_with(&["--interactive"], Some(&input), INPUT_COMMAND)
    );
    // Without input, descriptor 0 is the null device, as without `--interactive`.
    let (outcome_without, _) = ours(&spec, &image, None);
    assert_eq!(outcome_without, reference(INPUT_COMMAND));
}

/// What a program on a terminal sees of it.
const TERMINAL_COMMAND: &str = "tty; stty size; [ -t 0 ] && echo on a terminal; \
     stat -L -c %F /proc/self/fd/1; echo to error >&2; exit 3";

/// `process.terminal` gives the first process a terminal as `docker run --tty` does (its size 0 by
/// 0, as a client with no terminal of its own has), with all it shows on the one stream.
#[test]
fn a_terminal_matches_docker_tty() {
    let (layout, _) = images();
    let image = Image::oci(layout, "arm64").unwrap();
    let mut spec = with_args(image.spec().unwrap(), &["sh", "-c", TERMINAL_COMMAND]);
    let mut process = spec.process().clone().unwrap();
    process.set_terminal(Some(true));
    spec.set_process(Some(process));
    let (outcome, _) = ours(&spec, &image, None);
    assert_eq!(outcome, reference_with(&["--tty"], None, TERMINAL_COMMAND));
}

/// The command line program's `--interactive` gives the container its own standard input.
#[test]
fn the_command_line_program_passes_its_input() {
    let (_, files) = images();
    let input = input_lines();
    let mut child = Command::new(env!("CARGO_BIN_EXE_wcon"))
        .args([cli::TAR, files.to_str().unwrap(), cli::INTERACTIVE])
        .args([cli::LOOK_AHEAD, &look_ahead_argument()])
        .args([cli::END_OF_OPTIONS, "sh", "-c", INPUT_COMMAND])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let given = input.clone();
    let writer = std::thread::spawn(move || stdin.write_all(&given));
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap().unwrap();
    let expected = reference_with(&["--interactive"], Some(&input), INPUT_COMMAND);
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected.stdout);
    assert_eq!(output.status.code(), Some(expected.exit_code));
}
