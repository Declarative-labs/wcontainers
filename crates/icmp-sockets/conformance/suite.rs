//! The tests every implementation of wcontainers-icmp-sockets runs, written against its traits alone.
//!
//! Each runs as root in a network namespace of its own, with loopback up and every group allowed ICMP echo
//! sockets, on links, routes, rules and queueing disciplines it builds there through the kernel's routing socket, each
//! written as the `ip` or `tc` command it is. A packet socket of the namespace sees what leaves a link.
#![cfg(target_os = "linux")]

mod netlink;

use core::cell::{Cell, RefCell};
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use core::num::{NonZeroU16, NonZeroU32};
use std::future::Future;
use std::os::fd::{AsRawFd, OwnedFd};
use std::pin::{Pin, pin};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use etherparse::{
    IpDscp, IpEcn, IpNumber, Ipv4HeaderSlice, Ipv4Options, Ipv6FlowLabel, Ipv6FragmentHeaderSlice,
    Ipv6HeaderSlice, Ipv6RawExtHeaderSlice, icmpv4, icmpv6,
};
use mitsein::slice1::Slice1;
use wcontainers_abi::HostError;
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_icmp_sockets::{
    EchoBindAddress, EchoBinding, EchoIdentifier, IcmpSockets, IdentifierSharing, LocalIdentifier,
};
use wcontainers_internet_protocol::{
    AnycastAddress, BroadcastPermission, BufferSizes, ByteCount, BytesPerSecond, FragmentSize,
    HopLimit, IcmpCode, IcmpType, InterfaceIndex, InterfaceSelector, IpHeaderFields, IpSendFields,
    IpSocketSetting, IpVersion, Ipv4HeaderFields, Ipv4RouteSelectors, Ipv6HeaderFields,
    Ipv6RouteSelectors, LocalFragmentation, MaximumTransmissionUnit, Membership,
    MobileAddressPreference, MulticastGroup, MulticastMembership, NeighbourConfirmation,
    PacketMark, PacketPriority, PathMtuDiscovery, Received, ReceivedPacketFields, Report,
    RouteScope, SendError, SourceAddressCheck, SourceFilter, SourcePreferences,
    TemporaryAddressPreference, TimestampSource, Timestamping, TransmitClock, TransmitEvent,
    TransmitReportKey, TransmitReportRequest, TransmitStage, TransmitTime, TransmitTimeError,
    TransmitTimeMode, UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_readiness::{ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_readiness_conformance::{MANY_WAITS, Waiting};
use wcontainers_volatile_bytes::VolatileBytes;

#[doc(hidden)]
pub use wcontainers_configurable_conformance;
pub use wcontainers_readiness_conformance;

/// Moves the calling thread into a new network namespace (root only) with loopback up and every group allowed ICMP echo sockets.
pub fn enter_network_namespace() {
    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET)
        .expect("a new network namespace needs root");
    ip(LOOPBACK_UP);
    // This thread's namespace is the one /proc/sys/net shows it; the highest group id Linux takes is one below all ones.
    std::fs::write(PING_GROUP_RANGE, format!("{} {}", u32::MIN, u32::MAX - 1))
        .expect("the ping group range is written");
}

const LOOPBACK_UP: &str = "link set lo up";
pub const PING_GROUP_RANGE: &str = "/proc/sys/net/ipv4/ping_group_range";
pub const ECHO_IGNORE_BROADCASTS: &str = "/proc/sys/net/ipv4/icmp_echo_ignore_broadcasts";
/// The anycast addresses of this thread's network namespace.
pub const ANYCAST_LIST: &str = "/proc/thread-self/net/anycast6";
/// The third band of [`PRIO_QUEUE`], class 1:3, which its default map gives `TC_PRIO_FILLER` alone.
///
/// https://man7.org/linux/man-pages/man8/tc-prio.8.html
const THIRD_BAND: netlink_packet_route::tc::TcHandle =
    netlink_packet_route::tc::TcHandle { major: 1, minor: 3 };
/// The gateways of the dummy link, as neighbours last confirmed long ago; the link resolves neighbours so that IPv6 confirms them.
pub const STALE_GATEWAYS: &str = "link set ia arp on
neigh add 10.3.0.2 lladdr 02:00:00:00:00:02 dev ia nud stale
neigh add fd03::2 lladdr 02:00:00:00:00:02 dev ia nud stale";
/// `TC_PRIO_FILLER`, which the prio queue's default map puts in its third band, 1:3, where nothing else goes.
///
/// https://man7.org/linux/man-pages/man8/tc-prio.8.html
pub const FILLER: u32 = 1;
pub const PRIO_QUEUE: &str = "qdisc add dev ia root handle 1: prio";
/// A queue that paces each flow and keeps departure times.
pub const FAIR_QUEUE: &str = "qdisc add dev lo root fq";
pub const SELF_LINK: &str = "lo";
// https://man7.org/linux/man-pages/man7/socket.7.html
pub const MARK: PacketMark = PacketMark(7);
pub const MARK_PROHIBITED: &str = "rule add fwmark 7 prohibit";
pub const OWNER_PROHIBITED: &str = "rule add uidrange 1000-1000 prohibit";
/// Type 8 code 0, where the flow keeps a destination port.
const ECHO_PROHIBITED: &str = "rule add ipproto icmp dport 2048 prohibit";
/// Type 128 code 0, where the flow keeps a destination port.
const ECHO_PROHIBITED_V6: &str = "rule add ipproto ipv6-icmp dport 32768 prohibit";
/// A code an echo request does not carry.
///
/// https://www.rfc-editor.org/rfc/rfc792
pub const OTHER_CODE: IcmpCode = IcmpCode(1);
// https://www.rfc-editor.org/rfc/rfc791#section-3.1
pub const TIME_TO_LIVE: HopLimit = HopLimit(7);
// https://www.rfc-editor.org/rfc/rfc8200#section-4
pub const HOP_LIMIT: HopLimit = HopLimit(9);
// https://www.rfc-editor.org/rfc/rfc6437
pub const FLOW_LABEL: u32 = 0x5a5a5;
// https://docs.kernel.org/networking/timestamping.html
pub const REPORT_KEY: TransmitReportKey = TransmitReportKey(0x5eed);
/// An echo message longer than the links of the checks that fragment.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
const LONG_MESSAGE: usize = 1400;
/// The length of each paced message, and the rate a send buffer fills at.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
const PACED_MESSAGE: usize = 1100;
// https://man7.org/linux/man-pages/man7/socket.7.html
const SLOW_RATE: BytesPerSecond = BytesPerSecond(1000);
/// How many sends a check tries before it expects the send buffer to be full.
///
/// https://man7.org/linux/man-pages/man7/socket.7.html
const ATTEMPTS: usize = 64;
/// Where an echo message keeps its identifier, after its type, code and checksum.
///
/// https://www.rfc-editor.org/rfc/rfc792
const IDENTIFIER_BYTES: core::ops::Range<usize> = 4..6;
/// Where an ICMP message keeps its checksum.
///
/// https://www.rfc-editor.org/rfc/rfc792
pub const CHECKSUM_BYTES: core::ops::Range<usize> = 2..4;
// https://man7.org/linux/man-pages/man7/credentials.7.html
pub const ROOT: UserId = UserId(0);
// https://man7.org/linux/man-pages/man7/credentials.7.html
pub const OTHER_USER: UserId = UserId(1000);
pub const DUMMY: &str = "ia";
/// A dummy link with an IPv4 and an IPv6 prefix, a gateway on each and routes through it to far networks.
pub const NETWORK: &str = "link add ia type dummy
link set ia multicast on
link set ia up
address add 10.3.0.1/24 broadcast 10.3.0.255 dev ia
address add fd03::1/64 dev ia nodad
route add 10.9.0.0/16 via 10.3.0.2
route add fd09::/64 via fd03::2";
/// A second dummy link and routes to [`NOWHERE`] and [`NOWHERE_V6`] on both links, the first preferred.
pub const TWO_ROUTES: &str = "link add ib type dummy
link set ib up
route add 10.8.0.0/16 dev ia metric 1
route add 10.8.0.0/16 dev ib metric 2
route add fd08::/64 dev ia metric 1
route add fd08::/64 dev ib metric 2";
pub const SECOND: &str = "ib";
// https://www.rfc-editor.org/rfc/rfc1918#section-3
pub const ON_LINK: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 7);
// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
pub const ON_LINK_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 7);
// https://www.rfc-editor.org/rfc/rfc1918#section-3
pub const LOCAL: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 1);
// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
pub const LOCAL_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 1);
// https://www.rfc-editor.org/rfc/rfc1918#section-3
pub const GATEWAY: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 2);
// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
pub const GATEWAY_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 2);
// https://www.rfc-editor.org/rfc/rfc1918#section-3
pub const BROADCAST: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 255);
// https://www.rfc-editor.org/rfc/rfc1918#section-3
pub const FAR: Ipv4Addr = Ipv4Addr::new(10, 9, 1, 1);
// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
pub const FAR_V6: Ipv6Addr = Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 1);
// https://www.rfc-editor.org/rfc/rfc1918#section-3
pub const NOWHERE: Ipv4Addr = Ipv4Addr::new(10, 8, 0, 1);
// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
pub const NOWHERE_V6: Ipv6Addr = Ipv6Addr::new(0xfd08, 0, 0, 0, 0, 0, 0, 1);
pub const PAYLOAD: &[u8] = b"wcontainers echo";
// https://www.rfc-editor.org/rfc/rfc792
const IDENTIFIER: EchoIdentifier = EchoIdentifier(NonZeroU16::new(0x4a21).unwrap());
/// The other version's identifier, since both share Linux's table, where a closed socket's stays taken while a forked child holds it.
///
/// https://www.rfc-editor.org/rfc/rfc792
const IDENTIFIER_V6: EchoIdentifier = EchoIdentifier(NonZeroU16::new(0x4a22).unwrap());

fn identifier(version: IpVersion) -> EchoIdentifier {
    match version {
        IpVersion::V4 => IDENTIFIER,
        IpVersion::V6 => IDENTIFIER_V6,
    }
}
// https://www.rfc-editor.org/rfc/rfc792
const SEQUENCE: u16 = 0x0102;
/// How long a check sleeps before it asks again for what has not come yet.
///
/// https://doc.rust-lang.org/std/thread/fn.sleep.html
pub const LOOK_AGAIN_SOON: Duration = Duration::from_millis(1);
/// How long a check waits for a packet that should come.
///
/// https://man7.org/linux/man-pages/man7/icmp.7.html
pub const PATIENCE: Duration = Duration::from_secs(2);
/// The `ip` option that makes a command without an address IPv6's.
///
/// https://man7.org/linux/man-pages/man8/ip.8.html
const IPV6: &[&str] = &["-6"];
/// The option that pads an IPv6 extension header by its length and two bytes more.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4.2
pub const PAD_N: u8 = 1;
/// How many bytes PadN pads after its type and length in the smallest extension header.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4.2
pub const PADDED: u8 = 4;

/// Returns the packets the link's third prio band has sent, as `tc -s class show dev <link>` gives them for class 1:3.
pub fn third_band_packets(link: &str) -> u64 {
    netlink::class_packets(link, THIRD_BAND)
}

/// Returns the packets the action of the link's egress filter has taken, as `tc -s filter show dev <link> egress`
/// gives them.
pub fn egress_action_packets(link: &str) -> u64 {
    netlink::egress_action_packets(link)
}

/// Makes the calling thread's namespace track the connections of the packets it makes, as `nft -f` loads
/// `table ip tracked { chain output { type filter hook output priority 0; ct state new counter; }; }`.
pub fn track_made_connections() {
    netlink::track_made_connections();
}

/// Returns how long ago the link's neighbour entry for the address was confirmed, in whole seconds, as iproute2 counts.
///
/// Linux gives the age in clock ticks, which `ip -s neigh show` divides by the ticks of a second.
pub fn seconds_since_confirmed(address: IpAddr, link: &str) -> u64 {
    let ticks = netlink::confirmed_ticks(address, link)
        .unwrap_or_else(|| panic!("no neighbour entry for {address} on {link}"));
    u64::from(ticks) / rustix::param::clock_ticks_per_second()
}

/// Returns the temporary address Linux made on the link, once its duplicate address detection, which runs later, is done.
pub fn temporary_address(link: &str) -> Ipv6Addr {
    /// The time between two looks, a hundredth of the time duplicate address detection waits.
    ///
    /// https://www.rfc-editor.org/rfc/rfc4861#section-10
    const LOOK_AGAIN: std::time::Duration = std::time::Duration::from_millis(10);
    let deadline = std::time::Instant::now() + PATIENCE;
    while std::time::Instant::now() < deadline {
        if let Some(address) = netlink::temporary_address(link) {
            return address;
        }
        std::thread::sleep(LOOK_AGAIN);
    }
    panic!("no temporary address on {link}");
}

/// How long a check waits to be sure that a packet does not come.
///
/// https://man7.org/linux/man-pages/man7/icmp.7.html
pub const QUIET: Duration = Duration::from_millis(200);
/// The bytes of an ICMP header, as Linux reads one from an echo socket's message.
///
/// https://www.rfc-editor.org/rfc/rfc792
const ICMP_HEADER: usize = 8;
static NO_OPTIONS: std::sync::LazyLock<Ipv4Options> = std::sync::LazyLock::new(Ipv4Options::new);

pub fn ip(commands: &str) {
    batch(&[], commands);
}

pub fn ip6(commands: &str) {
    batch(IPV6, commands);
}

/// Does each line of `commands` as the `tc` command it is, through the routing socket, in order.
pub fn tc(commands: &str) {
    for line in commands.lines() {
        assert!(
            netlink::traffic_control_done(line),
            "tc {line}: not a command done here"
        );
    }
}

/// Does each line of `commands` as the `ip` command it is with these options, through the routing socket, in order.
///
/// Then it waits until each IPv6 address the lines add is local, as Linux makes it in its later address work.
pub fn batch(options: &[&str], commands: &str) {
    for line in commands.lines() {
        assert!(
            netlink::done(line, options),
            "ip {line}: not a command done here"
        );
    }
    settle_addresses(commands);
}

/// How long a check waits for Linux to finish what an `ip` command began, which its work behind the routing lock delays.
///
/// https://man7.org/linux/man-pages/man8/ip-address.8.html#:~:text=nodad
pub const SETTLING: Duration = Duration::from_secs(10);
/// The `ip` commands that give a link an address, and what they are followed by.
const ADDRESS: &str = "address";
/// The verbs that follow `address` when an `ip` command gives a link an address.
///
/// https://man7.org/linux/man-pages/man8/ip-address.8.html
const ADDING: [&str; 2] = ["add", "replace"];
/// What ends an address before its prefix length in an `ip` command.
const PREFIX_LENGTH: char = '/';

/// Waits until each IPv6 address the commands add is local: Linux routes a `nodad` one locally only in its later address work.
fn settle_addresses(commands: &str) {
    for line in commands.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some(ADDRESS) || !words.next().is_some_and(|verb| ADDING.contains(&verb))
        {
            continue;
        }
        let Some(Ok(IpAddr::V6(address))) = words
            .next()
            .and_then(|prefix| prefix.split(PREFIX_LENGTH).next())
            .map(str::parse)
        else {
            continue;
        };
        let start = Instant::now();
        while !netlink::locally_routed(address) {
            assert!(start.elapsed() < SETTLING, "{address} is not local yet");
            std::thread::sleep(LOOK_AGAIN_SOON);
        }
    }
}

/// The multicast groups of this thread's namespace, IPv4's and IPv6's, which `ip maddr show` lists.
///
/// https://man7.org/linux/man-pages/man8/ip-maddress.8.html
const GROUPS_V4: &str = "/proc/thread-self/net/igmp";
const GROUPS_V6: &str = "/proc/thread-self/net/igmp6";

/// Returns the multicast groups the link has joined, read where `ip maddr show dev <link>` reads them.
///
/// The IPv4 table names a link on a line of its own, `<index> <name> : ...`, with a line per group under it, each
/// address the hexadecimal of its four bytes in the machine's order; the IPv6 table gives one line per group,
/// `<index> <name> <32 hexadecimal digits> ...`.
pub fn joined_groups(link: &str) -> Vec<IpAddr> {
    /// The base of the tables' hexadecimal addresses.
    const HEX_RADIX: u32 = 16;
    let mut groups = Vec::new();
    let v4 =
        std::fs::read_to_string(GROUPS_V4).unwrap_or_else(|error| panic!("{GROUPS_V4}: {error}"));
    let mut current = None;
    for line in v4.lines().skip(1) {
        let mut words = line.split_whitespace();
        if line.starts_with(char::is_whitespace) {
            if current == Some(link)
                && let Some(Ok(value)) = words.next().map(|hex| u32::from_str_radix(hex, HEX_RADIX))
            {
                groups.push(IpAddr::V4(Ipv4Addr::from(value.to_ne_bytes())));
            }
        } else {
            // A name of ten characters or more meets the colon after it.
            current = words.nth(1).map(|name| name.trim_end_matches(':'));
        }
    }
    let v6 =
        std::fs::read_to_string(GROUPS_V6).unwrap_or_else(|error| panic!("{GROUPS_V6}: {error}"));
    for line in v6.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if let [_, name, hex, ..] = words.as_slice()
            && *name == link
            && let Ok(value) = u128::from_str_radix(hex, HEX_RADIX)
        {
            groups.push(IpAddr::V6(Ipv6Addr::from(value)));
        }
    }
    groups
}

/// Returns a link's link-layer address as iproute2 writes it, bytes in hexadecimal between colons.
pub fn link_address(link: &str) -> String {
    netlink::link_address(link)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn write_setting(path: &str, value: &str) {
    std::fs::write(path, value).unwrap_or_else(|error| panic!("{path}: {error}"));
}

pub fn index(name: &str) -> InterfaceIndex {
    let socket = rustix::net::socket(
        rustix::net::AddressFamily::INET,
        rustix::net::SocketType::DGRAM,
        None,
    )
    .expect("a socket opens");
    let index = rustix::net::netdevice::name_to_index(&socket, name).expect(name);
    InterfaceIndex(NonZeroU32::new(index).expect("an interface's index is not zero"))
}

/// Returns an echo request of the version with the sequence number and payload; Linux writes the identifier and checksum.
pub fn echo(version: IpVersion, payload: &[u8]) -> Vec<u8> {
    let kind = match version {
        IpVersion::V4 => icmpv4::TYPE_ECHO_REQUEST,
        IpVersion::V6 => icmpv6::TYPE_ECHO_REQUEST,
    };
    let mut message = vec![kind, u8::MIN, u8::MIN, u8::MIN, u8::MIN, u8::MIN];
    message.extend_from_slice(&SEQUENCE.to_be_bytes());
    message.extend_from_slice(payload);
    message
}

fn protocol(version: IpVersion) -> IpNumber {
    match version {
        IpVersion::V4 => IpNumber::ICMP,
        IpVersion::V6 => IpNumber::IPV6_ICMP,
    }
}

/// Returns the upper layer a message is routed by: its protocol, type and code.
fn upper(version: IpVersion, message: &[u8]) -> UpperLayer {
    UpperLayer {
        protocol: protocol(version),
        selector: Some(UpperLayerSelector::Icmp {
            message_type: IcmpType(message[0]),
            code: IcmpCode(message[1]),
        }),
    }
}

fn v4_route(destination: Ipv4Addr, message: &[u8]) -> Ipv4RouteSelectors<'static> {
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
        options: &NO_OPTIONS,
        upper_layer: upper(IpVersion::V4, message),
    }
}

