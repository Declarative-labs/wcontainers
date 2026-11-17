//! Programs in the container talking to a server outside it through the host's network hooks,
//! compared with real ARM64 Linux talking to the same server.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use wcontainers_kernel_test::assert_same_output;
use wcontainers_kernel_test::network::DOCKER_HOST_NAME;

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

/// A connection to the outside reads as reset after the container is saved and brought back on
/// a new host (plan section 12); without a save it goes on.
#[test]
fn outside_connections_read_reset_after_a_restore() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let (tcp, _) = server();
    let program = wcontainers_kernel_test::guest::build("outsidesave", &[], &out);
    let tcp = tcp.to_string();
    let arguments = ["/wc/outsidesave", DOCKER_HOST_NAME, &tcp];
    let directory = program.parent().expect("a directory");
    let kept = wcontainers_kernel_test::run_ours_saving(directory, arguments[0], &arguments, 1, 0);
    let kept = String::from_utf8_lossy(&kept.outcome.stdout).into_owned();
    assert!(kept.contains("got echo: x"), "{kept}");
    // Seed 1's one save falls after the connect.
    let saved = wcontainers_kernel_test::run_ours_saving(directory, arguments[0], &arguments, 1, 1);
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
fn serve_ours(seed: u64, saves: usize) -> (wcontainers_kernel_test::driver::Run, Vec<String>) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_kernel_test::guest::build("outsideserve", &[], &out);
    let directory = program.parent().expect("a directory");
    let (port, go) = go_server();
    let port = port.to_string();
    let arguments = ["/wc/outsideserve", DOCKER_HOST_NAME, &port];
    let published = free_ports();
    std::thread::scope(|scope| {
        let answers = scope.spawn(move || clients(&go, &published));
        let run = wcontainers_kernel_test::run_ours_published(
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
fn serve_theirs() -> (wcontainers_kernel_test::driver::Outcome, Vec<String>) {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_kernel_test::guest::build("outsideserve", &[], &out);
    let (port, go) = go_server();
    let port = port.to_string();
    let published = free_ports();
    let mut answers = Vec::new();
    let outcome = wcontainers_kernel_test::reference::run_published(
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
