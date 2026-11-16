//! The native reference: real ARM64 Linux in Docker, running `reference/` built for aarch64 Linux.

use crate::layout::*;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// The Docker image the reference runs in.
const IMAGE: &str = "alpine:latest";

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Build the reference runner for aarch64 Linux, once per process, and return a directory holding this
/// process's own copy of it. Cargo replaces the built file on every build, even with nothing to do, so a
/// container started from it during another build could find it missing.
pub fn build() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reference/Cargo.toml");
            let target = workspace().join("target/reference");
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let status = Command::new(cargo)
                .args(["build", "--release", "--quiet", "--manifest-path"])
                .arg(&manifest)
                .arg("--target-dir")
                .arg(&target)
                .current_dir(manifest.parent().unwrap())
                .status()
                .expect("run cargo for the reference");
            assert!(status.success(), "the reference runner did not build");
            let own = target.join("run").join(std::process::id().to_string());
            std::fs::create_dir_all(&own).unwrap();
            std::fs::copy(
                target.join("aarch64-unknown-linux-musl/release/wcontainers-reference"),
                own.join("wcontainers-reference"),
            )
            .unwrap();
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
    pub registers: [u64; 35],
    pub vectors: [u128; 32],
    /// FPSR and FPCR.
    pub float: [u32; 2],
    /// Changed data bytes: offset from `DATA`, new value.
    pub changed: Vec<(u32, u8)>,
}

/// A running reference that takes layer 1 cases one at a time.
pub struct Cases {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
}

impl Cases {
    pub fn start() -> Self {
        let directory = build();
        let mut child = Command::new("docker")
            .args(["run", "--rm", "-i", "--network", "none", "--name"])
            .arg(container_name("layer1"))
            .arg("-v")
            .arg(format!("{}:/r:ro", directory.display()))
            .args([IMAGE, "/r/wcontainers-reference", "cases"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start docker");
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
        registers: &[u64; 34],
        vectors: &[u128; 32],
        float: [u32; 2],
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
        let u32_at = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        let u64_at = |at: usize| u64::from_le_bytes(header[at..at + 8].try_into().unwrap());
        let mut landed = Landed {
            signal: u32_at(0),
            code: u32_at(4),
            address: u64_at(8),
            registers: [0; 35],
            vectors: [0; 32],
            float: [0; 2],
            changed: Vec::new(),
        };
        for (i, value) in landed.registers.iter_mut().enumerate() {
            *value = u64_at(16 + 8 * i);
        }
        for (i, value) in landed.vectors.iter_mut().enumerate() {
            let at = 16 + 35 * 8 + 16 * i;
            *value = u128::from_le_bytes(header[at..at + 16].try_into().unwrap());
        }
        let float_at = 16 + 35 * 8 + 32 * 16;
        landed.float = [u32_at(float_at), u32_at(float_at + 4)];
        let count = u32_at(float_at + 8) as usize;
        let mut changes = vec![0u8; 5 * count];
        self.output.read_exact(&mut changes).unwrap();
        landed.changed = changes
            .chunks(5)
            .map(|c| (u32::from_le_bytes(c[..4].try_into().unwrap()), c[4]))
            .collect();
        landed
    }
}

impl Drop for Cases {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
