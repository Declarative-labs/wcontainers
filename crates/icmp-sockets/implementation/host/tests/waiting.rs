//! What a waiting call on the host's ping sockets allocates.
#![cfg(target_os = "linux")]

use std::alloc::System;
use std::net::{IpAddr, Ipv4Addr};

use stats_alloc::{INSTRUMENTED_SYSTEM, StatsAlloc};
use wcontainers_icmp_sockets::IcmpSockets;
use wcontainers_icmp_sockets_conformance as suite;
use wcontainers_icmp_sockets_host::HostIcmpSockets;
use wcontainers_internet_protocol::{IpVersion, Received};
use wcontainers_kernel::UserId;
use wcontainers_readiness::{Readiness, ReadyFor};

/// The system's allocator, counting the allocations of the whole test process, of which this test is the only one.
#[global_allocator]
static COUNTING: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// The allocations and reallocations made so far.
fn allocations() -> usize {
    let stats = COUNTING.stats();
    stats.allocations + stats.reallocations
}

/// Waits that have to wait allocate nothing on the waiting thread once the socket's waiter list has grown.
#[test]
fn a_wait_allocates_nothing_once_warm() {
    suite::enter_network_namespace();
    let sockets = HostIcmpSockets::new().expect("the host's readiness");
    let socket = sockets
        .open(IpVersion::V4, UserId(u32::MIN))
        .expect("an echo socket opens");
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let message = suite::echo(IpVersion::V4, suite::PAYLOAD);
    let mut into = suite::buffer();
    let counted = suite::wait_allocations(
        || sockets.wait_until_ready(&socket, ReadyFor::READABLE),
        || {
            sockets
                .send_to(
                    &socket,
                    suite::header_to(to, &message),
                    suite::plain_fields(),
                    &message,
                )
                .expect("the request is sent");
        },
        || {
            assert!(matches!(
                sockets.receive(&socket, &mut into),
                Ok(Received::Packet(_))
            ));
        },
        allocations,
    );
    assert_eq!(counted, usize::MIN);
}
