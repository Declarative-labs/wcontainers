//! Programs in the container talking to a server outside it through the host's network hooks,
//! compared with real ARM64 Linux talking to the same server.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
use std::ops::ControlFlow;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use wcontainers_abi::SystemCallRequest;
use wcontainers_executor_unicorn::hooks::{Hooks, Process};
use wcontainers_executor_unicorn::network::DOCKER_HOST_NAME;
use wcontainers_executor_unicorn::{assert_same_output, driver, guest_directory_entry, reference};
use wcontainers_kernel_linux::linux::general;
use wcontainers_kernel_linux::{SystemCallResponse, TaskId};

/// A server on this machine: TCP answers what it read once the client finishes sending, UDP
/// answers each datagram. Returns its TCP and UDP ports.
fn server() -> (u16, u16) {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    let tcp = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut got = Vec::new();
                let _ = stream.read_to_end(&mut got);
                let mut answer = b"echo: ".to_vec();
                answer.extend_from_slice(&got);
                let _ = stream.write_all(&answer);
            });
        }
    });
    let socket = UdpSocket::bind("0.0.0.0:0").expect("a UDP port");
    let udp = socket.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1500];
        while let Ok((_, from)) = socket.recv_from(&mut buffer) {
            let _ = socket.send_to(b"pong", from);
        }
    });
    (tcp, udp)
}

/// A port nothing listens on.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    listener.local_addr().expect("an address").port()
}

#[test]
fn clients_talk_to_a_server_outside_as_linux() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let (tcp, udp) = server();
    let closed = closed_port();
    let (tcp, udp, closed) = (tcp.to_string(), udp.to_string(), closed.to_string());
    let ours = assert_same_output(&out, "outside", &[DOCKER_HOST_NAME, &tcp, &udp, &closed]);
    let text = String::from_utf8_lossy(&ours.outcome.stdout);
    for line in [
        "got echo: hello",
        "got echo: second",
        "udp pong from the server 1",
    ] {
        assert!(text.contains(line), "{line:?} missing from:\n{text}");
    }
}