fn v6_route(destination: Ipv6Addr, message: &[u8]) -> Ipv6RouteSelectors<'static> {
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
        upper_layer: upper(IpVersion::V6, message),
    }
}

fn v4_header(route: Ipv4RouteSelectors<'_>) -> Ipv4HeaderFields<'_> {
    Ipv4HeaderFields {
        route,
        time_to_live: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
    }
}

fn v6_header(route: Ipv6RouteSelectors<'_>) -> Ipv6HeaderFields<'_> {
    Ipv6HeaderFields {
        route,
        hop_limit: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
        local_fragmentation: LocalFragmentation::Allowed,
        fragment_mtu: None,
        hop_by_hop_options: None,
        routing_destination_options: None,
        destination_options: None,
    }
}

/// Returns the plain header of a message to the address.
pub fn header_to(destination: IpAddr, message: &[u8]) -> IpHeaderFields<'static> {
    match destination {
        IpAddr::V4(address) => IpHeaderFields::V4(v4_header(v4_route(address, message))),
        IpAddr::V6(address) => IpHeaderFields::V6(v6_header(v6_route(address, message))),
    }
}

pub fn no_reports() -> TransmitReportRequest {
    TransmitReportRequest {
        key: TransmitReportKey(u32::MIN),
        scheduled: Report::NotRequested,
        software: Report::NotRequested,
        hardware: Report::NotRequested,
        acknowledgement: Report::NotRequested,
        completion: Report::NotRequested,
        wireless_status: Report::NotRequested,
        packet: Report::NotRequested,
    }
}

pub fn plain_fields() -> IpSendFields {
    IpSendFields {
        neighbour: NeighbourConfirmation::Unconfirmed,
        priority: PacketPriority(u32::MIN),
        transmit_time: None,
        transmit_reports: no_reports(),
    }
}

pub fn version_of(address: IpAddr) -> IpVersion {
    match address {
        IpAddr::V4(_) => IpVersion::V4,
        IpAddr::V6(_) => IpVersion::V6,
    }
}

fn opened<S: IcmpSockets>(sockets: &S, version: IpVersion) -> S::Socket {
    sockets.open(version, ROOT).expect("an echo socket opens")
}

/// Returns the unspecified address of the version, for any address, with no scope.
pub fn any_address(version: IpVersion) -> EchoBindAddress {
    match version {
        IpVersion::V4 => EchoBindAddress::V4(Ipv4Addr::UNSPECIFIED),
        IpVersion::V6 => EchoBindAddress::V6 {
            address: Ipv6Addr::UNSPECIFIED,
            scope: None,
        },
    }
}

/// Binds the socket to any address of its version and the identifier, alone or shared as asked.
fn bind_requested<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    version: IpVersion,
    identifier: EchoIdentifier,
    sharing: IdentifierSharing,
) -> Result<EchoIdentifier, HostError> {
    sockets.bind(
        socket,
        EchoBinding {
            address: any_address(version),
            identifier: LocalIdentifier::Requested(identifier),
            sharing,
        },
    )
}

/// Binds the socket to any address of its version and the identifier alone, which must be the one bound.
fn bind_alone<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    version: IpVersion,
    identifier: EchoIdentifier,
) {
    assert_eq!(
        bind_requested(
            sockets,
            socket,
            version,
            identifier,
            IdentifierSharing::Exclusive
        ),
        Ok(identifier),
        "{version:?}"
    );
}

fn set<S: IcmpSockets>(sockets: &S, socket: &S::Socket, setting: IpSocketSetting<'_>) {
    sockets
        .configure(socket, setting)
        .unwrap_or_else(|error| panic!("{setting:?}: {error:?}"));
}

/// Sends an echo request with the payload to the address, plainly.
fn ping<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    to: IpAddr,
    payload: &[u8],
) -> Result<(), SendError> {
    let message = echo(version_of(to), payload);
    sockets.send_buffer(socket, header_to(to, &message), plain_fields(), &message)
}

pub fn buffer() -> Vec<u8> {
    vec![u8::MIN; usize::from(u16::MAX)]
}

/// The calls that take volatile bytes, made with the suite's own buffers lent as a `RefCell` over a mutable slice.
pub trait LentBuffers: IcmpSockets {
    /// Sends a copy of the message, held in a buffer the suite owns.
    fn send_buffer(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        message: &[u8],
    ) -> Result<(), SendError> {
        let mut held = message.to_vec();
        self.send_to(socket, header, fields, &RefCell::new(&mut held[..]))
    }

    fn receive_buffer(
        &self,
        socket: &Self::Socket,
        into: &mut [u8],
    ) -> Result<Received, HostError> {
        self.receive(socket, &RefCell::new(into))
    }
}

impl<S: IcmpSockets> LentBuffers for S {}

/// A byte the buffer a source is given holds before it reads, which differs from what the suite's buffers start as.
pub const UNREAD: u8 = u8::MAX;

/// Bytes held in cells rather than a buffer, so that a source or a sink reaches them only through a buffer of its own.
///
/// Only the bytes a source says it read reach the cells, so a receive that tells too few loses what it wrote.
pub struct CellBytes {
    cells: Vec<Cell<u8>>,
}

impl CellBytes {
    pub fn new(bytes: &[u8]) -> Self {
        Self {
            cells: bytes.iter().copied().map(Cell::new).collect(),
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.cells.iter().map(Cell::get).collect()
    }
}

impl VolatileBytes for CellBytes {
    fn len(&self) -> u64 {
        self.cells.len() as u64
    }

    fn read(&self, into: &mut [u8]) -> usize {
        let count = into.len().min(self.cells.len());
        for (byte, cell) in into.iter_mut().zip(&self.cells) {
            *byte = cell.get();
        }
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let count = bytes.len().min(self.cells.len());
        for (cell, &byte) in self.cells.iter().zip(bytes) {
            cell.set(byte);
        }
        count
    }

    fn read_to<Sink: ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        sink.write(&self.to_vec())
    }

    fn write_from<Source: ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut buffer = vec![UNREAD; self.cells.len()];
        let count = source.read(&mut buffer)?;
        assert!(
            count <= buffer.len(),
            "a source read {count} bytes into {}",
            buffer.len()
        );
        self.write(&buffer[..count]);
        Ok(count)
    }
}

/// Waits for what the socket receives next, into the bytes.
fn receive_into<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    into: &impl VolatileBytes,
) -> Received {
    let start = Instant::now();
    loop {
        match sockets.receive(socket, into) {
            Err(HostError::WouldBlock) if start.elapsed() < PATIENCE => {
                std::thread::sleep(LOOK_AGAIN_SOON);
            }
            other => return other.expect("something arrives"),
        }
    }
}

/// Waits for the packet the socket receives next, into the bytes.
fn receive_packet_into<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    into: &impl VolatileBytes,
) -> ReceivedPacketFields {
    match receive_into(sockets, socket, into) {
        Received::Packet(fields) => fields,
        other => panic!("a packet, not {other:?}"),
    }
}

/// Waits for what the socket receives next.
fn receive<S: IcmpSockets>(sockets: &S, socket: &S::Socket, into: &mut [u8]) -> Received {
    receive_into(sockets, socket, &RefCell::new(into))
}

fn receive_packet<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    into: &mut [u8],
) -> ReceivedPacketFields {
    receive_packet_into(sockets, socket, &RefCell::new(into))
}

/// Answers whether nothing arrives at the socket for a while.
fn nothing_arrives<S: IcmpSockets>(sockets: &S, socket: &S::Socket) -> bool {
    std::thread::sleep(QUIET);
    sockets.receive_buffer(socket, &mut buffer()) == Err(HostError::WouldBlock)
}

/// An IP packet's version, source, destination, upper-layer protocol and the bytes after its headers.
pub struct Seen<'a> {
    pub source: IpAddr,
    pub destination: IpAddr,
    pub protocol: IpNumber,
    pub payload: &'a [u8],
    pub extensions: Vec<(IpNumber, &'a [u8])>,
}

pub fn seen(packet: &[u8]) -> Seen<'_> {
    if let Ok(header) = Ipv4HeaderSlice::from_slice(packet) {
        return Seen {
            source: IpAddr::V4(header.source_addr()),
            destination: IpAddr::V4(header.destination_addr()),
            protocol: header.protocol(),
            payload: &packet[header.slice().len()..],
            extensions: Vec::new(),
        };
    }
    let header = Ipv6HeaderSlice::from_slice(packet).expect("an IPv4 or IPv6 packet");
    let mut next = header.next_header();
    let mut rest = &packet[header.slice().len()..];
    let mut extensions = Vec::new();
    while matches!(
        next,
        IpNumber::IPV6_HEADER_HOP_BY_HOP
            | IpNumber::IPV6_ROUTE_HEADER
            | IpNumber::IPV6_DESTINATION_OPTIONS
            | IpNumber::IPV6_FRAGMENTATION_HEADER
    ) {
        let length = if next == IpNumber::IPV6_FRAGMENTATION_HEADER {
            etherparse::Ipv6FragmentHeader::LEN
        } else {
            Ipv6RawExtHeaderSlice::from_slice(rest)
                .expect("an extension header")
                .slice()
                .len()
        };
        extensions.push((next, &rest[..length]));
        next = IpNumber(rest[0]);
        rest = &rest[length..];
    }
    Seen {
        source: IpAddr::V6(header.source_addr()),
        destination: IpAddr::V6(header.destination_addr()),
        protocol: next,
        payload: rest,
        extensions,
    }
}

/// A packet socket that sees every packet leaving one link of the namespace, from the moment it is made.
pub struct Capture {
    socket: OwnedFd,
    link: NonZeroU32,
}

impl Capture {
    /// Sees the packets of every link, of which [`Capture::within`] keeps those of the link named.
    pub fn on(link: &str) -> Self {
        use nix::sys::socket::{AddressFamily, SockFlag, SockProtocol, SockType};
        let socket = nix::sys::socket::socket(
            AddressFamily::Packet,
            SockType::Datagram,
            SockFlag::SOCK_NONBLOCK | SockFlag::SOCK_CLOEXEC,
            SockProtocol::EthAll,
        )
        .expect("a packet socket opens");
        Self {
            socket,
            link: index(link).0,
        }
    }

    /// Returns the next packet leaving the link that `wanted` takes, if one leaves within the time given.
    pub fn within(&self, time: Duration, wanted: impl Fn(&[u8]) -> bool) -> Option<Vec<u8>> {
        let start = Instant::now();
        let mut packet = buffer();
        while start.elapsed() < time {
            let Ok((read, Some(address))) = nix::sys::socket::recvfrom::<nix::sys::socket::LinkAddr>(
                self.socket.as_raw_fd(),
                &mut packet,
            ) else {
                std::thread::sleep(LOOK_AGAIN_SOON);
                continue;
            };
            let bytes = &packet[..read];
            if usize::try_from(self.link.get()) == Ok(address.ifindex())
                && address.pkttype() == libc::PACKET_OUTGOING
                && wanted(bytes)
            {
                return Some(bytes.to_vec());
            }
        }
        None
    }

    pub fn next(&self, wanted: impl Fn(&[u8]) -> bool) -> Vec<u8> {
        self.within(PATIENCE, wanted)
            .expect("the packet leaves the link")
    }

    pub fn echo_request(&self) -> Vec<u8> {
        self.next(is_echo_request)
    }
}

/// Where a fragment belongs: its packet's identification, its offset in eight-byte units, and whether more follow.
#[derive(Clone, Copy, Debug)]
pub struct Fragment {
    pub identification: u32,
    pub offset: u16,
    pub more: bool,
}

/// Returns the fragment an IP packet is, or none for a whole packet.
pub fn fragment(packet: &[u8]) -> Option<Fragment> {
    if let Ok(header) = Ipv4HeaderSlice::from_slice(packet) {
        return header.is_fragmenting_payload().then(|| Fragment {
            identification: u32::from(header.identification()),
            offset: header.fragments_offset().value(),
            more: header.more_fragments(),
        });
    }
    seen(packet)
        .extensions
        .iter()
        .find(|(kind, _)| *kind == IpNumber::IPV6_FRAGMENTATION_HEADER)
        .map(|(_, bytes)| {
            let header = Ipv6FragmentHeaderSlice::from_slice(bytes).expect("a fragment header");
            Fragment {
                identification: header.identification(),
                offset: header.fragment_offset().value(),
                more: header.more_fragments(),
            }
        })
}

/// Returns the lengths, in order, of the fragments of the next packet leaving the link whose first fragment `first` takes.
pub fn fragment_lengths(capture: &Capture, first: impl Fn(&[u8]) -> bool) -> Vec<usize> {
    let start = capture.next(|packet| {
        fragment(packet).is_some_and(|fragment| fragment.offset == u16::MIN) && first(packet)
    });
    let identification = fragment(&start).expect("a fragment").identification;
    let mut lengths = vec![start.len()];
    let mut more = true;
    while more {
        let next = capture.next(|packet| {
            fragment(packet).is_some_and(|fragment| fragment.identification == identification)
        });
        more = fragment(&next).expect("a fragment").more;
        lengths.push(next.len());
    }
    lengths
}

/// Loopback's MTU in the checks that fragment on it, the least IPv6 allows.
pub const SMALL_LOOPBACK: &str = "link set lo mtu 1280";
/// A payload that [`SMALL_LOOPBACK`] fragments.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
pub const FRAGMENTED_PAYLOAD: [u8; 3000] = [u8::MAX; 3000];

fn is_echo_request(packet: &[u8]) -> bool {
    let seen = seen(packet);
    matches!(
        (seen.protocol, seen.payload.first()),
        (IpNumber::ICMP, Some(&icmpv4::TYPE_ECHO_REQUEST))
            | (IpNumber::IPV6_ICMP, Some(&icmpv6::TYPE_ECHO_REQUEST))
    )
}

pub fn ipv4(packet: &[u8]) -> Ipv4HeaderSlice<'_> {
    Ipv4HeaderSlice::from_slice(packet).expect("an IPv4 packet")
}

pub fn ipv6(packet: &[u8]) -> Ipv6HeaderSlice<'_> {
    Ipv6HeaderSlice::from_slice(packet).expect("an IPv6 packet")
}

/// Returns the echo identifier of an ICMP echo message.
fn identifier_of(message: &[u8]) -> u16 {
    u16::from_be_bytes(
        message[IDENTIFIER_BYTES]
            .try_into()
            .expect("an echo message's identifier"),
    )
}

/// Accepts an honest refusal of what a host may be unable to send, and checks anything else.
fn unless_refused<T>(result: Result<T, SendError>, check: impl FnOnce(Result<T, SendError>)) {
    if !matches!(result, Err(SendError::Host(HostError::NotSupported))) {
        check(result);
    }
}

pub fn checks_out(message: &[u8], seen: &Seen<'_>) -> bool {
    match (seen.source, seen.destination) {
        (IpAddr::V6(source), IpAddr::V6(destination)) => {
            etherparse::checksum::Sum16BitWords::new()
                .add_16bytes(source.octets())
                .add_16bytes(destination.octets())
                .add_4bytes((message.len() as u32).to_be_bytes())
                .add_2bytes([u8::MIN, IpNumber::IPV6_ICMP.0])
                .add_slice(message)
                .ones_complement()
                == u16::MIN
        }
        _ => {
            etherparse::checksum::Sum16BitWords::new()
                .add_slice(message)
                .ones_complement()
                == u16::MIN
        }
    }
}

/// Checks that the IP headers given before a packet's payload are those it had on the wire.
///
/// Linux does not give an IPv4 header's identification and fragment fields; the checksum must hold over what is given.
pub fn headers_are_as_on_the_wire(packet: &[u8], on_the_wire: &[u8]) {
    let headers = packet.len() - seen(packet).payload.len();
    assert_eq!(headers, on_the_wire.len() - seen(on_the_wire).payload.len());
    match version_of(seen(packet).source) {
        IpVersion::V4 => {
            // Every field but the identification, the fragment flags and offset, and the checksum over them.
            // https://www.rfc-editor.org/rfc/rfc791#section-3.1
            const NOT_COMPARED: &[core::ops::Range<usize>] = &[4..8, 10..12];
            let compared = |header: &[u8]| {
                let mut header = header[..headers].to_vec();
                for range in NOT_COMPARED {
                    header[range.clone()].fill(u8::MIN);
                }
                header
            };
            assert_eq!(compared(packet), compared(on_the_wire));
            let given = ipv4(packet);
            assert_eq!(
                given.header_checksum(),
                given.to_header().calc_header_checksum()
            );
        }
        IpVersion::V6 => assert_eq!(&packet[..headers], &on_the_wire[..headers]),
    }
}

/// Returns an echo request to a local address gets its reply, written whole from its IP header as it was on the wire, with the identifier bound.
fn reply_arrives_whole<S: IcmpSockets>(sockets: S, to: IpAddr, options: &Ipv4Options) {
    let version = version_of(to);
    let socket = opened(&sockets, version);
    bind_alone(&sockets, &socket, version, IDENTIFIER);
    let wire = Capture::on(SELF_LINK);
    let request = echo(version, PAYLOAD);
    // A reply takes its request's traffic class, which is not zero, so that the received one is the wire's.
    let header = match to {
        IpAddr::V4(address) => IpHeaderFields::V4(v4_header(Ipv4RouteSelectors {
            options,
            dscp: IpDscp::EF,
            ecn: IpEcn::Ect0,
            ..v4_route(address, &request)
        })),
        IpAddr::V6(address) => IpHeaderFields::V6(v6_header(Ipv6RouteSelectors {
            dscp: IpDscp::EF,
            ecn: IpEcn::Ect1,
            ..v6_route(address, &request)
        })),
    };
    sockets
        .send_buffer(&socket, header, plain_fields(), &request)
        .expect("the request is sent");
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    let packet = &into[..fields.length];
    let seen = seen(packet);
    assert_eq!(
        (seen.source, seen.destination, seen.protocol),
        (to, to, protocol(version))
    );
    let reply = match version {
        IpVersion::V4 => icmpv4::TYPE_ECHO_REPLY,
        IpVersion::V6 => icmpv6::TYPE_ECHO_REPLY,
    };
    let on_the_wire = wire.next(|packet| {
        let seen = crate::seen(packet);
        seen.protocol == protocol(version) && seen.payload.first() == Some(&reply)
    });
    assert_eq!(packet.len(), on_the_wire.len());
    headers_are_as_on_the_wire(packet, &on_the_wire);
    match version {
        IpVersion::V4 => {
            assert_eq!(ipv4(packet).options().len(), options.len());
            assert_eq!(
                (ipv4(packet).dcp(), ipv4(packet).ecn()),
                (IpDscp::EF, IpEcn::Ect0)
            );
        }
        IpVersion::V6 => assert_eq!(
            (ipv6(packet).dscp(), ipv6(packet).ecn()),
            (IpDscp::EF, IpEcn::Ect1)
        ),
    }
    let message = seen.payload;
    assert_eq!((message[0], message[1]), (reply, u8::MIN));
    assert_eq!(identifier_of(message), IDENTIFIER.0.get());
    assert_eq!(
        &message[ICMP_HEADER - 2..ICMP_HEADER],
        SEQUENCE.to_be_bytes()
    );
    assert_eq!(&message[ICMP_HEADER..], PAYLOAD);
    assert!(checks_out(message, &seen), "{message:?}");
    let length = match version {
        IpVersion::V4 => usize::from(ipv4(packet).total_len()),
        IpVersion::V6 => usize::from(ipv6(packet).payload_length()) + etherparse::Ipv6Header::LEN,
    };
    assert_eq!(length, fields.length);
    assert_eq!(fields.largest_fragment, None, "a reply in one piece");
    // The preferred source of a reply to the packet, which only IPv4 has, if it is told at all.
    let preferred = match to {
        IpAddr::V4(address) => Some(address),
        IpAddr::V6(_) => None,
    };
    assert!(
        fields.specific_destination.is_none() || fields.specific_destination == preferred,
        "{:?}",
        fields.specific_destination
    );
    assert_eq!(fields.interface, index(SELF_LINK));
}

