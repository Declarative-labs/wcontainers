//! The native reference: real ARM64 Linux in Docker, running `reference/` built for aarch64 Linux.

use crate::layout::*;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use wcontainers_abi::RegisterBlock;
use zerocopy::FromBytes;

/// The Docker image the reference runs in.
pub const IMAGE: &str = "alpine:latest";
/// Docker's command line program, and `docker run`'s arguments for every container here: removed at its end,
/// with no network, and named by the argument that follows.
pub const DOCKER: &str = "docker";
pub const REFERENCE_CONTAINER: [&str; 5] = ["run", "--rm", "--network", "none", "--name"];
/// `docker run`'s flag that keeps the container's stdin open, for a runner that takes requests.
const DOCKER_INPUT: &str = "-i";
/// `docker run`'s flag that mounts a host folder, as `host:inside:ro`, and the parts of its value.
pub const DOCKER_MOUNT: &str = "-v";
const MOUNT_SEPARATOR: &str = ":";
const READ_ONLY: &str = "ro";
/// Where a container sees the reference runner's folder and the programs' folder.
pub const RUNNER_FOLDER: &str = "/r";
pub const PROGRAMS_FOLDER: &str = "/p";
/// The reference runner's program, its package's name.
pub const RUNNER_NAME: &str = "wcontainers-reference";
/// The reference runner's package folder beside this crate's manifest, also its folder under `target`.
const RUNNER_PACKAGE: &str = "reference";
/// Where each process keeps its own copy of the reference runner, under the runner's target folder.
const OWN_COPIES: &str = "run";
/// Rust's name for the guest's target, the one the reference runner and Rust test programs are built for.
pub const GUEST_TRIPLE: &str = "aarch64-unknown-linux-musl";
/// The workspace's folder of build output, from this crate's manifest folder.
const WORKSPACE_TARGET: &str = "../../target";
/// The environment variable in which Cargo gives a test the path of the Cargo that runs it, and the program
/// to run when a test binary is run without Cargo.
const CARGO_VARIABLE: &str = "CARGO";
const CARGO_PROGRAM: &str = "cargo";
/// Cargo's command for a quiet release build, its flags naming the target and the folder for its output, and
/// the folder of a target's output that a release build writes.
const QUIET_RELEASE_COMMAND: [&str; 3] = ["build", "--release", "--quiet"];
pub const TARGET_FLAG: &str = "--target";
pub const TARGET_FOLDER_FLAG: &str = "--target-dir";
const RELEASE: &str = "release";

/// Mount the host folder `host` read only at `inside` in a container.
pub fn mount(command: &mut Command, host: &Path, inside: &str) {
    mount_as(command, host, &[inside, READ_ONLY]);
}

/// Mount the host folder `host` at `inside` in a container, writable.
pub fn mount_writable(command: &mut Command, host: &Path, inside: &str) {
    mount_as(command, host, &[inside]);
}

fn mount_as(command: &mut Command, host: &Path, inside: &[&str]) {
    let host = host.to_str().expect("a folder named in UTF-8");
    command
        .arg(DOCKER_MOUNT)
        .arg([&[host], inside].concat().join(MOUNT_SEPARATOR));
}

/// A container of the reference runner in `mode`, for `purpose`, taking requests on stdin and answering on
/// stdout, with the further folders `add` mounts.
pub fn serving(purpose: &str, mode: &str, add: impl FnOnce(&mut Command)) -> Child {
    let mut command = Command::new(DOCKER);
    command
        .args(REFERENCE_CONTAINER)
        .arg(container_name(purpose))
        .arg(DOCKER_INPUT);
    mount(&mut command, &build(), RUNNER_FOLDER);
    add(&mut command);
    command
        .arg(IMAGE)
        .arg(Path::new(RUNNER_FOLDER).join(RUNNER_NAME))
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run docker")
}

/// The folder `name` of the workspace's build output.
pub fn target_folder(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(WORKSPACE_TARGET)
        .join(name)
}

/// The Cargo that runs this test, or the one on the path.
pub fn cargo() -> Command {
    Command::new(std::env::var_os(CARGO_VARIABLE).unwrap_or_else(|| CARGO_PROGRAM.into()))
}