/// A server on this machine that echoes every byte at once as it comes. Returns its port.
fn echoing_server() -> u16 {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_nodelay(true);
            std::thread::spawn(move || {
                let mut byte = [0u8];
                while matches!(stream.read(&mut byte), Ok(1)) {
                    if stream.write_all(&byte).is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

/// A blocking read whose host's bytes arrive while it parks (between its two pulls of the host's
/// stream) is woken within its own call and runs again, as Linux's reader woken between
/// `prepare_to_wait` and `schedule` does.
#[test]
fn reads_woken_while_they_park_run_again() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let port = echoing_server().to_string();
    let ours = assert_same_output(&out, "echoes", &[DOCKER_HOST_NAME, &port, "3000"]);
    let text = String::from_utf8_lossy(&ours.outcome.stdout);
    assert!(text.contains("echoed 3000 wrong 0 end 0"), "{text}");
}

/// Bytes the peer sends after the program shut its connection for reading are still queued and
/// read, as Linux queues them; a read finds the end only once they are read.
#[test]
fn bytes_after_a_read_shutdown_are_read() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let port = echoing_server().to_string();
    let ours = assert_same_output(&out, "shutread", &[DOCKER_HOST_NAME, &port]);
    let text = String::from_utf8_lossy(&ours.outcome.stdout);
    assert!(text.contains("echoed after shutdown: 2 bc"), "{text}");
}

/// How long the resetting server waits after a reset before it tells of it.
const TOLD_AFTER_RESET: Duration = Duration::from_millis(300);

/// How long a server waits for this machine's end of a connection it reset to take the reset.
const RESET_PATIENCE: Duration = Duration::from_secs(5);

/// Resets connection `data` (a close with a linger of zero) and waits until this machine's end of
/// it has taken the reset, when that end is in this network namespace (ours; the reference's is in
/// its container's): the end is no longer listed among this machine's TCP sockets, which Linux
/// leaves as the reset closes it. Sending a reset is not its arrival: under load the receiving end
/// may take it later. Whether it was taken within [`RESET_PATIENCE`].
fn reset_and_wait_for_its_arrival(data: TcpStream) -> bool {
    let (Ok(server), Ok(client)) = (data.local_addr(), data.peer_addr()) else {
        return false;
    };
    // A linger of zero makes the close a reset.
    let _ = socket2::SockRef::from(&data).set_linger(Some(Duration::ZERO));
    drop(data);
    let start = std::time::Instant::now();
    while start.elapsed() < RESET_PATIENCE {
        let listed = procfs::net::tcp().expect("this machine's TCP sockets");
        let open = |entry: &procfs::net::TcpNetEntry| {
            entry.local_address == client && entry.remote_address == server
        };
        if !listed.iter().any(open) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

/// A server on this machine that sends a connection up to 1 MiB (as much as goes within 2 s), and
/// once the next one comes, resets the first and tells the second "reset" once the reset has
/// arrived ("unheard" if it did not within [`RESET_PATIENCE`]), for each pair in turn. Returns its
/// port.
fn resetting_server() -> u16 {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        loop {
            let Ok((mut data, _)) = listener.accept() else {
                return;
            };
            let _ = data.set_write_timeout(Some(Duration::from_secs(2)));
            let chunk = vec![7u8; 1 << 16];
            let mut sent = 0;
            while sent < 1 << 20 {
                match data.write(&chunk) {
                    Ok(written) => sent += written,
                    Err(_) => break,
                }
            }
            let Ok((mut signal, _)) = listener.accept() else {
                return;
            };
            // The program's queue has taken what fits by now.
            std::thread::sleep(Duration::from_millis(500));
            let taken = reset_and_wait_for_its_arrival(data);
            std::thread::sleep(TOLD_AFTER_RESET);
            let word = if taken { "reset" } else { "unheard" };
            let _ = signal.write_all(word.as_bytes());
            let _ = signal.read_to_end(&mut Vec::new());
        }
    });
    port
}

/// A connection whose queue is full hears of a reset while its program reads nothing, as Linux's
/// does: the reset ends it at once, its queued bytes still read before the end.
#[test]
fn a_full_connection_hears_a_reset() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let port = resetting_server().to_string();
    let ours = assert_same_output(&out, "fullreset", &[DOCKER_HOST_NAME, &port]);
    let text = String::from_utf8_lossy(&ours.outcome.stdout);
    assert!(text.contains("so_error Connection reset by peer"), "{text}");
}

/// What holds a told-resetting server's reset back until a hook lets it go: the server says a reset
/// is held, waits to be let go, and says when the reset has arrived (or that it did not within
/// [`RESET_PATIENCE`]).
struct Gate {
    held: mpsc::Sender<()>,
    go: mpsc::Receiver<()>,
    done: mpsc::Sender<bool>,
}

/// A server on this machine that, for each pair of connections, reads a byte on the second,
/// answers "reset" on it, and resets the first: at once, or with a gate once the gate lets it;
/// then waits for the reset's arrival. Returns its port.
fn told_resetting_server(gate: Option<Gate>) -> u16 {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        loop {
            let (Ok((data, _)), Ok((mut signal, _))) = (listener.accept(), listener.accept())
            else {
                return;
            };
            if signal.read_exact(&mut [0u8]).is_err() {
                continue;
            }
            if let Some(gate) = &gate {
                let _ = gate.held.send(());
            }
            let _ = signal.write_all(b"reset");
            if let Some(gate) = &gate
                && gate.go.recv().is_err()
            {
                return;
            }
            let taken = reset_and_wait_for_its_arrival(data);
            if let Some(gate) = &gate {
                let _ = gate.done.send(taken);
            }
            let _ = signal.read_to_end(&mut Vec::new());
        }
    });
    port
}

/// Lets a held reset go at the start of the program's next look at a socket (poll, select or
/// getsockopt), after every earlier call has ended, and waits until it has arrived.
struct LetGoAtTheLook {
    held: mpsc::Receiver<()>,
    go: mpsc::Sender<()>,
    done: mpsc::Receiver<bool>,
}

impl Hooks for LetGoAtTheLook {
    fn intercept_system_call_request(
        &mut self,
        _process: Process<'_>,
        _thread: TaskId,
        request: SystemCallRequest,
    ) -> ControlFlow<SystemCallResponse, SystemCallRequest> {
        let looks = [
            general::__NR_ppoll,
            general::__NR_pselect6,
            general::__NR_getsockopt,
        ];
        let looking = looks.map(u64::from).contains(&request.number.0);
        if looking && self.held.try_recv().is_ok() {
            self.go.send(()).expect("the server waits");
            let arrived = self
                .done
                .recv_timeout(RESET_PATIENCE * 2)
                .expect("the server reset the connection");
            assert!(
                arrived,
                "the reset reached this machine's end of the connection"
            );
        }
        ControlFlow::Continue(request)
    }
}

