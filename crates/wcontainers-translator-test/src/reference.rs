//! The native reference: `reference/` built for the guest's Linux and run on a real CPU of the guest's kind,
//! in Docker when this host's CPU is the guest's and over SSH to the machine named by the guest's variable
//! otherwise. Both carry the same records over the runner's stdin and stdout (the wire of `layout.rs`).

use crate::layout::*;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

/// What the harness needs to reach a guest's reference.
pub struct Target {
    /// Rust's target for the runner, such as `aarch64-unknown-linux-musl`.
    pub rust_target: &'static str,
    /// Docker's platform for the runner's container when this host's CPU is the guest's.
    pub docker_platform: &'static str,
    /// The variable naming the reference's SSH host when it is not, such as `WC_REFERENCE_X86_64`.
    pub reference_host: &'static str,
    /// The state's fields, in wire order; the runner names its own first and they must be these.
    pub fields: &'static [&'static str],
}

pub const AARCH64: Target = Target {
    rust_target: "aarch64-unknown-linux-musl",
    docker_platform: "linux/arm64",
    reference_host: "WC_REFERENCE_AARCH64",
    fields: &[
        "x0",
        "x1",
        "x2",
        "x3",
        "x4",
        "x5",
        "x6",
        "x7",
        "x8",
        "x9",
        "x10",
        "x11",
        "x12",
        "x13",
        "x14",
        "x15",
        "x16",
        "x17",
        "x18",
        "x19",
        "x20",
        "x21",
        "x22",
        "x23",
        "x24",
        "x25",
        "x26",
        "x27",
        "x28",
        "x29",
        "x30",
        "sp",
        "pc",
        "nzcv",
        "tpidr_el0",
        "v0",
        "v1",
        "v2",
        "v3",
        "v4",
        "v5",
        "v6",
        "v7",
        "v8",
        "v9",
        "v10",
        "v11",
        "v12",
        "v13",
        "v14",
        "v15",
        "v16",
        "v17",
        "v18",
        "v19",
        "v20",
        "v21",
        "v22",
        "v23",
        "v24",
        "v25",
        "v26",
        "v27",
        "v28",
        "v29",
        "v30",
        "v31",
        "fpsr",
        "fpcr",
    ],
};

pub const X86_64: Target = Target {
    rust_target: "x86_64-unknown-linux-musl",
    docker_platform: "linux/amd64",
    reference_host: "WC_REFERENCE_X86_64",
    fields: &[
        "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12",
        "r13", "r14", "r15", "rip", "rflags", "fs_base", "gs_base", "mxcsr", "fcw", "fsw", "ftw",
        "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "xmm0", "xmm1", "xmm2", "xmm3", "xmm4",
        "xmm5", "xmm6", "xmm7", "xmm8", "xmm9", "xmm10", "xmm11", "xmm12", "xmm13", "xmm14",
        "xmm15", "ymmh0", "ymmh1", "ymmh2", "ymmh3", "ymmh4", "ymmh5", "ymmh6", "ymmh7", "ymmh8",
        "ymmh9", "ymmh10", "ymmh11", "ymmh12", "ymmh13", "ymmh14", "ymmh15",
    ],
};

/// The Docker image a local reference runs in.
const IMAGE: &str = "alpine:latest";
/// Where the runner is kept on an SSH reference, under the login's home.
const REMOTE_DIRECTORY: &str = "wcontainers-x86/reference";
/// How many times an SSH reference is tried before the run fails: the machine drops off its network at times.
const SSH_TRIES: u32 = 5;

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn target_directory() -> PathBuf {
    workspace().join("target/reference")
}

