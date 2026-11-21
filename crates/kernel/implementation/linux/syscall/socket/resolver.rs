//! The container's name server: name queries sent to [`crate::network::NAME_SERVER`] are read,
//! asked of the system's resolver, and answered with its answer. It asks for addresses (A and
//! AAAA) of the Internet class; other questions get "not implemented".

use alloc::vec::Vec;

use domain::base::iana::{Class, Opcode, Rcode, Rtype};
use domain::base::name::ToName;
use domain::base::{Header, Message, MessageBuilder, Name, ParsedName, StaticCompressor};
use domain::rdata::AllRecordData;

/// A question the guest asked, owning its name.
pub(crate) type Asked = domain::base::Question<Name<Vec<u8>>>;

/// What a query needs.
pub(super) enum Question {
    /// An answer at once.
    Answer(Vec<u8>),
    /// "Not implemented", for a question of this type.
    Unanswerable(Vec<u8>, u16),
    /// The system's answer to this question.
    Ask(Asked),
    /// Nothing: not a query at all.
    Dropped,
}

/// A reply carrying only a header with a response code, for a message that could not be read.
fn header_only(query: Header, code: Rcode) -> Vec<u8> {
    let mut reply = MessageBuilder::new_vec();
    let header = reply.header_mut();
    header.set_id(query.id());
    header.set_qr(true);
    header.set_opcode(query.opcode());
    header.set_rd(query.rd());
    header.set_ra(true);
    header.set_rcode(code);
    reply.finish()
}

/// Whether every record of every section after the question can be read.
fn records_read(message: &Message<[u8]>) -> bool {
    let Ok(mut section) = message.answer() else {
        return false;
    };
    loop {
        if section.by_ref().any(|record| record.is_err()) {
            return false;
        }
        match section.next_section() {
            Ok(Some(next)) => section = next,
            Ok(None) => return true,
            Err(_) => return false,
        }
    }
}

pub(super) fn question(data: &[u8]) -> Question {
    let Ok(message) = Message::from_slice(data) else {
        return Question::Dropped;
    };
    let header = message.header();
    if header.qr() {
        return Question::Dropped;
    }
    let format_error = || Question::Answer(header_only(header, Rcode::FORMERR));
    let not_implemented =
        |kind: u16| Question::Unanswerable(header_only(header, Rcode::NOTIMP), kind);
    let Ok(questions) = message.question().collect::<Result<Vec<_>, _>>() else {
        return format_error();
    };
    if !records_read(message) {
        return format_error();
    }
    if header.opcode() != Opcode::QUERY || questions.len() != 1 {
        return not_implemented(0);
    }
    let asked = &questions[0];
    let address = matches!(asked.qtype(), Rtype::A | Rtype::AAAA);
    if !address || asked.qclass() != Class::IN {
        return not_implemented(asked.qtype().to_int());
    }
    let name: Name<Vec<u8>> = asked.qname().to_name();
    Question::Ask(Asked::new(name, asked.qtype(), asked.qclass()))
}

/// The guest's reply to a query [`question`] asked about: the system's answer with the query's id
/// and flags, or a server failure when the system gave none.
pub(super) fn answer(query: &[u8], answer: Option<&Message<[u8]>>) -> Option<Vec<u8>> {
    let asked = Message::from_slice(query).ok()?;
    let failure = || header_only(asked.header(), Rcode::SERVFAIL);
    let Some(answer) = answer else {
        return Some(failure());
    };
    let built = (|| {
        let builder = MessageBuilder::from_target(StaticCompressor::new(Vec::new())).ok()?;
        let mut reply = builder.start_answer(asked, answer.header().rcode()).ok()?;
        reply.header_mut().set_ra(true);
        let records = answer
            .answer()
            .ok()?
            .limit_to::<AllRecordData<_, ParsedName<_>>>();
        for record in records {
            reply.push(record.ok()?).ok()?;
        }
        Some(reply.finish().into_target())
    })();
    Some(built.unwrap_or_else(failure))
}
