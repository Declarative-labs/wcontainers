#![cfg(target_os = "linux")]

use std::io::Write;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::process::{Command, Stdio};

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options, Ipv6FlowLabel};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BroadcastPermission, Ipv4RouteSelectors, Ipv6RouteSelectors, MobileAddressPreference,
    MobilityHeaderType, NeighbourConfirmation, PacketMark, RouteScope, RouteSelectors,
    SourceAddressCheck, SourcePreferences, TemporaryAddressPreference, UpperLayer,
    UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_route_resolver::RouteResolver;
use wcontainers_route_resolver_host::HostRouteResolver;

/// A dummy link with an IPv6 prefix that forwards, so that the prefix's subnet-router anycast address is a route.
const FORWARDING_LINK: &str = "link set lo up
link add da type dummy
link set da up
address add fd01::1/64 dev da nodad";
const FORWARDING: [&str; 2] = [
    "/proc/sys/net/ipv6/conf/all/forwarding",
    "/proc/sys/net/ipv6/conf/da/forwarding",
];

fn ip(commands: &str) {
    let mut child = Command::new("ip")
        .args(["-batch", "-"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("iproute2's ip runs");
    child
        .stdin
        .take()
        .expect("ip's input")
        .write_all(commands.as_bytes())
        .expect("ip reads its commands");
    assert!(child.wait().expect("ip ends").success(), "{commands}");
}

fn v4(options: &Ipv4Options) -> Ipv4RouteSelectors<'_> {
    Ipv4RouteSelectors {
        destination: Ipv4Addr::LOCALHOST,
        source: None,
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: IpDscp::ZERO,
        ecn: IpEcn::NotEct,
        mark: PacketMark(u32::MIN),
        scope: RouteScope::Universe,
        broadcast: BroadcastPermission::Refused,
        options,
        upper_layer: UpperLayer {
            protocol: IpNumber::UDP,
            selector: None,
        },
    }
}

fn v6(destination: Ipv6Addr) -> Ipv6RouteSelectors<'static> {
    Ipv6RouteSelectors {
        destination,
        source: None,
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: IpDscp::ZERO,
        ecn: IpEcn::NotEct,
        flow_label: Ipv6FlowLabel::ZERO,
        mark: PacketMark(u32::MIN),
        source_preferences: SourcePreferences {
            temporary: TemporaryAddressPreference::SystemDefault,
            mobile: MobileAddressPreference::Home,
        },
        routing_header: None,
        upper_layer: UpperLayer {
            protocol: IpNumber::UDP,
            selector: None,
        },
    }
}

fn resolve(
    resolver: &HostRouteResolver,
    route: RouteSelectors<'_>,
) -> Result<wcontainers_internet_protocol::Route, HostError> {
    resolver.resolve_route(route, UserId(u32::MIN), NeighbourConfirmation::Unconfirmed)
}

/// Each refused lookup is counted once under a reason of its own.
#[test]
fn refusals_are_counted_under_their_reasons() {
    // SAFETY: a new network namespace shares no memory or file with the rest of the process.
    assert_eq!(
        unsafe { libc::unshare(libc::CLONE_NEWNET) },
        0,
        "a new network namespace needs root"
    );
    ip(FORWARDING_LINK);
    for path in FORWARDING {
        std::fs::write(path, "1").expect(path);
    }
    let resolver = HostRouteResolver::open();
    let options = Ipv4Options::new();
    let link_scope = Ipv4RouteSelectors {
        scope: RouteScope::Link,
        ..v4(&options)
    };
    let mobility = Ipv6RouteSelectors {
        upper_layer: UpperLayer {
            protocol: IpNumber::MOBILITY_HEADER,
            selector: Some(UpperLayerSelector::MobilityHeader(MobilityHeaderType(
                u8::MIN,
            ))),
        },
        ..v6(Ipv6Addr::LOCALHOST)
    };
    let anycast = v6(Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 0));
    for route in [
        RouteSelectors::V4(link_scope),
        RouteSelectors::V6(mobility),
        RouteSelectors::V6(anycast),
    ] {
        assert_eq!(
            resolve(&resolver, route.clone()),
            Err(HostError::NotSupported),
            "{route:?}"
        );
    }
    let counters = resolver.counters();
    let mut reasons: Vec<_> = counters.iter().map(|reading| reading.reason).collect();
    reasons.sort();
    reasons.dedup();
    assert_eq!(reasons.len(), 3, "{counters:?}");
    assert!(
        counters.iter().all(|reading| reading.count == 1),
        "{counters:?}"
    );
}
