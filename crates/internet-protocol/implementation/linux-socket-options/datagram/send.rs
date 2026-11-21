//! The fields of `IpHeaderFields` and `IpSendFields` as socket options and `sendmsg` control messages.

use core::mem::size_of;
use std::net::{IpAddr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::BorrowedFd;

use libc::c_int;
use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BroadcastPermission, IpHeaderFields, IpSendFields, LocalFragmentation, MobileAddressPreference,
    NeighbourConfirmation, PathMtuDiscovery, Report, RouteScope, RouteSelectors,
    SourceAddressCheck, TemporaryAddressPreference, TransmitClock, TransmitTimeMode,
};

use crate::datagram::addresses::{in_addr, in6_addr, zeroed};
use crate::datagram::sticky::{Slot, Sticky};
use crate::set_option;

/// A send the host cannot make as asked, by the named reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Host(HostError),
    Unsupported(&'static str),
}

impl From<HostError> for Refusal {
    fn from(error: HostError) -> Self {
        Refusal::Host(error)
    }
}

pub const WIRELESS_STATUS_WITHOUT_KEY: &str =
    "linux reports wireless status without the key of its send";

const fn space(length: usize) -> usize {
    // SAFETY: CMSG_SPACE only computes a length.
    unsafe { libc::CMSG_SPACE(length as u32) as usize }
}

const SEND_CONTROL: usize = space(size_of::<uapi::in6_pktinfo>())
    + space(size_of::<c_int>())
    + space(size_of::<u64>())
    + space(size_of::<u32>())
    + space(size_of::<u32>())
    + space(size_of::<u16>());

/// The bytes of the control messages, in words so that they are aligned as `cmsghdr` needs.
struct SendBuffer([usize; SEND_CONTROL.div_ceil(size_of::<usize>())]);

/// The control messages of one `sendmsg`.
pub struct Control {
    buffer: SendBuffer,
    length: usize,
}

impl Control {
    pub fn new() -> Self {
        Self {
            buffer: SendBuffer([usize::MIN; SEND_CONTROL.div_ceil(size_of::<usize>())]),
            length: usize::MIN,
        }
    }

    pub fn push<T: Copy>(&mut self, level: u32, kind: u32, value: &T) {
        let size = size_of::<T>();
        assert!(
            self.length + space(size) <= SEND_CONTROL,
            "the send control buffer holds every message"
        );
        // SAFETY: the header and its data fit in the buffer at `length`, which is aligned for a header.
        unsafe {
            let header = self
                .buffer
                .0
                .as_mut_ptr()
                .cast::<u8>()
                .add(self.length)
                .cast::<libc::cmsghdr>();
            (*header).cmsg_len = libc::CMSG_LEN(size as u32) as usize;
            (*header).cmsg_level = level as c_int;
            (*header).cmsg_type = kind as c_int;
            core::ptr::copy_nonoverlapping(
                (value as *const T).cast::<u8>(),
                libc::CMSG_DATA(header),
                size,
            );
        }
        self.length += space(size);
    }

    pub fn bytes(&self) -> &[u8] {
        // SAFETY: the words are read as the bytes the messages were written to.
        unsafe { core::slice::from_raw_parts(self.buffer.0.as_ptr().cast::<u8>(), self.length) }
    }
}

/// The send-side state of one socket: its sticky options, its flow label leases and its timestamping setting.
pub struct SendState {
    pub sticky: Sticky,
    flow_labels: Vec<u32>,
    receive_timestamps: u32,
    bound_clock: Option<u32>,
    packet_reports: bool,
}

/// The `SO_TIMESTAMPING` flags every socket keeps: keys, control messages, one report per source, and receive
/// timestamps only when asked for.
const TIMESTAMPING_BASE: u32 = uapi::SOF_TIMESTAMPING_OPT_ID as u32
    | uapi::SOF_TIMESTAMPING_OPT_CMSG as u32
    | uapi::SOF_TIMESTAMPING_OPT_TX_SWHW as u32
    | uapi::SOF_TIMESTAMPING_OPT_RX_FILTER as u32
    | uapi::SOF_TIMESTAMPING_OPT_PKTINFO as u32
    | uapi::SOF_TIMESTAMPING_SOFTWARE as u32
    | uapi::SOF_TIMESTAMPING_RAW_HARDWARE as u32;

