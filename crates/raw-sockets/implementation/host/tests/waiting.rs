//! What a waiting call on the host's raw sockets allocates.
#![cfg(target_os = "linux")]

use std::net::{IpAddr, Ipv6Addr};

use wcontainers_internet_protocol::{IpVersion, Received};
use wcontainers_kernel::UserId;
use wcontainers_raw_sockets::RawSockets;
use wcontainers_raw_sockets_conformance as suite;
use wcontainers_raw_sockets_conformance::icmp_suite as tools;
use wcontainers_raw_sockets_host::HostRawSockets;

tools::counting_allocator!();

/// Waits that have to wait allocate nothing on the waiting thread once the socket's waiter list has grown.
#[test]
fn a_wait_allocates_nothing_once_warm() {
    suite::enter_network_namespace();
    let sockets = HostRawSockets::new().expect("the host's readiness");
    let open = || {
        sockets
            .open(IpVersion::V6, suite::EXPERIMENT, UserId(u32::MIN))
            .expect("a raw socket opens")
    };
    let (socket, sender) = (open(), open());
    let to = IpAddr::V6(Ipv6Addr::LOCALHOST);
    let mut into = tools::buffer();
    let counted = tools::wait_allocations(
        || sockets.wait_for_events(&socket, tools::READABLE),
        || {
            sockets
                .send_to(
                    &sender,
                    suite::payload_to(to, suite::EXPERIMENT, tools::PAYLOAD),
                    tools::plain_fields(),
                )
                .expect("the payload is sent");
        },
        || {
            assert!(matches!(
                sockets.receive(&socket, &mut into),
                Ok(Received::Packet(_))
            ));
            // The sender takes its own packet too.
            assert!(matches!(
                sockets.receive(&sender, &mut into),
                Ok(Received::Packet(_))
            ));
        },
        allocations,
    );
    assert_eq!(counted, usize::MIN);
}
