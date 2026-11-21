//! The tests every implementation of wcontainers-route-resolver runs, written against its traits alone.
//!
//! Each runs as root in a network namespace of its own, on links, routes and rules it builds there with
//! iproute2's `ip`, and asks the value made in that namespace about them.
#![cfg(target_os = "linux")]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options, Ipv6FlowLabel, Ipv6RawExtHeaderSlice};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BroadcastPermission, HopLimit, IcmpCode, IcmpType, InterfaceIndex, Ipv4RouteSelectors,
    Ipv6RouteSelectors, MaximumTransmissionUnit, MobileAddressPreference, NeighbourConfirmation,
    PacketMark, Port, Route, RouteKind, RouteScope, RouteSelectors, SourceAddressCheck,
    SourcePreferences, TemporaryAddressPreference, TransportPorts, UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_route_resolver::RouteResolver;

/// Moves the calling thread into a new, empty network namespace, which needs `CAP_SYS_ADMIN`.
pub fn enter_network_namespace() {
    // SAFETY: a new network namespace shares no memory or file with the rest of the process.
    unsafe { rustix::thread::unshare_unsafe(rustix::thread::UnshareFlags::NEWNET) }
        .expect("a new network namespace needs root");
}

const IP: &str = "ip";
const A: &str = "da";
const B: &str = "db";
const A_MTU: u32 = 1400;
const B_MTU: u32 = 1300;
const LOOPBACK_MTU: u32 = 65520;
/// Two dummy links, da and db, with routes to `FAR` through a gateway on each: the main table's through da, table 100's through db.
const NETWORK: &str = "link set lo up mtu 65520
link add da type dummy
link set da up mtu 1400
address add 10.1.0.1/24 dev da
address add fd01::1/64 dev da nodad
link add db type dummy
link set db up mtu 1300
address add 10.2.0.1/24 dev db
address add fd02::1/64 dev db nodad
route add 10.9.0.0/16 via 10.1.0.2
route add 10.9.0.0/16 via 10.2.0.2 table 100
route add fd09::/64 via fd01::2
route add fd09::/64 via fd02::2 table 100
";
const A_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 1);
const B_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 1);
const A_ADDRESS_V6: Ipv6Addr = Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 1);
const B_ADDRESS_V6: Ipv6Addr = Ipv6Addr::new(0xfd02, 0, 0, 0, 0, 0, 0, 1);
/// An address on da's link that the namespace does not have.
const A_NEIGHBOUR: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 5);
const A_FOREIGN: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 99);
const A_FOREIGN_V6: Ipv6Addr = Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 0x99);
const B_GATEWAY: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 2);
const B_GATEWAY_V6: Ipv6Addr = Ipv6Addr::new(0xfd02, 0, 0, 0, 0, 0, 0, 2);
const FAR: Ipv4Addr = Ipv4Addr::new(10, 9, 1, 1);
const FAR_V6: Ipv6Addr = Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 1);
const ROOT: UserId = UserId(0);

struct Network {
    a: InterfaceIndex,
    b: InterfaceIndex,
}

/// Runs each line of `commands` as one `ip` command with these options.
fn ip(options: &[&str], commands: &str) {
    let mut child = Command::new(IP)
        .args(options)
        .args(BATCH)
        .stdin(Stdio::piped())
        .spawn()
        .expect("iproute2's ip runs");
    child
        .stdin
        .take()
        .expect("ip's input")
        .write_all(commands.as_bytes())
        .expect("ip reads its commands");
    assert!(
        child.wait().expect("ip ends").success(),
        "ip {options:?}: {commands}"
    );
}

fn write_setting(path: &str, value: impl ToString) {
    fs::write(path, value.to_string()).unwrap_or_else(|error| panic!("{path}: {error}"));
}

fn index(name: &str) -> InterfaceIndex {
    let socket = rustix::net::socket(
        rustix::net::AddressFamily::INET,
        rustix::net::SocketType::DGRAM,
        None,
    )
    .expect("a socket opens");
    InterfaceIndex(rustix::net::netdevice::name_to_index(&socket, name).expect(name))
}

fn network() -> Network {
    ip(IPV4, NETWORK);
    Network {
        a: index(A),
        b: index(B),
    }
}