impl SendState {
    /// The state of a new socket, whose timestamping it sets up.
    pub fn open(fd: BorrowedFd<'_>, ipv6: bool) -> Result<Self, HostError> {
        let mut state = Self {
            sticky: Sticky::new(),
            flow_labels: Vec::new(),
            receive_timestamps: u32::MIN,
            bound_clock: None,
            packet_reports: true,
        };
        state.write_timestamping(fd)?;
        if ipv6 {
            crate::set_flag(fd, uapi::SOL_IPV6, uapi::IPV6_FLOWINFO_SEND, true)?;
            crate::set_flag(fd, uapi::SOL_IPV6, uapi::IPV6_AUTOFLOWLABEL, false)?;
        }
        Ok(state)
    }

    fn timestamping(&self) -> uapi::so_timestamping {
        let mut flags = TIMESTAMPING_BASE | self.receive_timestamps;
        if !self.packet_reports {
            flags |= uapi::SOF_TIMESTAMPING_OPT_TSONLY as u32;
        }
        if self.bound_clock.is_some() {
            flags |= uapi::SOF_TIMESTAMPING_BIND_PHC as u32;
        }
        uapi::so_timestamping {
            flags: flags as c_int,
            bind_phc: self.bound_clock.unwrap_or(u32::MIN) as c_int,
        }
    }

    fn write_timestamping(&mut self, fd: BorrowedFd<'_>) -> Result<(), HostError> {
        let value = self.timestamping();
        self.sticky.ensure(fd, Slot::Timestamping, &value)
    }

    /// Sets which receive timestamps the socket takes and the hardware clock it is bound to.
    pub fn set_receive_timestamps(
        &mut self,
        fd: BorrowedFd<'_>,
        timestamping: &wcontainers_internet_protocol::Timestamping,
    ) -> Result<(), HostError> {
        let previous = (self.receive_timestamps, self.bound_clock);
        self.receive_timestamps = crate::receive_timestamping_flags(timestamping);
        self.bound_clock = timestamping.bound_clock.map(|clock| clock.0);
        let result = self.write_timestamping(fd);
        if result.is_err() {
            (self.receive_timestamps, self.bound_clock) = previous;
        }
        result
    }

    /// Writes the options a route's selectors set and returns its destination with the given port.
    pub fn apply_route(
        &mut self,
        fd: BorrowedFd<'_>,
        route: &RouteSelectors<'_>,
        port: u16,
        ipv6_socket: bool,
    ) -> Result<SocketAddr, HostError> {
        let sticky = &mut self.sticky;
        match route {
            RouteSelectors::V4(route) => {
                let tos = (route.dscp.value() << IP_DSCP_SHIFT) | route.ecn.value();
                sticky.ensure_int(fd, Slot::TypeOfService, c_int::from(tos))?;
                let interface = route.interface.map_or(u32::MIN, |index| index.0);
                sticky.ensure(fd, Slot::Ipv4UnicastInterface, &interface.to_be())?;
                sticky.ensure(fd, Slot::Mark, &route.mark.0)?;
                sticky.ensure_flag(fd, Slot::DontRoute, route.scope == RouteScope::Link)?;
                sticky.ensure_flag(
                    fd,
                    Slot::Broadcast,
                    route.broadcast == BroadcastPermission::Allowed,
                )?;
                sticky.ensure_bytes(fd, Slot::Ipv4Options, route.options.as_slice())?;
                sticky.ensure_flag(
                    fd,
                    Slot::Ipv4Transparent,
                    route.source_address_check == SourceAddressCheck::Waived,
                )?;
                Ok(crate::datagram::addresses::for_family(
                    SocketAddr::V4(SocketAddrV4::new(route.destination, port)),
                    ipv6_socket,
                ))
            }
            RouteSelectors::V6(route) => {
                let class = (route.dscp.value() << IP_DSCP_SHIFT) | route.ecn.value();
                sticky.ensure_int(fd, Slot::TrafficClass, c_int::from(class))?;
                let interface = route.interface.map_or(u32::MIN, |index| index.0);
                // Linux takes this index in network byte order, as it takes IP_UNICAST_IF's.
                sticky.ensure_int(fd, Slot::Ipv6UnicastInterface, interface.to_be() as c_int)?;
                sticky.ensure(fd, Slot::Mark, &route.mark.0)?;
                sticky.ensure(fd, Slot::AddressPreferences, &address_preferences(route))?;
                sticky.ensure_bytes(
                    fd,
                    Slot::RoutingHeader,
                    route
                        .routing_header
                        .as_ref()
                        .map_or(&[][..], |header| header.slice()),
                )?;
                sticky.ensure_flag(
                    fd,
                    Slot::Ipv6Freebind,
                    route.source_address_check == SourceAddressCheck::Waived,
                )?;
                let label = route.flow_label.value();
                if label != u32::MIN && !self.flow_labels.contains(&label) {
                    lease_flow_label(fd, route.destination, label)?;
                    self.flow_labels.push(label);
                }
                Ok(SocketAddr::V6(SocketAddrV6::new(
                    route.destination,
                    port,
                    label,
                    interface,
                )))
            }
        }
    }