/// Checks that an IPv4 echo request to loopback gets its reply, whole from its IPv4 header.
pub fn echo_reply_arrives_whole_over_ipv4<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    reply_arrives_whole(sockets, IpAddr::V4(Ipv4Addr::LOCALHOST), &NO_OPTIONS);
}

/// A record route option with room for two addresses, then the end of the options.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const RECORD_ROUTE: [u8; 12] = [7, 11, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// Checks that an IPv4 echo request with a record route option gets its reply with the option Linux echoes, as on the wire.
pub fn an_echo_reply_arrives_with_its_ipv4_options<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    reply_arrives_whole(
        sockets,
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        &Ipv4Options::from(RECORD_ROUTE),
    );
}

/// Checks that an IPv6 echo request to loopback gets its reply, whole from its IPv6 header.
pub fn echo_reply_arrives_whole_over_ipv6<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    reply_arrives_whole(sockets, IpAddr::V6(Ipv6Addr::LOCALHOST), &NO_OPTIONS);
}

/// Checks that a reply that arrived in fragments tells the length of the largest, as it was on the wire.
pub fn a_fragmented_reply_tells_its_largest_fragment<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(SMALL_LOOPBACK);
    for (to, reply) in [
        (IpAddr::V4(Ipv4Addr::LOCALHOST), icmpv4::TYPE_ECHO_REPLY),
        (IpAddr::V6(Ipv6Addr::LOCALHOST), icmpv6::TYPE_ECHO_REPLY),
    ] {
        let socket = opened(&sockets, version_of(to));
        let wire = Capture::on(SELF_LINK);
        ping(&sockets, &socket, to, &FRAGMENTED_PAYLOAD).expect("the request is sent");
        let fields = receive_packet(&sockets, &socket, &mut buffer());
        let lengths =
            fragment_lengths(&wire, |packet| seen(packet).payload.first() == Some(&reply));
        assert!(lengths.len() > 1, "{to}: {lengths:?}");
        let largest = lengths
            .iter()
            .max()
            .map(|&length| FragmentSize(length as u16));
        assert_eq!(fields.largest_fragment, largest, "{to}: {lengths:?}");
    }
}

/// Checks that the request that leaves carries the bound identifier and the message's sequence number and payload.
pub fn the_request_carries_the_bound_identifier<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        let identifier = identifier(version_of(to));
        bind_alone(&sockets, &socket, version_of(to), identifier);
        let capture = Capture::on(DUMMY);
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        let request = capture.echo_request();
        let seen = seen(&request);
        assert_eq!(seen.destination, to);
        assert_eq!(identifier_of(seen.payload), identifier.0.get());
        assert_eq!(&seen.payload[ICMP_HEADER..], PAYLOAD);
        assert!(checks_out(seen.payload, &seen));
    }
}

/// Checks that two sockets take one identifier only when both share it.
pub fn identifiers_are_shared_only_when_both_ask<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for version in [IpVersion::V4, IpVersion::V6] {
        let first = opened(&sockets, version);
        let second = opened(&sockets, version);
        let identifier = identifier(version);
        bind_alone(&sockets, &first, version, identifier);
        assert_eq!(
            bind_requested(
                &sockets,
                &second,
                version,
                identifier,
                IdentifierSharing::Shared
            ),
            Err(HostError::Errno(libc::EADDRINUSE))
        );
        let third = opened(&sockets, version);
        let fourth = opened(&sockets, version);
        let shared = EchoIdentifier(identifier.0.saturating_add(u16::from(u8::MAX)));
        for socket in [&third, &fourth] {
            assert_eq!(
                bind_requested(&sockets, socket, version, shared, IdentifierSharing::Shared),
                Ok(shared),
                "a shared identifier binds twice"
            );
        }
    }
}

/// Checks that closing a socket frees its identifier, at once or once no child forked meanwhile holds its descriptor.
pub fn close_frees_the_identifier<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for version in [IpVersion::V4, IpVersion::V6] {
        let first = opened(&sockets, version);
        let identifier = identifier(version);
        bind_alone(&sockets, &first, version, identifier);
        let second = opened(&sockets, version);
        let alone = IdentifierSharing::Exclusive;
        assert_eq!(
            bind_requested(&sockets, &second, version, identifier, alone),
            Err(HostError::Errno(libc::EADDRINUSE))
        );
        sockets.close(first);
        let start = Instant::now();
        let rebound = loop {
            match bind_requested(&sockets, &second, version, identifier, alone) {
                Err(HostError::Errno(libc::EADDRINUSE)) if start.elapsed() < PATIENCE => {
                    std::thread::sleep(LOOK_AGAIN_SOON);
                }
                other => break other,
            }
        };
        assert_eq!(rebound, Ok(identifier), "{version:?}");
    }
}

/// Checks that an identifier the exit node chooses is returned, bound and carried by the request that leaves.
pub fn the_exit_node_chooses_an_identifier_when_asked<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        let chosen = sockets
            .bind(
                &socket,
                EchoBinding {
                    address: any_address(version),
                    identifier: LocalIdentifier::ChosenByExitNode,
                    sharing: IdentifierSharing::Exclusive,
                },
            )
            .expect("an identifier is chosen");
        let capture = Capture::on(DUMMY);
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        assert_eq!(
            identifier_of(seen(&capture.echo_request()).payload),
            chosen.0.get()
        );
        let other = opened(&sockets, version);
        assert_eq!(
            bind_requested(
                &sockets,
                &other,
                version,
                chosen,
                IdentifierSharing::Exclusive
            ),
            Err(HostError::Errno(libc::EADDRINUSE)),
            "{to}"
        );
        // Linux chooses a free identifier, so another socket that asks for one is given another.
        let again = opened(&sockets, version);
        let another = sockets
            .bind(
                &again,
                EchoBinding {
                    address: any_address(version),
                    identifier: LocalIdentifier::ChosenByExitNode,
                    sharing: IdentifierSharing::Exclusive,
                },
            )
            .expect("another identifier is chosen");
        assert_ne!(another, chosen, "{to}");
    }
}

/// A second address of the dummy link, of each version, for the checks that bind one.
const SECOND_ADDRESSES: &str = "address add 10.3.0.5/24 dev ia
address add fd03::5/64 dev ia nodad";
// https://www.rfc-editor.org/rfc/rfc1918#section-3
const SECOND_LOCAL: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 5);
// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
const SECOND_LOCAL_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 5);

/// Checks that a socket bound to an address of the namespace sends its requests from it.
pub fn a_bound_address_is_the_requests_source<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(SECOND_ADDRESSES);
    for (to, address, from) in [
        (
            IpAddr::V4(ON_LINK),
            EchoBindAddress::V4(SECOND_LOCAL),
            IpAddr::V4(SECOND_LOCAL),
        ),
        (
            IpAddr::V6(ON_LINK_V6),
            EchoBindAddress::V6 {
                address: SECOND_LOCAL_V6,
                scope: None,
            },
            IpAddr::V6(SECOND_LOCAL_V6),
        ),
    ] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        let identifier = identifier(version);
        let binding = EchoBinding {
            address,
            identifier: LocalIdentifier::Requested(identifier),
            sharing: IdentifierSharing::Exclusive,
        };
        assert_eq!(sockets.bind(&socket, binding), Ok(identifier));
        let capture = Capture::on(DUMMY);
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        let request = capture.echo_request();
        assert_eq!(seen(&request).source, from);
        assert_eq!(identifier_of(seen(&request).payload), identifier.0.get());
    }
}

// https://www.rfc-editor.org/rfc/rfc4291#section-2.5.6
const LINK_LOCAL: Ipv6Addr = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 5);
/// [`LINK_LOCAL`] on the second link, and on the dummy link.
const LINK_LOCAL_ON_SECOND: &str = "address add fe80::5/64 dev ib nodad";
const LINK_LOCAL_ON_DUMMY: &str = "address add fe80::5/64 dev ia nodad";
// https://www.rfc-editor.org/rfc/rfc1112#section-4
const ALL_HOSTS: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 1);

/// Checks that a link-local address binds the socket to the link of its scope, and any other address does not.
///
/// The requests of the first leave by the second link though the first is preferred; the others' by the first.
pub fn a_link_local_address_binds_the_socket_to_its_link<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(TWO_ROUTES);
    ip(LINK_LOCAL_ON_SECOND);
    let second = index(SECOND);
    for (address, link) in [
        (LINK_LOCAL, SECOND),
        // Linux only checks that the interface of another address's scope exists.
        (LOCAL_V6, DUMMY),
        (Ipv6Addr::UNSPECIFIED, DUMMY),
    ] {
        let socket = opened(&sockets, IpVersion::V6);
        let binding = EchoBinding {
            address: EchoBindAddress::V6 {
                address,
                scope: Some(second),
            },
            identifier: LocalIdentifier::ChosenByExitNode,
            sharing: IdentifierSharing::Exclusive,
        };
        sockets
            .bind(&socket, binding)
            .unwrap_or_else(|error| panic!("{address} binds: {error:?}"));
        let capture = Capture::on(link);
        ping(&sockets, &socket, IpAddr::V6(NOWHERE_V6), PAYLOAD).expect("the request is sent");
        let request = capture.echo_request();
        assert_eq!(
            seen(&request).destination,
            IpAddr::V6(NOWHERE_V6),
            "{address}"
        );
    }
}

/// Checks that bind refuses, with Linux's errors, what Linux's ping sockets refuse to bind.
pub fn binding_refuses_what_linux_refuses<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(TWO_ROUTES);
    ip(LINK_LOCAL_ON_DUMMY);
    let v6 = |address, scope| EchoBindAddress::V6 { address, scope };
    let no_link = InterfaceIndex(NonZeroU32::MAX);
    for (version, address, error) in [
        // An address the namespace does not have, and a group.
        (
            IpVersion::V4,
            EchoBindAddress::V4(NOWHERE),
            libc::EADDRNOTAVAIL,
        ),
        (
            IpVersion::V4,
            EchoBindAddress::V4(ALL_HOSTS),
            libc::EADDRNOTAVAIL,
        ),
        (IpVersion::V6, v6(NOWHERE_V6, None), libc::EADDRNOTAVAIL),
        // A scope no link has, for any address.
        (IpVersion::V6, v6(LOCAL_V6, Some(no_link)), libc::ENODEV),
        // A link-local address without its link, and with another.
        (IpVersion::V6, v6(LINK_LOCAL, None), libc::EINVAL),
        (
            IpVersion::V6,
            v6(LINK_LOCAL, Some(index(SECOND))),
            libc::EADDRNOTAVAIL,
        ),
        // An IPv4-mapped address, which a ping socket of IPv6 never sends from.
        (
            IpVersion::V6,
            v6(LOCAL.to_ipv6_mapped(), None),
            libc::EINVAL,
        ),
        // An address of the other version: Linux reads its family on an IPv4 socket and its length on an IPv6 one.
        (IpVersion::V4, v6(LOCAL_V6, None), libc::EAFNOSUPPORT),
        (IpVersion::V6, EchoBindAddress::V4(LOCAL), libc::EINVAL),
    ] {
        let socket = opened(&sockets, version);
        let binding = EchoBinding {
            address,
            identifier: LocalIdentifier::ChosenByExitNode,
            sharing: IdentifierSharing::Exclusive,
        };
        assert_eq!(
            sockets.bind(&socket, binding),
            Err(HostError::Errno(error)),
            "{address:?} on {version:?}"
        );
    }
    for version in [IpVersion::V4, IpVersion::V6] {
        let socket = opened(&sockets, version);
        let binding = EchoBinding {
            address: any_address(version),
            identifier: LocalIdentifier::ChosenByExitNode,
            sharing: IdentifierSharing::Exclusive,
        };
        sockets.bind(&socket, binding).expect("the first bind");
        assert_eq!(
            sockets.bind(&socket, binding),
            Err(HostError::Errno(libc::EINVAL)),
            "a second bind on {version:?}"
        );
    }
}

/// Reads a whole number from a file of `/proc/sys`.
fn sysctl_number(path: &str) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{path}: {error}"))
        .trim()
        .parse()
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// Checks the buffer sizes `read` gives: the namespace's defaults, then what `configure` asked, held to its limits and doubled.
pub fn buffer_sizes_are_read_back(
    read: impl Fn() -> BufferSizes,
    configure: impl Fn(IpSocketSetting<'_>),
) {
    // https://man7.org/linux/man-pages/man7/socket.7.html#:~:text=wmem_default
    const LIMITS: [&str; 4] = [
        "/proc/sys/net/core/wmem_default",
        "/proc/sys/net/core/rmem_default",
        "/proc/sys/net/core/wmem_max",
        "/proc/sys/net/core/rmem_max",
    ];
    let [send_default, receive_default, send_max, receive_max] = LIMITS.map(sysctl_number);
    assert_eq!(
        read(),
        BufferSizes {
            send: ByteCount(send_default),
            receive: ByteCount(receive_default),
        }
    );
    for (send, receive) in [
        (usize::from(u16::MAX), usize::from(u16::MAX) * 2),
        (send_max.saturating_mul(2), receive_max.saturating_mul(2)),
    ] {
        configure(IpSocketSetting::SendBuffer(ByteCount(send)));
        configure(IpSocketSetting::ReceiveBuffer(ByteCount(receive)));
        assert_eq!(
            read(),
            BufferSizes {
                send: ByteCount(2 * send.min(send_max)),
                receive: ByteCount(2 * receive.min(receive_max)),
            },
            "asked {send} and {receive}"
        );
    }
}

/// Checks that a socket's buffer sizes read back as Linux gives them.
pub fn buffer_sizes_are_read_as_linux_gives_them<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for version in [IpVersion::V4, IpVersion::V6] {
        let socket = opened(&sockets, version);
        buffer_sizes_are_read_back(
            || sockets.read_buffer_sizes(&socket),
            |setting| set(&sockets, &socket, setting),
        );
    }
}

pub fn receive_with_nothing_queued_would_block<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for version in [IpVersion::V4, IpVersion::V6] {
        let socket = opened(&sockets, version);
        assert_eq!(
            sockets.receive_buffer(&socket, &mut buffer()),
            Err(HostError::WouldBlock)
        );
    }
}

/// Where an IPv4 header keeps its total length.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const IPV4_TOTAL_LENGTH: core::ops::Range<usize> = 2..4;
/// Where an IPv6 header keeps its payload length.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4
const IPV6_PAYLOAD_LENGTH: core::ops::Range<usize> = 4..6;

/// Returns the whole length of a packet that the start of its header tells.
fn told_length(version: IpVersion, start: &[u8]) -> usize {
    let field = |range: core::ops::Range<usize>| {
        usize::from(u16::from_be_bytes(
            start[range].try_into().expect("a length of two bytes"),
        ))
    };
    match version {
        IpVersion::V4 => field(IPV4_TOTAL_LENGTH),
        IpVersion::V6 => field(IPV6_PAYLOAD_LENGTH) + etherparse::Ipv6Header::LEN,
    }
}

/// Checks that a buffer that cuts the reply in its IP header or ICMP message takes its start and tells the whole length.
///
/// Both the given header's total length and the received length tell it.
pub fn a_short_buffer_keeps_the_whole_length<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for (to, header, reply) in [
        (
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            etherparse::Ipv4Header::MIN_LEN,
            icmpv4::TYPE_ECHO_REPLY,
        ),
        (
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            etherparse::Ipv6Header::LEN,
            icmpv6::TYPE_ECHO_REPLY,
        ),
    ] {
        let socket = opened(&sockets, version_of(to));
        // Longer than the header, so that the message is cut by a buffer that holds the header and more.
        let payload = vec![u8::MAX; header + PAYLOAD.len()];
        let whole = header + ICMP_HEADER + payload.len();
        for short in [header / 2, header + ICMP_HEADER] {
            ping(&sockets, &socket, to, &payload).unwrap();
            let mut into = vec![u8::MIN; short];
            let fields = receive_packet(&sockets, &socket, &mut into);
            assert_eq!(fields.length, whole, "{to} into {short}");
            assert_eq!(
                told_length(version_of(to), &into),
                whole,
                "{to} into {short}"
            );
            if short > header {
                assert_eq!(into[header], reply, "{to}");
            }
        }
    }
}

/// Checks that the TTL, DSCP, ECN, options and source of an IPv4 send are in the header that leaves.
pub fn ipv4_header_fields_go_out<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    /// A router alert option, then the end of the options.
    ///
    /// https://www.rfc-editor.org/rfc/rfc2113
    const ROUTER_ALERT: [u8; 4] = [148, 4, 0, 0];
    let options = Ipv4Options::from(ROUTER_ALERT);
    let socket = opened(&sockets, IpVersion::V4);
    let message = echo(IpVersion::V4, PAYLOAD);
    let route = Ipv4RouteSelectors {
        source: Some(LOCAL),
        dscp: IpDscp::try_new(IpDscp::MAX_U8).unwrap(),
        ecn: IpEcn::Ect0,
        options: &options,
        ..v4_route(ON_LINK, &message)
    };
    let header = Ipv4HeaderFields {
        time_to_live: Some(TIME_TO_LIVE),
        ..v4_header(route)
    };
    let capture = Capture::on(DUMMY);
    sockets
        .send_buffer(
            &socket,
            IpHeaderFields::V4(header),
            plain_fields(),
            &message,
        )
        .expect("the request is sent");
    let request = capture.echo_request();
    let header = ipv4(&request);
    assert_eq!(header.ttl(), TIME_TO_LIVE.0);
    assert_eq!((header.dcp(), header.ecn()), (route.dscp, route.ecn));
    assert_eq!(header.options(), ROUTER_ALERT);
    assert_eq!(header.source_addr(), LOCAL);
    let socket = opened(&sockets, IpVersion::V4);
    ping(&sockets, &socket, IpAddr::V4(ON_LINK), PAYLOAD).unwrap();
    let plain = capture.echo_request();
    assert!(ipv4(&plain).options().is_empty());
    assert_eq!(ipv4(&plain).ttl(), default_ttl());
}

