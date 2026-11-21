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
use wcontainers_route_resolver_host::{
    HostRouteResolver, LINK_SCOPE, ROUTE_KIND, UPPER_LAYER_PROTOCOL,
};

wcontainers_route_resolver_conformance::tests!(
    HostRouteResolver::open().expect("a routing socket opens")
);

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
    let mut child = Command::new(wcontainers_route_resolver_conformance::IP)
        .args(wcontainers_route_resolver_conformance::BATCH)
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

/// Each refused lookup is counted once under its own reason.
#[test]
fn refusals_are_counted_under_their_reasons() {
    wcontainers_route_resolver_conformance::enter_network_namespace();
    ip(FORWARDING_LINK);
    for path in FORWARDING {
        std::fs::write(path, "1").expect(path);
    }
    let resolver = HostRouteResolver::open().expect("a routing socket opens");
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
    for (route, reason) in [
        (RouteSelectors::V4(link_scope), LINK_SCOPE),
        (RouteSelectors::V6(mobility), UPPER_LAYER_PROTOCOL),
        (RouteSelectors::V6(anycast), ROUTE_KIND),
    ] {
        assert_eq!(
            resolve(&resolver, route),
            Err(HostError::NotSupported),
            "{reason}"
        );
    }
    let mut counted: Vec<_> = resolver
        .counters()
        .into_iter()
        .map(|reading| (reading.reason, reading.count))
        .collect();
    counted.sort();
    let mut expected = vec![(LINK_SCOPE, 1), (UPPER_LAYER_PROTOCOL, 1), (ROUTE_KIND, 1)];
    expected.sort();
    assert_eq!(counted, expected);
}