    /// Writes the options and control messages of a send's header and returns its destination with the port.
    pub fn apply_header(
        &mut self,
        fd: BorrowedFd<'_>,
        control: &mut Control,
        header: &IpHeaderFields<'_>,
        port: u16,
        ipv6_socket: bool,
    ) -> Result<SocketAddr, HostError> {
        match header {
            IpHeaderFields::V4(header) => {
                let destination =
                    self.apply_route(fd, &RouteSelectors::V4(header.route), port, ipv6_socket)?;
                // The interface also goes here: a multicast send takes it from this, not from IP_UNICAST_IF.
                if header.route.source.is_some() || header.route.interface.is_some() {
                    let mut information = zeroed::<uapi::in_pktinfo>();
                    information.ipi_spec_dst = in_addr(
                        header
                            .route
                            .source
                            .unwrap_or(std::net::Ipv4Addr::UNSPECIFIED),
                    );
                    information.ipi_ifindex =
                        header.route.interface.map_or(u32::MIN, |index| index.0) as c_int;
                    control.push(uapi::SOL_IP, uapi::IP_PKTINFO, &information);
                }
                if let Some(limit) = header.time_to_live {
                    control.push(uapi::SOL_IP, uapi::IP_TTL, &c_int::from(limit.0));
                }
                let mode = path_mtu_discovery(header.path_mtu_discovery);
                self.sticky
                    .ensure_int(fd, Slot::Ipv4PathMtuDiscovery, mode)?;
                Ok(destination)
            }
            IpHeaderFields::V6(header) => {
                let destination = self.apply_route(
                    fd,
                    &RouteSelectors::V6(header.route.clone()),
                    port,
                    ipv6_socket,
                )?;
                if header.route.source.is_some() || header.route.interface.is_some() {
                    let mut information = zeroed::<uapi::in6_pktinfo>();
                    information.ipi6_addr = in6_addr(
                        header
                            .route
                            .source
                            .unwrap_or(std::net::Ipv6Addr::UNSPECIFIED),
                    );
                    information.ipi6_ifindex =
                        header.route.interface.map_or(u32::MIN, |index| index.0) as c_int;
                    control.push(uapi::SOL_IPV6, uapi::IPV6_PKTINFO, &information);
                }
                if let Some(limit) = header.hop_limit {
                    control.push(uapi::SOL_IPV6, uapi::IPV6_HOPLIMIT, &c_int::from(limit.0));
                }
                let sticky = &mut self.sticky;
                sticky.ensure_int(
                    fd,
                    Slot::Ipv6PathMtuDiscovery,
                    path_mtu_discovery(header.path_mtu_discovery),
                )?;
                sticky.ensure_flag(
                    fd,
                    Slot::Ipv6DontFragment,
                    header.local_fragmentation == LocalFragmentation::Forbidden,
                )?;
                let mtu = header.fragment_mtu.map_or(u32::MIN, |mtu| mtu.0);
                sticky.ensure_int(fd, Slot::Ipv6FragmentMtu, mtu as c_int)?;
                let raw = |header: &Option<etherparse::Ipv6RawExtHeaderSlice<'_>>| {
                    header
                        .as_ref()
                        .map_or(&[][..], |header| header.slice())
                        .to_vec()
                };
                sticky.ensure_bytes(fd, Slot::HopByHopOptions, &raw(&header.hop_by_hop_options))?;
                sticky.ensure_bytes(
                    fd,
                    Slot::RoutingDestinationOptions,
                    &raw(&header.routing_destination_options),
                )?;
                sticky.ensure_bytes(
                    fd,
                    Slot::DestinationOptions,
                    &raw(&header.destination_options),
                )?;
                Ok(destination)
            }
        }
    }

    /// Writes the options and control messages of a send's own fields and returns its `sendmsg` flags.
    pub fn apply_send_fields(
        &mut self,
        fd: BorrowedFd<'_>,
        control: &mut Control,
        fields: &IpSendFields,
    ) -> Result<c_int, Refusal> {
        let mut flags = libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL;
        if fields.neighbour == NeighbourConfirmation::Confirmed {
            flags |= libc::MSG_CONFIRM;
        }
        self.sticky.ensure(fd, Slot::Priority, &fields.priority.0)?;
        if let Some(time) = fields.transmit_time {
            let mut configuration = zeroed::<uapi::sock_txtime>();
            configuration.clockid = match time.clock {
                TransmitClock::Realtime => libc::CLOCK_REALTIME,
                TransmitClock::Monotonic => libc::CLOCK_MONOTONIC,
                TransmitClock::InternationalAtomicTime => libc::CLOCK_TAI,
            };
            if time.mode == TransmitTimeMode::Deadline {
                configuration.flags |= uapi::txtime_flags::SOF_TXTIME_DEADLINE_MODE as u32;
            }
            if time.errors == Report::Requested {
                configuration.flags |= uapi::txtime_flags::SOF_TXTIME_REPORT_ERRORS as u32;
            }
            self.sticky.ensure(fd, Slot::TransmitTime, &configuration)?;
            let at =
                u64::try_from(time.at.as_nanos()).map_err(|_| HostError::Errno(libc::EINVAL))?;
            control.push(uapi::SOL_SOCKET, uapi::SCM_TXTIME, &at);
        }
        let reports = &fields.transmit_reports;
        if reports.wireless_status == Report::Requested {
            return Err(Refusal::Unsupported(WIRELESS_STATUS_WITHOUT_KEY));
        }
        let mut record = u32::MIN;
        for (report, flag) in [
            (reports.scheduled, uapi::SOF_TIMESTAMPING_TX_SCHED as u32),
            (reports.software, uapi::SOF_TIMESTAMPING_TX_SOFTWARE as u32),
            (reports.hardware, uapi::SOF_TIMESTAMPING_TX_HARDWARE as u32),
            (
                reports.acknowledgement,
                uapi::SOF_TIMESTAMPING_TX_ACK as u32,
            ),
            (
                reports.completion,
                uapi::SOF_TIMESTAMPING_TX_COMPLETION as u32,
            ),
        ] {
            if report == Report::Requested {
                record |= flag;
            }
        }
        if record != u32::MIN {
            self.packet_reports = reports.packet == Report::Requested;
            self.write_timestamping(fd)?;
            control.push(uapi::SOL_SOCKET, uapi::SO_TIMESTAMPING, &record);
            control.push(uapi::SOL_SOCKET, uapi::SCM_TS_OPT_ID, &reports.key.0);
        }
        Ok(flags)
    }
}

