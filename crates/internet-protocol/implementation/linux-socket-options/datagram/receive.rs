//! The control messages of `recvmsg` and the error queue as `ReceivedPacketFields`, reports and whole packets.

use core::mem::size_of;
use core::num::NonZeroU32;
use core::time::Duration;
use std::io::IoSliceMut;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::BorrowedFd;

use etherparse::checksum::Sum16BitWords;
use etherparse::{
    IpDscp, IpEcn, IpNumber, Ipv4Header, Ipv6FlowLabel, Ipv6Header, Ipv6RawExtHeader,
};
use libc::c_int;
use linux_raw_sys::net as uapi;
use nix::sys::socket::SockaddrStorage;
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    ByteCount, DropCount, FragmentSize, FrameLength, InterfaceIndex, PacketMark, PacketPriority,
    ReceiveTimestamps, ReceivedPacketFields, TimestampSource, TransmitEvent, TransmitReport,
    TransmitReportKey, TransmitStage,
};

use zerocopy::FromBytes;

use crate::datagram::addresses::{
    Ipv4PacketInformation, Ipv6PacketInformation, address_of_storage, unmapped,
};
use crate::{ControlRoom, receive_vectored, uapi_mirror};

uapi_mirror! {
    /// `struct scm_ts_pktinfo`.
    struct TimestampingPacketInformation mirrors uapi::scm_ts_pktinfo {
        if_index: u32,
        pkt_length: u32,
        reserved: [u32; 2],
    }
}

uapi_mirror! {
    /// `struct sock_extended_err`.
    struct ExtendedError mirrors libc::sock_extended_err {
        ee_errno: u32,
        ee_origin: u8,
        ee_type: u8,
        ee_code: u8,
        ee_pad: u8,
        ee_info: u32,
        ee_data: u32,
    }
}

use crate::control_space as space;

/// The IPv6 extension headers Linux reports for one packet: hop-by-hop, destination, routing, destination.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4
const EXTENSION_HEADERS: usize = 4;

/// The integer-sized control messages.
///
/// They are TTL, TOS, hop limit, traffic class, flow information, two fragment sizes, mark, priority, drop counter and
/// segment size.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
const INTEGER_MESSAGES: usize = 11;

const RECEIVE_CONTROL: usize = space(size_of::<uapi::in_pktinfo>())
    + space(size_of::<uapi::in6_pktinfo>())
    + space(size_of::<uapi::sockaddr_in6>())
    + space(size_of::<uapi::sockaddr_in6>())
    + INTEGER_MESSAGES * space(size_of::<c_int>())
    + space(Ipv4Header::MAX_LEN)
    + EXTENSION_HEADERS * space(Ipv6RawExtHeader::MAX_LEN)
    + space(size_of::<[libc::timespec; crate::TIMESTAMPING_TIMES]>())
    + space(size_of::<uapi::scm_ts_pktinfo>())
    + space(size_of::<libc::sock_extended_err>() + size_of::<uapi::sockaddr_in6>());

/// The origin of an error queue entry about a transmit time, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h
pub const SO_EE_ORIGIN_TXTIME: u8 = 6;
/// The code of a transmit time its queue cannot take, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h
pub const SO_EE_CODE_TXTIME_INVALID_PARAM: u8 = 1;
/// The code of a transmit time its queue missed, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/errqueue.h
pub const SO_EE_CODE_TXTIME_MISSED: u8 = 2;

/// The control messages of one receive.
pub type ControlBuffer = ControlRoom<{ RECEIVE_CONTROL.div_ceil(size_of::<usize>()) }>;

/// What one `recvmsg` gave: its byte count, flags, sender and control messages.
pub struct Message<'a> {
    pub length: usize,
    pub source: Option<SocketAddr>,
    pub control: &'a [u8],
    pub control_truncated: bool,
}

/// One `recvmsg` into `into`, from the error queue or the receive queue.
pub fn receive<'a>(
    fd: BorrowedFd<'_>,
    into: &mut [u8],
    control: &'a mut ControlBuffer,
    error_queue: bool,
) -> Result<Message<'a>, HostError> {
    receive_with_tail(fd, into, &mut [], control, error_queue)
}