/// Reads the commands from the input.
const BATCH: [&str; 2] = ["-batch", "-"];
/// IPv4 is the family of a rule with no address.
const IPV4: &[&str] = &[];
const IPV6: &[&str] = &["-6"];

fn v4(options: &Ipv4Options, destination: Ipv4Addr) -> Ipv4RouteSelectors<'_> {
    Ipv4RouteSelectors {
        destination,
        source: None,
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: IpDscp::ZERO,
        ecn: IpEcn::NotEct,
        mark: PacketMark(0),
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
        mark: PacketMark(0),
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

fn resolve4<R: RouteResolver>(
    resolver: &R,
    route: Ipv4RouteSelectors<'_>,
) -> Result<Route, HostError> {
    resolver.resolve_route(
        RouteSelectors::V4(route),
        ROOT,
        NeighbourConfirmation::Unconfirmed,
    )
}

fn resolve6<R: RouteResolver>(
    resolver: &R,
    route: Ipv6RouteSelectors<'_>,
) -> Result<Route, HostError> {
    resolver.resolve_route(
        RouteSelectors::V6(route),
        ROOT,
        NeighbourConfirmation::Unconfirmed,
    )
}

fn interface(result: Result<Route, HostError>) -> InterfaceIndex {
    result.expect("the route resolves").interface
}

/// Accepts an honest refusal of what a host may be unable to ask its routing for, and checks anything else.
fn unless_refused(result: Result<Route, HostError>, check: impl FnOnce(Result<Route, HostError>)) {
    if result != Err(HostError::NotSupported) {
        check(result);
    }
}

/// An address of lo is a local route on lo from that address.
pub fn loopback_route_is_local<R: RouteResolver>(resolver: R) {
    let network = network();
    let options = Ipv4Options::new();
    let route = resolve4(&resolver, v4(&options, Ipv4Addr::LOCALHOST)).expect("loopback resolves");
    assert_eq!(route.kind, RouteKind::Local);
    assert!(![network.a, network.b].contains(&route.interface));
    assert_eq!(route.source, IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(route.device_mtu, MaximumTransmissionUnit(LOOPBACK_MTU));
    let route = resolve6(&resolver, v6(Ipv6Addr::LOCALHOST)).expect("loopback resolves");
    assert_eq!(route.kind, RouteKind::Local);
    assert!(![network.a, network.b].contains(&route.interface));
    assert_eq!(route.source, IpAddr::V6(Ipv6Addr::LOCALHOST));
}

/// A route through a gateway gives the link's interface, its address as the source, its MTU and the namespace's TTL.
pub fn ipv4_route_gives_link_source_and_sizes<R: RouteResolver>(resolver: R) {
    const DEFAULT_TTL: &str = "/proc/sys/net/ipv4/ip_default_ttl";
    const TTL: HopLimit = HopLimit(77);
    let network = network();
    write_setting(DEFAULT_TTL, TTL.0);
    let options = Ipv4Options::new();
    let route = resolve4(&resolver, v4(&options, FAR)).expect("the route resolves");
    assert_eq!(
        route,
        Route {
            kind: RouteKind::Unicast,
            source: IpAddr::V4(A_ADDRESS),
            interface: network.a,
            path_mtu: MaximumTransmissionUnit(A_MTU),
            device_mtu: MaximumTransmissionUnit(A_MTU),
            default_hop_limit: TTL,
        }
    );
}

/// An IPv6 route gives the link's IPv6 MTU as the path MTU and the link's IPv6 hop limit.
pub fn ipv6_route_gives_link_source_and_sizes<R: RouteResolver>(resolver: R) {
    const A_IPV6_MTU: &str = "/proc/sys/net/ipv6/conf/da/mtu";
    const A_HOP_LIMIT: &str = "/proc/sys/net/ipv6/conf/da/hop_limit";
    const IPV6_MTU: MaximumTransmissionUnit = MaximumTransmissionUnit(1350);
    const HOP_LIMIT: HopLimit = HopLimit(33);
    let network = network();
    write_setting(A_IPV6_MTU, IPV6_MTU.0);
    write_setting(A_HOP_LIMIT, HOP_LIMIT.0);
    let route = resolve6(&resolver, v6(FAR_V6)).expect("the route resolves");
    assert_eq!(
        route,
        Route {
            kind: RouteKind::Unicast,
            source: IpAddr::V6(A_ADDRESS_V6),
            interface: network.a,
            path_mtu: IPV6_MTU,
            device_mtu: MaximumTransmissionUnit(A_MTU),
            default_hop_limit: HOP_LIMIT,
        }
    );
}

/// A route's own MTU and hop limit are the path MTU and default hop limit, the device's MTU staying its own.
pub fn route_metrics_give_path_mtu_and_hop_limit<R: RouteResolver>(resolver: R) {
    const DESTINATION: Ipv4Addr = Ipv4Addr::new(10, 8, 0, 1);
    const DESTINATION_V6: Ipv6Addr = Ipv6Addr::new(0xfd08, 0, 0, 0, 0, 0, 0, 1);
    const ROUTE_MTU: MaximumTransmissionUnit = MaximumTransmissionUnit(1280);
    const ROUTE_MTU_V6: MaximumTransmissionUnit = MaximumTransmissionUnit(1290);
    const ROUTE_HOP_LIMIT: HopLimit = HopLimit(9);
    const ROUTE_HOP_LIMIT_V6: HopLimit = HopLimit(10);
    network();
    const ROUTES: &str = "route add 10.8.0.0/16 via 10.1.0.2 mtu 1280 hoplimit 9
route add fd08::/64 via fd01::2 mtu 1290 hoplimit 10";
    ip(IPV4, ROUTES);
    let options = Ipv4Options::new();
    let route = resolve4(&resolver, v4(&options, DESTINATION)).expect("resolves");
    assert_eq!(route.path_mtu, ROUTE_MTU);
    assert_eq!(route.device_mtu, MaximumTransmissionUnit(A_MTU));
    assert_eq!(route.default_hop_limit, ROUTE_HOP_LIMIT);
    let route = resolve6(&resolver, v6(DESTINATION_V6)).expect("resolves");
    assert_eq!(route.path_mtu, ROUTE_MTU_V6);
    assert_eq!(route.device_mtu, MaximumTransmissionUnit(A_MTU));
    assert_eq!(route.default_hop_limit, ROUTE_HOP_LIMIT_V6);
}

/// No route, a prohibited, a blackholed and an unreachable one fail with the kernel's errors.
pub fn failing_routes_give_their_errors<R: RouteResolver>(resolver: R) {
    const UNROUTED: Ipv4Addr = Ipv4Addr::new(10, 200, 0, 1);
    const PROHIBITED: Ipv4Addr = Ipv4Addr::new(10, 50, 0, 1);
    const BLACKHOLED: Ipv4Addr = Ipv4Addr::new(10, 51, 0, 1);
    const UNREACHABLE: Ipv4Addr = Ipv4Addr::new(10, 52, 0, 1);
    const UNROUTED_V6: Ipv6Addr = Ipv6Addr::new(0xfd99, 0, 0, 0, 0, 0, 0, 1);
    network();
    const ROUTES: &str = "route add prohibit 10.50.0.0/16
route add blackhole 10.51.0.0/16
route add unreachable 10.52.0.0/16";
    ip(IPV4, ROUTES);
    let options = Ipv4Options::new();
    for (destination, errno) in [
        (UNROUTED, libc::ENETUNREACH),
        (PROHIBITED, libc::EACCES),
        (BLACKHOLED, libc::EINVAL),
        (UNREACHABLE, libc::EHOSTUNREACH),
    ] {
        assert_eq!(
            resolve4(&resolver, v4(&options, destination)),
            Err(HostError::Errno(errno)),
            "{destination}"
        );
    }
    assert_eq!(
        resolve6(&resolver, v6(UNROUTED_V6)),
        Err(HostError::Errno(libc::ENETUNREACH))
    );
}

/// A rule on the mark sends only marked lookups to the other table.
pub fn mark_selects_the_route<R: RouteResolver>(resolver: R) {
    const MARK: PacketMark = PacketMark(7);
    let network = network();
    const RULE: &str = "rule add fwmark 7 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE);
    let options = Ipv4Options::new();
    let marked = Ipv4RouteSelectors {
        mark: MARK,
        ..v4(&options, FAR)
    };
    assert_eq!(interface(resolve4(&resolver, marked)), network.b);
    assert_eq!(interface(resolve4(&resolver, v4(&options, FAR))), network.a);
    let marked = Ipv6RouteSelectors {
        mark: MARK,
        ..v6(FAR_V6)
    };
    assert_eq!(interface(resolve6(&resolver, marked)), network.b);
    assert_eq!(interface(resolve6(&resolver, v6(FAR_V6))), network.a);
}

/// A rule on a user sends only that owner's lookups to the other table.
pub fn owner_selects_the_route<R: RouteResolver>(resolver: R) {
    const OWNER: UserId = UserId(4242);
    let network = network();
    const RULE: &str = "rule add uidrange 4242-4242 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE);
    let options = Ipv4Options::new();
    let owner = OWNER;
    let unconfirmed = NeighbourConfirmation::Unconfirmed;
    let route = resolver.resolve_route(RouteSelectors::V4(v4(&options, FAR)), owner, unconfirmed);
    assert_eq!(interface(route), network.b);
    assert_eq!(interface(resolve4(&resolver, v4(&options, FAR))), network.a);
    let route = resolver.resolve_route(RouteSelectors::V6(v6(FAR_V6)), owner, unconfirmed);
    assert_eq!(interface(route), network.b);
    assert_eq!(interface(resolve6(&resolver, v6(FAR_V6))), network.a);
}

/// A rule on the DSCP sends only lookups with that DSCP, whatever their ECN, to the other table.
pub fn dscp_selects_the_route<R: RouteResolver>(resolver: R) {
    const DSCP: IpDscp = IpDscp::CS1;
    let network = network();
    const RULE: &str = "rule add dscp 8 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE);
    let options = Ipv4Options::new();
    let dscp = DSCP;
    for ecn in [IpEcn::NotEct, IpEcn::CongestionExperienced] {
        let route = Ipv4RouteSelectors {
            dscp,
            ecn,
            ..v4(&options, FAR)
        };
        assert_eq!(interface(resolve4(&resolver, route)), network.b);
        let route = Ipv6RouteSelectors {
            dscp,
            ecn,
            ..v6(FAR_V6)
        };
        assert_eq!(interface(resolve6(&resolver, route)), network.b);
    }
    let route = Ipv4RouteSelectors {
        ecn: IpEcn::CongestionExperienced,
        ..v4(&options, FAR)
    };
    assert_eq!(interface(resolve4(&resolver, route)), network.a);
    assert_eq!(interface(resolve6(&resolver, v6(FAR_V6))), network.a);
}

/// A rule on the source sends lookups from that address to the other table, and the route keeps the source.
pub fn source_selects_the_route<R: RouteResolver>(resolver: R) {
    let network = network();
    const RULE: &str = "rule add from 10.2.0.1 lookup 100";
    const RULE_V6: &str = "rule add from fd02::1 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE_V6);
    let options = Ipv4Options::new();
    let route = Ipv4RouteSelectors {
        source: Some(B_ADDRESS),
        ..v4(&options, FAR)
    };
    let route = resolve4(&resolver, route).expect("resolves");
    assert_eq!(
        (route.interface, route.source),
        (network.b, IpAddr::V4(B_ADDRESS))
    );
    let route = Ipv6RouteSelectors {
        source: Some(B_ADDRESS_V6),
        ..v6(FAR_V6)
    };
    let route = resolve6(&resolver, route).expect("resolves");
    assert_eq!(
        (route.interface, route.source),
        (network.b, IpAddr::V6(B_ADDRESS_V6))
    );
}

/// A rule on the outgoing interface sends lookups bound to it to the other table.
pub fn interface_selects_the_route<R: RouteResolver>(resolver: R) {
    let network = network();
    const RULE: &str = "rule add oif db lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE);
    let options = Ipv4Options::new();
    let route = Ipv4RouteSelectors {
        interface: Some(network.b),
        ..v4(&options, FAR)
    };
    let route = resolve4(&resolver, route).expect("resolves");
    assert_eq!(
        (route.interface, route.source),
        (network.b, IpAddr::V4(B_ADDRESS))
    );
    assert_eq!(route.device_mtu, MaximumTransmissionUnit(B_MTU));
    let route = Ipv6RouteSelectors {
        interface: Some(network.b),
        ..v6(FAR_V6)
    };
    let route = resolve6(&resolver, route).expect("resolves");
    assert_eq!(
        (route.interface, route.source),
        (network.b, IpAddr::V6(B_ADDRESS_V6))
    );
}

/// A rule on protocol and port sends only that protocol's lookups to that port to the other table.
pub fn protocol_and_ports_select_the_route<R: RouteResolver>(resolver: R) {
    const DNS: u16 = 53;
    const OTHER: u16 = 54;
    const SOURCE_PORT: Port = Port(40000);
    let network = network();
    const RULE: &str = "rule add ipproto udp dport 53 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE);
    let options = Ipv4Options::new();
    let to = |port, protocol| UpperLayer {
        protocol,
        selector: Some(UpperLayerSelector::Ports(TransportPorts {
            source: SOURCE_PORT,
            destination: Port(port),
        })),
    };
    for (upper_layer, expected) in [
        (to(DNS, IpNumber::UDP), network.b),
        (to(OTHER, IpNumber::UDP), network.a),
        (to(DNS, IpNumber::TCP), network.a),
    ] {
        let route = Ipv4RouteSelectors {
            upper_layer,
            ..v4(&options, FAR)
        };
        assert_eq!(
            interface(resolve4(&resolver, route)),
            expected,
            "{upper_layer:?}"
        );
        let route = Ipv6RouteSelectors {
            upper_layer,
            ..v6(FAR_V6)
        };
        assert_eq!(
            interface(resolve6(&resolver, route)),
            expected,
            "{upper_layer:?}"
        );
    }
}

/// The kernel's flow keeps an ICMP type and code where a destination port is, so a port rule selects on them.
pub fn icmp_type_and_code_select_the_route<R: RouteResolver>(resolver: R) {
    const ECHO_REQUEST: u8 = 8;
    const ECHO_REPLY: u8 = 0;
    const ECHO_REQUEST_V6: u8 = 128;
    const ECHO_REPLY_V6: u8 = 129;
    const CODE: IcmpCode = IcmpCode(0);
    let network = network();
    const RULE: &str = "rule add ipproto icmp dport 2048 lookup 100";
    const RULE_V6: &str = "rule add ipproto ipv6-icmp dport 32768 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE_V6);
    let options = Ipv4Options::new();
    let icmp = |protocol, message_type| UpperLayer {
        protocol,
        selector: Some(UpperLayerSelector::Icmp {
            message_type: IcmpType(message_type),
            code: CODE,
        }),
    };
    let route = Ipv4RouteSelectors {
        upper_layer: icmp(IpNumber::ICMP, ECHO_REQUEST),
        ..v4(&options, FAR)
    };
    assert_eq!(interface(resolve4(&resolver, route)), network.b);
    let route = Ipv4RouteSelectors {
        upper_layer: icmp(IpNumber::ICMP, ECHO_REPLY),
        ..v4(&options, FAR)
    };
    assert_eq!(interface(resolve4(&resolver, route)), network.a);
    let route = Ipv6RouteSelectors {
        upper_layer: icmp(IpNumber::IPV6_ICMP, ECHO_REQUEST_V6),
        ..v6(FAR_V6)
    };
    assert_eq!(interface(resolve6(&resolver, route)), network.b);
    let route = Ipv6RouteSelectors {
        upper_layer: icmp(IpNumber::IPV6_ICMP, ECHO_REPLY_V6),
        ..v6(FAR_V6)
    };
    assert_eq!(interface(resolve6(&resolver, route)), network.a);
}

/// A rule on the flow label sends only lookups with that label to the other table.
pub fn flow_label_selects_the_route<R: RouteResolver>(resolver: R) {
    const LABEL: u32 = 0x12345;
    const OTHER_LABEL: u32 = 0x12346;
    let network = network();
    const RULE: &str = "rule add flowlabel 0x12345 lookup 100";
    ip(IPV6, RULE);
    let flow_label = Ipv6FlowLabel::try_new(LABEL).expect("a flow label");
    let route = Ipv6RouteSelectors {
        flow_label,
        ..v6(FAR_V6)
    };
    assert_eq!(interface(resolve6(&resolver, route)), network.b);
    let flow_label = Ipv6FlowLabel::try_new(OTHER_LABEL).expect("a flow label");
    let route = Ipv6RouteSelectors {
        flow_label,
        ..v6(FAR_V6)
    };
    assert_eq!(interface(resolve6(&resolver, route)), network.a);
}

/// A broadcast route is refused with `EACCES` unless broadcasting is allowed.
pub fn broadcast_needs_permission<R: RouteResolver>(resolver: R) {
    const BROADCAST: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 255);
    let network = network();
    let options = Ipv4Options::new();
    let broadcast = BROADCAST;
    assert_eq!(
        resolve4(&resolver, v4(&options, broadcast)),
        Err(HostError::Errno(libc::EACCES))
    );
    let allowed = Ipv4RouteSelectors {
        broadcast: BroadcastPermission::Allowed,
        ..v4(&options, broadcast)
    };
    let route = resolve4(&resolver, allowed).expect("an allowed broadcast resolves");
    assert_eq!(route.kind, RouteKind::Broadcast);
    assert_eq!(
        (route.interface, route.source),
        (network.a, IpAddr::V4(A_ADDRESS))
    );
}

/// A group address is a multicast route.
pub fn multicast_route_is_multicast<R: RouteResolver>(resolver: R) {
    const GROUP: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 3);
    const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 1);
    let network = network();
    const ROUTE: &str = "route add 224.0.0.0/4 dev da";
    ip(IPV4, ROUTE);
    let options = Ipv4Options::new();
    let route = resolve4(&resolver, v4(&options, GROUP)).expect("resolves");
    assert_eq!(
        (route.kind, route.interface),
        (RouteKind::Multicast, network.a)
    );
    let group = Ipv6RouteSelectors {
        interface: Some(network.a),
        ..v6(GROUP_V6)
    };
    let route = resolve6(&resolver, group).expect("resolves");
    assert_eq!(
        (route.kind, route.interface),
        (RouteKind::Multicast, network.a)
    );
}

