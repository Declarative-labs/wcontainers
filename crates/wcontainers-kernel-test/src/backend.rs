//! The first file backend in tests: fuse-backend-rs's passthrough server (`backend/`), built for
//! ARM64 Linux in a container and run in one, serving a directory in memory there. Our kernel
//! reaches it over the container's standard streams; the reference mounts the same server through
//! Linux's own FUSE.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::reference::{HOSTNAME, IMAGE};

/// The image the server is built in.
const BUILD_IMAGE: &str = "rust:1.91-alpine3.22";

/// Where the backend's directory is in its container, and where the reference mounts it.
pub const SERVED: &str = "/srv";

/// The FUSE device for the reference's container, put outside `/dev` so that `/dev` holds what a
/// default container's does: the server opens it through a link at `/dev/fuse`
/// ([`FUSE_DEVICE_LINK`]), which it removes once mounted.
pub const FUSE_DEVICE: &str = "/dev/fuse:/wcdev/fuse";

/// The shell command that makes the link.
pub const FUSE_DEVICE_LINK: &str = "ln -s /wcdev/fuse /dev/fuse";

/// Where tests build the server: the workspace's target directory.
pub fn out() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wc-kernel-test")
}

/// Builds the server once into `out/wc-backend/` and returns the directory holding it.
pub fn server(out: &Path) -> PathBuf {
    // Tests of one binary ask at once: one builds while the others wait for it.
    static BUILDING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _building = BUILDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let directory = out.join("wc-backend");
    let program = directory.join("wc-backend");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("backend");
    let newest_source = ["Cargo.toml", "Cargo.lock", "src/main.rs"]
        .iter()
        .filter_map(|file| std::fs::metadata(source.join(file)).ok()?.modified().ok())
        .max();
    let built = std::fs::metadata(&program)
        .ok()
        .and_then(|meta| meta.modified().ok());
    if built.is_some() && built >= newest_source {
        return directory;
    }
    std::fs::create_dir_all(&directory).expect("the server's directory");
    let cache = out.join("wc-backend-build");
    std::fs::create_dir_all(&cache).expect("the server's build directory");
    let name = format!("wc-kernel-backend-build-{}", std::process::id());
    let output = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "--name", &name])
        .arg("--volume")
        .arg(format!("{}:/src:ro", source.display()))
        .arg("--volume")
        .arg(format!(
            "{}:/out",
            cache.canonicalize().expect("cache").display()
        ))
        .args([
            "--env",
            "CARGO_HOME=/out/cargo",
            "--env",
            "CARGO_TARGET_DIR=/out/target",
        ])
        .args(["--workdir", "/src", BUILD_IMAGE])
        .args(["cargo", "build", "--release", "--locked"])
        .output()
        .expect("running docker");
    assert!(
        output.status.success(),
        "building the backend server: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Test binaries build side by side: each copies in under its own name and renames.
    let unique = directory.join(format!("wc-backend.{}", std::process::id()));
    std::fs::copy(cache.join("target/release/wc-backend"), &unique).expect("copying the server");
    std::fs::rename(&unique, &program).expect("moving the server into place");
    directory
}

/// The server as its container runs it: opening files for the page cache if `cached`.
pub fn serve_command(cached: bool) -> &'static str {
    if cached {
        "/wcb/wc-backend --cached"
    } else {
        "/wcb/wc-backend"
    }
}

/// FUSE requests that have no answer: FORGET, NOTIFY_REPLY and BATCH_FORGET.
fn answered(opcode: u32) -> bool {
    !matches!(opcode, 2 | 41 | 42)
}

/// One running server, reached over its container's standard streams.
pub struct Backend {
    child: Child,
    /// Its container's name.
    name: String,
    input: ChildStdin,
    output: ChildStdout,
    /// Requests sent whose answers have not been read, in order: the server answers in order.
    outstanding: VecDeque<u64>,
    /// How many messages went each way.
    pub sent: u64,
}

/// What the server sent: an answer to a request, or a notice nobody asked for.
pub enum Message {
    Answer(u64, Vec<u8>),
    Notice(Vec<u8>),
}

