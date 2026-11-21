//! The container's name server: name queries sent to [`crate::network::NAME_SERVER`] are read and
//! answered with `simple-dns`, from the host's lookup hook. It answers addresses (A and AAAA) of
//! the Internet class; other questions get "not implemented".

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use simple_dns::rdata::{A, AAAA, RData};
use simple_dns::{
    CLASS, Name, OPCODE, Packet, PacketFlag, QCLASS, QTYPE, RCODE, ResourceRecord, TYPE,
};

use crate::Errno;

/// What a query needs.
pub(super) enum Question {
    /// An answer at once.
    Answer(Vec<u8>),
    /// "Not implemented", for a question of this type.
    Unanswerable(Vec<u8>, u16),
    /// The host's lookup of this name.
    LookUp(String),
    /// Nothing: not a query at all.
    Dropped,
}

/// The size of a message's header.
const HEADER: usize = 12;

/// A reply carrying only a header with a response code, for a message that could not be read.
fn header_only(query: &[u8], code: u8) -> Vec<u8> {
    let mut reply = query[..HEADER].to_vec();
    // Response, with the query's opcode and recursion wish, recursion available.
    reply[2] = 0x80 | (query[2] & 0x79);
    reply[3] = 0x80 | code;
    reply[4..HEADER].fill(0);
    reply
}

pub(super) fn question(data: &[u8]) -> Question {
    if data.len() < HEADER || data[2] & 0x80 != 0 {
        return Question::Dropped;
    }
    let Ok(packet) = Packet::parse(data) else {
        return Question::Answer(header_only(data, RCODE::FormatError as u8));
    };
    let not_implemented =
        |kind: u16| Question::Unanswerable(header_only(data, RCODE::NotImplemented as u8), kind);
    if packet.opcode() != OPCODE::StandardQuery || packet.questions.len() != 1 {
        return not_implemented(0);
    }
    let asked = &packet.questions[0];
    let address = matches!(asked.qtype, QTYPE::TYPE(TYPE::A) | QTYPE::TYPE(TYPE::AAAA));
    if !address || asked.qclass != QCLASS::CLASS(CLASS::IN) {
        return not_implemented(u16::from(asked.qtype));
    }
    Question::LookUp(asked.qname.to_string())
}

/// The answer to an address query from the host's lookup: the addresses of the kind asked for
/// (each 16 bytes, IPv4 ones mapped), "no such name" for ENOENT, or a server failure.
pub(super) fn answer(query: &[u8], addresses: &[u8], error: i32) -> Vec<u8> {
    let Ok(packet) = Packet::parse(query) else {
        return header_only(query, RCODE::ServerFailure as u8);
    };
    let mut reply = Packet::new_reply(packet.id());
    let mut flags = PacketFlag::RECURSION_AVAILABLE;
    if packet.has_flags(PacketFlag::RECURSION_DESIRED) {
        flags |= PacketFlag::RECURSION_DESIRED;
    }
    reply.set_flags(flags);
    let asked = packet.questions[0].clone();
    let name: Name<'_> = asked.qname.clone();
    let wants_v4 = asked.qtype == QTYPE::TYPE(TYPE::A);
    *reply.rcode_mut() = match error {
        0 => RCODE::NoError,
        _ if error as u32 == Errno::ENOENT.0 => RCODE::NameError,
        _ => RCODE::ServerFailure,
    };
    reply.questions.push(asked);
    if error == 0 {
        for chunk in addresses.as_chunks::<16>().0 {
            let v4 = chunk[..12] == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];
            let data = match (wants_v4, v4) {
                (true, true) => RData::A(A {
                    address: u32::from_be_bytes(chunk[12..].try_into().expect("4 bytes")),
                }),
                (false, false) => RData::AAAA(AAAA {
                    address: u128::from_be_bytes(*chunk),
                }),
                _ => continue,
            };
            // How long the host's answer holds is not known, so it is not to be kept.
            reply
                .answers
                .push(ResourceRecord::new(name.clone(), CLASS::IN, 0, data));
        }
    }
    reply
        .build_bytes_vec_compressed()
        .unwrap_or_else(|_| header_only(query, RCODE::ServerFailure as u8))
}