/// Returns the namespace's default TTL, as a socket that never set its own reads it.
fn default_ttl() -> u8 {
    let socket = rustix::net::socket(
        rustix::net::AddressFamily::INET,
        rustix::net::SocketType::DGRAM,
        None,
    )
    .expect("a socket opens");
    rustix::net::sockopt::ip_ttl(&socket).expect("the default TTL") as u8
}

/// Checks that the hop limit, DSCP, ECN and flow label of an IPv6 send are in the header that leaves.
pub fn ipv6_header_fields_go_out<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6);
    let message = echo(IpVersion::V6, PAYLOAD);
    let route = Ipv6RouteSelectors {
        dscp: IpDscp::try_new(IpDscp::MAX_U8).unwrap(),
        ecn: IpEcn::Ect1,
        flow_label: Ipv6FlowLabel::try_new(FLOW_LABEL).unwrap(),
        ..v6_route(ON_LINK_V6, &message)
    };
    let header = Ipv6HeaderFields {
        hop_limit: Some(HOP_LIMIT),
        ..v6_header(route.clone())
    };
    let capture = Capture::on(DUMMY);
    sockets
        .send_buffer(
            &socket,
            IpHeaderFields::V6(header),
            plain_fields(),
            &message,
        )
        .expect("the request is sent");
    let request = capture.echo_request();
    let header = ipv6(&request);
    assert_eq!(header.hop_limit(), HOP_LIMIT.0);
    assert_eq!((header.dscp(), header.ecn()), (route.dscp, route.ecn));
    assert_eq!(header.flow_label(), route.flow_label);
}

/// The last segment of a segment routing header the checks send, beyond the link.
///
/// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
pub const FAR_V6_SEGMENT: Ipv6Addr = Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 9);
/// An IPv6 extension header of eight bytes: next header, length, then padding (PadN of four).
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4
pub const PADDING_HEADER: [u8; 8] = [0, 0, 1, 4, 0, 0, 0, 0];

/// A routing header's length in eight-byte units after its first eight, for two segments.
///
/// https://www.rfc-editor.org/rfc/rfc8754#section-2
pub const TWO_SEGMENTS: u8 = 4;
/// The segments left and the last entry's index of a routing header of two segments, at its first.
///
/// https://www.rfc-editor.org/rfc/rfc8754#section-2
const AT_THE_FIRST_OF_TWO: [u8; 2] = [1, 1];
/// The length of a segment routing header's flags and tag.
///
/// https://www.rfc-editor.org/rfc/rfc8754#section-2
const FLAGS_AND_TAG: usize = 3;

/// Returns a segment routing header of two segments, [`FAR_V6_SEGMENT`] last and [`ON_LINK_V6`] next.
pub fn segment_routing_header() -> Vec<u8> {
    let mut routing = vec![
        u8::MIN,
        TWO_SEGMENTS,
        linux_raw_sys::net::IPV6_SRCRT_TYPE_4 as u8,
    ];
    routing.extend_from_slice(&AT_THE_FIRST_OF_TWO);
    // The flags and the tag, none.
    routing.extend_from_slice(&[u8::MIN; FLAGS_AND_TAG]);
    routing.extend_from_slice(&FAR_V6_SEGMENT.octets());
    routing.extend_from_slice(&ON_LINK_V6.octets());
    routing
}

/// Returns the kinds of the extension headers a packet carries, in order.
pub fn extension_kinds(packet: &[u8]) -> Vec<IpNumber> {
    seen(packet)
        .extensions
        .iter()
        .map(|(kind, _)| *kind)
        .collect()
}

/// Checks that IPv6 extension headers and a routing header go out with the request, or are refused.
pub fn ipv6_extension_headers_go_out<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    let padding = Ipv6RawExtHeaderSlice::from_slice(&PADDING_HEADER).unwrap();
    let routing = segment_routing_header();
    let routing = Ipv6RawExtHeaderSlice::from_slice(&routing).unwrap();
    let message = echo(IpVersion::V6, PAYLOAD);
    let socket = opened(&sockets, IpVersion::V6);
    let capture = Capture::on(DUMMY);
    let header = Ipv6HeaderFields {
        hop_by_hop_options: Some(padding.clone()),
        routing_destination_options: Some(padding.clone()),
        destination_options: Some(padding),
        ..v6_header(Ipv6RouteSelectors {
            routing_header: Some(routing),
            ..v6_route(FAR_V6_SEGMENT, &message)
        })
    };
    let sent = sockets.send_buffer(
        &socket,
        IpHeaderFields::V6(header),
        plain_fields(),
        &message,
    );
    unless_refused(sent, |sent| {
        sent.expect("the request is sent");
        let request = capture.echo_request();
        assert_eq!(ipv6(&request).destination_addr(), ON_LINK_V6);
        assert_eq!(
            extension_kinds(&request),
            [
                IpNumber::IPV6_HEADER_HOP_BY_HOP,
                IpNumber::IPV6_DESTINATION_OPTIONS,
                IpNumber::IPV6_ROUTE_HEADER,
                IpNumber::IPV6_DESTINATION_OPTIONS
            ]
        );
    });
    let socket = opened(&sockets, IpVersion::V6);
    ping(&sockets, &socket, IpAddr::V6(ON_LINK_V6), PAYLOAD).unwrap();
    assert!(extension_kinds(&capture.echo_request()).is_empty());
}

/// Checks that an IPv6 request leaves from the local source its send gives, or that the source is refused.
///
/// Of two addresses on the link, either is taken when asked for, whichever the host would choose by itself.
pub fn an_ipv6_source_goes_out<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    const SECOND_ADDRESS: &str = "address add fd03::5/64 dev ia nodad";
    ip(SECOND_ADDRESS);
    // https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const SOURCE: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 5);
    let message = echo(IpVersion::V6, PAYLOAD);
    let socket = opened(&sockets, IpVersion::V6);
    let capture = Capture::on(DUMMY);
    for source in [SOURCE, LOCAL_V6] {
        let route = Ipv6RouteSelectors {
            source: Some(source),
            ..v6_route(ON_LINK_V6, &message)
        };
        let sent = sockets.send_buffer(
            &socket,
            IpHeaderFields::V6(v6_header(route)),
            plain_fields(),
            &message,
        );
        unless_refused(sent, |sent| {
            sent.expect("the request is sent");
            assert_eq!(seen(&capture.echo_request()).source, IpAddr::V6(source));
        });
    }
}

/// Checks that each path MTU discovery mode sends, with the don't-fragment bit on IPv4 only for the modes that set it.
pub fn every_path_mtu_discovery_mode_sends<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    let capture = Capture::on(DUMMY);
    for (mode, dont_fragment) in [
        (PathMtuDiscovery::Dont, false),
        (PathMtuDiscovery::Want, true),
        (PathMtuDiscovery::Do, true),
        (PathMtuDiscovery::Probe, true),
        (PathMtuDiscovery::Interface, false),
        (PathMtuDiscovery::Omit, false),
    ] {
        let socket = opened(&sockets, IpVersion::V4);
        let message = echo(IpVersion::V4, PAYLOAD);
        let header = Ipv4HeaderFields {
            path_mtu_discovery: mode,
            ..v4_header(v4_route(ON_LINK, &message))
        };
        sockets
            .send_buffer(
                &socket,
                IpHeaderFields::V4(header),
                plain_fields(),
                &message,
            )
            .expect("the request is sent");
        assert_eq!(
            ipv4(&capture.echo_request()).dont_fragment(),
            dont_fragment,
            "{mode:?}"
        );
        let socket = opened(&sockets, IpVersion::V6);
        let message = echo(IpVersion::V6, PAYLOAD);
        let header = Ipv6HeaderFields {
            path_mtu_discovery: mode,
            ..v6_header(v6_route(ON_LINK_V6, &message))
        };
        sockets
            .send_buffer(
                &socket,
                IpHeaderFields::V6(header),
                plain_fields(),
                &message,
            )
            .expect("the request is sent");
        capture.echo_request();
    }
}

pub const PATH_MTU_MODES: &[PathMtuDiscovery] = &[
    PathMtuDiscovery::Dont,
    PathMtuDiscovery::Want,
    PathMtuDiscovery::Do,
    PathMtuDiscovery::Probe,
    PathMtuDiscovery::Interface,
    PathMtuDiscovery::Omit,
];
/// The dummy link at Linux's usual MTU, with routes out of it to [`NARROW`] and [`NARROW_V6`] of a smaller MTU.
pub const NARROW_ROUTES: &str = "link set ia mtu 1500
route add 10.4.0.0/16 dev ia mtu 1300
route add fd04::/64 dev ia mtu 1300";
// https://www.rfc-editor.org/rfc/rfc1918#section-3
pub const NARROW: Ipv4Addr = Ipv4Addr::new(10, 4, 0, 1);
// https://www.rfc-editor.org/rfc/rfc4193#section-3.1
pub const NARROW_V6: Ipv6Addr = Ipv6Addr::new(0xfd04, 0, 0, 0, 0, 0, 0, 1);
/// The MTUs [`NARROW_ROUTES`] sets, of the routes and of the link.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
const ROUTE_MTU: usize = 1300;
// https://man7.org/linux/man-pages/man7/ip.7.html
const NARROW_LINK_MTU: usize = 1500;
/// IP packet lengths over the route's MTU alone, and over the link's too.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
const OVER_THE_ROUTE: usize = 1400;
// https://man7.org/linux/man-pages/man7/ip.7.html
const OVER_THE_LINK: usize = 1600;

/// Returns what each path MTU discovery mode makes of a packet over its route's MTU and of one over its link's.
///
/// `send` sends one of the length given toward [`NARROW`] or [`NARROW_V6`]. Dont and want fragment to the route's MTU;
/// do is too long for it; probe (with the don't-fragment bit on IPv4) and interface leave it whole and are too long for
/// the link's; omit leaves it whole and fragments to the link's MTU.
pub fn path_mtu_modes_take_their_mtu(
    send: impl Fn(IpAddr, PathMtuDiscovery, usize) -> Result<(), SendError>,
) {
    ip(NARROW_ROUTES);
    let capture = Capture::on(DUMMY);
    let too_long = |mtu: usize| {
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: MaximumTransmissionUnit(mtu as u32),
        })
    };
    for to in [IpAddr::V4(NARROW), IpAddr::V6(NARROW_V6)] {
        let toward = |packet: &[u8]| seen(packet).destination == to;
        for &mode in PATH_MTU_MODES {
            for length in [OVER_THE_ROUTE, OVER_THE_LINK] {
                let sent = send(to, mode, length);
                let context = format!("{to} {mode:?} {length}");
                match (mode, length > NARROW_LINK_MTU) {
                    (PathMtuDiscovery::Do, _) => assert_eq!(sent, too_long(ROUTE_MTU), "{context}"),
                    (PathMtuDiscovery::Probe | PathMtuDiscovery::Interface, true) => {
                        assert_eq!(sent, too_long(NARROW_LINK_MTU), "{context}")
                    }
                    (
                        PathMtuDiscovery::Probe
                        | PathMtuDiscovery::Interface
                        | PathMtuDiscovery::Omit,
                        false,
                    ) => {
                        sent.unwrap_or_else(|error| panic!("{context}: {error:?}"));
                        let packet = capture.next(toward);
                        assert!(fragment(&packet).is_none(), "{context}");
                        assert_eq!(packet.len(), length, "{context}");
                        if to.is_ipv4() {
                            assert_eq!(
                                ipv4(&packet).dont_fragment(),
                                mode == PathMtuDiscovery::Probe,
                                "{context}"
                            );
                        }
                    }
                    (PathMtuDiscovery::Omit, true)
                    | (PathMtuDiscovery::Dont | PathMtuDiscovery::Want, _) => {
                        sent.unwrap_or_else(|error| panic!("{context}: {error:?}"));
                        let lengths = fragment_lengths(&capture, toward);
                        let largest = lengths.iter().max().copied().unwrap_or_default();
                        if mode == PathMtuDiscovery::Omit {
                            assert!(
                                largest > ROUTE_MTU && largest <= NARROW_LINK_MTU,
                                "{context}: {lengths:?}"
                            );
                        } else {
                            assert!(largest <= ROUTE_MTU, "{context}: {lengths:?}");
                        }
                    }
                }
            }
        }
    }
}

/// Checks that each path MTU discovery mode takes the MTU of the route or of the link, and fragments or refuses, as Linux does.
pub fn each_path_mtu_discovery_mode_takes_its_mtu<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    path_mtu_modes_take_their_mtu(|to, mode, length| {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        let headers = match version {
            IpVersion::V4 => etherparse::Ipv4Header::MIN_LEN,
            IpVersion::V6 => etherparse::Ipv6Header::LEN,
        };
        let message = echo(version, &vec![u8::MAX; length - headers - ICMP_HEADER]);
        let header = match header_to(to, &message) {
            IpHeaderFields::V4(header) => IpHeaderFields::V4(Ipv4HeaderFields {
                path_mtu_discovery: mode,
                ..header
            }),
            IpHeaderFields::V6(header) => IpHeaderFields::V6(Ipv6HeaderFields {
                path_mtu_discovery: mode,
                ..header
            }),
        };
        sockets.send_buffer(&socket, header, plain_fields(), &message)
    });
}

/// A fragment MTU below the dummy link's MTU, which a long message does not fit.
///
/// https://man7.org/linux/man-pages/man7/ipv6.7.html
const FRAGMENT_MTU: u32 = 1320;

/// Checks that a message over the link or fragment MTU is too long where fragmenting is forbidden, and fragmented where allowed.
pub fn a_message_over_the_mtu_is_too_long_or_fragmented<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    /// The link's MTU at the end, which the commands below set.
    ///
    /// https://man7.org/linux/man-pages/man7/ip.7.html
    const LINK_MTU: u32 = 1360;
    const SMALL_LINK: &str = "link set ia mtu 1360";
    let long = vec![u8::MAX; LONG_MESSAGE];
    let too_long = |mtu| {
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: MaximumTransmissionUnit(mtu),
        })
    };
    let socket = opened(&sockets, IpVersion::V6);
    let message = echo(IpVersion::V6, &long);
    let forbidden = Ipv6HeaderFields {
        local_fragmentation: LocalFragmentation::Forbidden,
        fragment_mtu: Some(MaximumTransmissionUnit(FRAGMENT_MTU)),
        ..v6_header(v6_route(ON_LINK_V6, &message))
    };
    assert_eq!(
        sockets.send_buffer(
            &socket,
            IpHeaderFields::V6(forbidden.clone()),
            plain_fields(),
            &message
        ),
        too_long(FRAGMENT_MTU)
    );
    ip(SMALL_LINK);
    let forbidden = Ipv6HeaderFields {
        fragment_mtu: None,
        ..forbidden
    };
    assert_eq!(
        sockets.send_buffer(
            &socket,
            IpHeaderFields::V6(forbidden),
            plain_fields(),
            &message
        ),
        too_long(LINK_MTU)
    );
    let v4_socket = opened(&sockets, IpVersion::V4);
    let v4_message = echo(IpVersion::V4, &long);
    let header = |mode| {
        IpHeaderFields::V4(Ipv4HeaderFields {
            path_mtu_discovery: mode,
            ..v4_header(v4_route(ON_LINK, &v4_message))
        })
    };
    assert_eq!(
        sockets.send_buffer(
            &v4_socket,
            header(PathMtuDiscovery::Do),
            plain_fields(),
            &v4_message
        ),
        too_long(LINK_MTU)
    );
    let capture = Capture::on(DUMMY);
    sockets
        .send_buffer(
            &v4_socket,
            header(PathMtuDiscovery::Dont),
            plain_fields(),
            &v4_message,
        )
        .expect("the message is sent in fragments");
    let fragment = capture.next(|packet| {
        Ipv4HeaderSlice::from_slice(packet).is_ok_and(|header| header.more_fragments())
    });
    assert!(fragment.len() <= LINK_MTU as usize, "{}", fragment.len());
    let allowed = v6_header(v6_route(ON_LINK_V6, &message));
    sockets
        .send_buffer(
            &socket,
            IpHeaderFields::V6(allowed),
            plain_fields(),
            &message,
        )
        .expect("the message is sent in fragments");
    let fragment = capture.next(|packet| {
        seen(packet)
            .extensions
            .iter()
            .any(|(kind, _)| *kind == IpNumber::IPV6_FRAGMENTATION_HEADER)
    });
    assert!(fragment.len() <= LINK_MTU as usize, "{}", fragment.len());
}

/// Checks that a mark chooses the route a rule gives it.
pub fn mark_selects_the_route<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(MARK_PROHIBITED);
    ip6(MARK_PROHIBITED);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let mut header = header_to(to, &message);
        match &mut header {
            IpHeaderFields::V4(header) => header.route.mark = MARK,
            IpHeaderFields::V6(header) => header.route.mark = MARK,
        }
        assert_eq!(
            sockets.send_buffer(&socket, header, plain_fields(), &message),
            Err(SendError::Host(HostError::Errno(libc::EACCES))),
            "{to}"
        );
        ping(&sockets, &socket, to, PAYLOAD).expect("an unmarked request is sent");
    }
}

/// Checks that a socket's owner chooses the route a rule on its user gives it.
pub fn owner_selects_the_route<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(OWNER_PROHIBITED);
    ip6(OWNER_PROHIBITED);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let other = sockets
            .open(version_of(to), OTHER_USER)
            .expect("an echo socket of another user opens");
        assert_eq!(
            ping(&sockets, &other, to, PAYLOAD),
            Err(SendError::Host(HostError::Errno(libc::EACCES))),
            "{to}"
        );
        let root = opened(&sockets, version_of(to));
        ping(&sockets, &root, to, PAYLOAD).expect("the superuser's request is sent");
    }
}

/// Checks that a message's ICMP type and code choose the route a rule on them gives it; a selector other than the message's is refused or routed by.
pub fn icmp_type_and_code_select_the_route<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(ECHO_PROHIBITED);
    ip6(ECHO_PROHIBITED_V6);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        assert_eq!(
            ping(&sockets, &socket, to, PAYLOAD),
            Err(SendError::Host(HostError::Errno(libc::EACCES))),
            "{to}"
        );
        let message = echo(version_of(to), PAYLOAD);
        let mut header = header_to(to, &message);
        let other = Some(UpperLayerSelector::Icmp {
            message_type: IcmpType(message[0]),
            code: OTHER_CODE,
        });
        match &mut header {
            IpHeaderFields::V4(header) => header.route.upper_layer.selector = other,
            IpHeaderFields::V6(header) => header.route.upper_layer.selector = other,
        }
        unless_refused(
            sockets.send_buffer(&socket, header, plain_fields(), &message),
            |sent| sent.expect("a request routed by another code is sent"),
        );
    }
}

pub fn select_interface(header: &mut IpHeaderFields<'_>, selector: InterfaceSelector) {
    match header {
        IpHeaderFields::V4(header) => header.route.interface = Some(selector),
        IpHeaderFields::V6(header) => header.route.interface = Some(selector),
    }
}

