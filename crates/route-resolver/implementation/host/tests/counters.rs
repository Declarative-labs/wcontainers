#![cfg(target_os = "linux")]

use std::net::{Ipv4Addr, Ipv6Addr};
use std::num::NonZeroU32;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options, Ipv6FlowLabel};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BroadcastPermission, InterfaceIndex, InterfaceSelector, Ipv4RouteSelectors, Ipv6RouteSelectors,
    MobileAddressPreference, MobilityHeaderType, NeighbourConfirmation, PacketMark, RouteScope,
    RouteSelectors, SourceAddressCheck, SourcePreferences, TemporaryAddressPreference, UpperLayer,
    UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_route_resolver::RouteResolver;
use wcontainers_route_resolver_conformance::{LABEL, icmp_suite};
use wcontainers_route_resolver_host::HostRouteResolver;

/// A link whose prefix's subnet-router anycast address is a route, one whose route a flow label selects over a wider one, and a route over both.
const FORWARDING_LINK: &str = "link set lo up
link add da type dummy
link set da up
address add fd01::1/64 dev da nodad
link add db type dummy
link set db up
address add fd02::1/64 dev db nodad
route add fd09::/64 via fd02::2 dev db table 100
route add fd09::/16 via fd02::3 dev db
route add fd07::/64 nexthop via fd01::2 dev da nexthop via fd02::2 dev db";
/// Sends a flow label's lookups, and the mobility header's, to the second link's route.
const RULES: &str = "rule add flowlabel 0x12345 lookup 100
rule add ipproto 135 lookup 100";
const IPV4: &[&str] = &[];
const IPV6: &[&str] = &["-6"];
const FORWARDING: [&str; 2] = [
    "/proc/sys/net/ipv6/conf/all/forwarding",
    "/proc/sys/net/ipv6/conf/da/forwarding",
];

fn ip(options: &[&str], commands: &str) {
    icmp_suite::batch(icmp_suite::IP, options, commands);
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

/// Checks that each refused lookup is counted once under a reason of its own.
#[test]
fn refusals_are_counted_under_their_reasons() {
    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET)
        .expect("a new network namespace needs root");
    ip(IPV4, FORWARDING_LINK);
    ip(IPV6, RULES);
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
    let hashed = Ipv6RouteSelectors {
        upper_layer: UpperLayer {
            protocol: IpNumber::GRE,
            selector: None,
        },
        ..v6(Ipv6Addr::new(0xfd07, 0, 0, 0, 0, 0, 0, 1))
    };
    // A probe socket takes no flow label, so it connects along the wider route and is told nothing of this one.
    let labelled_with_preferences = Ipv6RouteSelectors {
        flow_label: Ipv6FlowLabel::try_new(LABEL).expect("a flow label"),
        source_preferences: SourcePreferences {
            temporary: TemporaryAddressPreference::Public,
            mobile: MobileAddressPreference::Home,
        },
        ..v6(Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 1))
    };
    let da = NonZeroU32::new(nix::net::if_::if_nametoindex("da").expect("da")).expect("da");
    // The routing socket's lookup leaves by db, which a socket bound to da would not take.
    let bound_elsewhere = Ipv6RouteSelectors {
        source: Some(Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 1)),
        interface: Some(InterfaceSelector::Bound(InterfaceIndex(da))),
        ..v6(Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 1))
    };
    let mapped_from_ipv6 = Ipv6RouteSelectors {
        source: Some(Ipv6Addr::LOCALHOST),
        ..v6(Ipv4Addr::LOCALHOST.to_ipv6_mapped())
    };
    for route in [
        RouteSelectors::V4(link_scope),
        RouteSelectors::V6(mobility),
        RouteSelectors::V6(anycast),
        RouteSelectors::V6(hashed),
        RouteSelectors::V6(labelled_with_preferences),
        RouteSelectors::V6(mapped_from_ipv6),
        RouteSelectors::V6(bound_elsewhere),
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
    assert_eq!(reasons.len(), 7, "{counters:?}");
    assert!(
        counters.iter().all(|reading| reading.count == 1),
        "{counters:?}"
    );
}
