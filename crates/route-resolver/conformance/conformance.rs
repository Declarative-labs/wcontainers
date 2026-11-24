//! The tests every implementation of wcontainers-route-resolver runs, written against its traits alone.
//!
//! Each runs as root in a network namespace of its own, on links, routes and rules it builds there with
//! iproute2's `ip`, and asks the value made in that namespace about them.

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use core::num::NonZeroU32;
use std::fs;
use std::process::Command;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options, Ipv6FlowLabel, Ipv6RawExtHeaderSlice};
use linux_raw_sys::net::{
    IPOPT_END, IPOPT_LSRR, IPOPT_MINOFF, IPOPT_SSRR, IPV6_SRCRT_TYPE_0, IPV6_SRCRT_TYPE_2,
    IPV6_SRCRT_TYPE_4,
};
use wcontainers_abi::HostError;
use wcontainers_icmp_sockets_conformance::{
    DEVICE, IP, PACKETS, TRAFFIC_CONTROL, batch, number_after, seconds_since_confirmed,
    temporary_address,
};
use wcontainers_internet_protocol::{
    BroadcastPermission, HopLimit, IcmpCode, IcmpType, InterfaceIndex, InterfaceSelector,
    Ipv4RouteSelectors, Ipv6RouteSelectors, MaximumTransmissionUnit, MobileAddressPreference,
    MobilityHeaderType, NeighbourConfirmation, PacketMark, Port, Route, RouteKind, RouteScope,
    RouteSelectors, SourceAddressCheck, SourcePreferences, TemporaryAddressPreference,
    TransportPorts, UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_route_resolver::RouteResolver;
#[cfg(target_os = "linux")]
use wcontainers_socket_options::{PlainOption, SocketOptions};
#[cfg(target_os = "linux")]
use wcontainers_socket_options_linux::LinuxSocketOptions;

pub use wcontainers_icmp_sockets_conformance as icmp_suite;

/// Moves the calling thread into a new, empty network namespace, which needs `CAP_SYS_ADMIN`.
fn enter_network_namespace() {
    #[cfg(target_os = "linux")]
    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET)
        .expect("a new network namespace needs root");
    #[cfg(not(target_os = "linux"))]
    panic!("the checks build their routes in a Linux network namespace");
}

const A: &str = "da";
const B: &str = "db";
/// The MTU of da, more than the least IPv6 takes.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-5
const A_MTU: u32 = 1400;
/// The MTU of db, less than da's and more than the least IPv6 takes.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-5
const B_MTU: u32 = 1310;
/// The MTU Linux gives loopback in a new network namespace.
///
/// https://github.com/torvalds/linux/blob/v6.18/drivers/net/loopback.c
const LOOPBACK_DEVICE_MTU: u32 = 64 * 1024;
/// Two dummy links, da and db, with routes to `FAR` through a gateway on each: the main table's through da, table 100's through db.
const NETWORK: &str = "link set lo up
link add da type dummy
link set da up mtu 1400
address add 10.1.0.1/24 dev da
address add fd01::1/64 dev da nodad
link add db type dummy
link set db up mtu 1310
address add 10.2.0.1/24 dev db
address add fd02::1/64 dev db nodad
route add 10.9.0.0/16 via 10.1.0.2
route add 10.9.0.0/16 via 10.2.0.2 table 100
route add fd09::/64 via fd01::2
route add fd09::/64 via fd02::2 table 100
";
/// The address of da, in private address space.
///
/// https://www.rfc-editor.org/rfc/rfc1918#section-3
const A_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 1);
/// The address of db, in private address space.
///
/// https://www.rfc-editor.org/rfc/rfc1918#section-3
const B_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 1);
/// The IPv6 address of da, a unique local one.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const A_ADDRESS_V6: Ipv6Addr = Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 1);
/// The IPv6 address of db, a unique local one.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const B_ADDRESS_V6: Ipv6Addr = Ipv6Addr::new(0xfd02, 0, 0, 0, 0, 0, 0, 1);
/// An address on da's link that the namespace does not have.
///
/// https://www.rfc-editor.org/rfc/rfc1918#section-3
const A_NEIGHBOUR: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 5);
/// Another address on da's link that the namespace does not have.
///
/// https://www.rfc-editor.org/rfc/rfc1918#section-3
const A_FOREIGN: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 99);
/// An IPv6 address on da's link that the namespace does not have.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const A_FOREIGN_V6: Ipv6Addr = Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 0x99);
/// The gateway of `FAR` on db.
///
/// https://www.rfc-editor.org/rfc/rfc1918#section-3
const B_GATEWAY: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 2);
/// The gateway of `FAR_V6` on db.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const B_GATEWAY_V6: Ipv6Addr = Ipv6Addr::new(0xfd02, 0, 0, 0, 0, 0, 0, 2);
/// An address beyond the gateways.
///
/// https://www.rfc-editor.org/rfc/rfc1918#section-3
const FAR: Ipv4Addr = Ipv4Addr::new(10, 9, 1, 1);
/// An IPv6 address beyond the gateways.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const FAR_V6: Ipv6Addr = Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 1);
/// The superuser.
///
/// https://man7.org/linux/man-pages/man7/credentials.7.html
const ROOT: UserId = UserId(0);

struct Network {
    a: InterfaceIndex,
    b: InterfaceIndex,
}

/// Runs each line of `commands` as one `ip` command with these options.
fn ip(options: &[&str], commands: &str) {
    batch(IP, options, commands);
}

fn write_setting(path: &str, value: impl ToString) {
    fs::write(path, value.to_string()).unwrap_or_else(|error| panic!("{path}: {error}"));
}

fn index(name: &str) -> InterfaceIndex {
    let index = nix::net::if_::if_nametoindex(name)
        .unwrap_or_else(|error| panic!("no link {name}: {error}"));
    InterfaceIndex(NonZeroU32::new(index).unwrap_or_else(|| panic!("no link {name}")))
}

/// Builds the two links, their addresses and routes in a network namespace of the calling thread's own.
fn network() -> Network {
    enter_network_namespace();
    ip(IPV4, NETWORK);
    Network {
        a: index(A),
        b: index(B),
    }
}

/// No family option, which makes a rule with no address an IPv4 one.
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

/// Checks that an address of lo is a local route on lo from that address, with the path MTU a socket connected to it is told.
pub fn loopback_route_is_local<R: RouteResolver>(resolver: R) {
    let network = network();
    let options = Ipv4Options::new();
    let routes = [
        resolve4(&resolver, v4(&options, Ipv4Addr::LOCALHOST)),
        resolve6(&resolver, v6(Ipv6Addr::LOCALHOST)),
    ];
    for (route, address) in routes.into_iter().zip([
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ]) {
        let route = route.expect("loopback resolves");
        assert_eq!(route.kind, RouteKind::Local);
        assert!(![network.a, network.b].contains(&route.interface));
        assert_eq!(route.source, address);
        assert_eq!(
            route.device_mtu,
            MaximumTransmissionUnit(LOOPBACK_DEVICE_MTU)
        );
        assert_eq!(
            route.path_mtu,
            MaximumTransmissionUnit(connected_path_mtu(address, DISCARD)),
            "{address}"
        );
    }
}

/// Checks that the unspecified destination routes as loopback's address, where Linux sends a datagram to it.
pub fn unspecified_destination_is_loopback<R: RouteResolver>(resolver: R) {
    network();
    let options = Ipv4Options::new();
    let routes = [
        (
            resolve4(&resolver, v4(&options, Ipv4Addr::UNSPECIFIED)),
            resolve4(&resolver, v4(&options, Ipv4Addr::LOCALHOST)),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        ),
        (
            resolve6(&resolver, v6(Ipv6Addr::UNSPECIFIED)),
            resolve6(&resolver, v6(Ipv6Addr::LOCALHOST)),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        ),
    ];
    for (unspecified, loopback, address) in routes {
        let route = unspecified.expect("the unspecified address resolves");
        assert_eq!(route, loopback.expect("loopback resolves"), "{address}");
        assert_eq!(
            (route.source, route.path_mtu),
            connected_source_and_path_mtu(address, DISCARD),
            "{address}"
        );
    }
}

