//! Programs in the container talking to a server outside it through the host's network hooks,
//! compared with real ARM64 Linux talking to the same server.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use wcontainers_executor_unicorn::assert_same_output;
use wcontainers_executor_unicorn::network::DOCKER_HOST_NAME;

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

/// A server on this machine that sends a connection up to 1 MiB (as much as goes within 2 s), and
/// once the next one comes, resets the first and tells the second "reset", for each pair in turn.
/// Returns its port.
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
            let linger = libc::linger {
                l_onoff: 1,
                l_linger: 0,
            };
            // SAFETY: a setsockopt with a value of its own size, on a socket this thread owns.
            unsafe {
                libc::setsockopt(
                    std::os::fd::AsRawFd::as_raw_fd(&data),
                    libc::SOL_SOCKET,
                    libc::SO_LINGER,
                    (&raw const linger).cast(),
                    size_of::<libc::linger>() as libc::socklen_t,
                );
            }
            drop(data);
            std::thread::sleep(TOLD_AFTER_RESET);
            let _ = signal.write_all(b"reset");
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