/// Build the package in the folder `package` quietly for release, for the target `triple`, with its output
/// in `target`, and return the folder that holds what it built.
pub fn release_build(package: &Path, triple: &str, target: &Path) -> PathBuf {
    let status = cargo()
        .args(QUIET_RELEASE_COMMAND)
        .args([TARGET_FLAG, triple, TARGET_FOLDER_FLAG])
        .arg(target)
        .current_dir(package)
        .status()
        .expect("run cargo");
    assert!(status.success(), "{} did not build", package.display());
    target.join(triple).join(RELEASE)
}

/// Build the reference runner for aarch64 Linux, once per process, and return a directory holding this
/// process's own copy of it. Cargo replaces the built file on every build, even with nothing to do, so a
/// container started from it during another build could find it missing.
pub fn build() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let target = target_folder(RUNNER_PACKAGE);
            let built = release_build(
                &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(RUNNER_PACKAGE),
                GUEST_TRIPLE,
                &target,
            );
            let own = target.join(OWN_COPIES).join(std::process::id().to_string());
            std::fs::create_dir_all(&own).unwrap();
            std::fs::copy(built.join(RUNNER_NAME), own.join(RUNNER_NAME)).unwrap();
            own
        })
        .clone()
}

/// A container name no other run uses.
pub fn container_name(purpose: &str) -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("wc-translator-{purpose}-{}-{n}", std::process::id())
}

/// What happened on the real CPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
    pub signal: u32,
    pub code: u32,
    pub address: u64,
    /// x0 to x30, sp, pc, flags, thread pointer.
    pub registers: [u64; LANDED_REGISTERS],
    pub vectors: [u128; VECTOR_REGISTERS],
    /// FPSR and FPCR.
    pub float: [u32; FLOAT_REGISTERS],
    /// Changed data bytes: offset from `DATA`, new value.
    pub changed: Vec<(u32, u8)>,
}

const _: () = assert!(
    GENERAL_REGISTERS == RegisterBlock::ZERO.x.len()
        && VECTOR_REGISTERS == RegisterBlock::ZERO.v.len()
);

/// The value at byte `at` of `bytes`, little endian as on every host here.
fn value_at<T: FromBytes>(bytes: &[u8], at: usize) -> T {
    T::read_from_prefix(&bytes[at..]).expect("a whole value").0
}

/// A running reference that takes layer 1 cases one at a time.
pub struct Cases {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

impl Cases {
    pub fn start() -> Self {
        let mut child = serving("layer1", "cases", |_| {});
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        Cases {
            child,
            input,
            output,
        }
    }

    /// Run one instruction from these registers (x0 to x30, sp, flags, thread pointer), vector registers,
    /// FPSR and FPCR, and data seed.
    pub fn run(
        &mut self,
        word: u32,
        seed: u64,
        registers: &[u64; REQUEST_REGISTERS],
        vectors: &[u128; VECTOR_REGISTERS],
        float: [u32; FLOAT_REGISTERS],
    ) -> Landed {
        let mut request = Vec::with_capacity(REQUEST_BYTES);
        request.extend_from_slice(&word.to_le_bytes());
        request.extend_from_slice(&seed.to_le_bytes());
        for value in registers {
            request.extend_from_slice(&value.to_le_bytes());
        }
        for value in vectors {
            request.extend_from_slice(&value.to_le_bytes());
        }
        for value in float {
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.input.write_all(&request).expect("send a case");
        self.input.flush().unwrap();
        let mut header = [0u8; RESULT_HEADER_BYTES];
        self.output
            .read_exact(&mut header)
            .expect("the reference answers");
        let landed = Landed {
            signal: value_at(&header, 0),
            code: value_at(&header, RESULT_CODE_AT),
            address: value_at(&header, RESULT_ADDRESS_AT),
            registers: value_at(&header, RESULT_REGISTERS_AT),
            vectors: value_at(&header, RESULT_VECTORS_AT),
            float: value_at(&header, RESULT_FLOAT_AT),
            changed: Vec::new(),
        };
        let count: u32 = value_at(&header, RESULT_CHANGES_AT);
        let mut changes = vec![0u8; CHANGE_BYTES * count as usize];
        self.output.read_exact(&mut changes).unwrap();
        let changed = changes
            .chunks(CHANGE_BYTES)
            .map(|c| (value_at(c, 0), c[size_of::<u32>()]))
            .collect();
        Landed { changed, ..landed }
    }
}

impl Drop for Cases {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