/// Checks that an interface, bound or requested, chooses among routes to a destination the one out of that link.
pub fn interface_selects_the_link<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(TWO_ROUTES);
    let second = index(SECOND);
    let first_capture = Capture::on(DUMMY);
    let second_capture = Capture::on(SECOND);
    for to in [IpAddr::V4(NOWHERE), IpAddr::V6(NOWHERE_V6)] {
        let socket = opened(&sockets, version_of(to));
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        assert_eq!(seen(&first_capture.echo_request()).destination, to);
        for selector in [
            InterfaceSelector::Requested(second),
            InterfaceSelector::Bound(second),
        ] {
            let message = echo(version_of(to), PAYLOAD);
            let mut header = header_to(to, &message);
            select_interface(&mut header, selector);
            sockets
                .send_buffer(&socket, header, plain_fields(), &message)
                .unwrap_or_else(|error| panic!("{to} by {selector:?}: {error:?}"));
            assert_eq!(seen(&second_capture.echo_request()).destination, to);
        }
        // A send that names no interface takes the preferred link again.
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        assert_eq!(seen(&first_capture.echo_request()).destination, to);
    }
}

/// Checks that a bound interface holds a ping from a bound address to its link, as a socket bound to the link is held.
///
/// Linux's IPv6 lookup with a source only prefers the interface a packet asks for, so an IPv6 ping by a link that
/// lookup does not prefer may be refused; no ping may leave by another link.
pub fn a_bound_interface_holds_a_ping_from_a_bound_address<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(TWO_ROUTES);
    let first = index(DUMMY);
    let second = index(SECOND);
    let first_capture = Capture::on(DUMMY);
    let second_capture = Capture::on(SECOND);
    for (to, address) in [
        (IpAddr::V4(NOWHERE), EchoBindAddress::V4(LOCAL)),
        (
            IpAddr::V6(NOWHERE_V6),
            EchoBindAddress::V6 {
                address: LOCAL_V6,
                scope: None,
            },
        ),
    ] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        let binding = EchoBinding {
            address,
            identifier: LocalIdentifier::ChosenByExitNode,
            sharing: IdentifierSharing::Exclusive,
        };
        sockets
            .bind(&socket, binding)
            .expect("the socket binds to its address");
        for (bound, capture, other) in [
            (first, &first_capture, &second_capture),
            (second, &second_capture, &first_capture),
        ] {
            let message = echo(version, PAYLOAD);
            let mut header = header_to(to, &message);
            select_interface(&mut header, InterfaceSelector::Bound(bound));
            match sockets.send_buffer(&socket, header, plain_fields(), &message) {
                Ok(()) => assert_eq!(seen(&capture.echo_request()).destination, to),
                Err(SendError::Host(HostError::NotSupported))
                    if version == IpVersion::V6 && bound == second => {}
                sent => panic!("{to} by {bound:?}: {sent:?}"),
            }
            let elsewhere = other.within(QUIET, |packet| {
                is_echo_request(packet) && seen(packet).destination == to
            });
            assert_eq!(elsewhere, None, "{to} by {bound:?} left by another link");
        }
    }
}

/// Checks that a requested interface holds an IPv4 ping from a bound address to its link, and only steers an IPv6 one.
///
/// Linux's IPv6 lookup with a source takes the interface a packet asks for only when no better route leaves by another,
/// so that ping leaves by the preferred link.
pub fn a_ping_from_a_bound_address_takes_a_requested_interface_as_linux_does<S: IcmpSockets>(
    sockets: S,
) {
    enter_network_namespace();
    ip(NETWORK);
    ip(TWO_ROUTES);
    let second = index(SECOND);
    let first_capture = Capture::on(DUMMY);
    let second_capture = Capture::on(SECOND);
    for (to, address, capture, other) in [
        (
            IpAddr::V4(NOWHERE),
            EchoBindAddress::V4(LOCAL),
            &second_capture,
            &first_capture,
        ),
        (
            IpAddr::V6(NOWHERE_V6),
            EchoBindAddress::V6 {
                address: LOCAL_V6,
                scope: None,
            },
            &first_capture,
            &second_capture,
        ),
    ] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        let binding = EchoBinding {
            address,
            identifier: LocalIdentifier::ChosenByExitNode,
            sharing: IdentifierSharing::Exclusive,
        };
        sockets
            .bind(&socket, binding)
            .expect("the socket binds to its address");
        let message = echo(version, PAYLOAD);
        let mut header = header_to(to, &message);
        select_interface(&mut header, InterfaceSelector::Requested(second));
        sockets
            .send_buffer(&socket, header, plain_fields(), &message)
            .unwrap_or_else(|error| panic!("{to} by the second link: {error:?}"));
        assert_eq!(seen(&capture.echo_request()).destination, to);
        let elsewhere = other.within(QUIET, |packet| {
            is_echo_request(packet) && seen(packet).destination == to
        });
        assert_eq!(elsewhere, None, "{to} also left by the other link");
    }
}

/// Checks that a socket its scoped link-local address binds to a link sends a ping naming that link, and refuses another.
pub fn a_link_local_bind_takes_pings_by_its_link_and_refuses_another<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(TWO_ROUTES);
    ip(LINK_LOCAL_ON_SECOND);
    let second = index(SECOND);
    let first_capture = Capture::on(DUMMY);
    let second_capture = Capture::on(SECOND);
    let socket = opened(&sockets, IpVersion::V6);
    let binding = EchoBinding {
        address: EchoBindAddress::V6 {
            address: LINK_LOCAL,
            scope: Some(second),
        },
        identifier: LocalIdentifier::ChosenByExitNode,
        sharing: IdentifierSharing::Exclusive,
    };
    sockets
        .bind(&socket, binding)
        .expect("the socket binds to its link-local address");
    let to = IpAddr::V6(NOWHERE_V6);
    let message = echo(IpVersion::V6, PAYLOAD);
    let first = index(DUMMY);
    for selector in [
        InterfaceSelector::Bound(second),
        InterfaceSelector::Requested(second),
    ] {
        let mut header = header_to(to, &message);
        select_interface(&mut header, selector);
        sockets
            .send_buffer(&socket, header, plain_fields(), &message)
            .unwrap_or_else(|error| panic!("by {selector:?}: {error:?}"));
        let request = second_capture.echo_request();
        assert_eq!(seen(&request).destination, to);
        assert_eq!(seen(&request).source, IpAddr::V6(LINK_LOCAL));
    }
    // Linux refuses a packet that names another link than the socket's, as `IPV6_PKTINFO` or `IPV6_UNICAST_IF`.
    for selector in [
        InterfaceSelector::Bound(first),
        InterfaceSelector::Requested(first),
    ] {
        let mut header = header_to(to, &message);
        select_interface(&mut header, selector);
        assert_eq!(
            sockets.send_buffer(&socket, header, plain_fields(), &message),
            Err(SendError::Host(HostError::Errno(libc::EINVAL))),
            "by {selector:?}"
        );
    }
    let elsewhere = first_capture.within(QUIET, |packet| {
        is_echo_request(packet) && seen(packet).destination == to
    });
    assert_eq!(elsewhere, None, "a ping left by the first link");
}

/// Checks that a link-scope send takes a destination on the link and refuses one through a gateway.
pub fn link_scope_takes_only_the_link<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V4);
    for (to, on_link) in [(ON_LINK, true), (FAR, false)] {
        let message = echo(IpVersion::V4, PAYLOAD);
        let route = Ipv4RouteSelectors {
            scope: RouteScope::Link,
            ..v4_route(to, &message)
        };
        let sent = sockets.send_buffer(
            &socket,
            IpHeaderFields::V4(v4_header(route)),
            plain_fields(),
            &message,
        );
        assert_eq!(sent.is_ok(), on_link, "{to}: {sent:?}");
        ping(&sockets, &socket, IpAddr::V4(to), PAYLOAD).expect("a universe-scope request is sent");
    }
}

/// Checks that a broadcast destination is refused without the permission and sent with it.
pub fn broadcast_needs_its_permission<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V4);
    let capture = Capture::on(DUMMY);
    assert_eq!(
        ping(&sockets, &socket, IpAddr::V4(BROADCAST), PAYLOAD),
        Err(SendError::Host(HostError::Errno(libc::EACCES)))
    );
    let message = echo(IpVersion::V4, PAYLOAD);
    let route = Ipv4RouteSelectors {
        broadcast: BroadcastPermission::Allowed,
        ..v4_route(BROADCAST, &message)
    };
    sockets
        .send_buffer(
            &socket,
            IpHeaderFields::V4(v4_header(route)),
            plain_fields(),
            &message,
        )
        .expect("the broadcast is sent");
    assert_eq!(
        seen(&capture.echo_request()).destination,
        IpAddr::V4(BROADCAST)
    );
}

/// Checks that a source the namespace lacks is refused when its check is required and sent from when it is waived.
pub fn a_waived_source_check_sends_from_an_absent_address<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    // https://www.rfc-editor.org/rfc/rfc1918#section-3
    const ABSENT: Ipv4Addr = Ipv4Addr::new(10, 99, 0, 1);
    let socket = opened(&sockets, IpVersion::V4);
    let capture = Capture::on(DUMMY);
    let message = echo(IpVersion::V4, PAYLOAD);
    let required = Ipv4RouteSelectors {
        source: Some(ABSENT),
        ..v4_route(ON_LINK, &message)
    };
    let refused = sockets.send_buffer(
        &socket,
        IpHeaderFields::V4(v4_header(required)),
        plain_fields(),
        &message,
    );
    assert!(
        matches!(refused, Err(SendError::Host(HostError::Errno(_)))),
        "the host's source check, not {refused:?}"
    );
    let waived = Ipv4RouteSelectors {
        source_address_check: SourceAddressCheck::Waived,
        ..required
    };
    sockets
        .send_buffer(
            &socket,
            IpHeaderFields::V4(v4_header(waived)),
            plain_fields(),
            &message,
        )
        .expect("the request is sent from the absent address");
    assert_eq!(seen(&capture.echo_request()).source, IpAddr::V4(ABSENT));
}

/// Makes the dummy link's address the source of temporary addresses, of which Linux makes one at once.
const TEMPORARY_ADDRESSES: &str = "/proc/sys/net/ipv6/conf/ia/use_tempaddr";
const MANAGED_TEMPORARY: &str = "address replace fd03::1/64 dev ia nodad mngtmpaddr";
/// The `use_tempaddr` value that makes temporary addresses but prefers public ones as sources.
const TEMPORARY_BUT_PUBLIC_PREFERRED: &str = "1";

/// The discard port, which a connect that only chooses a source names.
///
/// https://www.rfc-editor.org/rfc/rfc863
const DISCARD_PORT: u16 = 9;

/// Returns the source the host's own UDP socket takes to an address with the preferences' flags set by `options`.
pub fn host_source<O: wcontainers_linux_socket_options::LinuxSocketOptions>(
    options: &O,
    to: Ipv6Addr,
    preferences: SourcePreferences,
) -> Ipv6Addr {
    let temporary = match preferences.temporary {
        TemporaryAddressPreference::SystemDefault => libc::IPV6_PREFER_SRC_PUBTMP_DEFAULT,
        TemporaryAddressPreference::Temporary => libc::IPV6_PREFER_SRC_TMP,
        TemporaryAddressPreference::Public => libc::IPV6_PREFER_SRC_PUBLIC,
    };
    let mobile = match preferences.mobile {
        MobileAddressPreference::Home => libc::IPV6_PREFER_SRC_HOME,
        MobileAddressPreference::CareOf => libc::IPV6_PREFER_SRC_COA,
    };
    let socket =
        std::net::UdpSocket::bind((Ipv6Addr::UNSPECIFIED, u16::MIN)).expect("a UDP socket");
    let flags = (temporary | mobile).cast_unsigned();
    options
        .set_ipv6_addr_preferences(std::os::fd::AsFd::as_fd(&socket), flags)
        .expect("the source preferences are set");
    socket
        .connect((to, DISCARD_PORT))
        .expect("the socket connects");
    match socket.local_addr().expect("the source").ip() {
        IpAddr::V6(source) => source,
        other => panic!("{other}"),
    }
}

/// Gives the dummy link a temporary address beside [`LOCAL_V6`] and answers which source each preference takes.
///
/// A care-of preference chooses as the host's own socket does: Linux prefers a care-of address only over a Mobile IPv6
/// home address, which no link here has.
pub fn sources_by_preference<O: wcontainers_linux_socket_options::LinuxSocketOptions>(
    options: &O,
) -> Vec<(SourcePreferences, Ipv6Addr)> {
    write_setting(TEMPORARY_ADDRESSES, TEMPORARY_BUT_PUBLIC_PREFERRED);
    ip6(MANAGED_TEMPORARY);
    let temporary = temporary_address(DUMMY);
    let mut sources = Vec::new();
    for (preference, source) in [
        (TemporaryAddressPreference::Temporary, temporary),
        (TemporaryAddressPreference::Public, LOCAL_V6),
    ] {
        let home = SourcePreferences {
            temporary: preference,
            mobile: MobileAddressPreference::Home,
        };
        assert_eq!(host_source(options, ON_LINK_V6, home), source, "{home:?}");
        let care_of = SourcePreferences {
            mobile: MobileAddressPreference::CareOf,
            ..home
        };
        sources.push((home, source));
        sources.push((care_of, host_source(options, ON_LINK_V6, care_of)));
    }
    sources
}

/// Checks that IPv6 source preferences choose between the link's public address and a temporary one.
pub fn source_preferences_are_taken<
    S: IcmpSockets,
    O: wcontainers_linux_socket_options::LinuxSocketOptions,
>(
    sockets: S,
    options: O,
) {
    enter_network_namespace();
    ip(NETWORK);
    let sources = sources_by_preference(&options);
    let socket = opened(&sockets, IpVersion::V6);
    let capture = Capture::on(DUMMY);
    let message = echo(IpVersion::V6, PAYLOAD);
    for (preferences, source) in sources {
        let route = Ipv6RouteSelectors {
            source_preferences: preferences,
            ..v6_route(ON_LINK_V6, &message)
        };
        sockets
            .send_buffer(
                &socket,
                IpHeaderFields::V6(v6_header(route)),
                plain_fields(),
                &message,
            )
            .expect("the request is sent");
        assert_eq!(
            seen(&capture.echo_request()).source,
            IpAddr::V6(source),
            "{preferences:?}"
        );
    }
}

/// Checks that a confirmed send confirms its next hop's neighbour entry, or over IPv6, whose ping sockets drop the flag, is refused.
pub fn a_confirmed_send_confirms_the_neighbour<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    ip(STALE_GATEWAYS);
    for (to, gateway) in [
        (IpAddr::V4(FAR), IpAddr::V4(GATEWAY)),
        (IpAddr::V6(FAR_V6), IpAddr::V6(GATEWAY_V6)),
    ] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let fields = IpSendFields {
            neighbour: NeighbourConfirmation::Confirmed,
            ..plain_fields()
        };
        assert!(seconds_since_confirmed(gateway, DUMMY) > 0);
        let sent = sockets.send_buffer(&socket, header_to(to, &message), fields, &message);
        let confirmed = |sent: Result<(), SendError>| {
            sent.expect("the confirmed request is sent");
            assert_eq!(seconds_since_confirmed(gateway, DUMMY), 0, "{gateway}");
        };
        match to {
            IpAddr::V4(_) => confirmed(sent),
            IpAddr::V6(_) => unless_refused(sent, confirmed),
        }
    }
}

/// Checks that a send's priority chooses its band of a priority queue.
pub fn priority_chooses_the_band<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    tc(PRIO_QUEUE);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let third_band = || third_band_packets(DUMMY);
        let before = third_band();
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        assert_eq!(third_band(), before, "{to}: priority 0 is not filler");
        let fields = IpSendFields {
            priority: PacketPriority(FILLER),
            ..plain_fields()
        };
        sockets
            .send_buffer(&socket, header_to(to, &message), fields, &message)
            .expect("the request is sent");
        assert_eq!(third_band(), before + 1, "{to}");
    }
}

/// Checks that a send with a transmit time leaves no earlier than that time, on a queue that keeps departure times.
pub fn a_transmit_time_holds_the_send<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    tc(FAIR_QUEUE);
    // https://man7.org/linux/man-pages/man8/tc-etf.8.html
    const HELD: Duration = Duration::from_millis(300);
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
        let now = Duration::new(now.tv_sec as u64, now.tv_nsec as u32);
        let fields = IpSendFields {
            transmit_time: Some(TransmitTime {
                clock: TransmitClock::Monotonic,
                at: now + HELD,
                mode: TransmitTimeMode::Strict,
                errors: Report::NotRequested,
            }),
            ..plain_fields()
        };
        let start = Instant::now();
        sockets
            .send_buffer(&socket, header_to(to, &message), fields, &message)
            .expect("the request is sent");
        receive_packet(&sockets, &socket, &mut buffer());
        assert!(start.elapsed() >= HELD, "{to}: {:?}", start.elapsed());
    }
}

/// A schedule of transmit times on the dummy link that misses every time it holds: Linux's etf with a delta of 0.
///
/// It runs on the international atomic clock in strict mode.
pub const MISSING_SCHEDULE: &str = "qdisc add dev ia root etf clockid CLOCK_TAI delta 0";
/// How far a scheduled send is put ahead of now, and how much earlier a past one is.
///
/// https://man7.org/linux/man-pages/man8/tc-etf.8.html
pub const SCHEDULED_AHEAD: Duration = Duration::from_millis(100);

/// Returns the sends a rejected transmit times check makes, each with its time, its wish for errors and what is reported.
///
/// Each send's key is its place.
pub fn rejected_sends() -> Vec<(TransmitTime, Option<TransmitTimeError>)> {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Tai);
    let now = Duration::new(
        u64::try_from(now.tv_sec).expect("a time after the epoch"),
        u32::try_from(now.tv_nsec).expect("nanoseconds"),
    );
    let time = |at, mode, errors| TransmitTime {
        clock: TransmitClock::InternationalAtomicTime,
        at,
        mode,
        errors,
    };
    vec![
        (
            time(
                now + SCHEDULED_AHEAD,
                TransmitTimeMode::Strict,
                Report::NotRequested,
            ),
            None,
        ),
        (
            time(
                now - SCHEDULED_AHEAD,
                TransmitTimeMode::Strict,
                Report::Requested,
            ),
            Some(TransmitTimeError::InvalidParameter),
        ),
        (
            time(
                now + SCHEDULED_AHEAD,
                TransmitTimeMode::Deadline,
                Report::Requested,
            ),
            Some(TransmitTimeError::InvalidParameter),
        ),
        // A time of another clock than the schedule's is invalid; it is one the schedule's clock would only miss.
        // Linux tells a rejection by its time alone, so it goes before the missed send of the same time.
        (
            TransmitTime {
                clock: TransmitClock::Realtime,
                at: now + SCHEDULED_AHEAD,
                mode: TransmitTimeMode::Strict,
                errors: Report::Requested,
            },
            Some(TransmitTimeError::InvalidParameter),
        ),
        (
            time(
                now + SCHEDULED_AHEAD,
                TransmitTimeMode::Strict,
                Report::Requested,
            ),
            Some(TransmitTimeError::Missed),
        ),
    ]
}

/// Returns the send fields of a rejected transmit times check's send at its place: a missed time asks for the packet.
pub fn rejected_send_fields(
    place: usize,
    time: TransmitTime,
    reason: Option<TransmitTimeError>,
) -> IpSendFields {
    let mut fields = IpSendFields {
        transmit_time: Some(time),
        ..plain_fields()
    };
    fields.transmit_reports.key = TransmitReportKey(u32::try_from(place).expect("a small place"));
    if reason == Some(TransmitTimeError::Missed) {
        fields.transmit_reports.packet = Report::Requested;
    }
    fields
}