/// The largest payload of one receive: an IP packet's largest length.
const LARGEST_PAYLOAD: usize = u16::MAX as usize;

std::thread_local! {
    static TAIL: core::cell::RefCell<Box<[u8]>> =
        core::cell::RefCell::new(vec![u8::MIN; LARGEST_PAYLOAD].into_boxed_slice());
}

/// Runs `with` on this thread's room for a whole payload, made once so that a receive does not clear it.
///
/// Not reentrant: `with` may not call it again.
pub fn with_tail<R>(with: impl FnOnce(&mut [u8]) -> R) -> R {
    TAIL.with_borrow_mut(|tail| with(tail))
}

/// One `recvmsg` into `into` and, for what does not fit there, on into `tail`.
pub fn receive_with_tail<'a>(
    fd: BorrowedFd<'_>,
    into: &mut [u8],
    tail: &mut [u8],
    control: &'a mut ControlBuffer,
    error_queue: bool,
) -> Result<Message<'a>, HostError> {
    let flags = libc::MSG_DONTWAIT
        | if error_queue {
            libc::MSG_ERRQUEUE
        } else {
            libc::MSG_TRUNC
        };
    let message = receive_vectored(
        fd,
        &mut [IoSliceMut::new(into), IoSliceMut::new(tail)],
        flags,
        control,
        socket_address,
    )?;
    Ok(Message {
        length: message.length,
        source: message.source,
        control: message.control,
        control_truncated: message.flags & libc::MSG_CTRUNC != 0,
    })
}

/// The IPv4 or IPv6 address of a sender, IPv4-mapped ones as IPv4, its flow information in host order as elsewhere here.
fn socket_address(source: &SockaddrStorage) -> Option<SocketAddr> {
    let address = if let Some(v4) = source.as_sockaddr_in() {
        SocketAddr::V4(SocketAddrV4::from(*v4))
    } else {
        let mut address = SocketAddrV6::from(*source.as_sockaddr_in6()?);
        address.set_flowinfo(u32::from_be(address.flowinfo()));
        SocketAddr::V6(address)
    };
    Some(unmapped(address))
}

pub const CONTROL_TRUNCATED: &str =
    "the control messages of a packet did not fit the buffer for them";