/// A local copy of the runner built for `target` (once per process and target; cargo replaces the built
/// file on every build, so a container started from it during another build could find it missing), and
/// its content hash.
pub fn build(target: &Target) -> (PathBuf, String) {
    static BUILT: Mutex<Vec<(&str, PathBuf, String)>> = Mutex::new(Vec::new());
    let mut built = BUILT.lock().unwrap();
    if let Some((_, path, hash)) = built.iter().find(|(t, ..)| *t == target.rust_target) {
        return (path.clone(), hash.clone());
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reference/Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .args([
            "build",
            "--release",
            "--quiet",
            "--target",
            target.rust_target,
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
        .arg(target_directory())
        .current_dir(manifest.parent().unwrap())
        .status()
        .expect("run cargo for the reference");
    assert!(status.success(), "the reference runner did not build");
    let own = target_directory()
        .join("run")
        .join(std::process::id().to_string())
        .join(target.rust_target);
    std::fs::create_dir_all(&own).unwrap();
    let copy = own.join("wcontainers-reference");
    std::fs::copy(
        target_directory()
            .join(target.rust_target)
            .join("release/wcontainers-reference"),
        &copy,
    )
    .unwrap();
    let bytes = std::fs::read(&copy).unwrap();
    let hash = blake3::hash(&bytes).to_hex()[..32].to_string();
    built.push((target.rust_target, own.clone(), hash.clone()));
    (own, hash)
}

/// A container name no other run uses.
pub fn container_name(purpose: &str) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("wc-x86-{purpose}-{}-{n}", std::process::id())
}

/// Whether this host's CPU is the target's.
fn local(target: &Target) -> bool {
    target.rust_target.starts_with(std::env::consts::ARCH)
}

/// ssh with one connection kept open and shared by every reference of the run. The control socket's
/// path is relative to the reference's target directory, so it stays within the length a socket's path
/// may have.
fn ssh(host: &str) -> Command {
    let mut command = Command::new("ssh");
    command.current_dir(target_directory()).args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=20",
        "-o",
        "ServerAliveInterval=15",
        "-o",
        "ControlMaster=auto",
        "-o",
        "ControlPath=ssh-%C",
        "-o",
        "ControlPersist=120",
        host,
    ]);
    command
}