/// Checks the rejections a socket reported, in the order received, against the sends made.
pub fn check_rejections(
    received: impl FnMut(&mut [u8]) -> Result<Received, HostError>,
    sends: &[(TransmitTime, Option<TransmitTimeError>)],
) {
    let mut received = received;
    let mut reported = Vec::new();
    let mut into = buffer();
    let start = Instant::now();
    while start.elapsed() < SCHEDULED_AHEAD + PATIENCE {
        match received(&mut into) {
            Ok(Received::TransmitReport(report)) => {
                let TransmitEvent::TransmitTimeRejected(reason) = report.event else {
                    panic!("a rejection, not {report:?}")
                };
                assert_eq!(
                    report.packet_length.is_some(),
                    reason == TransmitTimeError::Missed
                );
                // Linux gives a rejected transmit time no sending interface.
                assert_eq!(report.interface, None, "{report:?}");
                reported.push((report.key, Some(reason)));
            }
            Ok(Received::Packet(packet)) => panic!("a report, not {packet:?}"),
            Err(HostError::WouldBlock) => std::thread::sleep(LOOK_AGAIN_SOON),
            Err(error) => panic!("receive: {error:?}"),
        }
        if reported.len() == sends.iter().filter(|send| send.1.is_some()).count() {
            break;
        }
    }
    reported.sort_by_key(|(key, _)| key.0);
    let expected: Vec<_> = sends
        .iter()
        .enumerate()
        .filter(|(_, send)| send.1.is_some())
        .map(|(place, send)| {
            (
                TransmitReportKey(u32::try_from(place).expect("a small place")),
                send.1,
            )
        })
        .collect();
    assert_eq!(reported, expected);
}

/// Checks that a transmit time the link's schedule cannot keep is reported with its send's key and why.
///
/// A time already past or a mode or clock other than the schedule's is invalid, and a time the schedule misses is
/// missed, with the packet as asked; a send that did not ask for errors gets no report.
pub fn rejected_transmit_times_are_reported_with_their_keys<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    tc(MISSING_SCHEDULE);
    let to = IpAddr::V4(ON_LINK);
    let socket = opened(&sockets, IpVersion::V4);
    let message = echo(IpVersion::V4, PAYLOAD);
    let sends = rejected_sends();
    for (place, &(time, reason)) in sends.iter().enumerate() {
        let fields = rejected_send_fields(place, time, reason);
        match sockets.send_buffer(&socket, header_to(to, &message), fields, &message) {
            Ok(()) | Err(SendError::Host(_)) => {}
            Err(error) => panic!("send {place}: {error:?}"),
        }
    }
    check_rejections(|into| sockets.receive_buffer(&socket, into), &sends);
}

/// Checks that transmit reports come on the socket with the key of their send, with the packet or without it as asked.
pub fn transmit_reports_carry_the_key_of_their_send<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        for packet in [Report::Requested, Report::NotRequested] {
            let socket = opened(&sockets, version_of(to));
            let message = echo(version_of(to), PAYLOAD);
            let key = REPORT_KEY;
            let fields = IpSendFields {
                transmit_reports: TransmitReportRequest {
                    key,
                    scheduled: Report::Requested,
                    software: Report::Requested,
                    packet,
                    ..no_reports()
                },
                ..plain_fields()
            };
            let wire = Capture::on(SELF_LINK);
            sockets
                .send_buffer(&socket, header_to(to, &message), fields, &message)
                .expect("the request is sent");
            let sent = wire.echo_request();
            check_loopback_reports(|into| receive(&sockets, &socket, into), packet, &sent);
        }
    }
}

/// The header of a frame on loopback, which a transmit report's packet starts with.
pub const LOOPBACK_LINK_HEADER: usize = etherparse::Ethernet2Header::LEN;

/// Checks the scheduled and software transmit reports `receive` gives of a send to loopback keyed [`REPORT_KEY`].
///
/// Each carries the key and, when the packet was asked for, the loopback link and the length of the `sent` IP packet's
/// frame.
pub fn check_loopback_reports(
    mut receive: impl FnMut(&mut [u8]) -> Received,
    packet: Report,
    sent: &[u8],
) {
    let mut stages = Vec::new();
    let mut into = buffer();
    while stages.len() < 2 {
        let Received::TransmitReport(report) = receive(&mut into) else {
            continue;
        };
        assert_eq!(report.key, REPORT_KEY);
        // Linux tells the sending interface only with the packet.
        if packet == Report::Requested {
            assert_eq!(report.interface, Some(index(SELF_LINK)), "{report:?}");
            assert_eq!(
                report.packet_length,
                Some(ByteCount(LOOPBACK_LINK_HEADER + sent.len())),
                "{report:?}"
            );
        } else {
            assert_eq!(report.interface, None, "{report:?}");
            assert_eq!(report.packet_length, None, "{report:?}");
        }
        let TransmitEvent::Timestamp { stage, source, .. } = report.event else {
            panic!("a timestamp, not {:?}", report.event);
        };
        assert_eq!(source, TimestampSource::Software);
        stages.push(stage);
    }
    stages.sort_by_key(|stage| *stage as u8);
    assert_eq!(
        stages,
        [TransmitStage::Scheduled, TransmitStage::Sent],
        "{:?}",
        seen(sent).destination
    );
}

/// Checks that receive timestamps come only when the setting asks for them; loopback has no hardware clock to bind to.
pub fn receive_timestamps_come_only_when_set<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to));
        ping(&sockets, &socket, to, PAYLOAD).unwrap();
        let fields = receive_packet(&sockets, &socket, &mut buffer());
        assert_eq!(fields.timestamps.software, None);
        let stamping = Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::Requested,
            bound_clock: None,
        };
        set(&sockets, &socket, IpSocketSetting::Timestamping(stamping));
        ping(&sockets, &socket, to, PAYLOAD).unwrap();
        let fields = receive_packet(&sockets, &socket, &mut buffer());
        assert!(fields.timestamps.software.is_some(), "{to}");
        assert_eq!(fields.timestamps.hardware, None);
        let bound = Timestamping {
            bound_clock: Some(wcontainers_internet_protocol::PtpHardwareClockIndex(
                u32::MIN,
            )),
            ..stamping
        };
        assert!(
            sockets
                .configure(&socket, IpSocketSetting::Timestamping(bound))
                .is_err()
        );
    }
}

/// Checks that an ICMP error about a request arrives on its socket whole, from its IP header as on the wire.
///
/// It quotes the request.
fn icmp_error_arrives_whole<S: IcmpSockets>(sockets: S, to: IpAddr) {
    ip(NETWORK);
    let version = version_of(to);
    let socket = opened(&sockets, version);
    bind_alone(&sockets, &socket, version, identifier(version));
    let wire = Capture::on(SELF_LINK);
    let request = provoke_icmp_error(&sockets, &socket, to, Quote::Least);
    let source = seen(&request).source;
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    let given = &into[..fields.length];
    let packet = seen(given);
    assert_eq!(packet.protocol, protocol(version));
    assert_eq!(packet.destination, source);
    assert_eq!(&packet.payload[..TYPE_AND_CODE], unreachable(version));
    assert!(checks_out(packet.payload, &packet));
    let quoted = seen(&packet.payload[ICMP_HEADER..]);
    assert_eq!((quoted.source, quoted.destination), (source, to));
    assert_eq!(identifier_of(quoted.payload), identifier(version).0.get());
    let on_the_wire = wire.next(|packet| is_unreachable(version, packet));
    assert_eq!(given.len(), on_the_wire.len());
    headers_are_as_on_the_wire(given, &on_the_wire);
    has_the_error_traffic_class(given);
}

/// Answers whether a packet is the ICMP error of the version that a host or address is unreachable.
fn is_unreachable(version: IpVersion, packet: &[u8]) -> bool {
    let seen = seen(packet);
    seen.protocol == protocol(version)
        && seen.payload.get(..TYPE_AND_CODE) == Some(&unreachable(version)[..])
}

/// Checks that an ICMP error cut in its outer header, ICMP header or quoted message keeps its start and tells its whole length.
pub fn a_short_buffer_keeps_the_start_of_an_icmp_error<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    for (to, header, inner) in [
        (
            IpAddr::V4(ON_LINK),
            etherparse::Ipv4Header::MIN_LEN,
            etherparse::Ipv4Header::MIN_LEN,
        ),
        (
            IpAddr::V6(ON_LINK_V6),
            etherparse::Ipv6Header::LEN,
            etherparse::Ipv6Header::LEN,
        ),
    ] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        bind_alone(&sockets, &socket, version, identifier(version));
        provoke_icmp_error(&sockets, &socket, to, Quote::Whole);
        let mut whole = buffer();
        let length = receive_packet(&sockets, &socket, &mut whole).length;
        let whole = &whole[..length];
        assert!(is_unreachable(version, whole), "{to}: {whole:?}");
        let in_the_quoted_message = header + ICMP_HEADER + inner + ICMP_HEADER / 2;
        for short in [header / 2, header + ICMP_HEADER / 2, in_the_quoted_message] {
            provoke_icmp_error(&sockets, &socket, to, Quote::Whole);
            let mut into = vec![u8::MIN; short];
            let fields = receive_packet(&sockets, &socket, &mut into);
            assert_eq!(fields.length, whole.len(), "{to} into {short}");
            assert_eq!(&into[..], &whole[..short], "{to} into {short}");
        }
    }
}

/// A payload that makes an echo message longer than a page of memory, which a receive must not cut there.
///
/// https://man7.org/linux/man-pages/man2/getpagesize.2.html
pub const LONGER_THAN_A_PAGE: usize = 5000;

/// Checks that bytes that are not a buffer carry a request, and its reply longer than a page whole or cut short.
pub fn bytes_that_are_not_a_buffer_carry_what_a_buffer_does<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for (to, header) in [
        (
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            etherparse::Ipv4Header::MIN_LEN,
        ),
        (IpAddr::V6(Ipv6Addr::LOCALHOST), etherparse::Ipv6Header::LEN),
    ] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        bind_alone(&sockets, &socket, version, identifier(version));
        let payload: Vec<u8> = (u8::MIN..=u8::MAX)
            .cycle()
            .take(LONGER_THAN_A_PAGE)
            .collect();
        let message = echo(version, &payload);
        let send = || {
            sockets
                .send_to(
                    &socket,
                    header_to(to, &message),
                    plain_fields(),
                    &CellBytes::new(&message),
                )
                .expect("the request is sent from cells");
        };
        send();
        let mut plain = buffer();
        let length = receive_packet(&sockets, &socket, &mut plain).length;
        let plain = &plain[..length];
        assert_eq!(length, header + message.len(), "{to}");
        assert_eq!(&seen(plain).payload[ICMP_HEADER..], payload, "{to}");
        send();
        let cells = CellBytes::new(&buffer());
        let fields = receive_packet_into(&sockets, &socket, &cells);
        assert_eq!(fields.length, length, "{to}");
        assert_eq!(&cells.to_vec()[..length], plain, "{to}");
        send();
        let short = header + ICMP_HEADER;
        let cut = CellBytes::new(&vec![u8::MIN; short]);
        let fields = receive_packet_into(&sockets, &socket, &cut);
        assert_eq!(fields.length, length, "{to} into {short}");
        assert_eq!(cut.to_vec(), &plain[..short], "{to} into {short}");
    }
}

/// Checks that an ICMP error is received into bytes that are not a buffer as into a buffer.
pub fn an_icmp_error_reaches_bytes_that_are_not_a_buffer<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        bind_alone(&sockets, &socket, version, identifier(version));
        provoke_icmp_error(&sockets, &socket, to, Quote::Whole);
        let mut plain = buffer();
        let length = receive_packet(&sockets, &socket, &mut plain).length;
        let plain = &plain[..length];
        assert!(is_unreachable(version, plain), "{to}: {plain:?}");
        provoke_icmp_error(&sockets, &socket, to, Quote::Whole);
        let cells = CellBytes::new(&buffer());
        let fields = receive_packet_into(&sockets, &socket, &cells);
        assert_eq!(fields.length, length, "{to}");
        assert_eq!(&cells.to_vec()[..length], plain, "{to}");
    }
}

pub fn an_icmp_error_arrives_whole_over_ipv4<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    icmp_error_arrives_whole(sockets, IpAddr::V4(ON_LINK));
}

pub fn an_icmp_error_arrives_whole_over_ipv6<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    icmp_error_arrives_whole(sockets, IpAddr::V6(ON_LINK_V6));
}

/// The DSCP of every ICMP message a check sends from a raw socket, not zero so that a received one shows it was read.
pub const ERROR_DSCP: IpDscp = IpDscp::EF;
/// The ECN of every ICMP message a check sends from a raw socket.
pub const ERROR_ECN: IpEcn = IpEcn::Ect0;

/// Checks that a received ICMP error's outer header carries the traffic class the check sent it with.
pub fn has_the_error_traffic_class(packet: &[u8]) {
    let class = match version_of(seen(packet).source) {
        IpVersion::V4 => (ipv4(packet).dcp(), ipv4(packet).ecn()),
        IpVersion::V6 => (ipv6(packet).dscp(), ipv6(packet).ecn()),
    };
    assert_eq!(class, (ERROR_DSCP, ERROR_ECN));
}

/// Sends an ICMP message from a raw socket of the namespace to a local address; the checksum is written here for IPv4.
pub fn send_raw_icmp(version: IpVersion, to: IpAddr, message: &mut [u8]) {
    let (family, protocol) = match version {
        IpVersion::V4 => (rustix::net::AddressFamily::INET, rustix::net::ipproto::ICMP),
        IpVersion::V6 => (
            rustix::net::AddressFamily::INET6,
            rustix::net::ipproto::ICMPV6,
        ),
    };
    if version == IpVersion::V4 {
        let checksum = etherparse::checksum::Sum16BitWords::new()
            .add_slice(message)
            .ones_complement()
            .to_ne_bytes();
        message[CHECKSUM_BYTES].copy_from_slice(&checksum);
    }
    let raw = rustix::net::socket(family, rustix::net::SocketType::RAW, Some(protocol))
        .expect("a raw socket opens");
    let mut class = etherparse::Ipv6Header::default();
    class.set_dscp(ERROR_DSCP);
    class.set_ecn(ERROR_ECN);
    match version {
        IpVersion::V4 => rustix::net::sockopt::set_ip_tos(&raw, class.traffic_class),
        IpVersion::V6 => rustix::net::sockopt::set_ipv6_tclass(&raw, class.traffic_class.into()),
    }
    .expect("the traffic class is set");
    rustix::net::sendto(
        &raw,
        message,
        rustix::net::SendFlags::empty(),
        &std::net::SocketAddr::new(to, u16::MIN),
    )
    .expect("the ICMP message is sent");
}

/// Messages held by a pacing rate wait in the send buffer and are counted unsent; a delivered send leaves none.
pub fn pacing_holds_messages_in_the_send_buffer<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    tc(FAIR_QUEUE);
    // https://man7.org/linux/man-pages/man7/socket.7.html
    const RATE: BytesPerSecond = BytesPerSecond(20_000);
    // https://man7.org/linux/man-pages/man7/socket.7.html
    const MESSAGES: usize = 4;
    let long = vec![u8::MAX; PACED_MESSAGE];
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to));
        ping(&sockets, &socket, to, PAYLOAD).unwrap();
        receive_packet(&sockets, &socket, &mut buffer());
        assert_eq!(sockets.count_unsent_bytes(&socket), ByteCount(usize::MIN));
        set(
            &sockets,
            &socket,
            IpSocketSetting::MaximumPacingRate(Some(RATE)),
        );
        let start = Instant::now();
        for _ in 0..MESSAGES {
            ping(&sockets, &socket, to, &long).expect("a paced request is sent");
        }
        let unsent = sockets.count_unsent_bytes(&socket);
        assert!(unsent.0 >= long.len(), "{to}: {unsent:?}");
        for _ in 0..MESSAGES {
            receive_packet(&sockets, &socket, &mut buffer());
        }
        // Each request of about a thousand bytes waits its time at the rate behind the one before.
        let least =
            Duration::from_secs_f64(((MESSAGES - 1) * PACED_MESSAGE) as f64 / RATE.0 as f64 / 2.0);
        assert!(start.elapsed() >= least, "{to}: {:?}", start.elapsed());
    }
}

/// Checks that the smallest send buffer makes a send that would wait behind paced ones would block.
pub fn a_full_send_buffer_would_block<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    tc(FAIR_QUEUE);
    let long = vec![u8::MAX; PACED_MESSAGE];
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let socket = opened(&sockets, IpVersion::V4);
    set(
        &sockets,
        &socket,
        IpSocketSetting::SendBuffer(ByteCount(usize::MIN)),
    );
    set(
        &sockets,
        &socket,
        IpSocketSetting::MaximumPacingRate(Some(SLOW_RATE)),
    );
    let blocked = (0..ATTEMPTS)
        .any(|_| ping(&sockets, &socket, to, &long) == Err(SendError::Host(HostError::WouldBlock)));
    assert!(blocked);
}

/// Checks that a receive buffer too small for every reply drops some, and the next packet tells how many.
pub fn a_small_receive_buffer_counts_its_drops<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    // https://man7.org/linux/man-pages/man7/socket.7.html
    const REQUESTS: usize = 64;
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let socket = opened(&sockets, IpVersion::V4);
    set(
        &sockets,
        &socket,
        IpSocketSetting::ReceiveBuffer(ByteCount(usize::MIN)),
    );
    let long = vec![u8::MAX; PACED_MESSAGE];
    for _ in 0..REQUESTS {
        ping(&sockets, &socket, to, &long).unwrap();
    }
    std::thread::sleep(QUIET);
    let mut received = usize::MIN;
    let mut dropped = usize::MIN;
    let mut into = buffer();
    while let Ok(Received::Packet(fields)) = sockets.receive_buffer(&socket, &mut into) {
        received += 1;
        dropped += fields.drops_since_previous.0 as usize;
    }
    assert!(received < REQUESTS, "{received} of {REQUESTS} arrived");
    set(
        &sockets,
        &socket,
        IpSocketSetting::ReceiveBuffer(ByteCount(usize::from(u16::MAX))),
    );
    ping(&sockets, &socket, to, PAYLOAD).unwrap();
    let last = receive_packet(&sockets, &socket, &mut into);
    dropped += last.drops_since_previous.0 as usize;
    assert_eq!(received + dropped, REQUESTS);
}

/// The namespace's multicast source filters, IPv4's and IPv6's, as Linux lists them by link, group and source.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
pub const SOURCE_FILTERS: &[&str] = &[
    "/proc/thread-self/net/mcfilter",
    "/proc/thread-self/net/mcfilter6",
];