/// The control messages of one received packet or error queue entry.
#[derive(Default)]
pub struct Ancillary<'a> {
    pub interface: Option<u32>,
    pub specific_destination: Option<Ipv4Addr>,
    pub header_destination: Option<IpAddr>,
    pub original_destination: Option<SocketAddr>,
    pub hop_limit: Option<u8>,
    pub traffic_class: Option<u8>,
    pub ipv4_options: &'a [u8],
    pub flow_information: Option<u32>,
    pub extension_headers: [Option<(IpNumber, &'a [u8])>; EXTENSION_HEADERS],
    pub largest_fragment: Option<u16>,
    pub mark: Option<u32>,
    pub priority: Option<u32>,
    pub drops: Option<u32>,
    pub software: Option<Duration>,
    pub hardware: Option<Duration>,
    pub link: Option<(u32, u32)>,
    pub error: Option<(libc::sock_extended_err, Option<SocketAddr>)>,
    pub segment_size: Option<u16>,
}

/// Reads the value a control message's data starts with.
fn read<T: FromBytes>(data: &[u8]) -> Option<T> {
    T::read_from_prefix(data).ok().map(|(value, _)| value)
}

impl<'a> Ancillary<'a> {
    pub fn parse(control: &'a [u8]) -> Self {
        let mut found = Self::default();
        let mut headers = usize::MIN;
        for (level, kind, data) in crate::ControlMessages::new(control) {
            found.take(level as u32, kind as u32, data, &mut headers);
        }
        found
    }

    fn take(&mut self, level: u32, kind: u32, data: &'a [u8], headers: &mut usize) {
        let integer = || read::<c_int>(data);
        match (level, kind) {
            (uapi::SOL_IP, uapi::IP_PKTINFO) => {
                if let Some(information) = read::<Ipv4PacketInformation>(data) {
                    self.interface = Some(information.ipi_ifindex as u32);
                    self.specific_destination = Some(Ipv4Addr::from(information.ipi_spec_dst));
                    self.header_destination =
                        Some(IpAddr::V4(Ipv4Addr::from(information.ipi_addr)));
                }
            }
            (uapi::SOL_IPV6, uapi::IPV6_PKTINFO) => {
                if let Some(information) = read::<Ipv6PacketInformation>(data) {
                    self.interface = Some(information.ipi6_ifindex as u32);
                    self.header_destination =
                        Some(IpAddr::V6(Ipv6Addr::from(information.ipi6_addr)));
                }
            }
            (uapi::SOL_IP, uapi::IP_ORIGDSTADDR) | (uapi::SOL_IPV6, uapi::IPV6_ORIGDSTADDR) => {
                self.original_destination = address(data);
            }
            (uapi::SOL_IP, uapi::IP_TTL) | (uapi::SOL_IPV6, uapi::IPV6_HOPLIMIT) => {
                self.hop_limit = integer().map(|value| value as u8);
            }
            (uapi::SOL_IP, uapi::IP_TOS) => self.traffic_class = data.first().copied(),
            (uapi::SOL_IPV6, uapi::IPV6_TCLASS) => {
                self.traffic_class = integer().map(|value| value as u8)
            }
            (uapi::SOL_IP, uapi::IP_RECVOPTS) => self.ipv4_options = data,
            (uapi::SOL_IPV6, uapi::IPV6_FLOWINFO) => {
                self.flow_information = read::<u32>(data).map(u32::from_be)
            }
            (uapi::SOL_IPV6, uapi::IPV6_HOPOPTS | uapi::IPV6_DSTOPTS | uapi::IPV6_RTHDR) => {
                let number = match kind {
                    uapi::IPV6_HOPOPTS => IpNumber::IPV6_HEADER_HOP_BY_HOP,
                    uapi::IPV6_DSTOPTS => IpNumber::IPV6_DESTINATION_OPTIONS,
                    _ => IpNumber::IPV6_ROUTE_HEADER,
                };
                if let Some(slot) = self.extension_headers.get_mut(*headers) {
                    *slot = Some((number, data));
                    *headers += 1;
                }
            }
            (uapi::SOL_IP, uapi::IP_RECVFRAGSIZE) | (uapi::SOL_IPV6, uapi::IPV6_RECVFRAGSIZE) => {
                self.largest_fragment = integer().map(|value| value as u16);
            }
            (uapi::SOL_SOCKET, uapi::SO_MARK) => self.mark = read::<u32>(data),
            (uapi::SOL_SOCKET, uapi::SO_PRIORITY) => self.priority = read::<u32>(data),
            (uapi::SOL_SOCKET, uapi::SO_RXQ_OVFL) => self.drops = read::<u32>(data),
            (uapi::SOL_SOCKET, uapi::SCM_TIMESTAMPING) => {
                (self.software, self.hardware) = crate::timestamps(data);
            }
            (uapi::SOL_SOCKET, uapi::SCM_TIMESTAMPING_PKTINFO) => {
                self.link = read::<TimestampingPacketInformation>(data)
                    .map(|information| (information.if_index, information.pkt_length));
            }
            (uapi::SOL_IP, uapi::IP_RECVERR) | (uapi::SOL_IPV6, uapi::IPV6_RECVERR) => {
                if let Some(error) = read::<ExtendedError>(data) {
                    let error = libc::sock_extended_err {
                        ee_errno: error.ee_errno,
                        ee_origin: error.ee_origin,
                        ee_type: error.ee_type,
                        ee_code: error.ee_code,
                        ee_pad: error.ee_pad,
                        ee_info: error.ee_info,
                        ee_data: error.ee_data,
                    };
                    self.error = Some((error, address(&data[size_of::<ExtendedError>()..])));
                }
            }
            (level, kind) if level == libc::SOL_UDP as u32 && kind == libc::UDP_GRO as u32 => {
                self.segment_size = integer().map(|value| value as u16);
            }
            _ => {}
        }
    }

    /// The fields of a received packet of the given length, refused with `EPROTO` when Linux named no arrival interface.
    pub fn packet_fields(
        &self,
        length: usize,
        drops_since_previous: u32,
    ) -> Result<ReceivedPacketFields, HostError> {
        let interface = self.interface().ok_or(HostError::Errno(libc::EPROTO))?;
        Ok(ReceivedPacketFields {
            length,
            interface,
            specific_destination: self.specific_destination,
            largest_fragment: self.largest_fragment.map(FragmentSize),
            link_layer_length: self.link.map(|(_, length)| FrameLength(length)),
            timestamps: ReceiveTimestamps {
                software: self.software,
                hardware: self.hardware,
            },
            mark: PacketMark(self.mark.unwrap_or(u32::MIN)),
            priority: PacketPriority(self.priority.unwrap_or(u32::MIN)),
            drops_since_previous: DropCount(drops_since_previous),
        })
    }

    /// The interface the packet information names, none when it is missing or names none.
    pub fn interface(&self) -> Option<InterfaceIndex> {
        self.interface.and_then(NonZeroU32::new).map(InterfaceIndex)
    }

    /// The transmit report of a timestamp entry of the error queue, with the length of the packet it holds.
    pub fn transmit_report(
        &self,
        error: &libc::sock_extended_err,
        packet: Option<usize>,
    ) -> Option<TransmitReport> {
        let stage = match error.ee_info {
            crate::TIMESTAMP_STAGE_SENT => TransmitStage::Sent,
            crate::TIMESTAMP_STAGE_SCHEDULED => TransmitStage::Scheduled,
            crate::TIMESTAMP_STAGE_ACKNOWLEDGED => TransmitStage::Acknowledged,
            crate::TIMESTAMP_STAGE_COMPLETED => TransmitStage::Completed,
            _ => return None,
        };
        let (source, time) = match (self.hardware, self.software) {
            (Some(time), _) => (TimestampSource::Hardware, time),
            (None, Some(time)) => (TimestampSource::Software, time),
            (None, None) => return None,
        };
        Some(TransmitReport {
            key: TransmitReportKey(error.ee_data),
            interface: self.interface(),
            event: TransmitEvent::Timestamp {
                stage,
                source,
                time,
            },
            packet_length: packet.map(ByteCount),
        })
    }
}

/// A `sockaddr_in` or `sockaddr_in6` at the start of a control message's data.
fn address(data: &[u8]) -> Option<SocketAddr> {
    address_of_storage(data)
}

/// The largest run of headers written before a payload.
///
/// It is IPv6, its extension headers, ICMPv6 and a quoted IPv6 and UDP header.
const HEADERS: usize =
    Ipv6Header::LEN * 2 + EXTENSION_HEADERS * Ipv6RawExtHeader::MAX_LEN + UDP_HEADER * 2;
const UDP_HEADER: usize = etherparse::UdpHeader::LEN;

/// The headers of a packet written in order, then put before a payload that arrived at the start of a buffer.
pub struct Headers {
    bytes: [u8; HEADERS],
    length: usize,
}

impl Headers {
    pub fn new() -> Self {
        Self {
            bytes: [u8::MIN; HEADERS],
            length: usize::MIN,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.length == usize::MIN
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.length]
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.bytes[self.length..self.length + bytes.len()].copy_from_slice(bytes);
        self.length += bytes.len();
    }

    /// Moves the payload of `copied` bytes at the start of `into` up behind the headers and writes them.
    ///
    /// The tail that no longer fits is cut; gives the bytes now in `into`.
    pub fn place(&self, into: &mut [u8], copied: usize) -> usize {
        let length = self.length;
        if length >= into.len() {
            let fit = into.len();
            into.copy_from_slice(&self.bytes[..fit]);
            return fit;
        }
        let kept = copied.min(into.len() - length);
        into.copy_within(..kept, length);
        into[..length].copy_from_slice(&self.bytes[..length]);
        length + kept
    }
}

/// An IPv4 header of the given fields; `None` when the lengths do not fit one.
pub fn ipv4_header(
    source: Ipv4Addr,
    destination: Ipv4Addr,
    protocol: IpNumber,
    payload: usize,
    ancillary: &Ancillary<'_>,
) -> Option<Ipv4Header> {
    let mut header = Ipv4Header::new(
        u16::MIN,
        ancillary.hop_limit.unwrap_or(u8::MIN),
        protocol,
        source.octets(),
        destination.octets(),
    )
    .ok()?;
    header.options = etherparse::Ipv4Options::try_from(ancillary.ipv4_options).ok()?;
    header.set_payload_len(payload).ok()?;
    let class = ancillary.traffic_class.unwrap_or(u8::MIN);
    header.dscp = IpDscp::try_new(class >> DSCP_SHIFT).ok()?;
    header.ecn = IpEcn::try_new(class & IpEcn::MAX_U8).ok()?;
    header.dont_fragment = false;
    header.header_checksum = header.calc_header_checksum();
    Some(header)
}

const DSCP_SHIFT: u8 = IpEcn::MAX_U8.count_ones() as u8;

/// An IPv6 header and, behind it, the extension headers Linux reported, chained to `protocol`.
pub fn push_ipv6_headers(
    headers: &mut Headers,
    source: Ipv6Addr,
    destination: Ipv6Addr,
    protocol: IpNumber,
    payload: usize,
    ancillary: &Ancillary<'_>,
) -> Option<()> {
    let extensions: Vec<(IpNumber, &[u8])> = ancillary
        .extension_headers
        .iter()
        .flatten()
        .copied()
        .collect();
    let extension_length: usize = extensions.iter().map(|(_, bytes)| bytes.len()).sum();
    let class = ancillary.traffic_class.unwrap_or(u8::MIN);
    let label = ancillary.flow_information.unwrap_or(u32::MIN) & Ipv6FlowLabel::MAX_U32;
    let header = Ipv6Header {
        traffic_class: class,
        flow_label: Ipv6FlowLabel::try_new(label).ok()?,
        payload_length: u16::try_from(extension_length + payload).ok()?,
        next_header: extensions.first().map_or(protocol, |(number, _)| *number),
        hop_limit: ancillary.hop_limit.unwrap_or(u8::MIN),
        source: source.octets(),
        destination: destination.octets(),
    };
    headers.push(&header.to_bytes());
    for (index, (_, bytes)) in extensions.iter().enumerate() {
        let next = extensions
            .get(index + 1)
            .map_or(protocol, |(number, _)| *number);
        let start = headers.len();
        headers.push(bytes);
        headers.bytes[start] = next.0;
    }
    Some(())
}

/// The UDP or UDP-Lite checksum of a datagram: pseudo-header, the header with a zero checksum, the payload.
pub fn udp_checksum(
    source: IpAddr,
    destination: IpAddr,
    protocol: IpNumber,
    length: usize,
    header: &[u8],
    payload: &[&[u8]],
) -> u16 {
    let mut sum = match (source, destination) {
        (IpAddr::V4(source), IpAddr::V4(destination)) => Sum16BitWords::new()
            .add_4bytes(source.octets())
            .add_4bytes(destination.octets()),
        (source, destination) => Sum16BitWords::new()
            .add_16bytes(mapped(source).octets())
            .add_16bytes(mapped(destination).octets()),
    };
    sum = sum
        .add_2bytes([u8::MIN, protocol.0])
        .add_4bytes((length as u32).to_be_bytes());
    sum = sum.add_slice(header);
    // A part of odd length leaves its last byte to pair with the first of the next.
    let mut carried = None;
    for part in payload {
        let mut part = *part;
        if let Some(first) = carried {
            let Some((&second, rest)) = part.split_first() else {
                continue;
            };
            sum = sum.add_2bytes([first, second]);
            part = rest;
        }
        let (words, last) = part.split_at(part.len() - part.len() % size_of::<u16>());
        sum = sum.add_slice(words);
        carried = last.first().copied();
    }
    if let Some(last) = carried {
        sum = sum.add_slice(&[last]);
    }
    sum.to_ones_complement_with_no_zero().to_be()
}

fn mapped(address: IpAddr) -> Ipv6Addr {
    match address {
        IpAddr::V4(v4) => v4.to_ipv6_mapped(),
        IpAddr::V6(v6) => v6,
    }
}

impl Default for Headers {
    fn default() -> Self {
        Self::new()
    }
}