/// The namespace's default IPv4 TTL.
const DEFAULT_TTL: &str = "/proc/sys/net/ipv4/ip_default_ttl";
/// Checks that a route through a gateway gives the link's interface, its address as the source, its MTU and the namespace's TTL.
pub fn ipv4_route_gives_link_source_and_sizes<R: RouteResolver>(resolver: R) {
    /// A TTL other than Linux's default.
    ///
    /// https://www.kernel.org/doc/html/latest/networking/ip-sysctl.html
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

/// Checks that an IPv6 route gives the link's IPv6 MTU as the path MTU and the link's IPv6 hop limit.
pub fn ipv6_route_gives_link_source_and_sizes<R: RouteResolver>(resolver: R) {
    const A_IPV6_MTU: &str = "/proc/sys/net/ipv6/conf/da/mtu";
    const A_HOP_LIMIT: &str = "/proc/sys/net/ipv6/conf/da/hop_limit";
    /// An IPv6 MTU of da less than its link's.
    ///
    /// https://www.kernel.org/doc/html/latest/networking/ip-sysctl.html
    const IPV6_MTU: MaximumTransmissionUnit = MaximumTransmissionUnit(1350);
    /// A hop limit other than Linux's default.
    ///
    /// https://www.kernel.org/doc/html/latest/networking/ip-sysctl.html
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

/// Checks that a route's own MTU and hop limit are the path MTU and default hop limit, the device's MTU staying its own.
pub fn route_metrics_give_path_mtu_and_hop_limit<R: RouteResolver>(resolver: R) {
    /// An address the check routes with metrics of its own.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const DESTINATION: Ipv4Addr = Ipv4Addr::new(10, 8, 0, 1);
    /// An IPv6 address the check routes with metrics of its own.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const DESTINATION_V6: Ipv6Addr = Ipv6Addr::new(0xfd08, 0, 0, 0, 0, 0, 0, 1);
    /// A route MTU less than da's.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-route.8.html
    const ROUTE_MTU: MaximumTransmissionUnit = MaximumTransmissionUnit(1280);
    /// An IPv6 route MTU less than da's.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-route.8.html
    const ROUTE_MTU_V6: MaximumTransmissionUnit = MaximumTransmissionUnit(1290);
    /// A route's hop limit.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-route.8.html
    const ROUTE_HOP_LIMIT: HopLimit = HopLimit(9);
    /// An IPv6 route's hop limit.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-route.8.html
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

/// Checks that a route whose MTU is locked with no value takes the device's MTU, capped through a gateway at the size every IPv4 host reassembles.
pub fn locked_mtu_without_value_uses_the_device_mtu<R: RouteResolver>(resolver: R) {
    /// [RFC 791, section 3.1](https://www.rfc-editor.org/rfc/rfc791#section-3.1).
    const MINIMUM_REASSEMBLY: MaximumTransmissionUnit = MaximumTransmissionUnit(576);
    /// An address routed through a gateway with a locked MTU.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const THROUGH_GATEWAY: Ipv4Addr = Ipv4Addr::new(10, 7, 0, 1);
    /// An address routed on the link with a locked MTU.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const ON_LINK: Ipv4Addr = Ipv4Addr::new(10, 6, 0, 1);
    /// An address routed through an IPv6 gateway with a locked MTU.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const THROUGH_IPV6_GATEWAY: Ipv4Addr = Ipv4Addr::new(10, 21, 0, 1);
    network();
    const ROUTES: &str = "route add 10.7.0.0/16 via 10.1.0.2 mtu lock 0
route add 10.6.0.0/16 dev da mtu lock 0
route add 10.21.0.0/16 via inet6 fd01::2 dev da mtu lock 0";
    ip(IPV4, ROUTES);
    let options = Ipv4Options::new();
    for (destination, path_mtu) in [
        (THROUGH_GATEWAY, MINIMUM_REASSEMBLY),
        (ON_LINK, MaximumTransmissionUnit(A_MTU)),
        (THROUGH_IPV6_GATEWAY, MINIMUM_REASSEMBLY),
    ] {
        let route = resolve4(&resolver, v4(&options, destination)).expect("resolves");
        assert_eq!(route.path_mtu, path_mtu, "{destination}");
        assert_eq!(route.device_mtu, MaximumTransmissionUnit(A_MTU));
    }
}

/// Checks that an encapsulating route's path MTU is what a socket connected through it is told, IPv4 and IPv6.
pub fn encapsulating_route_leaves_room_for_its_headers<R: RouteResolver>(resolver: R) {
    /// An address behind the encapsulating route.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const DESTINATION: Ipv4Addr = Ipv4Addr::new(10, 5, 0, 1);
    /// An IPv6 address behind the encapsulating route.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const DESTINATION_V6: Ipv6Addr = Ipv6Addr::new(0xfd05, 0, 0, 0, 0, 0, 0, 1);
    network();
    const ROUTE: &str = "route add 10.5.0.0/16 encap seg6 mode encap segs fd01::7 dev da";
    const ROUTE_V6: &str = "route add fd05::/64 encap seg6 mode encap segs fd01::7 dev da";
    ip(IPV4, ROUTE);
    ip(IPV6, ROUTE_V6);
    let options = Ipv4Options::new();
    for (route, destination) in [
        (
            resolve4(&resolver, v4(&options, DESTINATION)),
            IpAddr::V4(DESTINATION),
        ),
        (
            resolve6(&resolver, v6(DESTINATION_V6)),
            IpAddr::V6(DESTINATION_V6),
        ),
    ] {
        let told = connected_path_mtu(destination, DISCARD);
        assert!(told < A_MTU, "{destination}: {told}");
        assert_eq!(
            route.expect("resolves").path_mtu,
            MaximumTransmissionUnit(told),
            "{destination}"
        );
    }
}

/// Checks that a rule on the DSCP takes lookups to an encapsulating route, whose path MTU a socket with that DSCP is told.
pub fn dscp_rule_takes_an_encapsulating_route<R: RouteResolver>(resolver: R) {
    /// A DSCP the rule selects, and the traffic class a socket sends it in.
    const DSCP: IpDscp = IpDscp::CS1;
    /// An address behind an encapsulating route in table 100 and a plain one in main.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const DESTINATION: Ipv4Addr = Ipv4Addr::new(10, 5, 0, 1);
    /// An IPv6 address behind an encapsulating route in table 100 and a plain one in main.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const DESTINATION_V6: Ipv6Addr = Ipv6Addr::new(0xfd05, 0, 0, 0, 0, 0, 0, 1);
    let network = network();
    const ROUTES: &str = "route add 10.5.0.0/16 dev da
route add 10.5.0.0/16 encap seg6 mode encap segs fd02::7 dev db table 100
rule add dscp 8 lookup 100";
    const ROUTES_V6: &str = "route add fd05::/64 dev da
route add fd05::/64 encap seg6 mode encap segs fd02::7 dev db table 100
rule add dscp 8 lookup 100";
    ip(IPV4, ROUTES);
    ip(IPV6, ROUTES_V6);
    let options = Ipv4Options::new();
    let ecn = IpEcn::Ect0;
    let traffic_class = (DSCP.value() << IpEcn::MAX_U8.count_ones()) | ecn.value();
    for (marked, plain, destination) in [
        (
            resolve4(
                &resolver,
                Ipv4RouteSelectors {
                    dscp: DSCP,
                    ecn,
                    ..v4(&options, DESTINATION)
                },
            ),
            resolve4(&resolver, v4(&options, DESTINATION)),
            IpAddr::V4(DESTINATION),
        ),
        (
            resolve6(
                &resolver,
                Ipv6RouteSelectors {
                    dscp: DSCP,
                    ecn,
                    ..v6(DESTINATION_V6)
                },
            ),
            resolve6(&resolver, v6(DESTINATION_V6)),
            IpAddr::V6(DESTINATION_V6),
        ),
    ] {
        let told = told_source_and_path_mtu(destination, DISCARD, traffic_class).1;
        assert!(told.0 < B_MTU, "{destination}: {told:?}");
        let marked = marked.expect("resolves");
        assert_eq!(
            (marked.interface, marked.path_mtu),
            (network.b, told),
            "{destination}"
        );
        let plain = plain.expect("resolves");
        assert_eq!(
            (plain.interface, plain.path_mtu),
            (network.a, MaximumTransmissionUnit(A_MTU)),
            "{destination}"
        );
    }
}

/// The discard port ([RFC 863](https://www.rfc-editor.org/rfc/rfc863)); connecting sends nothing.
const DISCARD: u16 = 9;

/// Returns the path MTU Linux tells a datagram socket connected to the address.
fn connected_path_mtu(address: IpAddr, port: u16) -> u32 {
    connected_source_and_path_mtu(address, port).1.0
}

/// Returns the source Linux gives a datagram socket connected to the address, and the path MTU it tells it.
fn connected_source_and_path_mtu(address: IpAddr, port: u16) -> (IpAddr, MaximumTransmissionUnit) {
    told_source_and_path_mtu(address, port, u8::MIN)
}

/// Returns the source and path MTU Linux gives a datagram socket of that traffic class connected to the address.
fn told_source_and_path_mtu(
    address: IpAddr,
    port: u16,
    traffic_class: u8,
) -> (IpAddr, MaximumTransmissionUnit) {
    let any = match address {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    let socket = std::net::UdpSocket::bind((any, u16::MIN)).expect("a socket opens");
    #[cfg(target_os = "linux")]
    match address {
        IpAddr::V4(_) => rustix::net::sockopt::set_ip_tos(&socket, traffic_class),
        IpAddr::V6(_) => rustix::net::sockopt::set_ipv6_tclass(&socket, u32::from(traffic_class)),
    }
    .expect("a socket takes a traffic class");
    #[cfg(not(target_os = "linux"))]
    let _ = traffic_class;
    socket.connect((address, port)).expect("connects");
    let source = socket
        .local_addr()
        .expect("a connected socket's source")
        .ip();
    #[cfg(target_os = "linux")]
    return (
        source,
        MaximumTransmissionUnit(
            match address {
                IpAddr::V4(_) => rustix::net::sockopt::ip_mtu(&socket),
                IpAddr::V6(_) => rustix::net::sockopt::ipv6_mtu(&socket),
            }
            .expect("a connected socket's path MTU"),
        ),
    );
    #[cfg(not(target_os = "linux"))]
    panic!("only Linux tells a connected socket's path MTU, not {source}");
}

/// Checks that no route, a prohibited, a blackholed and an unreachable one fail with the kernel's errors.
pub fn failing_routes_give_their_errors<R: RouteResolver>(resolver: R) {
    /// An address no route takes.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const UNROUTED: Ipv4Addr = Ipv4Addr::new(10, 200, 0, 1);
    /// An address of a prohibited route.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const PROHIBITED: Ipv4Addr = Ipv4Addr::new(10, 50, 0, 1);
    /// An address of a blackhole route.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const BLACKHOLED: Ipv4Addr = Ipv4Addr::new(10, 51, 0, 1);
    /// An address of an unreachable route.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const UNREACHABLE: Ipv4Addr = Ipv4Addr::new(10, 52, 0, 1);
    /// An IPv6 address no route takes.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
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

/// Checks that a rule on the mark sends only marked lookups to the other table.
pub fn mark_selects_the_route<R: RouteResolver>(resolver: R) {
    /// A mark the rule selects.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-rule.8.html
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

/// Checks that a rule on a user sends only that owner's lookups to the other table.
pub fn owner_selects_the_route<R: RouteResolver>(resolver: R) {
    /// A user the rule selects.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-rule.8.html
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

/// Checks that a rule on the DSCP sends only lookups with that DSCP, whatever their ECN, to the other table.
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

/// Checks that a rule on the source sends lookups from that address to the other table, and the route keeps the source.
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

/// Checks that a rule on the outgoing interface sends lookups bound to it or asking for it to the other table.
pub fn interface_selects_the_route<R: RouteResolver>(resolver: R) {
    let network = network();
    const RULE: &str = "rule add oif db lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE);
    let options = Ipv4Options::new();
    for selector in [
        InterfaceSelector::Bound(network.b),
        InterfaceSelector::Requested(network.b),
    ] {
        let route = Ipv4RouteSelectors {
            interface: Some(selector),
            ..v4(&options, FAR)
        };
        let route = resolve4(&resolver, route).expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (network.b, IpAddr::V4(B_ADDRESS)),
            "{selector:?}"
        );
        assert_eq!(route.device_mtu, MaximumTransmissionUnit(B_MTU));
        let route = Ipv6RouteSelectors {
            interface: Some(selector),
            ..v6(FAR_V6)
        };
        let route = resolve6(&resolver, route).expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (network.b, IpAddr::V6(B_ADDRESS_V6)),
            "{selector:?}"
        );
    }
}

/// Checks that a rule on protocol and port sends only that protocol's lookups to that port to the other table.
pub fn protocol_and_ports_select_the_route<R: RouteResolver>(resolver: R) {
    /// The DNS port, which the rule selects.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1035#section-4.2
    const DNS: u16 = 53;
    /// A port the rule does not select.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6335#section-6
    const OTHER: u16 = 54;
    /// A source port in the registered range.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6335#section-6
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

/// Checks that a rule on the source port sends only lookups from that port to the other table.
pub fn source_port_selects_the_route<R: RouteResolver>(resolver: R) {
    /// A source port in the registered range, which the rule selects.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6335#section-6
    const SOURCE_PORT: u16 = 40000;
    /// A source port the rule does not select.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6335#section-6
    const OTHER: u16 = 40001;
    /// The DNS port.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1035#section-4.2
    const DESTINATION_PORT: Port = Port(53);
    let network = network();
    const RULE: &str = "rule add ipproto udp sport 40000 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE);
    let options = Ipv4Options::new();
    for (port, expected) in [(SOURCE_PORT, network.b), (OTHER, network.a)] {
        let upper_layer = UpperLayer {
            protocol: IpNumber::UDP,
            selector: Some(UpperLayerSelector::Ports(TransportPorts {
                source: Port(port),
                destination: DESTINATION_PORT,
            })),
        };
        let route = Ipv4RouteSelectors {
            upper_layer,
            ..v4(&options, FAR)
        };
        assert_eq!(interface(resolve4(&resolver, route)), expected, "{port}");
        let route = Ipv6RouteSelectors {
            upper_layer,
            ..v6(FAR_V6)
        };
        assert_eq!(interface(resolve6(&resolver, route)), expected, "{port}");
    }
}

/// Checks that the kernel's flow keeps an ICMP type and code where a destination port is, so a port rule selects on them.
pub fn icmp_type_and_code_select_the_route<R: RouteResolver>(resolver: R) {
    /// The ICMP echo request's type.
    ///
    /// https://www.rfc-editor.org/rfc/rfc792
    const ECHO_REQUEST: u8 = 8;
    /// The ICMPv6 echo request's type.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4443#section-4.1
    const ECHO_REQUEST_V6: u8 = 128;
    /// A code an echo request does not carry, so that the rule matches the code too.
    ///
    /// https://www.rfc-editor.org/rfc/rfc792
    const CODE: u8 = 3;
    let network = network();
    /// Type 8 and code 3, and type 128 and code 3, as the port the flow keeps them in.
    const RULE: &str = "rule add ipproto icmp dport 2051 lookup 100";
    const RULE_V6: &str = "rule add ipproto ipv6-icmp dport 32771 lookup 100";
    ip(IPV4, RULE);
    ip(IPV6, RULE_V6);
    let options = Ipv4Options::new();
    let icmp = |protocol, message_type, code| UpperLayer {
        protocol,
        selector: Some(UpperLayerSelector::Icmp {
            message_type: IcmpType(message_type),
            code: IcmpCode(code),
        }),
    };
    for (message_type, code, expected) in [
        (ECHO_REQUEST, CODE, network.b),
        (ECHO_REQUEST, u8::MIN, network.a),
        (CODE, ECHO_REQUEST, network.a),
    ] {
        let route = Ipv4RouteSelectors {
            upper_layer: icmp(IpNumber::ICMP, message_type, code),
            ..v4(&options, FAR)
        };
        assert_eq!(
            interface(resolve4(&resolver, route)),
            expected,
            "{message_type} {code}"
        );
    }
    for (message_type, code, expected) in [
        (ECHO_REQUEST_V6, CODE, network.b),
        (ECHO_REQUEST_V6, u8::MIN, network.a),
        (CODE, ECHO_REQUEST_V6, network.a),
    ] {
        let route = Ipv6RouteSelectors {
            upper_layer: icmp(IpNumber::IPV6_ICMP, message_type, code),
            ..v6(FAR_V6)
        };
        assert_eq!(
            interface(resolve6(&resolver, route)),
            expected,
            "{message_type} {code}"
        );
    }
}

/// Checks that the kernel's flow keeps a mobility header's type in the first byte of a destination port, so a port rule selects on it.
pub fn mobility_header_type_selects_the_route<R: RouteResolver>(resolver: R) {
    /// The binding update's mobility header type.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6275#section-6.1
    const BINDING_UPDATE: u8 = 5;
    /// The binding acknowledgement's mobility header type.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6275#section-6.1
    const BINDING_ACKNOWLEDGEMENT: u8 = 6;
    /// The binding error's mobility header type.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6275#section-6.1
    const BINDING_ERROR: u8 = 7;
    let network = network();
    /// Type 5 as the port the flow keeps it in, for any protocol.
    const PORT_RULE: &str = "rule add dport 1280 lookup 100";
    /// Type 6 as the port the flow keeps it in, for the mobility header alone.
    const PROTOCOL_RULE: &str = "rule add ipproto 135 dport 1536 lookup 100";
    let check = |refusable: bool, cases: &[(u8, InterfaceIndex)]| {
        for &(message_type, expected) in cases {
            let route = Ipv6RouteSelectors {
                upper_layer: UpperLayer {
                    protocol: IpNumber::MOBILITY_HEADER,
                    selector: Some(UpperLayerSelector::MobilityHeader(MobilityHeaderType(
                        message_type,
                    ))),
                },
                ..v6(FAR_V6)
            };
            let route = resolve6(&resolver, route);
            if refusable {
                unless_refused(route, |route| {
                    assert_eq!(interface(route), expected, "{message_type}");
                });
            } else {
                assert_eq!(interface(route), expected, "{message_type}");
            }
        }
    };
    ip(IPV6, PORT_RULE);
    check(
        false,
        &[
            (BINDING_UPDATE, network.b),
            (BINDING_ACKNOWLEDGEMENT, network.a),
            (BINDING_ERROR, network.a),
        ],
    );
    ip(IPV6, PROTOCOL_RULE);
    check(
        true,
        &[
            (BINDING_UPDATE, network.b),
            (BINDING_ACKNOWLEDGEMENT, network.b),
            (BINDING_ERROR, network.a),
        ],
    );
}

/// A flow label a rule selects.
///
/// https://www.rfc-editor.org/rfc/rfc6437#section-2
pub const LABEL: u32 = 0x12345;

/// Checks that a rule on the flow label sends only lookups with that label to the other table.
pub fn flow_label_selects_the_route<R: RouteResolver>(resolver: R) {
    /// A flow label the rule does not select.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6437#section-2
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

/// Checks that a broadcast route is refused with `EACCES` unless broadcasting is allowed.
pub fn broadcast_needs_permission<R: RouteResolver>(resolver: R) {
    /// The broadcast address of da's subnet.
    ///
    /// https://www.rfc-editor.org/rfc/rfc919#section-7
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

/// Checks that a group address is a multicast route.
pub fn multicast_route_is_multicast<R: RouteResolver>(resolver: R) {
    /// An administratively scoped group.
    ///
    /// https://www.rfc-editor.org/rfc/rfc2365#section-6
    const GROUP: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 3);
    /// A site-local group.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4291#section-2.7
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
        interface: Some(InterfaceSelector::Requested(network.a)),
        ..v6(GROUP_V6)
    };
    let route = resolve6(&resolver, group).expect("resolves");
    assert_eq!(
        (route.kind, route.interface),
        (RouteKind::Multicast, network.a)
    );
}

/// Checks that a required source check refuses a source the namespace does not have, with the host's error rather than a refusal.
pub fn required_source_check_refuses_a_foreign_address<R: RouteResolver>(resolver: R) {
    network();
    let options = Ipv4Options::new();
    let foreign = Ipv4RouteSelectors {
        source: Some(A_FOREIGN),
        ..v4(&options, FAR)
    };
    let result = resolve4(&resolver, foreign);
    assert!(matches!(result, Err(HostError::Errno(_))), "{result:?}");
    let foreign = Ipv6RouteSelectors {
        source: Some(A_FOREIGN_V6),
        ..v6(FAR_V6)
    };
    let result = resolve6(&resolver, foreign);
    assert!(matches!(result, Err(HostError::Errno(_))), "{result:?}");
}

/// The forwarding switches of every link and of da, which make da's subnet-router anycast address a route of its own.
const FORWARDING_V6: &[&str] = &[
    "/proc/sys/net/ipv6/conf/all/forwarding",
    "/proc/sys/net/ipv6/conf/da/forwarding",
];
/// The subnet-router anycast address of da's prefix ([RFC 4291](https://www.rfc-editor.org/rfc/rfc4291#section-2.6.1)).
const A_SUBNET_ROUTER: Ipv6Addr = Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 0);

/// Checks that an anycast route, a kind the contract has no name for, is refused as not supported or given as the local delivery Linux makes of it.
pub fn an_anycast_route_is_not_given_as_another_kind<R: RouteResolver>(resolver: R) {
    network();
    for path in FORWARDING_V6 {
        write_setting(path, u8::from(true));
    }
    let result = resolve6(&resolver, v6(A_SUBNET_ROUTER));
    assert!(
        result.as_ref().map_or_else(
            |error| *error == HostError::NotSupported,
            |route| route.kind == RouteKind::Local
        ),
        "{result:?}"
    );
}

/// Checks that a link-local destination with no interface takes a route, as a datagram Linux sends to it does.
pub fn a_link_local_destination_without_an_interface_routes<R: RouteResolver>(resolver: R) {
    /// A link-local address.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4291#section-2.5.6
    const LINK_LOCAL: Ipv6Addr = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);
    let network = network();
    let route = resolve6(&resolver, v6(LINK_LOCAL)).expect("a link-local destination resolves");
    assert_eq!(route.kind, RouteKind::Unicast);
    assert!(
        [network.a, network.b].contains(&route.interface),
        "{route:?}"
    );
    assert!(
        matches!(route.source, IpAddr::V6(source) if source.is_unicast_link_local()),
        "{route:?}"
    );
}

/// Checks that a waived source check routes from a source the namespace does not have, keeping it as the source.
pub fn waived_source_check_takes_a_foreign_address<R: RouteResolver>(resolver: R) {
    let network = network();
    for nonlocal_bind in [false, true] {
        write_setting(NONLOCAL_BIND, u8::from(nonlocal_bind));
        waived_source_check(&resolver, &network);
    }
}

/// Lets `bind` take an address the namespace lacks, which waives no route lookup's source check.
const NONLOCAL_BIND: &str = "/proc/sys/net/ipv4/ip_nonlocal_bind";

fn waived_source_check<R: RouteResolver>(resolver: &R, network: &Network) {
    let options = Ipv4Options::new();
    let source = A_FOREIGN;
    let foreign = Ipv4RouteSelectors {
        source: Some(source),
        source_address_check: SourceAddressCheck::Waived,
        ..v4(&options, FAR)
    };
    unless_refused(resolve4(resolver, foreign), |route| {
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
    unless_refused(resolve6(resolver, foreign), |route| {
        let route = route.expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (network.a, IpAddr::V6(source))
        );
    });
}

/// The length of a source route option of one address: its type, length and pointer, and the address.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const SOURCE_ROUTE_LENGTH: u8 = 7;

/// Returns IPv4 options of one source route of the given kind, through the address.
fn source_route(kind: u32, first_hop: Ipv4Addr) -> Ipv4Options {
    let byte = |value: u32| u8::try_from(value).expect("a byte of an option");
    let [a, b, c, d] = first_hop.octets();
    Ipv4Options::from([
        byte(kind),
        SOURCE_ROUTE_LENGTH,
        byte(IPOPT_MINOFF),
        a,
        b,
        c,
        d,
        byte(IPOPT_END),
    ])
}

/// Checks that a loose source route is looked up to its first hop.
pub fn loose_source_route_goes_to_its_first_hop<R: RouteResolver>(resolver: R) {
    let network = network();
    let options = source_route(IPOPT_LSRR, B_GATEWAY);
    let route = resolve4(&resolver, v4(&options, FAR)).expect("resolves");
    assert_eq!(
        (route.interface, route.source),
        (network.b, IpAddr::V4(B_ADDRESS))
    );
}

/// Checks that a strict source route takes only a first hop on a link, and is looked up to it.
pub fn strict_source_route_takes_only_a_first_hop_on_the_link<R: RouteResolver>(resolver: R) {
    let network = network();
    let strict = |first_hop| source_route(IPOPT_SSRR, first_hop);
    let on_link = strict(B_GATEWAY);
    unless_refused(resolve4(&resolver, v4(&on_link, FAR)), |route| {
        let route = route.expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (network.b, IpAddr::V4(B_ADDRESS))
        );
    });
    let through_gateway = strict(FAR);
    unless_refused(
        resolve4(&resolver, v4(&through_gateway, B_GATEWAY)),
        |route| assert!(route.is_err(), "{route:?}"),
    );
}

/// Where a routing header's addresses start, after the same eight bytes for each type.
const FIRST_ADDRESS: usize = core::mem::offset_of!(linux_raw_sys::net::rt0_hdr, addr);

/// Checks that a type 2 header is looked up to its address and a type 4 one to its next segment; a type 0 one is refused as `IPV6_RTHDR` refuses it.
pub fn routing_header_goes_to_its_first_hop<R: RouteResolver>(resolver: R) {
    let network = network();
    let header = |kind: u32, segments_left: usize, addresses: &[Ipv6Addr]| {
        let byte = |value: usize| u8::try_from(value).expect("a byte of a routing header");
        // The length counts eight-byte units after the first eight, two for each address.
        let length = byte(addresses.len() * 2);
        let last_entry = byte(addresses.len() - 1);
        let kind = u8::try_from(kind).expect("a routing type");
        let mut header = vec![
            IpNumber::IPV6_NO_NEXT_HEADER.0,
            length,
            kind,
            byte(segments_left),
            last_entry,
        ];
        header.resize(FIRST_ADDRESS, u8::MIN);
        for address in addresses {
            header.extend_from_slice(&address.octets());
        }
        header
    };
    // A type 2 header holds one segment left, its address; a type 4 one goes to the entry its segments left names, here its middle one.
    let home = [B_GATEWAY_V6];
    let segments = [FAR_V6, B_GATEWAY_V6, A_FOREIGN_V6];
    let home = header(IPV6_SRCRT_TYPE_2, home.len(), &home);
    let segments = header(IPV6_SRCRT_TYPE_4, segments.len() - 2, &segments);
    for header in [home, segments] {
        let routing_header = Ipv6RawExtHeaderSlice::from_slice(&header).expect("a routing header");
        let route = Ipv6RouteSelectors {
            routing_header: Some(routing_header),
            ..v6(FAR_V6)
        };
        let route = resolve6(&resolver, route).expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (network.b, IpAddr::V6(B_ADDRESS_V6)),
            "{header:?}"
        );
    }
    let deprecated = [B_GATEWAY_V6];
    let deprecated = header(IPV6_SRCRT_TYPE_0, deprecated.len(), &deprecated);
    let routing_header = Ipv6RawExtHeaderSlice::from_slice(&deprecated).expect("a routing header");
    let route = Ipv6RouteSelectors {
        routing_header: Some(routing_header),
        ..v6(FAR_V6)
    };
    assert_eq!(
        resolve6(&resolver, route),
        Err(HostError::Errno(libc::EINVAL))
    );
}

/// Checks that a link-scope lookup takes a route on the link and refuses one through a gateway.
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

/// Checks that only a confirmed lookup confirms the next hop's neighbour, both give one route, and neither sends.
pub fn confirmed_neighbour_is_confirmed<R: RouteResolver>(resolver: R) {
    /// The gateways of `FAR` on da as stale neighbours, on a link that resolves them.
    const NEIGHBOURS: &str = "link set da arp on
neigh add 10.1.0.2 lladdr 02:00:00:00:00:02 dev da nud stale
neigh add fd01::2 lladdr 02:00:00:00:00:02 dev da nud stale";
    /// The gateway of `FAR` on da.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const GATEWAY: Ipv4Addr = Ipv4Addr::new(10, 1, 0, 2);
    /// The gateway of `FAR_V6` on da.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const GATEWAY_V6: Ipv6Addr = Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 2);
    /// Counts what da sends to the gateways' link address, where anything a lookup sent would go.
    const GATEWAY_COUNTER: &str = "qdisc add dev da clsact
filter add dev da egress protocol all flower dst_mac 02:00:00:00:00:02 action pass";
    network();
    ip(IPV4, NEIGHBOURS);
    batch(TRAFFIC_CONTROL, IPV4, GATEWAY_COUNTER);
    let options = Ipv4Options::new();
    let lookups = [
        (RouteSelectors::V4(v4(&options, FAR)), IpAddr::V4(GATEWAY)),
        (RouteSelectors::V6(v6(FAR_V6)), IpAddr::V6(GATEWAY_V6)),
    ];
    for (route, gateway) in lookups {
        let sent = sent_to_gateways();
        let unconfirmed =
            resolver.resolve_route(route.clone(), ROOT, NeighbourConfirmation::Unconfirmed);
        let age = seconds_since_confirmed(gateway, A);
        assert!(age > 0, "{gateway} was confirmed {age} seconds ago");
        let confirmed = resolver.resolve_route(route, ROOT, NeighbourConfirmation::Confirmed);
        assert_eq!(confirmed, unconfirmed);
        assert_eq!(seconds_since_confirmed(gateway, A), 0, "{gateway}");
        assert_eq!(
            sent_to_gateways(),
            sent,
            "a lookup towards {gateway} sent a packet"
        );
    }
}

/// Returns the number of packets da has sent to the gateways' link address, from its egress filter's statistics.
fn sent_to_gateways() -> u64 {
    /// Shows da's egress filters with their statistics, as JSON.
    const SHOW: &[&str] = &["-s", "-j", "filter", "show"];
    const EGRESS: &str = "egress";
    const STATISTICS: &str = "\"stats\":";
    let output = Command::new(TRAFFIC_CONTROL)
        .args(SHOW)
        .args([DEVICE, A, EGRESS])
        .output()
        .expect("tc runs");
    let text = String::from_utf8(output.stdout).expect("tc writes text");
    number_after(&text, STATISTICS, PACKETS)
}

/// Makes da's and db's addresses the sources of temporary addresses, of which Linux makes one each at once.
const TEMPORARY_ADDRESSES: &[&str] = &[
    "/proc/sys/net/ipv6/conf/da/use_tempaddr",
    "/proc/sys/net/ipv6/conf/db/use_tempaddr",
];
const MANAGED_TEMPORARY: &str = "address replace fd01::1/64 dev da nodad mngtmpaddr
address replace fd02::1/64 dev db nodad mngtmpaddr";

/// Checks that a temporary or public preference chooses between a link's address and Linux's temporary one, also on a link a mark or owner selects.
pub fn source_preferences_choose_among_the_link_addresses<R: RouteResolver>(resolver: R) {
    /// A mark the rule selects.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-rule.8.html
    const MARK: PacketMark = PacketMark(7);
    /// A user the rule selects.
    ///
    /// https://man7.org/linux/man-pages/man8/ip-rule.8.html
    const OWNER: UserId = UserId(4242);
    const RULES: &str = "rule add fwmark 7 lookup 100
rule add uidrange 4242-4242 lookup 100";
    let network = network();
    ip(IPV6, RULES);
    for path in TEMPORARY_ADDRESSES {
        write_setting(path, u8::from(true));
    }
    ip(IPV6, MANAGED_TEMPORARY);
    let (a_temporary, b_temporary) = (temporary_address(A), temporary_address(B));
    let unmarked = PacketMark(u32::MIN);
    for (temporary, mark, owner, expected) in [
        (
            TemporaryAddressPreference::Temporary,
            unmarked,
            ROOT,
            (network.a, a_temporary),
        ),
        (
            TemporaryAddressPreference::Public,
            unmarked,
            ROOT,
            (network.a, A_ADDRESS_V6),
        ),
        (
            TemporaryAddressPreference::Temporary,
            MARK,
            ROOT,
            (network.b, b_temporary),
        ),
        (
            TemporaryAddressPreference::Public,
            unmarked,
            OWNER,
            (network.b, B_ADDRESS_V6),
        ),
    ] {
        let route = Ipv6RouteSelectors {
            mark,
            source_preferences: SourcePreferences {
                temporary,
                mobile: MobileAddressPreference::Home,
            },
            ..v6(FAR_V6)
        };
        let route = resolver.resolve_route(
            RouteSelectors::V6(route),
            owner,
            NeighbourConfirmation::Unconfirmed,
        );
        let route = route.expect("resolves");
        assert_eq!(
            (route.interface, route.source),
            (expected.0, IpAddr::V6(expected.1)),
            "{temporary:?} {mark:?} {owner:?}"
        );
    }
}

/// Checks that a protocol no rule names routes as any protocol would, that SCTP's ports select as any protocol's, and that a rule on GRE selects it.
pub fn other_protocols_route<R: RouteResolver>(resolver: R) {
    /// A port SCTP's rule selects, M3UA's.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4666#section-1.4.8
    const M3UA: u16 = 2905;
    /// A port no rule selects.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6335#section-6
    const OTHER: u16 = 2906;
    /// A source port in the registered range.
    ///
    /// https://www.rfc-editor.org/rfc/rfc6335#section-6
    const SOURCE_PORT: Port = Port(40000);
    let network = network();
    const PORT_RULE: &str = "rule add dport 2905 lookup 100";
    const UDP_RULE: &str = "rule add ipproto udp lookup 100";
    const GRE_RULE: &str = "rule add ipproto gre lookup 100";
    let options = Ipv4Options::new();
    let check4 = |upper_layer: UpperLayer, expected: InterfaceIndex, refusable: bool| {
        let route = Ipv4RouteSelectors {
            upper_layer,
            ..v4(&options, FAR)
        };
        let route = resolve4(&resolver, route);
        if refusable {
            unless_refused(route, |route| {
                assert_eq!(interface(route), expected, "{upper_layer:?}");
            });
        } else {
            assert_eq!(interface(route), expected, "{upper_layer:?}");
        }
    };
    let check6 = |upper_layer: UpperLayer, expected: InterfaceIndex, refusable: bool| {
        let route = Ipv6RouteSelectors {
            upper_layer,
            ..v6(FAR_V6)
        };
        let route = resolve6(&resolver, route);
        if refusable {
            unless_refused(route, |route| {
                assert_eq!(interface(route), expected, "{upper_layer:?}");
            });
        } else {
            assert_eq!(interface(route), expected, "{upper_layer:?}");
        }
    };
    let gre = UpperLayer {
        protocol: IpNumber::GRE,
        selector: None,
    };
    let sctp = |port| UpperLayer {
        protocol: IpNumber::SCTP,
        selector: Some(UpperLayerSelector::Ports(TransportPorts {
            source: SOURCE_PORT,
            destination: Port(port),
        })),
    };
    ip(IPV4, PORT_RULE);
    ip(IPV6, UDP_RULE);
    ip(IPV6, PORT_RULE);
    check4(gre, network.a, false);
    check4(sctp(M3UA), network.b, false);
    check4(sctp(OTHER), network.a, false);
    check6(gre, network.a, false);
    check6(sctp(M3UA), network.b, false);
    check6(sctp(OTHER), network.a, false);
    ip(IPV4, UDP_RULE);
    check4(gre, network.a, true);
    check4(sctp(M3UA), network.b, true);
    check4(sctp(OTHER), network.a, true);
    ip(IPV4, GRE_RULE);
    ip(IPV6, GRE_RULE);
    check4(gre, network.b, true);
    check6(gre, network.b, true);
    check6(sctp(OTHER), network.a, false);
}

/// Checks that IPv4 options `IP_OPTIONS` refuses are refused with its error, and well-formed ones are taken.
pub fn malformed_ipv4_options_are_refused_as_a_send_refuses_them<R: RouteResolver>(resolver: R) {
    /// Where a source route's pointer is, after its type and length.
    ///
    /// https://www.rfc-editor.org/rfc/rfc791#section-3.1
    const POINTER: usize = 2;
    /// Where an option's length is, after its type.
    ///
    /// https://www.rfc-editor.org/rfc/rfc791#section-3.1
    const LENGTH: usize = 1;
    network();
    let whole = source_route(IPOPT_LSRR, B_GATEWAY);
    let route = whole.as_slice();
    let mut pointer_past_its_end = route.to_vec();
    pointer_past_its_end[POINTER] = SOURCE_ROUTE_LENGTH + 1;
    let mut length_of_no_address = route.to_vec();
    length_of_no_address[LENGTH] = SOURCE_ROUTE_LENGTH + 1;
    let one_route = &route[..usize::from(SOURCE_ROUTE_LENGTH)];
    let mut two_source_routes = [one_route, one_route].concat();
    two_source_routes.resize(route.len() + route.len(), u8::MIN);
    for options in [
        pointer_past_its_end,
        length_of_no_address,
        two_source_routes,
    ] {
        let options = Ipv4Options::try_from(options.as_slice()).expect("options");
        let refused = ip_options_error(&options);
        assert_ne!(refused, None, "{options:?}");
        assert_eq!(
            resolve4(&resolver, v4(&options, FAR)).err(),
            refused,
            "{options:?}"
        );
    }
    assert_eq!(ip_options_error(&whole), None);
    assert!(resolve4(&resolver, v4(&whole, FAR)).is_ok());
}

/// Returns the error a datagram socket's `IP_OPTIONS` gives for the options, if it refuses them.
fn ip_options_error(options: &Ipv4Options) -> Option<HostError> {
    #[cfg(target_os = "linux")]
    {
        let socket =
            std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, u16::MIN)).expect("a socket opens");
        let refused = LinuxSocketOptions.set_option(
            std::os::fd::AsFd::as_fd(&socket),
            PlainOption::IpOptions,
            options.as_slice(),
        );
        refused
            .err()
            .map(|errno| HostError::Errno(errno.raw_os_error()))
    }
    #[cfg(not(target_os = "linux"))]
    panic!("only Linux sets IP options as the checks know them: {options:?}");
}

/// Checks that a route in a namespace without an IPv4 address goes from the unspecified address, as a socket connected along it does.
pub fn a_namespace_without_an_ipv4_address_routes_from_the_unspecified_address<R: RouteResolver>(
    resolver: R,
) {
    /// The MTU of the link without addresses, more than the least IPv6 takes and unlike da's and db's.
    ///
    /// https://www.rfc-editor.org/rfc/rfc8200#section-5
    const BARE_MTU: u32 = 1450;
    /// An address routed out of the link without addresses.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const DESTINATION: Ipv4Addr = Ipv4Addr::new(10, 30, 0, 1);
    const BARE: &str = "dc";
    // No link of the namespace has an IPv4 address of universe scope to lend.
    enter_network_namespace();
    const BARE_LINK: &str = "link set lo up
link add dc type dummy
link set dc addrgenmode none
link set dc up mtu 1450
route add 10.30.0.0/16 dev dc";
    ip(IPV4, BARE_LINK);
    let options = Ipv4Options::new();
    let route = resolve4(&resolver, v4(&options, DESTINATION)).expect("resolves");
    let told = connected_source_and_path_mtu(IpAddr::V4(DESTINATION), DISCARD);
    assert_eq!(
        told,
        (
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            MaximumTransmissionUnit(BARE_MTU)
        )
    );
    assert_eq!(
        (route.interface, route.source, route.path_mtu),
        (index(BARE), told.0, told.1)
    );
}

/// Checks that an IPv4-mapped IPv6 destination routes as a dual-stack socket sends to it, over IPv4 from a mapped source.
pub fn an_ipv4_mapped_destination_routes_over_ipv4<R: RouteResolver>(resolver: R) {
    let network = network();
    for (destination, interface) in [(FAR, network.a), (B_GATEWAY, network.b)] {
        let mapped = destination.to_ipv6_mapped();
        let route = resolve6(&resolver, v6(mapped)).expect("resolves");
        assert_eq!(
            (route.interface, route.source, route.path_mtu),
            {
                let (source, path_mtu) = connected_source_and_path_mtu(IpAddr::V6(mapped), DISCARD);
                (interface, source, path_mtu)
            },
            "{mapped}"
        );
    }
}

/// Checks that a link whose MTU passes the largest packet of each version gives the path MTU Linux caps a connected socket's at.
pub fn a_path_mtu_is_capped_at_the_largest_packet_of_its_version<R: RouteResolver>(resolver: R) {
    /// A link MTU above the largest IPv4 packet and the largest IPv6 packet but a jumbogram, which a dummy link takes.
    ///
    /// https://www.rfc-editor.org/rfc/rfc2675#section-1
    const LARGE_MTU: u32 = 100_000;
    /// An address on the large link that the namespace does not have.
    ///
    /// https://www.rfc-editor.org/rfc/rfc1918#section-3
    const LARGE_NEIGHBOUR: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 5);
    /// An IPv6 address on the large link that the namespace does not have.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const LARGE_NEIGHBOUR_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 5);
    const LARGE: &str = "dx";
    network();
    const LARGE_LINK: &str = "link add dx type dummy
link set dx up mtu 100000
address add 10.3.0.1/24 dev dx
address add fd03::1/64 dev dx nodad";
    ip(IPV4, LARGE_LINK);
    let options = Ipv4Options::new();
    for (route, destination) in [
        (
            resolve4(&resolver, v4(&options, LARGE_NEIGHBOUR)),
            IpAddr::V4(LARGE_NEIGHBOUR),
        ),
        (
            resolve6(&resolver, v6(LARGE_NEIGHBOUR_V6)),
            IpAddr::V6(LARGE_NEIGHBOUR_V6),
        ),
    ] {
        let route = route.expect("resolves");
        let told = connected_path_mtu(destination, DISCARD);
        assert!(told < LARGE_MTU, "{destination}: {told}");
        assert_eq!(
            (route.interface, route.device_mtu, route.path_mtu),
            (
                index(LARGE),
                MaximumTransmissionUnit(LARGE_MTU),
                MaximumTransmissionUnit(told)
            ),
            "{destination}"
        );
    }
}