/// A reset that reaches this machine just before the program looks at its connection is seen by
/// that look, by poll, select or SO_ERROR, as on Linux, which handled the reset as it came: the
/// kernel asks the system at the look, rather than answering from what the system told it before.
/// On ours the server resets each connection at the start of the call that looks, after the end of
/// every call before it, where the kernel last heard the outside, and the call goes on once the
/// reset has reached this machine's end of the connection.
#[test]
fn a_look_hears_a_reset_the_system_has_not_told_yet() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_executor_unicorn::guest::build("toldreset", &[], &out);
    let port = told_resetting_server(None).to_string();
    let theirs = reference::run(&program, &[DOCKER_HOST_NAME, &port]);
    let (held, hold) = mpsc::channel();
    let (let_go, go) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    let port = told_resetting_server(Some(Gate { held, go, done })).to_string();
    let hooks = LetGoAtTheLook {
        held: hold,
        go: let_go,
        done: finished,
    };
    let mut files = wcontainers_executor_unicorn::image::reference_image().to_vec();
    files.extend(wcontainers_executor_unicorn::image::host_directory(
        program.parent().expect("a directory"),
        guest_directory_entry(),
    ));
    let path = reference::in_guest_directory(program.file_name().expect("a name"));
    let arguments = [path.as_str(), DOCKER_HOST_NAME, &port];
    let container = driver::Container {
        files: &files,
        path: &path,
        arguments: &arguments,
        environment: reference::ENVIRONMENT,
        working_directory: reference::ROOT_DIRECTORY,
        backend: None,
        cached: false,
        published: &[],
        root_image: None,
        address_space: wcontainers_executor_unicorn::standin::SPACE_32_BIT,
    };
    let prepared = driver::Prepared {
        hooks: Some(Box::new(hooks)),
        ..driver::prepare(&container)
    };
    let ours = driver::run_prepared(prepared, &container, 0, Default::default())
        .expect("an outcome on our kernel");
    let ours_text = String::from_utf8_lossy(&ours.outcome.stdout);
    let theirs_text = String::from_utf8_lossy(&theirs.stdout);
    assert_eq!(ours_text, theirs_text);
    assert_eq!(ours.outcome, theirs);
    for line in [" err", " readable", "so_error 0 Connection reset by peer"] {
        assert!(
            theirs_text.contains(line),
            "{line:?} missing from:\n{theirs_text}"
        );
    }
}

/// How long the telling server waits to hear what the first connection of a pair does.
const HEARING_PATIENCE: Duration = Duration::from_secs(2);

/// A server on this machine that, for each pair of connections, tells the second what the first
/// heard by the time the second came and up to [`HEARING_PATIENCE`] after: "reset", "end",
/// "bytes" or "nothing". Returns its port.
fn telling_server() -> u16 {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    let port = listener.local_addr().expect("an address").port();
    std::thread::spawn(move || {
        loop {
            let Ok((mut first, _)) = listener.accept() else {
                return;
            };
            let (heard, hearing) = mpsc::channel();
            std::thread::spawn(move || {
                let mut byte = [0u8];
                let what = match first.read(&mut byte) {
                    Ok(0) => "end",
                    Ok(_) => "bytes",
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => "reset",
                    Err(_) => "an error",
                };
                let _ = heard.send(what);
            });
            let Ok((mut second, _)) = listener.accept() else {
                return;
            };
            let what = hearing.recv_timeout(HEARING_PATIENCE).unwrap_or("nothing");
            let _ = second.write_all(what.as_bytes());
        }
    });
    port
}

/// A connection to the outside disconnected with a connect to `AF_UNSPEC` is reset at once, as
/// Linux's `tcp_disconnect` sends a reset, though its descriptor stays open.
#[test]
fn a_disconnected_connection_resets_its_outside_end() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let port = telling_server().to_string();
    let ours = assert_same_output(&out, "disconnectout", &[DOCKER_HOST_NAME, &port]);
    let text = String::from_utf8_lossy(&ours.outcome.stdout);
    assert!(text.contains("the server heard: reset"), "{text}");
}

/// A connection to the outside reads as reset after the container is saved and brought back on
/// a new host (plan section 12); without a save it goes on.
#[test]
fn outside_connections_read_reset_after_a_restore() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let (tcp, _) = server();
    let program = wcontainers_executor_unicorn::guest::build("outsidesave", &[], &out);
    let tcp = tcp.to_string();
    let arguments = ["/wc/outsidesave", DOCKER_HOST_NAME, &tcp];
    let directory = program.parent().expect("a directory");
    let kept =
        wcontainers_executor_unicorn::run_ours_saving(directory, arguments[0], &arguments, 1, 0);
    let kept = String::from_utf8_lossy(&kept.outcome.stdout).into_owned();
    assert!(kept.contains("got echo: x"), "{kept}");
    // Seed 1's one save falls after the connect.
    let saved =
        wcontainers_executor_unicorn::run_ours_saving(directory, arguments[0], &arguments, 1, 1);
    assert_eq!(saved.saved, 1);
    let text = String::from_utf8_lossy(&saved.outcome.stdout).into_owned();
    assert_eq!(
        text,
        "connect: 0\nwrite: -1 Connection reset by peer\nshutdown: -1 Socket not connected\nread: 0\n"
    );
}

