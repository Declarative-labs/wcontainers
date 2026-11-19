//! The reference: the same program in a default Docker container on real ARM64 Linux.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::driver::Outcome;

/// The image the reference runs in. Static test programs need nothing from it.
pub const IMAGE: &str = "alpine:3.22";

/// The container's host name, set so that both runners see the same environment.
pub const HOSTNAME: &str = "wc-kernel";

/// What `docker run -t` adds to the environment.
pub const TERMINAL_VARIABLE: &str = "TERM=xterm";

/// Where the reference and our kernel mount a test's own files.
pub const GUEST_DIRECTORY: &str = "/wc";

/// Where the image keeps its commands (on its PATH).
pub const IMAGE_BINARIES: &str = "/bin";

/// The platform the reference runs on.
pub const PLATFORM: &str = "linux/arm64";

/// Turns off the Docker client's hints, which it adds to a terminal after a failed run.
const NO_HINTS: (&str, &str) = ("DOCKER_CLI_HINTS", "false");

/// A default container's working directory.
pub const ROOT_DIRECTORY: &str = "/";

/// The Docker client and the words of its command line.
pub mod docker {
    /// The client's program name.
    pub const CLIENT: &str = "docker";
    /// Creates and starts a container.
    pub const START_CONTAINER: &str = "run";
    /// Writes a container's files as a tar archive.
    pub const EXPORT: &str = "export";
    /// Removes a container.
    pub const REMOVE: &str = "rm";
    /// With [`REMOVE`], stops a running container first.
    pub const FORCE: &str = "--force";
    /// With [`START_CONTAINER`], removes the container when it exits.
    pub const REMOVE_ON_EXIT: &str = "--rm";
    /// The platform option.
    pub const PLATFORM_OPTION: &str = "--platform";
    /// The container name option.
    pub const NAME_OPTION: &str = "--name";
    /// The host name option.
    pub const HOSTNAME_OPTION: &str = "--hostname";
    /// The bind mount option.
    pub const VOLUME_OPTION: &str = "--volume";
    /// Between the fields of a bind mount or a published port.
    pub const FIELD_SEPARATOR: &str = ":";
    /// A bind mount's option for read-only.
    pub const READ_ONLY: &str = "ro";
    /// Keeps standard input open, from the client's.
    pub const INTERACTIVE: &str = "-i";
    /// Gives the container a terminal.
    pub const TERMINAL: &str = "-t";
    /// Publishes a container port.
    pub const PUBLISH_OPTION: &str = "--publish";
}

/// Between the parts of a container's name.
const NAME_SEPARATOR: &str = "-";

/// The names of the containers each kind of run makes.
const REFERENCE_CONTAINERS: &str = "wc-kernel-reference";
const COMMAND_CONTAINERS: &str = "wc-kernel-command";
const STREAMS_CONTAINERS: &str = "wc-kernel-streams";

/// A container name no other run of these tests uses: its kind, this process and a count.
fn container_name(kind: &str) -> String {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    [
        kind,
        &std::process::id().to_string(),
        &RUNS.fetch_add(1, Ordering::Relaxed).to_string(),
    ]
    .join(NAME_SEPARATOR)
}