/// A required source check refuses a source the namespace does not have.
pub fn required_source_check_refuses_a_foreign_address<R: RouteResolver>(resolver: R) {
    network();
    let options = Ipv4Options::new();
    let foreign = Ipv4RouteSelectors {
        source: Some(A_FOREIGN),
        ..v4(&options, FAR)
    };
    assert!(resolve4(&resolver, foreign).is_err());
    let foreign = Ipv6RouteSelectors {
        source: Some(A_FOREIGN_V6),
        ..v6(FAR_V6)
    };
    assert!(resolve6(&resolver, foreign).is_err());
}

/// A waived source check routes from a source the namespace does not have, keeping it as the source.
pub fn waived_source_check_takes_a_foreign_address<R: RouteResolver>(resolver: R) {
    let network = network();
    let options = Ipv4Options::new();
    let source = A_FOREIGN;
    let foreign = Ipv4RouteSelectors {
        source: Some(source),
        source_address_check: SourceAddressCheck::Waived,
        ..v4(&options, FAR)
    };
    unless_refused(resolve4(&resolver, foreign), |route| {
        let route = route.expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (network.a, IpAddr::V4(source))
        );
    });
    let source = A_FOREIGN_V6;
    let foreign = Ipv6RouteSelectors {
        source: Some(source),
        source_address_check: SourceAddressCheck::Waived,
        ..v6(FAR_V6)
    };
    unless_refused(resolve6(&resolver, foreign), |route| {
        let route = route.expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (network.a, IpAddr::V6(source))
        );
    });
}