impl Backend {
    /// Starts a server on a directory in memory, empty or with `image` unpacked into it first,
    /// opening files for the page cache if `cached`, else for direct reads and writes.
    pub fn start(out: &Path, image: Option<&[crate::image::File]>, cached: bool) -> Self {
        let serve = serve_command(cached);
        static RUNS: AtomicU32 = AtomicU32::new(0);
        let server = server(out);
        let name = format!(
            "wc-kernel-backend-{}-{}",
            std::process::id(),
            RUNS.fetch_add(1, Ordering::Relaxed)
        );
        let mut command = Command::new("docker");
        command
            .args([
                "run",
                "--rm",
                "-i",
                "--platform",
                "linux/arm64",
                "--name",
                &name,
            ])
            .args(["--tmpfs", SERVED])
            .arg("--volume")
            .arg(format!("{}:/wcb:ro", server.display()));
        let script = match image {
            Some(files) => {
                let images = out.join("wc-backend-images");
                std::fs::create_dir_all(&images).expect("a directory for images");
                let archive = images
                    .canonicalize()
                    .expect("the images' directory")
                    .join(format!("{name}.tar"));
                std::fs::write(&archive, crate::image::to_tar(files)).expect("writing the image");
                command
                    .arg("--volume")
                    .arg(format!("{}:/wci/image.tar:ro", archive.display()));
                format!("tar -xpf /wci/image.tar -C {SERVED} && exec {serve} pipe {SERVED}")
            }
            None => format!("exec {serve} pipe {SERVED}"),
        };
        let mut child = command
            .args([IMAGE, "sh", "-c", &script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("running docker");
        let input = child.stdin.take().expect("the server's input");
        let output = child.stdout.take().expect("the server's output");
        Self {
            child,
            name,
            input,
            output,
            outstanding: VecDeque::new(),
            sent: 0,
        }
    }

    /// Ends the connection as Linux does when it unmounts (DESTROY): the server lets go of every
    /// node and handle, and a client may connect again with INIT. Answers owed are read first.
    pub fn destroy(&mut self) {
        use zerocopy::IntoBytes;
        while self.waiting() {
            self.receive();
        }
        let header = fuse_abi::linux::fuse_in_header {
            len: size_of::<fuse_abi::linux::fuse_in_header>() as u32,
            opcode: fuse_abi::linux::fuse_opcode::FUSE_DESTROY as u32,
            unique: u64::MAX >> 1,
            ..Default::default()
        };
        self.send(header.unique, header.as_bytes());
        while self.waiting() {
            self.receive();
        }
    }

    /// Empties the served directory, owned by root with the mode Docker's tmpfs has, as the
    /// reference's layer 1 runner empties its own after each sequence.
    pub fn empty(&self) {
        let script =
            format!("find {SERVED} -mindepth 1 -delete; chown 0:0 {SERVED}; chmod 1777 {SERVED}");
        let output = Command::new("docker")
            .args(["exec", &self.name, "sh", "-c", &script])
            .output()
            .expect("running docker exec");
        assert!(
            output.status.success(),
            "emptying the backend: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The files of the served directory now, as a tar archive of it gives them.
    pub fn files(&self) -> Vec<crate::image::File> {
        let output = Command::new("docker")
            .args(["exec", &self.name, "tar", "-cf", "-", "-C", SERVED, "."])
            .output()
            .expect("running docker exec");
        assert!(
            output.status.success(),
            "reading the backend's files: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        crate::image::from_tar(&output.stdout)
    }

    /// Hands the server one request.
    pub fn send(&mut self, unique: u64, bytes: &[u8]) {
        self.input.write_all(bytes).expect("writing to the backend");
        self.input.flush().expect("writing to the backend");
        self.sent += 1;
        let opcode = u32::from_le_bytes(bytes[4..8].try_into().expect("a request header"));
        if answered(opcode) {
            self.outstanding.push_back(unique);
        }
    }

    /// Whether an answer is still to come.
    pub fn waiting(&self) -> bool {
        !self.outstanding.is_empty()
    }

    /// Reads the server's next message, waiting for it; none when no answer is to come.
    pub fn receive(&mut self) -> Option<Message> {
        if self.outstanding.is_empty() {
            return None;
        }
        let mut header = [0u8; 16];
        self.output
            .read_exact(&mut header)
            .expect("reading from the backend");
        let length = u32::from_le_bytes(header[0..4].try_into().expect("four")) as usize;
        let unique = u64::from_le_bytes(header[8..16].try_into().expect("eight"));
        let mut message = header.to_vec();
        message.resize(length.max(16), 0);
        self.output
            .read_exact(&mut message[16..])
            .expect("reading from the backend");
        if unique == 0 {
            return Some(Message::Notice(message));
        }
        let expected = self.outstanding.pop_front();
        assert_eq!(expected, Some(unique), "the backend answers in order");
        Some(Message::Answer(unique, message))
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs `program` in the reference with the same server mounted through Linux's FUSE at `at` and
/// the program started there, without the power to mount, as the reference for a backend.
pub fn reference_run(
    out: &Path,
    program: &Path,
    at: &str,
    arguments: &[&str],
    cached: bool,
) -> crate::driver::Outcome {
    let serve = serve_command(cached);
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let server = server(out);
    let name = format!(
        "wc-kernel-fuse-reference-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    );
    let directory = program
        .parent()
        .expect("program has a directory")
        .canonicalize()
        .expect("program directory exists");
    let file = Path::new("/wc").join(program.file_name().expect("program has a name"));
    let script =
        format!("{FUSE_DEVICE_LINK} && mkdir -p {at} && exec {serve} mount {SERVED} {at} \"$@\"");
    let output = Command::new("docker")
        .args(["run", "--rm", "--platform", "linux/arm64", "--name", &name])
        .args(["--hostname", HOSTNAME])
        .args(["--device", FUSE_DEVICE, "--cap-add", "SYS_ADMIN"])
        .args(["--tmpfs", SERVED])
        .arg("--volume")
        .arg(format!("{}:/wc:ro", directory.display()))
        .arg("--volume")
        .arg(format!("{}:/wcb:ro", server.display()))
        .args([IMAGE, "sh", "-c", &script, "sh"])
        .arg(&file)
        .args(arguments)
        .output()
        .expect("running docker");
    let exit_code = output.status.code().expect("docker exited normally");
    crate::driver::Outcome {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code,
    }
}
