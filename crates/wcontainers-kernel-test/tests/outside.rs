//! Programs in the container talking to a server outside it through the host's network hooks,
//! compared with real ARM64 Linux talking to the same server.

use std::io::{Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::path::Path;

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