/// Counts how many of the namespace's sockets include and exclude the source for the group, if any names it.
pub fn source_filter_counts(group: IpAddr, source: IpAddr) -> Option<(u64, u64)> {
    let (list, group, source) = match (group, source) {
        (IpAddr::V4(group), IpAddr::V4(source)) => (
            SOURCE_FILTERS[0],
            format!("{:#010x}", u32::from(group)),
            format!("{:#010x}", u32::from(source)),
        ),
        (IpAddr::V6(group), IpAddr::V6(source)) => (
            SOURCE_FILTERS[1],
            format!("{:032x}", u128::from(group)),
            format!("{:032x}", u128::from(source)),
        ),
        _ => panic!("a group and a source of one version"),
    };
    std::fs::read_to_string(list)
        .expect("the source filter list")
        .lines()
        .find_map(
            |line| match line.split_whitespace().collect::<Vec<_>>()[..] {
                [_, _, listed_group, listed_source, included, excluded]
                    if listed_group == group && listed_source == source =>
                {
                    Some((included.parse().ok()?, excluded.parse().ok()?))
                }
                _ => None,
            },
        )
}

/// Returns the setting that joins the group on the dummy link, including or excluding the sources.
pub fn filtered_membership(group: IpAddr, include: bool, sources: &[IpAddr]) -> FilteredMembership {
    FilteredMembership {
        group,
        include,
        sources: sources.to_vec(),
    }
}

/// A group to join with a source filter, which `setting` gives as the contract's setting.
pub struct FilteredMembership {
    group: IpAddr,
    include: bool,
    sources: Vec<IpAddr>,
}

impl FilteredMembership {
    pub fn with_setting<R>(&self, then: impl FnOnce(IpSocketSetting<'_>) -> R) -> R {
        let interface = Some(index(DUMMY));
        let include = self.include;
        /// Including no source is leaving the group, the one way to write it.
        fn joined<A>(include: bool, sources: &[A]) -> MulticastMembership<'_, A> {
            if include {
                Slice1::try_from_slice(sources).map_or(MulticastMembership::Left, |sources| {
                    MulticastMembership::Joined(SourceFilter::Include(sources))
                })
            } else {
                MulticastMembership::Joined(SourceFilter::Exclude(sources))
            }
        }
        match self.group {
            IpAddr::V4(group) => {
                let sources: Vec<Ipv4Addr> = self
                    .sources
                    .iter()
                    .map(|source| match source {
                        IpAddr::V4(source) => *source,
                        IpAddr::V6(_) => panic!("an IPv4 source"),
                    })
                    .collect();
                then(IpSocketSetting::MulticastV4(MulticastGroup {
                    group,
                    interface,
                    membership: joined(include, &sources[..]),
                }))
            }
            IpAddr::V6(group) => {
                let sources: Vec<Ipv6Addr> = self
                    .sources
                    .iter()
                    .map(|source| match source {
                        IpAddr::V6(source) => *source,
                        IpAddr::V4(_) => panic!("an IPv6 source"),
                    })
                    .collect();
                then(IpSocketSetting::MulticastV6(MulticastGroup {
                    group,
                    interface,
                    membership: joined(include, &sources[..]),
                }))
            }
        }
    }
}

/// Joins the group on the dummy link with each kind of source filter through `configure`, checking the host holds each.
///
/// It leaves the group after each.
pub fn source_filters_are_written(
    group: IpAddr,
    (first, second): (IpAddr, IpAddr),
    configure: impl Fn(IpSocketSetting<'_>),
) {
    // https://man7.org/linux/man-pages/man7/ip.7.html
    const INCLUDED: Option<(u64, u64)> = Some((1, 0));
    // https://man7.org/linux/man-pages/man7/ip.7.html
    const EXCLUDED: Option<(u64, u64)> = Some((0, 1));
    for (include, given, expected) in [
        (true, &[first][..], [INCLUDED, None]),
        (true, &[first, second][..], [INCLUDED, INCLUDED]),
        (false, &[second][..], [None, EXCLUDED]),
        (false, &[][..], [None, None]),
    ] {
        filtered_membership(group, include, given).with_setting(&configure);
        assert_eq!(
            [
                source_filter_counts(group, first),
                source_filter_counts(group, second)
            ],
            expected,
            "{group} include {include} {given:?}"
        );
    }
    let left = match group {
        IpAddr::V4(group) => IpSocketSetting::MulticastV4(MulticastGroup {
            group,
            interface: Some(index(DUMMY)),
            membership: MulticastMembership::Left,
        }),
        IpAddr::V6(group) => IpSocketSetting::MulticastV6(MulticastGroup {
            group,
            interface: Some(index(DUMMY)),
            membership: MulticastMembership::Left,
        }),
    };
    configure(left);
}

/// Checks that a group's source filter, of either kind and any sources, is the host's as written.
pub fn multicast_source_filters_are_written<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    // https://www.rfc-editor.org/rfc/rfc2365#section-6
    const GROUP: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 5);
    // https://www.rfc-editor.org/rfc/rfc4291#section-2.7
    const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0, 0x124);
    for (group, sources) in [
        (
            IpAddr::V4(GROUP),
            (IpAddr::V4(ON_LINK), IpAddr::V4(GATEWAY)),
        ),
        (
            IpAddr::V6(GROUP_V6),
            (IpAddr::V6(ON_LINK_V6), IpAddr::V6(GATEWAY_V6)),
        ),
    ] {
        let socket = opened(&sockets, version_of(group));
        source_filters_are_written(group, sources, |setting| set(&sockets, &socket, setting));
    }
}

/// Checks that joined groups and anycast addresses are the host's while joined, and multicast loopback brings a reply.
pub fn memberships_are_taken<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    write_setting(ECHO_IGNORE_BROADCASTS, &u8::MIN.to_string());
    // https://www.rfc-editor.org/rfc/rfc2365#section-6
    const GROUP: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 3);
    // https://www.rfc-editor.org/rfc/rfc4291#section-2.7
    const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0, 0x123);
    // https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const ANYCAST: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 0x99);
    let link = Some(index(DUMMY));
    let socket = opened(&sockets, IpVersion::V4);
    let joined = |group: IpAddr| joined_groups(DUMMY).contains(&group);
    // An echo request to the group from the dummy link, which the namespace answers only if it takes the request.
    let replies_looped = |socket: &S::Socket, group: IpAddr| {
        for (looped, replied) in [(false, false), (true, true)] {
            set(&sockets, socket, IpSocketSetting::MulticastLoopback(looped));
            let message = echo(version_of(group), PAYLOAD);
            let header = match group {
                IpAddr::V4(group) => IpHeaderFields::V4(v4_header(Ipv4RouteSelectors {
                    interface: link.map(InterfaceSelector::Requested),
                    ..v4_route(group, &message)
                })),
                IpAddr::V6(group) => IpHeaderFields::V6(v6_header(Ipv6RouteSelectors {
                    interface: link.map(InterfaceSelector::Requested),
                    ..v6_route(group, &message)
                })),
            };
            sockets
                .send_buffer(socket, header, plain_fields(), &message)
                .expect("the multicast request is sent");
            assert_eq!(
                !nothing_arrives(&sockets, socket),
                replied,
                "{group} loopback {looped}"
            );
        }
    };
    let group = |membership| MulticastGroup {
        group: GROUP,
        interface: link,
        membership,
    };
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV4(group(MulticastMembership::Joined(SourceFilter::Exclude(
            &[],
        )))),
    );
    assert!(joined(IpAddr::V4(GROUP)));
    replies_looped(&socket, IpAddr::V4(GROUP));
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV4(group(MulticastMembership::Left)),
    );
    assert!(!joined(IpAddr::V4(GROUP)));
    for enabled in [false, true] {
        set(
            &sockets,
            &socket,
            IpSocketSetting::MulticastAllGroups {
                version: IpVersion::V4,
                enabled,
            },
        );
    }
    let socket = opened(&sockets, IpVersion::V6);
    let group_v6 = |membership| MulticastGroup {
        group: GROUP_V6,
        interface: link,
        membership,
    };
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV6(group_v6(MulticastMembership::Joined(
            SourceFilter::Exclude(&[]),
        ))),
    );
    assert!(joined(IpAddr::V6(GROUP_V6)));
    replies_looped(&socket, IpAddr::V6(GROUP_V6));
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV6(group_v6(MulticastMembership::Left)),
    );
    assert!(!joined(IpAddr::V6(GROUP_V6)));
    let anycast = |membership| AnycastAddress {
        address: ANYCAST,
        interface: link,
        membership,
    };
    let listed = || {
        std::fs::read_to_string(ANYCAST_LIST)
            .expect("the anycast list")
            .contains(&format!("{:032x}", u128::from(ANYCAST)))
    };
    set(
        &sockets,
        &socket,
        IpSocketSetting::Anycast(anycast(Membership::Joined)),
    );
    assert!(listed());
    set(
        &sockets,
        &socket,
        IpSocketSetting::Anycast(anycast(Membership::Left)),
    );
    assert!(!listed());
}

/// How many waits [`wait_allocations`] counts, after as many that warm the socket.
///
/// https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html
pub const COUNTED_WAITS: usize = 100;

/// Returns what `allocations` counts over [`COUNTED_WAITS`] waits, after as many that warm the socket, each made to wait and then finish.
pub fn wait_allocations<F: Future<Output = Result<ReadinessReport, HostError>>>(
    mut wait: impl FnMut() -> F,
    mut arrive: impl FnMut(),
    mut take: impl FnMut(),
    allocations: fn() -> usize,
) -> usize {
    let mut counted = usize::MIN;
    for round in 0..COUNTED_WAITS * 2 {
        let wakes = Arc::new(Wakes::default());
        let mut during = usize::MIN;
        let dropping = {
            let before = allocations();
            let mut call = pin!(wait());
            assert!(
                poll_with(call.as_mut(), &wakes).is_pending(),
                "the wait has to wait"
            );
            during += allocations() - before;
            arrive();
            assert!(wakes.first_came(), "the wait is woken");
            let before = allocations();
            assert!(
                finished(poll_with(call.as_mut(), &wakes))
                    .ready_for
                    .contains(ReadyFor::READABLE)
            );
            during += allocations() - before;
            allocations()
        };
        during += allocations() - dropping;
        take();
        if round >= COUNTED_WAITS {
            counted += during;
        }
    }
    counted
}

/// The wakes a waiting call has given, counted, for a check that polls the call by hand.
#[derive(Default)]
pub struct Wakes {
    count: Mutex<usize>,
    changed: Condvar,
}

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        *self.count.lock().expect("the wakes") += 1;
        self.changed.notify_all();
    }
}

impl Wakes {
    pub fn count(&self) -> usize {
        *self.count.lock().expect("the wakes")
    }

    /// Tells whether the call has been woken `count` times before the patience of a check ends.
    pub fn first_came(&self) -> bool {
        let wakes = self.count.lock().expect("the wakes");
        let (wakes, _) = self
            .changed
            .wait_timeout_while(wakes, PATIENCE, |wakes| *wakes == usize::MIN)
            .expect("the wakes");
        *wakes != usize::MIN
    }
}

/// Checks that the wait is woken in time, and if not, whether its facts came anyway, telling a lost wake from a lost fact.
pub fn woken<F: Future<Output = Result<ReadinessReport, HostError>>>(
    call: Pin<&mut F>,
    wakes: &Arc<Wakes>,
    by: &str,
) {
    if !wakes.first_came() {
        let now = poll_with(call, wakes);
        panic!("not woken by {by}; polled again, the wait gives {now:?}");
    }
}

/// Polls a waiting call once with a waker that counts into `wakes`.
pub fn poll_with<F: Future>(call: Pin<&mut F>, wakes: &Arc<Wakes>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::clone(wakes));
    call.poll(&mut Context::from_waker(&waker))
}

/// Returns the report a finished wait gave.
pub fn finished(poll: Poll<Result<ReadinessReport, HostError>>) -> ReadinessReport {
    match poll {
        Poll::Ready(facts) => facts.expect("the wait succeeds"),
        Poll::Pending => panic!("the wait did not finish"),
    }
}

/// Returns the report a wait gives at once or when it is next woken.
pub fn finished_soon<F: Future<Output = Result<ReadinessReport, HostError>>>(
    mut call: Pin<&mut F>,
    wakes: &Arc<Wakes>,
) -> ReadinessReport {
    match poll_with(call.as_mut(), wakes) {
        Poll::Pending => {
            if !wakes.first_came() {
                let now = poll_with(call.as_mut(), wakes);
                panic!("the wait is not woken; polled again, it gives {now:?}");
            }
            finished(poll_with(call, wakes))
        }
        ready => finished(ready),
    }
}

/// Checks that a wait on each of many sockets at once is woken once when its reply arrives, however many more do.
pub fn waits_on_many_sockets_each_wake_once<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let watched: Vec<_> = (0..MANY_WAITS)
        .map(|_| opened(&sockets, IpVersion::V4))
        .collect();
    let mut waits: Vec<_> = watched
        .iter()
        .map(|socket| {
            (
                Box::pin(sockets.wait_until_ready(socket, ReadyFor::READABLE)),
                Arc::new(Wakes::default()),
            )
        })
        .collect();
    for (wait, wakes) in &mut waits {
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    for socket in &watched {
        ping(&sockets, socket, to, PAYLOAD).expect("the request is sent");
    }
    for (wait, wakes) in &mut waits {
        assert!(wakes.first_came(), "a wait woken when its reply arrives");
        assert!(
            finished(poll_with(wait.as_mut(), wakes))
                .ready_for
                .contains(ReadyFor::READABLE)
        );
    }
    std::thread::sleep(QUIET);
    for (_, wakes) in &waits {
        assert_eq!(wakes.count(), 1, "a wait woken more than once");
    }
}

/// The length of an ICMP message's type and code, which it starts with.
///
/// https://www.rfc-editor.org/rfc/rfc792
const TYPE_AND_CODE: usize = 2;

/// Returns the type and code of the ICMP error the checks send: the destination host is unreachable.
fn unreachable(version: IpVersion) -> [u8; TYPE_AND_CODE] {
    match version {
        IpVersion::V4 => [icmpv4::TYPE_DEST_UNREACH, icmpv4::CODE_DST_UNREACH_HOST],
        IpVersion::V6 => [icmpv6::TYPE_DST_UNREACH, icmpv6::CODE_DST_UNREACH_ADDR],
    }
}

/// How much of a packet an ICMP error quotes.
#[derive(Clone, Copy)]
enum Quote {
    /// For IPv4 the header and the first eight bytes after it, the least an IPv4 error quotes; for IPv6 all of it.
    Least,
    Whole,
}

/// Sends an echo request to an address on the dummy link and an ICMP error about it to its source, giving the request.
fn provoke_icmp_error<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    to: IpAddr,
    quote: Quote,
) -> Vec<u8> {
    let capture = Capture::on(DUMMY);
    ping(sockets, socket, to, PAYLOAD).expect("the request is sent");
    let request = capture.echo_request();
    send_icmp_error_about(&request, quote);
    request
}

/// Sends the ICMP error that a request's destination is unreachable to its source, quoting it.
fn send_icmp_error_about(request: &[u8], quote: Quote) {
    let version = version_of(seen(request).source);
    let quoted_length = match (version, quote) {
        (IpVersion::V4, Quote::Least) => ipv4(request).slice().len() + ICMP_HEADER,
        _ => request.len(),
    };
    let mut message = unreachable(version).to_vec();
    message.resize(ICMP_HEADER, u8::MIN);
    message.extend_from_slice(&request[..quoted_length]);
    send_raw_icmp(version, seen(request).source, &mut message);
}

/// How many ICMP errors a check has arrive before it receives any.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
pub const QUEUED_ERRORS: usize = 3;

/// Checks that ICMP errors that all arrive before a receive are each received, and then nothing more.
pub fn icmp_errors_queued_together_each_arrive<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        let capture = Capture::on(DUMMY);
        // Every request leaves first: Linux fails a ping socket's send with the error an ICMP error left.
        let requests: Vec<Vec<u8>> = (0..QUEUED_ERRORS)
            .map(|_| {
                ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
                capture.echo_request()
            })
            .collect();
        for request in &requests {
            send_icmp_error_about(request, Quote::Least);
        }
        std::thread::sleep(QUIET);
        for _ in 0..QUEUED_ERRORS {
            let mut into = buffer();
            let fields = receive_packet(&sockets, &socket, &mut into);
            assert!(is_unreachable(version, &into[..fields.length]), "{to}");
        }
        assert!(nothing_arrives(&sockets, &socket), "{to}");
    }
}

/// Checks that an ICMP error still arrives after a send that met it, which Linux fails with the error the ICMP error left.
pub fn an_icmp_error_a_send_met_still_arrives<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        provoke_icmp_error(&sockets, &socket, to, Quote::Least);
        std::thread::sleep(QUIET);
        let sent = ping(&sockets, &socket, to, PAYLOAD);
        assert!(
            matches!(sent, Ok(()) | Err(SendError::Host(HostError::Errno(_)))),
            "{to}: {sent:?}"
        );
        let mut into = buffer();
        let fields = receive_packet(&sockets, &socket, &mut into);
        assert!(is_unreachable(version, &into[..fields.length]), "{to}");
        assert!(nothing_arrives(&sockets, &socket), "{to}");
    }
}

/// Checks that an ICMP error that a too long send read off the host while looking for its own error still ends a wait at once.
pub fn a_wait_sees_an_error_a_send_read_first<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    let to = IpAddr::V6(ON_LINK_V6);
    let socket = opened(&sockets, IpVersion::V6);
    let wakes = Arc::new(Wakes::default());
    provoke_icmp_error(&sockets, &socket, to, Quote::Least);
    let facts = finished_soon(
        pin!(sockets.wait_until_ready(&socket, ReadyFor::READABLE)),
        &wakes,
    );
    assert!(
        facts.unconditional.contains(UnconditionalEvents::ERROR),
        "{facts:?}"
    );
    let message = echo(IpVersion::V6, &vec![u8::MAX; LONG_MESSAGE]);
    let forbidden = Ipv6HeaderFields {
        local_fragmentation: LocalFragmentation::Forbidden,
        fragment_mtu: Some(MaximumTransmissionUnit(FRAGMENT_MTU)),
        ..v6_header(v6_route(ON_LINK_V6, &message))
    };
    assert_eq!(
        sockets.send_buffer(
            &socket,
            IpHeaderFields::V6(forbidden),
            plain_fields(),
            &message
        ),
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: MaximumTransmissionUnit(FRAGMENT_MTU)
        })
    );
    let facts = finished(poll_with(
        pin!(sockets.wait_until_ready(&socket, ReadyFor::READABLE)),
        &wakes,
    ));
    assert!(
        facts.unconditional.contains(UnconditionalEvents::ERROR),
        "{facts:?}"
    );
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    assert_eq!(seen(&into[..fields.length]).protocol, IpNumber::IPV6_ICMP);
    assert!(nothing_arrives(&sockets, &socket));
    let wait = pin!(sockets.wait_until_ready(&socket, ReadyFor::READABLE));
    assert!(
        poll_with(wait, &wakes).is_pending(),
        "an error held after its entry was received"
    );
}

/// The facts no ICMP echo socket has, for it has no urgent data and no peer that finishes sending.
pub const NEVER_ON_DATAGRAMS: [ReadyFor; 2] =
    [ReadyFor::URGENT_DATA, ReadyFor::PEER_FINISHED_SENDING];