/// A loose source route is looked up to its first hop.
pub fn loose_source_route_goes_to_its_first_hop<R: RouteResolver>(resolver: R) {
    let network = network();
    let [a, b, c, d] = B_GATEWAY.octets();
    /// The option's type, length and pointer, and one address.
    const OPTION_LENGTH: u8 = 7;
    let options = Ipv4Options::from([
        libc::IPOPT_LSRR,
        OPTION_LENGTH,
        libc::IPOPT_MINOFF,
        a,
        b,
        c,
        d,
        libc::IPOPT_END,
    ]);
    let route = resolve4(&resolver, v4(&options, FAR)).expect("resolves");
    assert_eq!(
        (route.interface, route.source),
        (network.b, IpAddr::V4(B_ADDRESS))
    );
}

/// An IPv6 routing header is looked up to its first segment.
pub fn routing_header_goes_to_its_first_segment<R: RouteResolver>(resolver: R) {
    let network = network();
    /// A type 0 routing header's start: no next header, a length of two eights after the first, one segment left, and reserved bytes.
    const START: [u8; 8] = [IpNumber::IPV6_NO_NEXT_HEADER.0, 2, 0, 1, 0, 0, 0, 0];
    let mut header = START.to_vec();
    header.extend_from_slice(&B_GATEWAY_V6.octets());
    let routing_header = Ipv6RawExtHeaderSlice::from_slice(&header).expect("a routing header");
    let route = Ipv6RouteSelectors {
        routing_header: Some(routing_header),
        ..v6(FAR_V6)
    };
    unless_refused(resolve6(&resolver, route), |route| {
        assert_eq!(interface(route), network.b);
    });
}

