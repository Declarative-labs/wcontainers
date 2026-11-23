//! The first file backend in tests: fuse-backend-rs's passthrough server (`backend/`), built for
//! ARM64 Linux in a container and run in one, serving a directory in memory there. Our kernel
//! reaches it over TCP through a port the container publishes (Docker's standard input took about
//! 14 ms for each message over 8 KiB on this Mac); the reference mounts the same server through
//! Linux's own FUSE.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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

/// Where tests build the server, relative to this crate: under the workspace's target directory.
const OUT_FROM_CRATE: &str = "../../../../target/wc-kernel-test";

/// Where tests build the server: the workspace's target directory.
pub fn out() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(OUT_FROM_CRATE)
}

/// When the newest file under `path` was last changed.
fn newest_modified(path: &Path) -> Option<std::time::SystemTime> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_dir() {
        return meta.modified().ok();
    }
    std::fs::read_dir(path)
        .ok()?
        .filter_map(|entry| newest_modified(&entry.ok()?.path()))
        .max()
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
    let newest_source = newest_modified(&source);
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

/// Busybox `dd`'s words for the file it writes and for keeping that file's length.
const DD_OUTPUT_FILE: &str = "of=";
const DD_IN_PLACE: &str = "conv=notrunc";

/// Writes `bytes` over the start of `path` in the served directory of container `name`, from
/// outside FUSE.
fn change_served(name: &str, path: &str, bytes: &[u8]) {
    use crate::reference::docker;
    let mut writer = Command::new(docker::CLIENT)
        .args([
            docker::EXEC,
            docker::INTERACTIVE,
            name,
            crate::busybox::DD_APPLET,
        ])
        .arg(format!(
            "{DD_OUTPUT_FILE}{}",
            Path::new(SERVED).join(path).display()
        ))
        .arg(DD_IN_PLACE)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("running docker exec");
    writer
        .stdin
        .take()
        .expect("the writer's input")
        .write_all(bytes)
        .expect("writing to docker exec");
    let output = writer.wait_with_output().expect("docker exec ran");
    assert!(
        output.status.success(),
        "changing a served file: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The port the server listens at in its container.
const PORT: u16 = 9000;

/// FUSE requests that have no answer: FORGET, NOTIFY_REPLY and BATCH_FORGET.
fn answered(opcode: u32) -> bool {
    !matches!(opcode, 2 | 41 | 42)
}

/// One running server, reached over one connection to its container.
pub struct Backend {
    child: Child,
    /// Its container's name.
    name: String,
    input: TcpStream,
    output: TcpStream,
    /// Requests sent whose answers have not been read, in order: the server answers in order.
    outstanding: VecDeque<u64>,
    /// The server's bytes read while a send waited for room, not yet taken as messages.
    unread: VecDeque<u8>,
    /// Messages read from the server and not yet handed to the kernel, each with its place in
    /// what the server sent: a host may hand them over in any order, as a backend that answers
    /// from many threads does, but no reply before a notice sent ahead of it.
    held: Vec<(u64, Message)>,
    /// How many messages were read from the server.
    read: u64,
    /// How many messages went each way.
    pub sent: u64,
    /// The image archive its container unpacked, removed when the backend ends.
    archive: Option<std::path::PathBuf>,
}

/// The directory a server serves.
#[derive(Clone, Copy)]
pub enum Served<'a> {
    /// An empty one in memory.
    Empty,
    /// One in memory with these files unpacked into it.
    Files(&'a [crate::image::File]),
    /// The one an image holds at [`SERVED`], which its container changes without changing the
    /// image: for an image too large to unpack for every run.
    Image(&'a str),
}

/// What the server sent: an answer to a request, or a notice nobody asked for.
pub enum Message {
    Answer(u64, Vec<u8>),
    Notice(Vec<u8>),
}

impl Backend {
    /// Starts a server on the directory `served` says, opening files for the page cache if
    /// `cached`, else for direct reads and writes.
    pub fn start(out: &Path, served: Served<'_>, cached: bool) -> Self {
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
                "--platform",
                "linux/arm64",
                "--name",
                &name,
                "--publish",
                &format!("127.0.0.1::{PORT}"),
            ])
            .arg("--volume")
            .arg(format!("{}:/wcb:ro", server.display()));
        let mut kept = None;
        let mut image = IMAGE;
        let script = match served {
            Served::Files(files) => {
                command.args(["--tmpfs", SERVED]);
                let images = out.join("wc-backend-images");
                std::fs::create_dir_all(&images).expect("a directory for images");
                let archive = images
                    .canonicalize()
                    .expect("the images' directory")
                    .join(format!("{name}.tar"));
                std::fs::write(&archive, crate::image::to_tar(files)).expect("writing the image");
                kept = Some(archive.clone());
                command
                    .arg("--volume")
                    .arg(format!("{}:/wci/image.tar:ro", archive.display()));
                format!("tar -xpf /wci/image.tar -C {SERVED} && exec {serve} listen {SERVED}")
            }
            Served::Empty => {
                command.args(["--tmpfs", SERVED]);
                format!("exec {serve} listen {SERVED}")
            }
            // The files Docker writes into every container, written over the image's as
            // `as_started` writes them for a root in memory.
            Served::Image(name) => {
                image = name;
                command
                    .arg("--env")
                    .arg(format!("WC_HOSTNAME={}\n", crate::reference::HOSTNAME))
                    .arg("--env")
                    .arg(format!("WC_HOSTS={}", crate::image::hosts()))
                    .arg("--env")
                    .arg(format!("WC_RESOLV={}", crate::image::resolver()));
                format!(
                    "printf %s \"$WC_HOSTNAME\" > {SERVED}/etc/hostname && \
                     printf %s \"$WC_HOSTS\" > {SERVED}/etc/hosts && \
                     printf %s \"$WC_RESOLV\" > {SERVED}/etc/resolv.conf && \
                     chmod 644 {SERVED}/etc/hostname {SERVED}/etc/hosts {SERVED}/etc/resolv.conf && \
                     exec {serve} listen {SERVED}"
                )
            }
        };
        let mut child = command
            .args([image, "sh", "-c", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("running docker");
        let input = connect(&mut child, &name);
        let output = input.try_clone().expect("the connection");
        Self {
            child,
            name,
            input,
            output,
            outstanding: VecDeque::new(),
            unread: VecDeque::new(),
            held: Vec::new(),
            read: 0,
            sent: 0,
            archive: kept,
        }
    }

    /// Ends the connection as Linux does when it unmounts (DESTROY): the server lets go of every
    /// node and handle, and a client may connect again with INIT. Answers owed are read first.
    pub fn destroy(&mut self) {
        use zerocopy::IntoBytes;
        while self.waiting() {
            self.receive();
        }
        self.held.clear();
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

    /// Writes `bytes` over the start of `path` in the served directory, from outside FUSE.
    pub fn change(&self, path: &str, bytes: &[u8]) {
        change_served(&self.name, path, bytes);
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

    /// Hands the server one request. Any number may be sent before their answers are read, as
    /// Linux's FUSE queues requests: the server reads no more while its answers wait to be read, so
    /// while there is no room for the request its answers are read ahead.
    pub fn send(&mut self, unique: u64, bytes: &[u8]) {
        use std::os::fd::AsRawFd;
        let mut rest = bytes;
        while !rest.is_empty() {
            let mut ready = libc::pollfd {
                fd: self.input.as_raw_fd(),
                events: libc::POLLOUT | libc::POLLIN,
                revents: 0,
            };
            // SAFETY: waits on one descriptor this backend owns.
            let polled = unsafe { libc::poll(&mut ready, 1, -1) };
            if polled < 0 {
                let error = std::io::Error::last_os_error();
                assert_eq!(error.kind(), std::io::ErrorKind::Interrupted, "{error}");
                continue;
            }
            self.input
                .set_nonblocking(true)
                .expect("a nonblocking send");
            if ready.revents & libc::POLLIN != 0 {
                let mut buffer = [0u8; 1 << 16];
                loop {
                    match self.output.read(&mut buffer) {
                        Ok(0) => panic!("the backend ended its connection"),
                        Ok(read) => self.unread.extend(&buffer[..read]),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(error) => panic!("reading from the backend: {error}"),
                    }
                }
            }
            match self.input.write(rest) {
                Ok(written) => rest = &rest[written..],
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => panic!("writing to the backend: {error}"),
            }
            self.input
                .set_nonblocking(false)
                .expect("a blocking connection");
        }
        self.sent += 1;
        let opcode = u32::from_le_bytes(bytes[4..8].try_into().expect("a request header"));
        if answered(opcode) {
            self.outstanding.push_back(unique);
        }
    }

    /// Whether an answer is still to come, read or not.
    pub fn waiting(&self) -> bool {
        !self.outstanding.is_empty() || !self.held.is_empty()
    }

    /// The next message to hand to the kernel, chosen by `random` among those read so far, after
    /// reading some more of the server's (at least one when none is held), waiting for them; none
    /// when no answer is to come.
    pub fn next(&mut self, random: &mut rand::rngs::StdRng) -> Option<Message> {
        use rand::Rng;
        while !self.outstanding.is_empty() && (self.held.is_empty() || random.random_bool(0.5)) {
            let message = self.receive().expect("an answer is owed");
            self.read += 1;
            self.held.push((self.read, message));
        }
        if self.held.is_empty() {
            return None;
        }
        let mut index = random.random_range(0..self.held.len());
        // A reply completes only after the notices sent before it (the FUSE server interface's
        // rule), so the first of those goes in its place.
        if let (place, Message::Answer(..)) = self.held[index]
            && let Some(notice) = (0..self.held.len())
                .filter(|other| {
                    matches!(self.held[*other], (before, Message::Notice(_)) if before < place)
                })
                .min_by_key(|other| self.held[*other].0)
        {
            index = notice;
        }
        Some(self.held.swap_remove(index).1)
    }

    /// Reads the server's next message, waiting for it; none when no answer is to come.
    pub fn receive(&mut self) -> Option<Message> {
        if self.outstanding.is_empty() {
            return None;
        }
        let mut header = [0u8; 16];
        self.read_exact(&mut header);
        let length = u32::from_le_bytes(header[0..4].try_into().expect("four")) as usize;
        let unique = u64::from_le_bytes(header[8..16].try_into().expect("eight"));
        let mut message = header.to_vec();
        message.resize(length.max(16), 0);
        self.read_exact(&mut message[16..]);
        if unique == 0 {
            return Some(Message::Notice(message));
        }
        let expected = self.outstanding.pop_front();
        assert_eq!(expected, Some(unique), "the server answers in order");
        Some(Message::Answer(unique, message))
    }

    /// Fills `buffer` with the server's next bytes: those read ahead first, then the
    /// connection's, waiting for them.
    fn read_exact(&mut self, buffer: &mut [u8]) {
        let ahead = buffer.len().min(self.unread.len());
        for (byte, unread) in buffer.iter_mut().zip(self.unread.drain(..ahead)) {
            *byte = unread;
        }
        self.output
            .read_exact(&mut buffer[ahead..])
            .expect("reading from the backend");
    }
}

/// Connects to the server of container `name`, once it says it is served: Docker's proxy takes
/// connections before the server listens and ends them.
fn connect(child: &mut Child, name: &str) -> TcpStream {
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("the server's container") {
            panic!("the backend's container {name} ended: {status}");
        }
        let port = Command::new("docker")
            .args(["port", name, &PORT.to_string()])
            .stderr(Stdio::null())
            .output()
            .expect("running docker port");
        let address = String::from_utf8_lossy(&port.stdout);
        if let Some(address) = address.lines().next()
            && let Ok(mut stream) = TcpStream::connect(address.trim())
        {
            let mut served = [0u8];
            if stream.read_exact(&mut served).is_ok() {
                stream.set_nodelay(true).expect("no delay");
                return stream;
            }
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(600),
            "the backend's container {name} did not serve"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(archive) = &self.archive {
            let _ = std::fs::remove_file(archive);
        }
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
    steps: &[crate::streams::Step],
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
    let mut child = Command::new("docker")
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("running docker");
    let reader = follow(&name, &mut child, steps);
    let mut output = child.wait_with_output().expect("docker ran");
    if let Some(reader) = reader {
        output.stdout = reader.join().expect("the reader");
    }
    let exit_code = output.status.code().expect("docker exited normally");
    crate::driver::Outcome {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code,
    }
}

/// Does a script's steps in the reference container `name` as its output shows what they wait for:
/// [`Step::Expect`](crate::streams::Step::Expect) on its output and
/// [`Step::ChangeServed`](crate::streams::Step::ChangeServed). With a script, what it read of the
/// output comes from the thread returned once the container has ended.
fn follow(
    name: &str,
    child: &mut Child,
    script: &[crate::streams::Step],
) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    use std::io::BufRead;

    use crate::streams::Step;
    if script.is_empty() {
        return None;
    }
    let stdout = child.stdout.take().expect("the reference's output");
    let shown = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let reader = {
        let shown = shown.clone();
        std::thread::spawn(move || {
            let mut stdout = std::io::BufReader::new(stdout);
            while let Ok(piece) = stdout.fill_buf()
                && !piece.is_empty()
            {
                let count = piece.len();
                shown.lock().expect("shown").extend_from_slice(piece);
                stdout.consume(count);
            }
            std::mem::take(&mut *shown.lock().expect("shown"))
        })
    };
    let mut found = crate::streams::Found::default();
    for step in script {
        match step {
            Step::Expect(bytes) => {
                let started = std::time::Instant::now();
                while !found.look(&shown.lock().expect("shown"), &[], bytes) {
                    if started.elapsed() >= crate::reference::EXPECT_PATIENCE {
                        crate::reference::remove_container(name);
                        panic!(
                            "the reference never showed {:?}",
                            String::from_utf8_lossy(bytes)
                        );
                    }
                    std::thread::sleep(crate::reference::EXPECT_LOOK);
                }
            }
            Step::ChangeServed { path, bytes } => change_served(name, path, bytes),
            other => panic!("a backend run does no {other:?}"),
        }
    }
    Some(reader)
}
