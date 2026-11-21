//! ICMP errors of a socket's error queue written whole from their IP header.

use std::mem::size_of;
use std::net::{IpAddr, SocketAddr};

use etherparse::checksum::Sum16BitWords;
use etherparse::{Icmpv4Header, Icmpv6Header, IpNumber, icmpv4, icmpv6};

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
    let (headers, length) = payload_error(
        &into[..copied],
        ancillary,
        error,
        offender,
        local,
        destination,
        protocol,
        quoted,
    )?;
    headers.place(into, copied);
    Some(length)
}

/// Writes the ICMP error as `write_icmp_error` does, of all the `data` Linux gave, which `into` may cut.
#[allow(clippy::too_many_arguments)]
pub fn write_whole_icmp_error(
    into: &mut [u8],
    data: &[u8],
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: Option<SocketAddr>,
    local: IpAddr,
    destination: IpAddr,
    protocol: IpNumber,
    quoted: &[u8],
) -> Option<usize> {
    let error = payload_error(
        data,
        ancillary,
        error,
        offender,
        local,
        destination,
        protocol,
        quoted,
    )?;
    Some(place_whole(into, data, error))
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
    let (headers, length) = header_error(&into[..copied], ancillary, error, offender, local)?;
    headers.place(into, copied);
    Some(length)
}

/// Writes the ICMP error as `write_icmp_error_quoting_header` does, of all the `data` Linux gave, which `into`
/// may cut.
pub fn write_whole_icmp_error_quoting_header(
    into: &mut [u8],
    data: &[u8],
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: Option<SocketAddr>,
    local: IpAddr,
) -> Option<usize> {
    let error = header_error(data, ancillary, error, offender, local)?;
    Some(place_whole(into, data, error))
}

/// Puts the headers and then as much of `data` as fits into `into`, giving the error's whole length.
fn place_whole(into: &mut [u8], data: &[u8], (headers, length): (Headers, usize)) -> usize {
    let fit = data.len().min(into.len());
    into[..fit].copy_from_slice(&data[..fit]);
    headers.place(into, fit);
    length
}

/// The headers before `data` of an ICMP error about a payload, from its outer IP header to `quoted`, and the
/// error's whole length.
#[allow(clippy::too_many_arguments)]
fn payload_error(
    data: &[u8],
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: Option<SocketAddr>,
    local: IpAddr,
    destination: IpAddr,
    protocol: IpNumber,
    quoted: &[u8],
) -> Option<(Headers, usize)> {
    let us = ancillary.header_destination.unwrap_or(local).to_canonical();
    let quiet = Ancillary::default();
    let inner_payload = quoted.len() + data.len();
    let mut inner = Headers::new();
    match (us, destination.to_canonical()) {
        (IpAddr::V4(us), IpAddr::V4(destination)) => {
            inner.push(&ipv4_header(us, destination, protocol, inner_payload, &quiet)?.to_bytes())
        }
        (IpAddr::V6(us), IpAddr::V6(destination)) => {
            push_ipv6_headers(&mut inner, us, destination, protocol, inner_payload, &quiet)?;
        }
        _ => return None,
    }
    inner.push(quoted);
    error_headers(ancillary, error, offender?, us, inner.as_slice(), data)
}

/// The headers before `data` of an ICMP error that quotes a packet from its IP header on, and the error's whole
/// length.
fn header_error(
    data: &[u8],
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: Option<SocketAddr>,
    local: IpAddr,
) -> Option<(Headers, usize)> {
    let us = ancillary.header_destination.unwrap_or(local).to_canonical();
    error_headers(ancillary, error, offender?, us, &[], data)
}

/// The outer IP header, the ICMP header with its checksum over `quoted` and `data`, then `quoted`, of an error from
/// `offender` to `us`; and the error's whole length. `quoted` is whole headers, of an even length.
fn error_headers(
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: SocketAddr,
    us: IpAddr,
    quoted: &[u8],
    data: &[u8],
) -> Option<(Headers, usize)> {
    let mut headers = Headers::new();
    let mut words = Sum16BitWords::new()
        .add_2bytes([error.ee_type, error.ee_code])
        .add_4bytes(rest_of_header(error));
    let icmp_length;
    let sum = match (offender.ip().to_canonical(), us) {
        (IpAddr::V4(offender), IpAddr::V4(us)) => {
            icmp_length = Icmpv4Header::MIN_LEN + quoted.len() + data.len();
            let outer = ipv4_header(offender, us, IpNumber::ICMP, icmp_length, ancillary)?;
            headers.push(&outer.to_bytes());
            words
        }
        (IpAddr::V6(offender), IpAddr::V6(us)) => {
            icmp_length = Icmpv6Header::MIN_LEN + quoted.len() + data.len();
            push_ipv6_headers(
                &mut headers,
                offender,
                us,
                IpNumber::IPV6_ICMP,
                icmp_length,
                ancillary,
            )?;
            words
                .add_16bytes(offender.octets())
                .add_16bytes(us.octets())
                .add_4bytes(u32::try_from(icmp_length).ok()?.to_be_bytes())
                .add_2bytes([u8::MIN, IpNumber::IPV6_ICMP.0])
        }
        _ => return None,
    };
    let checksum = sum.add_slice(quoted).add_slice(data).ones_complement();
    let length = headers.len() + icmp_length;
    headers.push(&[error.ee_type, error.ee_code]);
    headers.push(&checksum.to_ne_bytes());
    headers.push(&rest_of_header(error));
    headers.push(quoted);
    Some((headers, length))
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
