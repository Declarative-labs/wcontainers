//! ICMP errors of a socket's error queue written whole from their IP header.

use std::mem::size_of;
use std::net::{IpAddr, SocketAddr};

use etherparse::{
    Icmpv4Header, Icmpv4Type, Icmpv6Header, Icmpv6Type, IpNumber, Ipv6Header, icmpv4, icmpv6,
};

use crate::datagram::receive::{Ancillary, Headers, ipv4_header, push_ipv6_headers};

/// Writes the ICMP error of an error queue entry whole from its IP header, before the `copied` bytes Linux gave at the
/// start of `into`, which follow `quoted`, the part of the quoted transport header Linux took off. Returns its length.
#[allow(clippy::too_many_arguments)]
pub fn write_icmp_error(
    into: &mut [u8],
    copied: usize,
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: Option<SocketAddr>,
    local: IpAddr,
    destination: IpAddr,
    protocol: IpNumber,
    quoted: &[u8],
) -> Option<usize> {
    let offender = offender?.ip().to_canonical();
    let destination = destination.to_canonical();
    let us = ancillary.header_destination.unwrap_or(local).to_canonical();
    let quiet = Ancillary::default();
    let inner_payload = quoted.len() + copied;
    let rest = rest_of_header(error);
    let mut headers = Headers::new();
    let icmp_start;
    let icmp_length;
    let icmp_header;
    match (offender, us, destination) {
        (IpAddr::V4(offender), IpAddr::V4(us), IpAddr::V4(destination)) => {
            let inner = ipv4_header(us, destination, protocol, inner_payload, &quiet)?;
            icmp_length = Icmpv4Header::MIN_LEN + inner.header_len() + inner_payload;
            let outer = ipv4_header(offender, us, IpNumber::ICMP, icmp_length, ancillary)?;
            headers.push(&outer.to_bytes());
            icmp_start = headers.len();
            icmp_header = Icmpv4Header::MIN_LEN;
            headers.push(&[u8::MIN; Icmpv4Header::MIN_LEN]);
            headers.push(&inner.to_bytes());
        }
        (IpAddr::V6(offender), IpAddr::V6(us), IpAddr::V6(destination)) => {
            icmp_length = Icmpv6Header::MIN_LEN + Ipv6Header::LEN + inner_payload;
            push_ipv6_headers(
                &mut headers,
                offender,
                us,
                IpNumber::IPV6_ICMP,
                icmp_length,
                ancillary,
            )?;
            icmp_start = headers.len();
            icmp_header = Icmpv6Header::MIN_LEN;
            headers.push(&[u8::MIN; Icmpv6Header::MIN_LEN]);
            push_ipv6_headers(
                &mut headers,
                us,
                destination,
                protocol,
                inner_payload,
                &quiet,
            )?;
        }
        _ => return None,
    }
    headers.push(quoted);
    let placed = headers.place(into, copied);
    if placed >= headers.len() {
        write_icmp_header(
            into,
            icmp_start,
            icmp_header,
            placed,
            offender,
            us,
            rest,
            error,
        )?;
    }
    Some(icmp_start + icmp_length)
}

/// Writes the ICMP error of an error queue entry whose `copied` bytes at the start of `into` quote the packet
/// from its IP header on, as Linux gives them to a raw socket that writes its own headers. Returns its length.
pub fn write_icmp_error_quoting_header(
    into: &mut [u8],
    copied: usize,
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: Option<SocketAddr>,
    local: IpAddr,
) -> Option<usize> {
    let offender = offender?.ip().to_canonical();
    let us = ancillary.header_destination.unwrap_or(local).to_canonical();
    let rest = rest_of_header(error);
    let mut headers = Headers::new();
    let icmp_header;
    let icmp_length;
    match (offender, us) {
        (IpAddr::V4(offender), IpAddr::V4(us)) => {
            icmp_header = Icmpv4Header::MIN_LEN;
            icmp_length = icmp_header + copied;
            let outer = ipv4_header(offender, us, IpNumber::ICMP, icmp_length, ancillary)?;
            headers.push(&outer.to_bytes());
        }
        (IpAddr::V6(offender), IpAddr::V6(us)) => {
            icmp_header = Icmpv6Header::MIN_LEN;
            icmp_length = icmp_header + copied;
            push_ipv6_headers(
                &mut headers,
                offender,
                us,
                IpNumber::IPV6_ICMP,
                icmp_length,
                ancillary,
            )?;
        }
        _ => return None,
    }
    let icmp_start = headers.len();
    headers.push(&[u8::MIN; Icmpv6Header::MIN_LEN][..icmp_header]);
    let placed = headers.place(into, copied);
    if placed >= headers.len() {
        write_icmp_header(
            into,
            icmp_start,
            icmp_header,
            placed,
            offender,
            us,
            rest,
            error,
        )?;
    }
    Some(icmp_start + icmp_length)
}

/// Writes the ICMP header with its checksum over the body placed after it.
#[allow(clippy::too_many_arguments)]
fn write_icmp_header(
    into: &mut [u8],
    icmp_start: usize,
    icmp_header: usize,
    placed: usize,
    offender: IpAddr,
    us: IpAddr,
    rest: [u8; size_of::<u32>()],
    error: &libc::sock_extended_err,
) -> Option<()> {
    let body = &into[icmp_start + icmp_header..placed];
    let header = match (offender, us) {
        (IpAddr::V6(offender), IpAddr::V6(us)) => Icmpv6Header::with_checksum(
            Icmpv6Type::Unknown {
                type_u8: error.ee_type,
                code_u8: error.ee_code,
                bytes5to8: rest,
            },
            offender.octets(),
            us.octets(),
            body,
        )
        .ok()?
        .to_bytes()
        .to_vec(),
        _ => Icmpv4Header::with_checksum(
            Icmpv4Type::Unknown {
                type_u8: error.ee_type,
                code_u8: error.ee_code,
                bytes5to8: rest,
            },
            body,
        )
        .to_bytes()
        .to_vec(),
    };
    into[icmp_start..icmp_start + header.len()].copy_from_slice(&header);
    Some(())
}

/// The four bytes after an ICMP message's checksum: the MTU of a packet too big, the pointer of a parameter
/// problem, else unused.
fn rest_of_header(error: &libc::sock_extended_err) -> [u8; size_of::<u32>()] {
    let v4 = error.ee_origin == libc::SO_EE_ORIGIN_ICMP;
    let mtu = if v4 {
        error.ee_type == icmpv4::TYPE_DEST_UNREACH
            && error.ee_code == icmpv4::CODE_DST_UNREACH_NEED_FRAG
    } else {
        error.ee_type == icmpv6::TYPE_PACKET_TOO_BIG
    };
    let pointer = if v4 {
        error.ee_type == icmpv4::TYPE_PARAMETER_PROBLEM
    } else {
        error.ee_type == icmpv6::TYPE_PARAMETER_PROBLEM
    };
    if mtu || (pointer && !v4) {
        error.ee_info.to_be_bytes()
    } else if pointer {
        [error.ee_info as u8, u8::MIN, u8::MIN, u8::MIN]
    } else {
        [u8::MIN; size_of::<u32>()]
    }
}