/// Put the runner on the SSH host under its hash unless it is there already; returns its path there.
fn upload(host: &str, local: &std::path::Path, hash: &str) -> Result<String, String> {
    let remote = format!("{REMOTE_DIRECTORY}/{hash}");
    let script = format!(
        "mkdir -p {REMOTE_DIRECTORY} && {{ test -x {remote} || {{ cat > {remote}.part && chmod +x {remote}.part && mv {remote}.part {remote}; }}; }}"
    );
    let file = std::fs::File::open(local.join("wcontainers-reference")).unwrap();
    let output = ssh(host)
        .arg(script)
        .stdin(file)
        .output()
        .map_err(|e| format!("run ssh: {e}"))?;
    if output.status.success() {
        Ok(remote)
    } else {
        Err(format!(
            "uploading the reference runner to {host}: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// What a case did on the real CPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
    pub signal: u32,
    pub code: u32,
    pub address: u64,
    /// The state where control stopped, one value a field.
    pub state: Vec<u128>,
    /// Changed data bytes: offset from `DATA`, new value.
    pub changed: Vec<(u32, u8)>,
}

/// A blob to run on the reference.
pub struct BlobRequest<'a> {
    pub bytes: &'a [u8],
    pub load: u64,
    pub stack_top: u64,
    pub stack_bytes: u64,
    /// Where its program headers are in memory and how many.
    pub headers: Option<(u64, u64)>,
    pub stdin: &'a [u8],
    /// How many timed runs; 0 runs it once untimed.
    pub runs: u32,
}

/// What a blob did on the reference.
#[derive(Clone, Debug)]
pub struct BlobRan {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The exit status, or 128 and the signal that ended it.
    pub status: i32,
    /// The best time of the timed runs, in seconds.
    pub seconds: f64,
    /// When a signal ended it: the signal, its `si_code`, then pc and the general registers as the runner's
    /// `Machine::blob_stop` gives them.
    pub stop: Option<(u32, u32, Vec<u64>)>,
}

/// A running reference runner for one guest.
pub struct Reference {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    /// The reference CPU's model, as its `/proc/cpuinfo` names it.
    pub model: String,
    fields: usize,
}

impl Reference {
    /// Start a runner for `target`: in Docker here, or over SSH, retried while the machine cannot be
    /// reached. Fails with a message naming the variable when the host is needed and not set.
    pub fn start(target: &Target) -> Self {
        let (directory, hash) = build(target);
        if local(target) {
            let mut command = Command::new("docker");
            command
                .args(["run", "--rm", "-i", "--network", "none", "--platform"])
                .arg(target.docker_platform)
                .arg("--name")
                .arg(container_name("reference"))
                .arg("-v")
                .arg(format!("{}:/r:ro", directory.display()))
                .args([IMAGE, "/r/wcontainers-reference", "serve"]);
            return Self::connect(&mut command, target).expect("the reference in Docker");
        }
        let host = std::env::var(target.reference_host).unwrap_or_else(|_| {
            panic!(
                "this host is not {}: set {} to the SSH host of a reference machine",
                target.rust_target, target.reference_host
            )
        });
        let mut error = String::new();
        for attempt in 0..SSH_TRIES {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
            let started = upload(&host, &directory, &hash)
                .and_then(|remote| Self::connect(ssh(&host).arg(remote).arg("serve"), target));
            match started {
                Ok(reference) => return reference,
                Err(e) => error = e,
            }
        }
        panic!("the reference on {host} could not be reached after {SSH_TRIES} tries: {error}");
    }

    fn connect(command: &mut Command, target: &Target) -> Result<Self, String> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("start the reference: {e}"))?;
        let input = child.stdin.take().unwrap();
        let mut output = child.stdout.take().unwrap();
        let hello = read_record(&mut output).ok_or("the reference did not answer")?;
        let mut hello = Reader(&hello);
        let count = hello.u32() as usize;
        let mut name = || String::from_utf8_lossy(hello.bytes_u8()).into_owned();
        let fields: Vec<String> = (0..count).map(|_| name()).collect();
        let model = name();
        assert_eq!(
            fields, target.fields,
            "the reference runner's fields are the guest's"
        );
        Ok(Reference {
            child,
            input,
            output,
            model,
            fields: count,
        })
    }

    fn request(&mut self, record: &[u8]) -> Result<Vec<u8>, String> {
        write_record(&mut self.input, record);
        let result = read_record(&mut self.output).expect("the reference answers");
        match result[0] {
            RESULT_RAN => Ok(result[1..].to_vec()),
            _ => Err(String::from_utf8_lossy(&result[1..]).into_owned()),
        }
    }

    /// Run one instruction from `state` (its pc field is ignored: the instruction is at CODE) and the data
    /// area of `seed`. `Err` is the runner's own failure, never the guest's.
    pub fn case(&mut self, code: &[u8], seed: u64, state: &[u128]) -> Result<Landed, String> {
        assert_eq!(state.len(), self.fields, "a value for every field");
        let mut record = vec![RECORD_CASE, code.len() as u8];
        record.extend_from_slice(code);
        record.extend_from_slice(&seed.to_le_bytes());
        for value in state {
            record.extend_from_slice(&value.to_le_bytes());
        }
        let result = self.request(&record)?;
        let mut result = Reader(&result);
        let (signal, code, address) = (result.u32(), result.u32(), result.u64());
        let state = (0..self.fields).map(|_| result.u128()).collect();
        let changed = (0..result.u32())
            .map(|_| (result.u32(), result.take(1)[0]))
            .collect();
        Ok(Landed {
            signal,
            code,
            address,
            state,
            changed,
        })
    }

    /// Run a blob. `Err` is the runner's own failure.
    pub fn blob(&mut self, blob: &BlobRequest) -> Result<BlobRan, String> {
        let mut record = vec![RECORD_BLOB];
        let (headers_at, headers_count) = blob.headers.unwrap_or((0, 0));
        for word in [
            blob.load,
            blob.stack_top,
            blob.stack_bytes,
            headers_at,
            headers_count,
        ] {
            record.extend_from_slice(&word.to_le_bytes());
        }
        record.extend_from_slice(&blob.runs.to_le_bytes());
        record.extend_from_slice(&(blob.stdin.len() as u32).to_le_bytes());
        record.extend_from_slice(blob.stdin);
        record.extend_from_slice(blob.bytes);
        let result = self.request(&record)?;
        let mut result = Reader(&result);
        let status = result.u32() as i32;
        let seconds = f64::from_le_bytes(result.take(8).try_into().unwrap());
        let stdout = result.bytes_u32().to_vec();
        let stderr = result.bytes_u32().to_vec();
        let mut stop = Reader(result.bytes_u32());
        let stop = (!stop.0.is_empty()).then(|| {
            let word = stop.u64();
            let count = stop.u64() as usize;
            let registers = (0..count).map(|_| stop.u64()).collect();
            (word as u32, (word >> 32) as u32, registers)
        });
        Ok(BlobRan {
            stdout,
            stderr,
            status,
            seconds,
            stop,
        })
    }
}