/// Two free ports of this machine's loopback to publish the container's 8080 and 8081 on.
fn free_ports() -> [(u16, u16); 2] {
    let first = TcpListener::bind("127.0.0.1:0").expect("a port");
    let second = TcpListener::bind("127.0.0.1:0").expect("a port");
    [
        (8080, first.local_addr().expect("an address").port()),
        (8081, second.local_addr().expect("an address").port()),
    ]
}

/// A server on this machine whose first connection says the program listens; returns its port
/// and a receiver of that word.
fn go_server() -> (u16, mpsc::Receiver<()>) {
    let listener = TcpListener::bind("0.0.0.0:0").expect("a TCP port");
    let port = listener.local_addr().expect("an address").port();
    let (told, go) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut got = Vec::new();
            let _ = stream.read_to_end(&mut got);
            let _ = told.send(());
        }
    });
    (port, go)
}

/// Clients on this machine: once `go` comes, each published port in turn is sent a line and
/// what comes back is read, trying again while nothing answers there yet (Docker's port accepts
/// and closes while the program does not listen). Returns the answers.
fn clients(go: &mpsc::Receiver<()>, published: &[(u16, u16)]) -> Vec<String> {
    go.recv_timeout(Duration::from_secs(600))
        .expect("the program listens");
    published
        .iter()
        .map(|&(inner, outer)| {
            for _ in 0..300 {
                if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", outer)) {
                    let _ = stream.write_all(format!("hello {inner}").as_bytes());
                    let _ = stream.shutdown(Shutdown::Write);
                    let mut got = String::new();
                    let _ = stream.read_to_string(&mut got);
                    if !got.is_empty() {
                        return got;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            String::new()
        })
        .collect()
}

/// Runs guest/outsideserve.c with clients outside, on our kernel saved and brought back `saves`
/// times under `seed`; returns our run and the answers our clients got.
fn serve_ours(seed: u64, saves: usize) -> (wcontainers_executor_unicorn::driver::Run, Vec<String>) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_executor_unicorn::guest::build("outsideserve", &[], &out);
    let directory = program.parent().expect("a directory");
    let (port, go) = go_server();
    let port = port.to_string();
    let arguments = ["/wc/outsideserve", DOCKER_HOST_NAME, &port];
    let published = free_ports();
    std::thread::scope(|scope| {
        let answers = scope.spawn(move || clients(&go, &published));
        let run = wcontainers_executor_unicorn::run_ours_published(
            directory,
            arguments[0],
            &arguments,
            seed,
            saves,
            &published,
        );
        (run, answers.join().expect("the clients ran"))
    })
}

/// The reference's run of guest/outsideserve.c with clients outside, and the answers they got.
fn serve_theirs() -> (wcontainers_executor_unicorn::driver::Outcome, Vec<String>) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_executor_unicorn::guest::build("outsideserve", &[], &out);
    let (port, go) = go_server();
    let port = port.to_string();
    let published = free_ports();
    let mut answers = Vec::new();
    let outcome = wcontainers_executor_unicorn::reference::run_published(
        &program,
        &[DOCKER_HOST_NAME, &port],
        &published,
        || answers = clients(&go, &published),
    );
    (outcome, answers)
}

/// Servers inside the container accept clients outside through published ports, and see them as
/// Linux in a default Docker container does.
#[test]
fn servers_inside_accept_clients_outside_as_linux() {
    let (theirs, their_answers) = serve_theirs();
    let (ours, our_answers) = serve_ours(0, 0);
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        String::from_utf8_lossy(&theirs.stdout)
    );
    assert_eq!(ours.outcome, theirs);
    assert_eq!(our_answers, their_answers);
    assert_eq!(their_answers, ["echo: hello 8080", "echo: hello 8081"]);
}

/// Listeners listen outside again after the container is saved and brought back on a new host.
#[test]
fn listeners_listen_again_after_a_restore() {
    let (theirs, their_answers) = serve_theirs();
    // Seed 1's one save falls after the listens and before the clients come.
    let (ours, our_answers) = serve_ours(1, 1);
    assert_eq!(ours.saved, 1);
    assert_eq!(
        String::from_utf8_lossy(&ours.outcome.stdout),
        String::from_utf8_lossy(&theirs.stdout)
    );
    assert_eq!(ours.outcome, theirs);
    assert_eq!(our_answers, their_answers);
}