/// Checks that a wait for a fact no echo socket has waits on while it is readable and writable, until an ICMP error ends it.
pub fn a_wait_for_a_fact_icmp_never_has_waits_on<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    let to = IpAddr::V4(ON_LINK);
    let socket = opened(&sockets, IpVersion::V4);
    ping(&sockets, &socket, IpAddr::V4(Ipv4Addr::LOCALHOST), PAYLOAD).expect("the request is sent");
    finished_soon(
        pin!(sockets.wait_until_ready(&socket, ReadyFor::READABLE)),
        &Arc::new(Wakes::default()),
    );
    let mut waits: Vec<_> = NEVER_ON_DATAGRAMS
        .iter()
        .map(|interest| {
            (
                Box::pin(sockets.wait_until_ready(&socket, *interest)),
                Arc::new(Wakes::default()),
            )
        })
        .collect();
    for (wait, wakes) in &mut waits {
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    std::thread::sleep(QUIET);
    for (wait, wakes) in &mut waits {
        assert_eq!(wakes.count(), 0, "woken with no fact asked for");
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    provoke_icmp_error(&sockets, &socket, to, Quote::Least);
    for ((wait, wakes), interest) in waits.iter_mut().zip(NEVER_ON_DATAGRAMS) {
        woken(
            wait.as_mut(),
            wakes,
            &format!("{interest:?}: the ICMP error"),
        );
        let facts = finished(poll_with(wait.as_mut(), wakes));
        assert!(
            facts.unconditional.contains(UnconditionalEvents::ERROR),
            "{interest:?}: {facts:?}"
        );
        for never in NEVER_ON_DATAGRAMS {
            assert!(!facts.ready_for.contains(never), "{interest:?}: {facts:?}");
        }
    }
}

/// Checks that a socket waited on for data, made ready by the reply to an echo request it sends, for the readiness suite.
pub fn waiting_for_a_reply<S: IcmpSockets>(
    sockets: S,
) -> Waiting<S, S::Socket, impl FnOnce(&S, &S::Socket)> {
    enter_network_namespace();
    let socket = opened(&sockets, IpVersion::V4);
    Waiting {
        readiness: sockets,
        handle: socket,
        interest: ReadyFor::READABLE,
        make_ready: |sockets: &S, socket: &S::Socket| {
            ping(sockets, socket, IpAddr::V4(Ipv4Addr::LOCALHOST), PAYLOAD)
                .expect("the request is sent");
        },
    }
}

/// Checks that a socket waited on for data, which an ICMP error about a request it sends fails, for the readiness suite.
pub fn waiting_for_data_until_an_icmp_error<S: IcmpSockets>(
    sockets: S,
) -> Waiting<S, S::Socket, impl FnOnce(&S, &S::Socket)> {
    enter_network_namespace();
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6);
    Waiting {
        readiness: sockets,
        handle: socket,
        interest: ReadyFor::READABLE,
        make_ready: |sockets: &S, socket: &S::Socket| {
            provoke_icmp_error(sockets, socket, IpAddr::V6(ON_LINK_V6), Quote::Least);
        },
    }
}

/// Checks that a header of the other version than the socket's gets the error the host's own ping socket gives it.
///
/// The host's socket is sent the same message to the address in the form its family takes: IPv4-mapped for IPv6.
pub fn a_header_of_the_other_version_gets_the_hosts_error<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for (version, to) in [
        (IpVersion::V6, IpAddr::V4(Ipv4Addr::LOCALHOST)),
        (IpVersion::V4, IpAddr::V6(Ipv6Addr::LOCALHOST)),
    ] {
        let socket = opened(&sockets, version);
        let message = echo(version_of(to), PAYLOAD);
        let ours = sockets.send_buffer(&socket, header_to(to, &message), plain_fields(), &message);
        let (family, protocol, address) = match (version, to) {
            (IpVersion::V6, IpAddr::V4(address)) => (
                rustix::net::AddressFamily::INET6,
                rustix::net::ipproto::ICMPV6,
                IpAddr::V6(address.to_ipv6_mapped()),
            ),
            _ => (
                rustix::net::AddressFamily::INET,
                rustix::net::ipproto::ICMP,
                to,
            ),
        };
        let host = rustix::net::socket(family, rustix::net::SocketType::DGRAM, Some(protocol))
            .expect("the host's ping socket opens");
        let error = rustix::net::sendto(
            &host,
            &message,
            rustix::net::SendFlags::DONTWAIT,
            &std::net::SocketAddr::new(address, u16::MIN),
        )
        .expect_err("the host refuses the address");
        assert_eq!(
            ours,
            Err(SendError::Host(HostError::Errno(error.raw_os_error()))),
            "{version:?} to {to}"
        );
    }
}

/// Checks that a wait for writing on a new socket finishes at its first poll, writable and not readable.
pub fn a_wait_for_writing_finishes_at_once<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    for version in [IpVersion::V4, IpVersion::V6] {
        let socket = opened(&sockets, version);
        for interest in [ReadyFor::WRITABLE, ReadyFor::READABLE | ReadyFor::WRITABLE] {
            let wakes = Arc::new(Wakes::default());
            let facts = finished(poll_with(
                pin!(sockets.wait_until_ready(&socket, interest)),
                &wakes,
            ));
            assert_eq!(
                facts,
                ReadinessReport {
                    ready_for: ReadyFor::WRITABLE,
                    unconditional: UnconditionalEvents::empty(),
                },
                "{version:?} {interest:?}"
            );
        }
    }
}

/// How long a send that fills a send buffer is held on its link's fair queue: longer than a check waits on it.
///
/// https://man7.org/linux/man-pages/man8/tc-fq.8.html
pub const HELD_TO_FILL: Duration = Duration::from_secs(5);

/// The fair queue on the dummy link, which holds the sends that fill a send buffer there.
pub const FAIR_QUEUE_ON_DUMMY: &str = "qdisc add dev ia root fq";
/// Removes loopback's fair queue, and with it every send it holds.
pub const NO_FAIR_QUEUE: &str = "qdisc del dev lo root";

/// Returns send fields that hold a send on a fair queue for [`HELD_TO_FILL`].
pub fn held_fields() -> IpSendFields {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let now = Duration::new(
        u64::try_from(now.tv_sec).expect("a time after boot"),
        u32::try_from(now.tv_nsec).expect("nanoseconds"),
    );
    IpSendFields {
        transmit_time: Some(TransmitTime {
            clock: TransmitClock::Monotonic,
            at: now + HELD_TO_FILL,
            mode: TransmitTimeMode::Strict,
            errors: Report::NotRequested,
        }),
        ..plain_fields()
    }
}

/// Fills the smallest send buffer of a socket with requests to the address that its link's fair queue holds.
fn fill_send_buffer<S: IcmpSockets>(sockets: &S, socket: &S::Socket, to: IpAddr) {
    set(
        sockets,
        socket,
        IpSocketSetting::SendBuffer(ByteCount(usize::MIN)),
    );
    let message = echo(version_of(to), &vec![u8::MAX; PACED_MESSAGE]);
    let full = (0..ATTEMPTS).any(|_| {
        match sockets.send_buffer(socket, header_to(to, &message), held_fields(), &message) {
            Ok(()) => false,
            Err(SendError::Host(HostError::WouldBlock)) => true,
            Err(error) => panic!("{to}: {error:?}"),
        }
    });
    assert!(full, "{to}: the send buffer fills");
}

/// Returns a socket waited on for writing, its send buffer full of held requests, which removing the queue frees.
pub fn waiting_to_write<S: IcmpSockets>(
    sockets: S,
) -> Waiting<S, S::Socket, impl FnOnce(&S, &S::Socket)> {
    enter_network_namespace();
    tc(FAIR_QUEUE);
    let socket = opened(&sockets, IpVersion::V4);
    fill_send_buffer(&sockets, &socket, IpAddr::V4(Ipv4Addr::LOCALHOST));
    Waiting {
        readiness: sockets,
        handle: socket,
        interest: ReadyFor::WRITABLE,
        make_ready: |_: &S, _: &S::Socket| tc(NO_FAIR_QUEUE),
    }
}

/// Returns a socket waited on for writing, its send buffer full, which an ICMP error about its first request fails.
pub fn waiting_to_write_until_an_icmp_error<S: IcmpSockets>(
    sockets: S,
) -> Waiting<S, S::Socket, impl FnOnce(&S, &S::Socket)> {
    enter_network_namespace();
    ip(NETWORK);
    tc(FAIR_QUEUE_ON_DUMMY);
    let to = IpAddr::V4(ON_LINK);
    let socket = opened(&sockets, IpVersion::V4);
    let capture = Capture::on(DUMMY);
    ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
    let request = capture.echo_request();
    fill_send_buffer(&sockets, &socket, to);
    Waiting {
        readiness: sockets,
        handle: socket,
        interest: ReadyFor::WRITABLE,
        make_ready: move |_: &S, _: &S::Socket| send_icmp_error_about(&request, Quote::Least),
    }
}

/// Returns a socket with an IPv6 group membership in a namespace of its own, for Configurable's suite to set twice.
pub fn configurable_value<S: IcmpSockets>(sockets: S) -> (S, S::Socket, IpSocketSetting<'static>) {
    enter_network_namespace();
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6);
    (sockets, socket, multicast_setting())
}

/// Returns a membership of an IPv6 group, which Linux would refuse to join a second time.
pub fn multicast_setting() -> IpSocketSetting<'static> {
    // https://www.rfc-editor.org/rfc/rfc4291#section-2.7
    const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0, 0x124);
    IpSocketSetting::MulticastV6(MulticastGroup {
        group: GROUP_V6,
        interface: None,
        membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
    })
}

/// Returns an anycast address, which Linux would count twice.
pub fn anycast_setting() -> IpSocketSetting<'static> {
    // https://www.rfc-editor.org/rfc/rfc4193#section-3.1
    const ANYCAST: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 0x98);
    IpSocketSetting::Anycast(AnycastAddress {
        address: ANYCAST,
        interface: None,
        membership: Membership::Joined,
    })
}

pub fn send_buffer_setting() -> IpSocketSetting<'static> {
    IpSocketSetting::SendBuffer(ByteCount(usize::from(u16::MAX)))
}

pub fn timestamping_setting() -> IpSocketSetting<'static> {
    IpSocketSetting::Timestamping(Timestamping {
        receive_software: Report::Requested,
        receive_hardware: Report::NotRequested,
        bound_clock: None,
    })
}

/// Checks that every other kind of setting set twice answers as it did once, as Configurable's suite checks the first.
pub fn other_settings_set_twice_answer_as_once<S: IcmpSockets>(sockets: S) {
    enter_network_namespace();
    ip(NETWORK);
    for setting in [
        anycast_setting(),
        send_buffer_setting(),
        timestamping_setting(),
    ] {
        let socket = opened(&sockets, IpVersion::V6);
        let once = sockets.configure(&socket, setting);
        assert_eq!(sockets.configure(&socket, setting), once);
    }
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr, $options:expr) => {
        #[test]
        fn echo_reply_arrives_whole_over_ipv4() {
            $crate::echo_reply_arrives_whole_over_ipv4($value);
        }

        #[test]
        fn echo_reply_arrives_whole_over_ipv6() {
            $crate::echo_reply_arrives_whole_over_ipv6($value);
        }

        #[test]
        fn an_echo_reply_arrives_with_its_ipv4_options() {
            $crate::an_echo_reply_arrives_with_its_ipv4_options($value);
        }

        #[test]
        fn a_fragmented_reply_tells_its_largest_fragment() {
            $crate::a_fragmented_reply_tells_its_largest_fragment($value);
        }

        #[test]
        fn the_request_carries_the_bound_identifier() {
            $crate::the_request_carries_the_bound_identifier($value);
        }

        #[test]
        fn identifiers_are_shared_only_when_both_ask() {
            $crate::identifiers_are_shared_only_when_both_ask($value);
        }

        #[test]
        fn close_frees_the_identifier() {
            $crate::close_frees_the_identifier($value);
        }

        #[test]
        fn the_exit_node_chooses_an_identifier_when_asked() {
            $crate::the_exit_node_chooses_an_identifier_when_asked($value);
        }

        #[test]
        fn a_bound_address_is_the_requests_source() {
            $crate::a_bound_address_is_the_requests_source($value);
        }

        #[test]
        fn a_link_local_address_binds_the_socket_to_its_link() {
            $crate::a_link_local_address_binds_the_socket_to_its_link($value);
        }

        #[test]
        fn binding_refuses_what_linux_refuses() {
            $crate::binding_refuses_what_linux_refuses($value);
        }

        #[test]
        fn buffer_sizes_are_read_as_linux_gives_them() {
            $crate::buffer_sizes_are_read_as_linux_gives_them($value);
        }

        #[test]
        fn receive_with_nothing_queued_would_block() {
            $crate::receive_with_nothing_queued_would_block($value);
        }

        #[test]
        fn a_short_buffer_keeps_the_whole_length() {
            $crate::a_short_buffer_keeps_the_whole_length($value);
        }

        #[test]
        fn ipv4_header_fields_go_out() {
            $crate::ipv4_header_fields_go_out($value);
        }

        #[test]
        fn ipv6_header_fields_go_out() {
            $crate::ipv6_header_fields_go_out($value);
        }

        #[test]
        fn ipv6_extension_headers_go_out() {
            $crate::ipv6_extension_headers_go_out($value);
        }

        #[test]
        fn an_ipv6_source_goes_out() {
            $crate::an_ipv6_source_goes_out($value);
        }

        #[test]
        fn every_path_mtu_discovery_mode_sends() {
            $crate::every_path_mtu_discovery_mode_sends($value);
        }

        #[test]
        fn each_path_mtu_discovery_mode_takes_its_mtu() {
            $crate::each_path_mtu_discovery_mode_takes_its_mtu($value);
        }

        #[test]
        fn a_message_over_the_mtu_is_too_long_or_fragmented() {
            $crate::a_message_over_the_mtu_is_too_long_or_fragmented($value);
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
        fn icmp_type_and_code_select_the_route() {
            $crate::icmp_type_and_code_select_the_route($value);
        }

        #[test]
        fn interface_selects_the_link() {
            $crate::interface_selects_the_link($value);
        }

        #[test]
        fn a_bound_interface_holds_a_ping_from_a_bound_address() {
            $crate::a_bound_interface_holds_a_ping_from_a_bound_address($value);
        }

        #[test]
        fn a_ping_from_a_bound_address_takes_a_requested_interface_as_linux_does() {
            $crate::a_ping_from_a_bound_address_takes_a_requested_interface_as_linux_does($value);
        }

        #[test]
        fn a_link_local_bind_takes_pings_by_its_link_and_refuses_another() {
            $crate::a_link_local_bind_takes_pings_by_its_link_and_refuses_another($value);
        }

        #[test]
        fn link_scope_takes_only_the_link() {
            $crate::link_scope_takes_only_the_link($value);
        }

        #[test]
        fn broadcast_needs_its_permission() {
            $crate::broadcast_needs_its_permission($value);
        }

        #[test]
        fn a_waived_source_check_sends_from_an_absent_address() {
            $crate::a_waived_source_check_sends_from_an_absent_address($value);
        }

        #[test]
        fn source_preferences_are_taken() {
            $crate::source_preferences_are_taken($value, $options);
        }

        #[test]
        fn a_confirmed_send_confirms_the_neighbour() {
            $crate::a_confirmed_send_confirms_the_neighbour($value);
        }

        #[test]
        fn priority_chooses_the_band() {
            $crate::priority_chooses_the_band($value);
        }

        #[test]
        fn a_transmit_time_holds_the_send() {
            $crate::a_transmit_time_holds_the_send($value);
        }

        #[test]
        fn transmit_reports_carry_the_key_of_their_send() {
            $crate::transmit_reports_carry_the_key_of_their_send($value);
        }

        #[test]
        fn receive_timestamps_come_only_when_set() {
            $crate::receive_timestamps_come_only_when_set($value);
        }

        #[test]
        fn an_icmp_error_arrives_whole_over_ipv4() {
            $crate::an_icmp_error_arrives_whole_over_ipv4($value);
        }

        #[test]
        fn an_icmp_error_arrives_whole_over_ipv6() {
            $crate::an_icmp_error_arrives_whole_over_ipv6($value);
        }

        #[test]
        fn a_short_buffer_keeps_the_start_of_an_icmp_error() {
            $crate::a_short_buffer_keeps_the_start_of_an_icmp_error($value);
        }

        #[test]
        fn bytes_that_are_not_a_buffer_carry_what_a_buffer_does() {
            $crate::bytes_that_are_not_a_buffer_carry_what_a_buffer_does($value);
        }

        #[test]
        fn an_icmp_error_reaches_bytes_that_are_not_a_buffer() {
            $crate::an_icmp_error_reaches_bytes_that_are_not_a_buffer($value);
        }

        #[test]
        fn icmp_errors_queued_together_each_arrive() {
            $crate::icmp_errors_queued_together_each_arrive($value);
        }

        #[test]
        fn an_icmp_error_a_send_met_still_arrives() {
            $crate::an_icmp_error_a_send_met_still_arrives($value);
        }

        #[test]
        fn pacing_holds_messages_in_the_send_buffer() {
            $crate::pacing_holds_messages_in_the_send_buffer($value);
        }

        #[test]
        fn a_full_send_buffer_would_block() {
            $crate::a_full_send_buffer_would_block($value);
        }

        #[test]
        fn a_small_receive_buffer_counts_its_drops() {
            $crate::a_small_receive_buffer_counts_its_drops($value);
        }

        #[test]
        fn memberships_are_taken() {
            $crate::memberships_are_taken($value);
        }

        #[test]
        fn multicast_source_filters_are_written() {
            $crate::multicast_source_filters_are_written($value);
        }

        #[test]
        fn waits_on_many_sockets_each_wake_once() {
            $crate::waits_on_many_sockets_each_wake_once($value);
        }

        #[test]
        fn a_wait_sees_an_error_a_send_read_first() {
            $crate::a_wait_sees_an_error_a_send_read_first($value);
        }

        #[test]
        fn a_wait_for_a_fact_icmp_never_has_waits_on() {
            $crate::a_wait_for_a_fact_icmp_never_has_waits_on($value);
        }

        #[test]
        fn rejected_transmit_times_are_reported_with_their_keys() {
            $crate::rejected_transmit_times_are_reported_with_their_keys($value);
        }

        #[test]
        fn other_settings_set_twice_answer_as_once() {
            $crate::other_settings_set_twice_answer_as_once($value);
        }

        #[test]
        fn a_header_of_the_other_version_gets_the_hosts_error() {
            $crate::a_header_of_the_other_version_gets_the_hosts_error($value);
        }

        #[test]
        fn a_wait_for_writing_finishes_at_once() {
            $crate::a_wait_for_writing_finishes_at_once($value);
        }

        $crate::wcontainers_readiness_conformance::tests!(
            readiness,
            ready: $crate::waiting_for_a_reply($value),
            failing: $crate::waiting_for_data_until_an_icmp_error($value)
        );

        $crate::wcontainers_readiness_conformance::tests!(
            readiness_for_writing,
            ready: $crate::waiting_to_write($value),
            failing: $crate::waiting_to_write_until_an_icmp_error($value)
        );

        mod configurable {
            $crate::wcontainers_configurable_conformance::tests!($crate::configurable_value($value));
        }
    };
}