impl Drop for Reference {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runners not in use, per target, so that many blobs share a few runners.
pub fn with_reference<R>(target: &'static Target, work: impl FnOnce(&mut Reference) -> R) -> R {
    static FREE: OnceLock<Mutex<Vec<(&'static str, Reference)>>> = OnceLock::new();
    let free = FREE.get_or_init(|| Mutex::new(Vec::new()));
    let taken = {
        let mut free = free.lock().unwrap();
        free.iter()
            .position(|(t, _)| *t == target.rust_target)
            .map(|at| free.swap_remove(at).1)
    };
    let mut reference = taken.unwrap_or_else(|| Reference::start(target));
    let result = work(&mut reference);
    free.lock().unwrap().push((target.rust_target, reference));
    result
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        head
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn u128(&mut self) -> u128 {
        u128::from_le_bytes(self.take(16).try_into().unwrap())
    }
    fn bytes_u8(&mut self) -> &'a [u8] {
        let n = self.take(1)[0] as usize;
        self.take(n)
    }
    fn bytes_u32(&mut self) -> &'a [u8] {
        let n = self.u32() as usize;
        self.take(n)
    }
}

fn read_record(from: &mut impl Read) -> Option<Vec<u8>> {
    let mut length = [0u8; 4];
    from.read_exact(&mut length).ok()?;
    let mut record = vec![0u8; u32::from_le_bytes(length) as usize];
    from.read_exact(&mut record).ok()?;
    Some(record)
}

fn write_record(to: &mut impl Write, record: &[u8]) {
    to.write_all(&(record.len() as u32).to_le_bytes())
        .expect("send to the reference");
    to.write_all(record).expect("send to the reference");
    to.flush().unwrap();
}

/// ARM64's layer 1 cases as they are drawn today, on the generic reference.
pub struct Cases(Reference);

/// What happened on the real CPU, in ARM64's shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmLanded {
    pub signal: u32,
    pub code: u32,
    pub address: u64,
    /// x0 to x30, sp, pc, flags, thread pointer.
    pub registers: [u64; 35],
    pub vectors: [u128; 32],
    /// FPSR and FPCR.
    pub float: [u32; 2],
    pub changed: Vec<(u32, u8)>,
}

impl Cases {
    pub fn start() -> Self {
        Cases(Reference::start(&AARCH64))
    }

    /// Run one instruction from these registers (x0 to x30, sp, flags, thread pointer), vector registers,
    /// FPSR and FPCR, and data seed.
    pub fn run(
        &mut self,
        word: u32,
        seed: u64,
        registers: &[u64; 34],
        vectors: &[u128; 32],
        float: [u32; 2],
    ) -> ArmLanded {
        let mut state: Vec<u128> = registers[..32].iter().map(|&r| r.into()).collect();
        state.push(CODE.into());
        state.extend(registers[32..].iter().map(|&r| u128::from(r)));
        state.extend(vectors);
        state.extend(float.map(u128::from));
        let landed = self
            .0
            .case(&word.to_le_bytes(), seed, &state)
            .unwrap_or_else(|e| panic!("the reference runner failed: {e}"));
        let s = &landed.state;
        ArmLanded {
            signal: landed.signal,
            code: landed.code,
            address: landed.address,
            registers: std::array::from_fn(|i| s[i] as u64),
            vectors: std::array::from_fn(|i| s[35 + i]),
            float: [s[67] as u32, s[68] as u32],
            changed: landed.changed,
        }
    }
}