/// A link-scope lookup takes a route on the link and refuses one through a gateway.
pub fn link_scope_takes_only_routes_on_the_link<R: RouteResolver>(resolver: R) {
    let network = network();
    let options = Ipv4Options::new();
    let on_link = Ipv4RouteSelectors {
        scope: RouteScope::Link,
        ..v4(&options, A_NEIGHBOUR)
    };
    unless_refused(resolve4(&resolver, on_link), |route| {
        assert_eq!(interface(route), network.a);
    });
    let through_gateway = Ipv4RouteSelectors {
        scope: RouteScope::Link,
        ..v4(&options, FAR)
    };
    unless_refused(resolve4(&resolver, through_gateway), |route| {
        assert!(route.is_err(), "{route:?}");
    });
}

/// A confirmed neighbour changes nothing about the route.
pub fn confirmed_neighbour_keeps_the_route<R: RouteResolver>(resolver: R) {
    network();
    let options = Ipv4Options::new();
    let unconfirmed = resolve4(&resolver, v4(&options, FAR));
    let confirmed = resolver.resolve_route(
        RouteSelectors::V4(v4(&options, FAR)),
        ROOT,
        NeighbourConfirmation::Confirmed,
    );
    unless_refused(confirmed, |confirmed| assert_eq!(confirmed, unconfirmed));
}

