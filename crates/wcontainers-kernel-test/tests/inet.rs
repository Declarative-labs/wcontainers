//! TCP and UDP inside the container compared with real ARM64 Linux.

use std::path::Path;

use wcontainers_kernel_test::assert_same_output;

#[test]
fn internet_sockets_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "inet", &[]);
}

#[test]
fn interfaces_and_routes_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "netroute", &[]);
}

#[test]
fn proc_net_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "procnet", &[]);
}

#[test]
fn transfers_with_sockets_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sockxfer", &[]);
}

#[test]
fn datagram_control_messages_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "udpcontrol", &[]);
}

#[test]
fn receive_timestamps_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "timestamps", &[]);
}

#[test]
fn urgent_flags_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "inetoob", &[]);
}

#[test]
fn io_signals_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "inetasync", &[]);
}

#[test]
fn socket_mappings_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "tcpmap", &[]);
}

#[test]
fn error_queue_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "recverr", &[]);
}

#[test]
fn tcp_urgent_data_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "tcpoob", &[]);
}