/// Returns what Linux gives a datagram socket held to the interface as the selector says, from the source if any, when it connects.
///
/// That is the source and path MTU it is told, or the error of the connect. A requested interface is named for the
/// socket's unicast sends (`IP_UNICAST_IF`, `IPV6_UNICAST_IF`), as a send names it per packet; a bound one binds the
/// socket to the device (`SO_BINDTOIFINDEX`).
fn told_by_interface(
    destination: IpAddr,
    source: Option<IpAddr>,
    selector: InterfaceSelector,
) -> Result<(IpAddr, MaximumTransmissionUnit), HostError> {
    let any = match destination {
        IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    let socket =
        std::net::UdpSocket::bind((source.unwrap_or(any), u16::MIN)).expect("a socket binds");
    #[cfg(target_os = "linux")]
    {
        let fd = std::os::fd::AsFd::as_fd(&socket);
        let result = match (selector, destination) {
            // Both versions take the index of a requested interface in network byte order.
            (InterfaceSelector::Requested(index), IpAddr::V4(_)) => LinuxSocketOptions
                .set_option(
                    fd,
                    PlainOption::IpUnicastIf,
                    &index.0.get().to_be().to_ne_bytes(),
                )
                .map_err(std::io::Error::from),
            (InterfaceSelector::Requested(index), IpAddr::V6(_)) => LinuxSocketOptions
                .set_option(
                    fd,
                    PlainOption::Ipv6UnicastIf,
                    &index.0.get().to_be().to_ne_bytes(),
                )
                .map_err(std::io::Error::from),
            // On Linux socket2 binds by `SO_BINDTOIFINDEX` for either version.
            (InterfaceSelector::Bound(index), _) => {
                socket2::SockRef::from(&socket).bind_device_by_index_v4(Some(index.0))
            }
        };
        assert!(result.is_ok(), "{destination} by {selector:?}: {result:?}");
    }
    socket
        .connect((destination, DISCARD))
        .map_err(|error| HostError::Errno(error.raw_os_error().expect("an errno")))?;
    let source = socket
        .local_addr()
        .expect("a connected socket's source")
        .ip();
    #[cfg(target_os = "linux")]
    return Ok((
        source,
        MaximumTransmissionUnit(
            match destination {
                IpAddr::V4(_) => rustix::net::sockopt::ip_mtu(&socket),
                IpAddr::V6(_) => rustix::net::sockopt::ipv6_mtu(&socket),
            }
            .expect("a connected socket's path MTU"),
        ),
    ));
    #[cfg(not(target_os = "linux"))]
    panic!("only Linux tells a connected socket's path MTU, not {source} by {selector:?}");
}

/// An IPv6 address the main table routes by da first and by db after.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const TWO_WAY_V6: Ipv6Addr = Ipv6Addr::new(0xfd0a, 0, 0, 0, 0, 0, 0, 5);
/// An IPv6 address of an unreachable route.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const UNREACHABLE_V6: Ipv6Addr = Ipv6Addr::new(0xfd52, 0, 0, 0, 0, 0, 0, 2);

/// Builds the network with the routes of the interface checks and returns the cases they ask.
///
/// Each case is a destination, a source if any and the interface asked for.
fn interface_cases() -> Vec<(IpAddr, Option<IpAddr>, InterfaceIndex)> {
    let network = network();
    const ROUTES: &str = "route add fd0a::/64 via fd01::2 metric 1
route add fd0a::/64 via fd02::2 metric 2
route add unreachable fd52::/64";
    ip(IPV6, ROUTES);
    vec![
        (IpAddr::V4(FAR), None, network.b),
        (IpAddr::V4(FAR), Some(IpAddr::V4(B_ADDRESS)), network.b),
        (IpAddr::V4(FAR), Some(IpAddr::V4(A_ADDRESS)), network.b),
        (IpAddr::V6(TWO_WAY_V6), None, network.b),
        (
            IpAddr::V6(TWO_WAY_V6),
            Some(IpAddr::V6(B_ADDRESS_V6)),
            network.b,
        ),
        (
            IpAddr::V6(TWO_WAY_V6),
            Some(IpAddr::V6(A_ADDRESS_V6)),
            network.a,
        ),
        (
            IpAddr::V6(FAR_V6),
            Some(IpAddr::V6(B_ADDRESS_V6)),
            network.b,
        ),
        (
            IpAddr::V6(UNREACHABLE_V6),
            Some(IpAddr::V6(A_ADDRESS_V6)),
            network.a,
        ),
    ]
}

/// Resolves a route to the destination from the source if any, held to the interface as the selector says.
fn resolve_by_interface<R: RouteResolver>(
    resolver: &R,
    destination: IpAddr,
    source: Option<IpAddr>,
    selector: InterfaceSelector,
) -> Result<Route, HostError> {
    let options = Ipv4Options::new();
    match destination {
        IpAddr::V4(destination) => resolve4(
            resolver,
            Ipv4RouteSelectors {
                source: source.map(|source| match source {
                    IpAddr::V4(source) => source,
                    IpAddr::V6(source) => panic!("{source} is not of the destination's version"),
                }),
                interface: Some(selector),
                ..v4(&options, destination)
            },
        ),
        IpAddr::V6(destination) => resolve6(
            resolver,
            Ipv6RouteSelectors {
                source: source.map(|source| match source {
                    IpAddr::V6(source) => source,
                    IpAddr::V4(source) => panic!("{source} is not of the destination's version"),
                }),
                interface: Some(selector),
                ..v6(destination)
            },
        ),
    }
}

/// Checks that a lookup requesting an interface routes as a send naming it does, so that over IPv6 with a source it only prefers its routes.
///
/// A send names its interface per packet (`IP_PKTINFO`, `IPV6_PKTINFO`) as `IP_UNICAST_IF` does.
pub fn an_interface_routes_as_a_send_that_names_it<R: RouteResolver>(resolver: R) {
    let cases = interface_cases();
    for (destination, source, interface) in cases {
        let selector = InterfaceSelector::Requested(interface);
        assert_eq!(
            resolve_by_interface(&resolver, destination, source, selector)
                .map(|route| (route.source, route.path_mtu)),
            told_by_interface(destination, source, selector),
            "{destination} from {source:?} by {interface:?}"
        );
    }
}

/// Checks that a lookup bound to an interface routes as a socket bound to the device does, held to its routes alone.
///
/// Over IPv6 with a source this differs from a requested interface: a destination routed only by the other link, or
/// by an unreachable route, has no route.
pub fn a_bound_interface_routes_as_a_bound_socket<R: RouteResolver>(resolver: R) {
    let cases = interface_cases();
    for (destination, source, interface) in cases {
        let selector = InterfaceSelector::Bound(interface);
        let told = told_by_interface(destination, source, selector);
        let route = resolve_by_interface(&resolver, destination, source, selector);
        // Linux's IPv6 lookup from a source only prefers an interface, so only where it leaves by another may one be refused.
        let requested = InterfaceSelector::Requested(interface);
        let refusable = destination.is_ipv6()
            && source.is_some()
            && resolve_by_interface(&resolver, destination, source, requested)
                .map_or(true, |route| route.interface != interface);
        if refusable && route == Err(HostError::NotSupported) {
            continue;
        }
        assert_eq!(
            route.map(|route| (route.source, route.path_mtu)),
            told,
            "{destination} from {source:?} bound to {interface:?}"
        );
    }
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn loopback_route_is_local() {
            $crate::loopback_route_is_local($value);
        }

        #[test]
        fn unspecified_destination_is_loopback() {
            $crate::unspecified_destination_is_loopback($value);
        }

        #[test]
        fn ipv4_route_gives_link_source_and_sizes() {
            $crate::ipv4_route_gives_link_source_and_sizes($value);
        }

        #[test]
        fn ipv6_route_gives_link_source_and_sizes() {
            $crate::ipv6_route_gives_link_source_and_sizes($value);
        }

        #[test]
        fn route_metrics_give_path_mtu_and_hop_limit() {
            $crate::route_metrics_give_path_mtu_and_hop_limit($value);
        }

        #[test]
        fn locked_mtu_without_value_uses_the_device_mtu() {
            $crate::locked_mtu_without_value_uses_the_device_mtu($value);
        }

        #[test]
        fn encapsulating_route_leaves_room_for_its_headers() {
            $crate::encapsulating_route_leaves_room_for_its_headers($value);
        }

        #[test]
        fn dscp_rule_takes_an_encapsulating_route() {
            $crate::dscp_rule_takes_an_encapsulating_route($value);
        }

        #[test]
        fn failing_routes_give_their_errors() {
            $crate::failing_routes_give_their_errors($value);
        }

        #[test]
        fn mark_selects_the_route() {
            $crate::mark_selects_the_route($value);
        }

        #[test]
        fn owner_selects_the_route() {
            $crate::owner_selects_the_route($value);
        }

        #[test]
        fn dscp_selects_the_route() {
            $crate::dscp_selects_the_route($value);
        }

        #[test]
        fn source_selects_the_route() {
            $crate::source_selects_the_route($value);
        }

        #[test]
        fn interface_selects_the_route() {
            $crate::interface_selects_the_route($value);
        }

        #[test]
        fn protocol_and_ports_select_the_route() {
            $crate::protocol_and_ports_select_the_route($value);
        }

        #[test]
        fn source_port_selects_the_route() {
            $crate::source_port_selects_the_route($value);
        }

        #[test]
        fn icmp_type_and_code_select_the_route() {
            $crate::icmp_type_and_code_select_the_route($value);
        }

        #[test]
        fn mobility_header_type_selects_the_route() {
            $crate::mobility_header_type_selects_the_route($value);
        }

        #[test]
        fn flow_label_selects_the_route() {
            $crate::flow_label_selects_the_route($value);
        }

        #[test]
        fn broadcast_needs_permission() {
            $crate::broadcast_needs_permission($value);
        }

        #[test]
        fn multicast_route_is_multicast() {
            $crate::multicast_route_is_multicast($value);
        }

        #[test]
        fn required_source_check_refuses_a_foreign_address() {
            $crate::required_source_check_refuses_a_foreign_address($value);
        }

        #[test]
        fn an_anycast_route_is_not_given_as_another_kind() {
            $crate::an_anycast_route_is_not_given_as_another_kind($value);
        }

        #[test]
        fn a_link_local_destination_without_an_interface_routes() {
            $crate::a_link_local_destination_without_an_interface_routes($value);
        }

        #[test]
        fn waived_source_check_takes_a_foreign_address() {
            $crate::waived_source_check_takes_a_foreign_address($value);
        }

        #[test]
        fn loose_source_route_goes_to_its_first_hop() {
            $crate::loose_source_route_goes_to_its_first_hop($value);
        }

        #[test]
        fn strict_source_route_takes_only_a_first_hop_on_the_link() {
            $crate::strict_source_route_takes_only_a_first_hop_on_the_link($value);
        }

        #[test]
        fn routing_header_goes_to_its_first_hop() {
            $crate::routing_header_goes_to_its_first_hop($value);
        }

        #[test]
        fn link_scope_takes_only_routes_on_the_link() {
            $crate::link_scope_takes_only_routes_on_the_link($value);
        }

        #[test]
        fn confirmed_neighbour_is_confirmed() {
            $crate::confirmed_neighbour_is_confirmed($value);
        }

        #[test]
        fn source_preferences_choose_among_the_link_addresses() {
            $crate::source_preferences_choose_among_the_link_addresses($value);
        }

        #[test]
        fn other_protocols_route() {
            $crate::other_protocols_route($value);
        }

        #[test]
        fn malformed_ipv4_options_are_refused_as_a_send_refuses_them() {
            $crate::malformed_ipv4_options_are_refused_as_a_send_refuses_them($value);
        }

        #[test]
        fn a_namespace_without_an_ipv4_address_routes_from_the_unspecified_address() {
            $crate::a_namespace_without_an_ipv4_address_routes_from_the_unspecified_address($value);
        }

        #[test]
        fn an_ipv4_mapped_destination_routes_over_ipv4() {
            $crate::an_ipv4_mapped_destination_routes_over_ipv4($value);
        }

        #[test]
        fn a_path_mtu_is_capped_at_the_largest_packet_of_its_version() {
            $crate::a_path_mtu_is_capped_at_the_largest_packet_of_its_version($value);
        }

        #[test]
        fn an_interface_routes_as_a_send_that_names_it() {
            $crate::an_interface_routes_as_a_send_that_names_it($value);
        }

        #[test]
        fn a_bound_interface_routes_as_a_bound_socket() {
            $crate::a_bound_interface_routes_as_a_bound_socket($value);
        }
    };
}