/// Source preferences choose among the link's addresses, here only one.
pub fn source_preferences_choose_among_the_link_addresses<R: RouteResolver>(resolver: R) {
    let network = network();
    for temporary in [
        TemporaryAddressPreference::Temporary,
        TemporaryAddressPreference::Public,
    ] {
        let route = Ipv6RouteSelectors {
            source_preferences: SourcePreferences {
                temporary,
                mobile: MobileAddressPreference::CareOf,
            },
            ..v6(FAR_V6)
        };
        unless_refused(resolve6(&resolver, route), |route| {
            let route = route.expect("resolves");
            assert_eq!(
                (route.interface, route.source),
                (network.a, IpAddr::V6(A_ADDRESS_V6))
            );
        });
    }
}

/// A protocol without ports routes as any other.
pub fn other_protocols_route<R: RouteResolver>(resolver: R) {
    let network = network();
    let options = Ipv4Options::new();
    let gre = UpperLayer {
        protocol: IpNumber::GRE,
        selector: None,
    };
    let route = Ipv4RouteSelectors {
        upper_layer: gre,
        ..v4(&options, FAR)
    };
    unless_refused(resolve4(&resolver, route), |route| {
        assert_eq!(interface(route), network.a);
    });
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test in a network namespace of its own.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        $crate::each_check!(
            $value;
            loopback_route_is_local,
            ipv4_route_gives_link_source_and_sizes,
            ipv6_route_gives_link_source_and_sizes,
            route_metrics_give_path_mtu_and_hop_limit,
            failing_routes_give_their_errors,
            mark_selects_the_route,
            owner_selects_the_route,
            dscp_selects_the_route,
            source_selects_the_route,
            interface_selects_the_route,
            protocol_and_ports_select_the_route,
            icmp_type_and_code_select_the_route,
            flow_label_selects_the_route,
            broadcast_needs_permission,
            multicast_route_is_multicast,
            required_source_check_refuses_a_foreign_address,
            waived_source_check_takes_a_foreign_address,
            loose_source_route_goes_to_its_first_hop,
            routing_header_goes_to_its_first_segment,
            link_scope_takes_only_routes_on_the_link,
            confirmed_neighbour_keeps_the_route,
            source_preferences_choose_among_the_link_addresses,
            other_protocols_route,
        );
    };
}

/// One test per named check, each entering its own network namespace before the value is made.
#[doc(hidden)]
#[macro_export]
macro_rules! each_check {
    ($value:expr; $($check:ident),* $(,)?) => {
        $(
            #[test]
            fn $check() {
                $crate::enter_network_namespace();
                $crate::$check($value);
            }
        )*
    };
}