/// The bits of the type of service byte below its DSCP, the ECN field.
const IP_DSCP_SHIFT: u8 = etherparse::IpEcn::MAX_U8.count_ones() as u8;

fn path_mtu_discovery(mode: PathMtuDiscovery) -> c_int {
    (match mode {
        PathMtuDiscovery::Dont => uapi::IP_PMTUDISC_DONT,
        PathMtuDiscovery::Want => uapi::IP_PMTUDISC_WANT,
        PathMtuDiscovery::Do => uapi::IP_PMTUDISC_DO,
        PathMtuDiscovery::Probe => uapi::IP_PMTUDISC_PROBE,
        PathMtuDiscovery::Interface => uapi::IP_PMTUDISC_INTERFACE,
        PathMtuDiscovery::Omit => uapi::IP_PMTUDISC_OMIT,
    }) as c_int
}

fn address_preferences(route: &wcontainers_internet_protocol::Ipv6RouteSelectors<'_>) -> c_int {
    let temporary = match route.source_preferences.temporary {
        TemporaryAddressPreference::SystemDefault => uapi::IPV6_PREFER_SRC_PUBTMP_DEFAULT,
        TemporaryAddressPreference::Temporary => uapi::IPV6_PREFER_SRC_TMP,
        TemporaryAddressPreference::Public => uapi::IPV6_PREFER_SRC_PUBLIC,
    };
    let mobile = match route.source_preferences.mobile {
        MobileAddressPreference::Home => uapi::IPV6_PREFER_SRC_HOME,
        MobileAddressPreference::CareOf => uapi::IPV6_PREFER_SRC_COA,
    };
    (temporary | mobile) as c_int
}

/// Takes a lease of a flow label for the socket, which Linux needs before the label may be sent.
fn lease_flow_label(
    fd: BorrowedFd<'_>,
    destination: std::net::Ipv6Addr,
    label: u32,
) -> Result<(), HostError> {
    let mut request = zeroed::<uapi::in6_flowlabel_req>();
    request.flr_dst = in6_addr(destination);
    request.flr_label = label.to_be();
    request.flr_action = uapi::IPV6_FL_A_GET as u8;
    request.flr_share = uapi::IPV6_FL_S_ANY as u8;
    request.flr_flags = uapi::IPV6_FL_F_CREATE as u16;
    set_option(fd, uapi::SOL_IPV6, uapi::IPV6_FLOWLABEL_MGR, &request)
}

/// The unspecified address of a destination's family, for binding before a connect or send.
pub fn unspecified_like(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V4(_) => IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
        IpAddr::V6(_) => IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
    }
}

impl Default for Control {
    fn default() -> Self {
        Self::new()
    }
}
