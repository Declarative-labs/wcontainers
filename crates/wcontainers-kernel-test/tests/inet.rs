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

#[test]
fn orders_of_checks_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sockorder", &[]);
}

#[test]
fn deferred_accepts_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "tcpdefer", &[]);
}

#[test]
fn tcp_info_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "tcpinfo", &[]);
}

#[test]
fn send_control_messages_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "inetsend", &[]);
}

#[test]
fn udp_segmentation_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "udpsegment", &[]);
}

#[test]
fn timestamping_matches_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "timestamping", &[]);
}

#[test]
fn multicast_and_broadcast_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "multicast", &[]);
}

#[test]
fn option_corners_and_neighbours_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "inetcorners", &[]);
}

#[test]
fn option_reads_and_socket_corners_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "optreads", &[]);
}

#[test]
fn route_questions_and_rules_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "routeget", &[]);
}

#[test]
fn socket_edges_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sockedges", &[]);
}

#[test]
fn options_not_acted_on_are_counted() {
    use wcontainers_kernel::reason;
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    let program = wcontainers_kernel_test::guest::build("sockedges", &[], &out);
    let (ours, theirs) = wcontainers_kernel_test::run_both(&program, &["unacted"]);
    assert_eq!(ours.outcome.stdout, theirs.stdout);
    let mut counted: Vec<u64> = ours
        .counters
        .iter()
        .filter(|counter| counter.reason == reason::OPTION_NOT_ACTED_ON)
        .map(|counter| counter.detail)
        .collect();
    counted.sort();
    // SOL_SOCKET's SO_MAX_PACING_RATE, then TCP_SAVE_SYN, TCP_FASTOPEN_CONNECT and TCP_TX_DELAY
    // (MSG_MORE and TCP_CORK on a connection inside the container are acted on).
    let option = |level: u64, name: u64| (level << 32) | name;
    assert_eq!(
        counted,
        [option(1, 47), option(6, 27), option(6, 30), option(6, 37)]
    );
}

/// A TCP disconnect lets go of what the program did not choose, and an accepted socket chose what
/// its listener's program chose.
#[test]
fn tcp_disconnects_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "tcpdisconnect", &[]);
}

/// Socket ioctls' order of checks.
#[test]
fn socket_ioctl_checks_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "sockioctls", &[]);
}

/// TCP bytes held back by MSG_MORE and TCP_CORK, and what sends them: a send without MSG_MORE,
/// the cork's removal, TCP_NODELAY, the probe timer and the connection's end.
#[test]
fn held_back_sends_match_reference() {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("guest");
    assert_same_output(&out, "tcpmore", &[]);
}