/// A path under [`GUEST_DIRECTORY`].
pub fn in_guest_directory(name: impl AsRef<Path>) -> String {
    Path::new(GUEST_DIRECTORY)
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// `--volume`'s value mounting `directory` read-only at [`GUEST_DIRECTORY`].
fn guest_volume(directory: &Path) -> String {
    [
        &directory.to_string_lossy(),
        GUEST_DIRECTORY,
        docker::READ_ONLY,
    ]
    .join(docker::FIELD_SEPARATOR)
}

/// Checks Docker ran the container: its own failures are the statuses a shell gives a command it
/// could not run.
fn assert_docker_ran(exit_code: i32, stderr: &[u8]) {
    assert!(
        !(125..=127).contains(&exit_code),
        "docker could not run the container (exit {exit_code}): {}",
        String::from_utf8_lossy(stderr)
    );
}

/// Stops and removes a container by name, whatever state it is in.
fn remove_container(name: &str) {
    let _ = Command::new(docker::CLIENT)
        .args([docker::REMOVE, docker::FORCE, name])
        .output();
}

/// The environment a default Docker container gives its first process.
pub const ENVIRONMENT: &[&str] = &[
    "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    "HOSTNAME=wc-kernel",
    "HOME=/root",
];

/// Runs `program` as the first process of a default container, with `arguments` after its name. The
/// program's directory is mounted read-only at `/wc`.
pub fn run(program: &Path, arguments: &[&str]) -> Outcome {
    run_with(program, arguments, &[])
}

/// As [`run`], on one CPU like the made-up machine, for behaviour that depends on who runs while
/// another waits.
pub fn run_one_cpu(program: &Path, arguments: &[&str]) -> Outcome {
    run_with(program, arguments, &["--cpuset-cpus=0"])
}

/// As [`run`], with ports of this machine's loopback reaching the container's as (container
/// port, outside port), and `beside` run while the container runs.
pub fn run_published(
    program: &Path,
    arguments: &[&str],
    published: &[(u16, u16)],
    beside: impl FnOnce(),
) -> Outcome {
    let options: Vec<String> = published
        .iter()
        .flat_map(|(inner, outer)| {
            [
                docker::PUBLISH_OPTION.to_string(),
                [
                    std::net::Ipv4Addr::LOCALHOST.to_string(),
                    outer.to_string(),
                    inner.to_string(),
                ]
                .join(docker::FIELD_SEPARATOR),
            ]
        })
        .collect();
    let options: Vec<&str> = options.iter().map(String::as_str).collect();
    run_beside(program, arguments, &options, beside)
}

fn run_with(program: &Path, arguments: &[&str], options: &[&str]) -> Outcome {
    run_beside(program, arguments, options, || {})
}

fn run_beside(
    program: &Path,
    arguments: &[&str],
    options: &[&str],
    beside: impl FnOnce(),
) -> Outcome {
    let name = container_name(REFERENCE_CONTAINERS);
    let directory = program
        .parent()
        .expect("program has a directory")
        .canonicalize()
        .expect("program directory exists");
    let file = program.file_name().expect("program has a name");
    let output = Command::new(docker::CLIENT)
        .args([docker::START_CONTAINER, docker::REMOVE_ON_EXIT])
        .args([
            docker::PLATFORM_OPTION,
            PLATFORM,
            docker::NAME_OPTION,
            &name,
        ])
        .args([docker::HOSTNAME_OPTION, HOSTNAME])
        .args(options)
        .arg(docker::VOLUME_OPTION)
        .arg(guest_volume(&directory))
        .arg(IMAGE)
        .arg(in_guest_directory(file))
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("running docker");
    beside();
    let output = output.wait_with_output().expect("docker ran");
    let exit_code = output.status.code().expect("docker exited normally");
    assert_docker_ran(exit_code, &output.stderr);
    Outcome {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code,
    }
}

/// Runs a command of the image (by its name in the image's PATH) as the first process of a default
/// container, and returns what it did and the container's files afterwards, as `docker export` gives
/// them.
pub fn run_command(command: &[&str]) -> (Outcome, Vec<crate::image::File>) {
    let name = container_name(COMMAND_CONTAINERS);
    let output = Command::new(docker::CLIENT)
        .arg(docker::START_CONTAINER)
        .args([
            docker::PLATFORM_OPTION,
            PLATFORM,
            docker::NAME_OPTION,
            &name,
        ])
        .args([docker::HOSTNAME_OPTION, HOSTNAME])
        .arg(IMAGE)
        .args(command)
        .output()
        .expect("running docker");
    let exported = Command::new(docker::CLIENT)
        .args([docker::EXPORT, &name])
        .output()
        .expect("running docker export");
    remove_container(&name);
    let exit_code = output.status.code().expect("docker exited normally");
    assert_docker_ran(exit_code, &output.stderr);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    (
        Outcome {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code,
        },
        crate::image::from_tar(&exported.stdout),
    )
}

/// How long the reference waits for output a script expects.
const EXPECT_PATIENCE: std::time::Duration = std::time::Duration::from_secs(60);

/// How often the reference looks again for output a script expects.
const EXPECT_LOOK: std::time::Duration = std::time::Duration::from_millis(5);

/// How long a slow outside pauses after each piece of output it takes.
const SLOW_PAUSE: std::time::Duration = std::time::Duration::from_millis(20);

/// The most a reader of the client's output takes at once.
const READ_CHUNK: usize = 1 << 16;

/// What the reference's client has shown: its output (or all its terminal shows) and its error.
type Shown = std::sync::Arc<std::sync::Mutex<(Vec<u8>, Vec<u8>)>>;

/// Runs `command` (a path in the image, or under `/wc`, where `directory` is mounted read-only) as
/// the first process of a default container whose standard streams have the outside given: `-i`
/// for input, `-t` for a terminal (with input, the client on a pseudo-terminal of that size), and
/// its script followed as on our kernel. A script may not close the output: Docker's daemon reads a
/// container's output for its log whatever the client does.
pub fn run_outside(
    directory: &Path,
    command: &[&str],
    outside: &crate::streams::Outside,
) -> Outcome {
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::process::Stdio;

    use crate::streams::Step;

    let name = container_name(STREAMS_CONTAINERS);
    let directory = directory.canonicalize().expect("program directory exists");
    let mut client = Command::new(docker::CLIENT);
    client
        .env(NO_HINTS.0, NO_HINTS.1)
        .args([docker::START_CONTAINER, docker::REMOVE_ON_EXIT])
        .args([
            docker::PLATFORM_OPTION,
            PLATFORM,
            docker::NAME_OPTION,
            &name,
        ])
        .args([docker::HOSTNAME_OPTION, HOSTNAME])
        .arg(docker::VOLUME_OPTION)
        .arg(guest_volume(&directory));
    if outside.input {
        client.arg(docker::INTERACTIVE);
    }
    if outside.terminal.is_some() {
        client.arg(docker::TERMINAL);
    }
    client.arg(IMAGE).args(command);
    let shown: Shown = Default::default();
    let pace = outside.output_pace.map(|pace| pace as usize);
    let reader = |mut from: std::fs::File, error: bool, pace: Option<usize>| {
        let shown = shown.clone();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buffer = vec![0u8; pace.unwrap_or(READ_CHUNK)];
            // A terminal's master reads EIO once the client has gone.
            while let Ok(count) = from.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                let mut shown = shown.lock().expect("shown");
                let into = if error { &mut shown.1 } else { &mut shown.0 };
                into.extend_from_slice(&buffer[..count]);
                drop(shown);
                if pace.is_some() {
                    std::thread::sleep(SLOW_PAUSE);
                }
            }
        })
    };
    // The client's input: a pipe written from a thread, so a container that stops reading never
    // holds the script up, or a pseudo-terminal's master.
    let mut master: Option<std::fs::File> = None;
    let mut input: Option<std::sync::mpsc::Sender<Vec<u8>>> = None;
    let mut threads = Vec::new();
    let mut child = if let (Some((columns, rows)), true) = (outside.terminal, outside.input) {
        let mut master_fd = libc::c_int::default();
        let mut slave_fd = libc::c_int::default();
        let mut size = libc::winsize {
            ws_row: rows,
            ws_col: columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: openpty fills the two descriptors; the slave is made raw so the client's
        // terminal passes bytes unchanged both ways, as the client itself sets it.
        let (master_file, slave) = unsafe {
            assert_eq!(
                libc::openpty(
                    &mut master_fd,
                    &mut slave_fd,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut size
                ),
                0,
                "openpty"
            );
            let mut settings: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(slave_fd, &mut settings), 0, "tcgetattr");
            libc::cfmakeraw(&mut settings);
            assert_eq!(
                libc::tcsetattr(slave_fd, libc::TCSANOW, &settings),
                0,
                "tcsetattr"
            );
            (
                std::fs::File::from_raw_fd(master_fd),
                OwnedFd::from_raw_fd(slave_fd),
            )
        };
        let child = client
            .stdin(Stdio::from(slave.try_clone().expect("slave")))
            .stdout(Stdio::from(slave.try_clone().expect("slave")))
            .stderr(Stdio::from(slave))
            .spawn()
            .expect("running docker");
        // The command holds the slave's copies until it goes; the master reads EIO only once
        // every copy is closed.
        drop(client);
        threads.push(reader(
            master_file.try_clone().expect("master"),
            false,
            pace,
        ));
        master = Some(master_file);
        child
    } else {
        let mut child = client
            .stdin(if outside.input {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("running docker");
        let stdout = std::fs::File::from(OwnedFd::from(child.stdout.take().expect("stdout")));
        let stderr = std::fs::File::from(OwnedFd::from(child.stderr.take().expect("stderr")));
        threads.push(reader(stdout, false, pace));
        threads.push(reader(stderr, true, None));
        if let Some(mut stdin) = child.stdin.take() {
            let (sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
            input = Some(sender);
            std::thread::spawn(move || {
                for bytes in receiver {
                    // A container that closed its input leaves the rest unread.
                    if stdin.write_all(&bytes).is_err() {
                        break;
                    }
                }
            });
        }
        child
    };
    let mut found = crate::streams::Found::default();
    for step in &outside.script {
        match step {
            Step::Expect(bytes) => {
                let started = std::time::Instant::now();
                loop {
                    let seen = {
                        let shown = shown.lock().expect("shown");
                        found.look(&shown.0, &shown.1, bytes)
                    };
                    if seen {
                        break;
                    }
                    if started.elapsed() >= EXPECT_PATIENCE {
                        remove_container(&name);
                        let _ = child.kill();
                        panic!(
                            "the reference never showed {:?}; shown {:?}",
                            String::from_utf8_lossy(bytes),
                            shown.lock().expect("shown")
                        );
                    }
                    std::thread::sleep(EXPECT_LOOK);
                }
            }
            Step::Send(bytes) => match (&mut master, &input) {
                (Some(master), _) => master.write_all(bytes).expect("writing the terminal"),
                (None, Some(input)) => {
                    let _ = input.send(bytes.clone());
                }
                (None, None) => panic!("a script that sends has an input"),
            },
            Step::Resize { columns, rows } => {
                let master = master.as_ref().expect("a terminal with input to resize");
                let size = libc::winsize {
                    ws_row: *rows,
                    ws_col: *columns,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                // SAFETY: sets the client's terminal size and tells the client, as a terminal
                // window does.
                unsafe {
                    assert_eq!(
                        libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &size),
                        0,
                        "setting the terminal size"
                    );
                    libc::kill(child.id() as i32, libc::SIGWINCH);
                }
            }
            Step::CloseInput => {
                assert!(master.is_none(), "the reference's terminal is not closed");
                input = None;
            }
            Step::CloseOutput => panic!("Docker reads the output for its log; see run_outside"),
        }
    }
    let status = child.wait().expect("docker ran");
    drop(input);
    drop(master);
    for thread in threads {
        thread.join().expect("a reader");
    }
    let exit_code = status.code().expect("docker exited normally");
    let (stdout, stderr) = std::mem::take(&mut *shown.lock().expect("shown"));
    assert_docker_ran(exit_code, &stderr);
    Outcome {
        stdout,
        stderr,
        exit_code,
    }
}
