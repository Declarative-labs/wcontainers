#![no_std]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use core::time::Duration;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options, Ipv6FlowLabel, Ipv6RawExtHeaderSlice};
use wcontainers_abi::HostError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpVersion {
    V4,
    V6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HopLimit(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaximumTransmissionUnit(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FragmentSize(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLength(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteCount(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BytesPerSecond(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketMark(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketPriority(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropCount(pub u32);

/// A [network interface](https://man7.org/linux/man-pages/man7/netdevice.7.html) of the exit node, by its index there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterfaceIndex(pub u32);

/// A [PTP hardware clock](https://docs.kernel.org/driver-api/ptp.html) of the exit node, by its index there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PtpHardwareClockIndex(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IcmpType(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IcmpCode(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MobilityHeaderType(pub u8);

/// The value of an IPv6 [router alert](https://www.iana.org/assignments/ipv6-routeralert-values) option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouterAlertValue(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransmitReportKey(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Port(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrefixLength(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpPrefix {
    pub address: IpAddr,
    pub length: PrefixLength,
}

/// A processor of the exit node, by its number there, as [`SO_INCOMING_CPU`](https://man7.org/linux/man-pages/man7/socket.7.html#:~:text=SO_INCOMING_CPU) names one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessorIndex(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportPorts {
    pub source: Port,
    pub destination: Port,
}

/// A mode of [path MTU discovery](https://man7.org/linux/man-pages/man2/IP_MTU_DISCOVER.2const.html).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathMtuDiscovery {
    Dont,
    Want,
    Do,
    Probe,
    Interface,
    Omit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalFragmentation {
    Allowed,
    Forbidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteScope {
    Universe,
    Link,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BroadcastPermission {
    Refused,
    Allowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporaryAddressPreference {
    SystemDefault,
    Temporary,
    Public,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobileAddressPreference {
    Home,
    CareOf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePreferences {
    pub temporary: TemporaryAddressPreference,
    pub mobile: MobileAddressPreference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpperLayerSelector {
    Icmp {
        message_type: IcmpType,
        code: IcmpCode,
    },
    MobilityHeader(MobilityHeaderType),
    Ports(TransportPorts),
}

/// The protocol above IP and the fields of its header that choose a route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpperLayer {
    pub protocol: IpNumber,
    pub selector: Option<UpperLayerSelector>,
}

/// The check that the exit node has the source address of a route lookup.
///
/// [`IP_TRANSPARENT`](https://man7.org/linux/man-pages/man2/IP_TRANSPARENT.2const.html) waives it, as [`IP_HDRINCL`](https://man7.org/linux/man-pages/man2/IP_HDRINCL.2const.html) also does for IPv4 and [`IP_FREEBIND`](https://man7.org/linux/man-pages/man2/IP_FREEBIND.2const.html) or the `net.ipv6.ip_nonlocal_bind` sysctl also do for IPv6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceAddressCheck {
    Required,
    Waived,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4RouteSelectors<'a> {
    pub destination: Ipv4Addr,
    pub source: Option<Ipv4Addr>,
    pub source_address_check: SourceAddressCheck,
    pub interface: Option<InterfaceIndex>,
    pub dscp: IpDscp,
    pub ecn: IpEcn,
    pub mark: PacketMark,
    pub scope: RouteScope,
    pub broadcast: BroadcastPermission,
    /// [IP options](https://man7.org/linux/man-pages/man2/IP_OPTIONS.2const.html) in the form `IP_OPTIONS` takes, with a source route's first hop in its first slot.
    pub options: &'a Ipv4Options,
    pub upper_layer: UpperLayer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ipv6RouteSelectors<'a> {
    pub destination: Ipv6Addr,
    pub source: Option<Ipv6Addr>,
    pub source_address_check: SourceAddressCheck,
    pub interface: Option<InterfaceIndex>,
    pub dscp: IpDscp,
    pub ecn: IpEcn,
    pub flow_label: Ipv6FlowLabel,
    pub mark: PacketMark,
    pub source_preferences: SourcePreferences,
    pub routing_header: Option<Ipv6RawExtHeaderSlice<'a>>,
    pub upper_layer: UpperLayer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteSelectors<'a> {
    V4(Ipv4RouteSelectors<'a>),
    V6(Ipv6RouteSelectors<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4HeaderFields<'a> {
    pub route: Ipv4RouteSelectors<'a>,
    pub time_to_live: Option<HopLimit>,
    pub path_mtu_discovery: PathMtuDiscovery,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ipv6HeaderFields<'a> {
    pub route: Ipv6RouteSelectors<'a>,
    pub hop_limit: Option<HopLimit>,
    pub path_mtu_discovery: PathMtuDiscovery,
    pub local_fragmentation: LocalFragmentation,
    pub fragment_mtu: Option<MaximumTransmissionUnit>,
    pub hop_by_hop_options: Option<Ipv6RawExtHeaderSlice<'a>>,
    pub routing_destination_options: Option<Ipv6RawExtHeaderSlice<'a>>,
    pub destination_options: Option<Ipv6RawExtHeaderSlice<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpHeaderFields<'a> {
    V4(Ipv4HeaderFields<'a>),
    V6(Ipv6HeaderFields<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighbourConfirmation {
    Unconfirmed,
    Confirmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteKind {
    Unicast,
    Local,
    Broadcast,
    Multicast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub kind: RouteKind,
    pub source: IpAddr,
    pub interface: InterfaceIndex,
    pub path_mtu: MaximumTransmissionUnit,
    pub device_mtu: MaximumTransmissionUnit,
    pub default_hop_limit: HopLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    NotRequested,
    Requested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransmitReportRequest {
    pub key: TransmitReportKey,
    pub scheduled: Report,
    pub software: Report,
    pub hardware: Report,
    pub acknowledgement: Report,
    pub completion: Report,
    pub wireless_status: Report,
    pub packet: Report,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransmitClock {
    Realtime,
    Monotonic,
    InternationalAtomicTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransmitTimeMode {
    Strict,
    Deadline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransmitTime {
    pub clock: TransmitClock,
    pub at: Duration,
    pub mode: TransmitTimeMode,
    pub errors: Report,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpSendFields {
    pub neighbour: NeighbourConfirmation,
    pub priority: PacketPriority,
    pub transmit_time: Option<TransmitTime>,
    pub transmit_reports: TransmitReportRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendError {
    Host(HostError),
    MessageTooLong {
        maximum_transmission_unit: MaximumTransmissionUnit,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Membership {
    Joined,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFilter<'a, A> {
    Include(&'a [A]),
    Exclude(&'a [A]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulticastMembership<'a, A> {
    Joined(SourceFilter<'a, A>),
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MulticastGroup<'a, A> {
    pub group: A,
    pub interface: Option<InterfaceIndex>,
    pub membership: MulticastMembership<'a, A>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnycastAddress {
    pub address: Ipv6Addr,
    pub interface: Option<InterfaceIndex>,
    pub membership: Membership,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timestamping {
    pub receive_software: Report,
    pub receive_hardware: Report,
    pub bound_clock: Option<PtpHardwareClockIndex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpSocketSetting<'a> {
    MulticastV4(MulticastGroup<'a, Ipv4Addr>),
    MulticastV6(MulticastGroup<'a, Ipv6Addr>),
    Anycast(AnycastAddress),
    MaximumPacingRate(Option<BytesPerSecond>),
    SendBuffer(ByteCount),
    ReceiveBuffer(ByteCount),
    Timestamping(Timestamping),
    MulticastLoopback(bool),
    MulticastAllGroups { version: IpVersion, enabled: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ipv6Families {
    V6Only,
    DualStack,
}

/// An address a socket is [bound](https://man7.org/linux/man-pages/man2/bind.2.html) to, the unspecified one for any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindAddress {
    V4(Ipv4Addr),
    V6 {
        address: Ipv6Addr,
        families: Ipv6Families,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortRange {
    pub first: Port,
    pub last: Port,
}

/// The ports a local port is chosen from, the [`IP_LOCAL_PORT_RANGE`](https://man7.org/linux/man-pages/man2/IP_LOCAL_PORT_RANGE.2const.html) less the reserved ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortChoice<'a> {
    pub range: PortRange,
    pub reserved: &'a [PortRange],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalPort<'a> {
    Requested(Port),
    ChosenByKernel(Port),
    ChosenByExitNode(PortChoice<'a>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalBinding<'a> {
    pub address: BindAddress,
    pub port: LocalPort<'a>,
}

/// The sharing of a local address that [`SO_REUSEADDR`](https://man7.org/linux/man-pages/man7/socket.7.html#:~:text=SO_REUSEADDR) allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressSharing {
    Exclusive,
    Shared,
}

/// The sharing of a local port that [`SO_REUSEPORT`](https://man7.org/linux/man-pages/man7/socket.7.html#:~:text=SO_REUSEPORT%20%28since) allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortSharing {
    Exclusive,
    Shared,
}

/// The permission to use addresses the exit node does not have, which [`IP_FREEBIND`](https://man7.org/linux/man-pages/man2/IP_FREEBIND.2const.html) and [`IP_TRANSPARENT`](https://man7.org/linux/man-pages/man2/IP_TRANSPARENT.2const.html) give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonlocalAddresses {
    Refused,
    Bindable,
    Transparent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterOperation(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterJump(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterOperand(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterInstruction {
    pub operation: FilterOperation,
    pub jump_if_true: FilterJump,
    pub jump_if_false: FilterJump,
    pub operand: FilterOperand,
}

/// A classic BPF program, as [`SO_ATTACH_FILTER`](https://man7.org/linux/man-pages/man7/socket.7.html#:~:text=SO_ATTACH_FILTER) takes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SocketFilter<'a>(pub &'a [FilterInstruction]);

/// A setting of how a socket holds its local address and port among the exit node's sockets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingSetting<'a> {
    AddressSharing(AddressSharing),
    PortSharing(PortSharing),
    NonlocalAddresses(NonlocalAddresses),
    BoundInterface(Option<InterfaceIndex>),
    GroupFilter(Option<SocketFilter<'a>>),
    IncomingProcessor(Option<ProcessorIndex>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowLabelChoice {
    Unlabelled,
    Label(Ipv6FlowLabel),
    Automatic,
}

/// A field a connected socket's stack gives every packet it sends, set once for all of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpPacketSetting<'a> {
    HopLimit {
        version: IpVersion,
        limit: Option<HopLimit>,
    },
    TrafficClass {
        version: IpVersion,
        dscp: IpDscp,
        ecn: IpEcn,
    },
    PathMtuDiscovery {
        version: IpVersion,
        mode: PathMtuDiscovery,
    },
    Ipv4Options(&'a Ipv4Options),
    HopByHopOptions(Option<Ipv6RawExtHeaderSlice<'a>>),
    RoutingDestinationOptions(Option<Ipv6RawExtHeaderSlice<'a>>),
    RoutingHeader(Option<Ipv6RawExtHeaderSlice<'a>>),
    DestinationOptions(Option<Ipv6RawExtHeaderSlice<'a>>),
    FlowLabel(FlowLabelChoice),
    Mark(PacketMark),
    Priority(PacketPriority),
    UnicastInterface(Option<InterfaceIndex>),
    RouteScope(RouteScope),
    SourcePreferences(SourcePreferences),
}

/// Arrival times, the software one on the realtime clock since the Unix epoch and the hardware one on the receiving device's clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiveTimestamps {
    pub software: Option<Duration>,
    pub hardware: Option<Duration>,
}

/// The length of a received IP packet, header included, and what arrived with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceivedPacketFields {
    pub length: usize,
    pub interface: InterfaceIndex,
    /// The preferred source address for replies to the packet, the `ipi_spec_dst` of an [`in_pktinfo`](https://man7.org/linux/man-pages/man2/in_pktinfo.2type.html).
    pub specific_destination: Option<Ipv4Addr>,
    pub largest_fragment: Option<FragmentSize>,
    pub link_layer_length: Option<FrameLength>,
    pub timestamps: ReceiveTimestamps,
    pub mark: PacketMark,
    pub priority: PacketPriority,
    pub drops_since_previous: DropCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransmitStage {
    Scheduled,
    Sent,
    Acknowledged,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimestampSource {
    Software,
    Hardware,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WirelessAcknowledgement {
    Acknowledged,
    NotAcknowledged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransmitTimeError {
    InvalidParameter,
    Missed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransmitEvent {
    Timestamp {
        stage: TransmitStage,
        source: TimestampSource,
        time: Duration,
    },
    WirelessStatus(WirelessAcknowledgement),
    TransmitTimeRejected(TransmitTimeError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransmitReport {
    pub key: TransmitReportKey,
    pub interface: InterfaceIndex,
    pub event: TransmitEvent,
    pub packet_length: Option<ByteCount>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Received {
    Packet(ReceivedPacketFields),
    TransmitReport(TransmitReport),
}
