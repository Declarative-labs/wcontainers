//! The local system's resolver: a name is looked up by the host's resolver on a thread of its own,
//! and the answer is a DNS message made from the addresses found.

use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Poll, Waker};

use domain::base::iana::{Class, Rcode, Rtype};
use domain::base::name::ToName;
use domain::base::{Message, MessageBuilder, Name, Question, Ttl};
use domain::rdata::{A, Aaaa};
use wcontainers_abi::HostError;
use wcontainers_kernel::errno::Errno;

use crate::arrivals::{Arrival, Arrivals};

/// The name of the threads that look names up.
const LOOKUP_THREAD: &str = "wcontainers lookup";

/// A resolver's answer, the message's bytes.
pub struct NameAnswer(Message<Vec<u8>>);

impl NameAnswer {
    pub fn new(bytes: Vec<u8>) -> Result<Self, HostError> {
        Message::from_octets(bytes)
            .map(Self)
            .map_err(|_| HostError::Errno(Errno::EBADMSG as i32))
    }
}

impl std::ops::Deref for NameAnswer {
    type Target = Message<[u8]>;

    fn deref(&self) -> &Message<[u8]> {
        self.0.for_slice()
    }
}

/// The answer to an address question from the addresses found for its name, of the kind asked
/// (IPv4 ones for A, others for AAAA), each to be kept no time; "no such name" when none was found.
pub fn address_answer(question: Question<&Name<[u8]>>, found: Option<&[IpAddr]>) -> Vec<u8> {
    let rcode = if found.is_some() {
        Rcode::NOERROR
    } else {
        Rcode::NXDOMAIN
    };
    let mut builder = MessageBuilder::new_vec();
    builder.header_mut().set_qr(true);
    builder.header_mut().set_rcode(rcode);
    let mut questions = builder.question();
    questions.push(question).expect("a question fits");
    let mut answers = questions.answer();
    let name = *question.qname();
    for address in found.unwrap_or_default() {
        let v4 = match address {
            IpAddr::V4(v4) => Some(*v4),
            IpAddr::V6(v6) => v6.to_ipv4_mapped(),
        };
        let pushed = match (question.qtype(), v4, address) {
            (Rtype::A, Some(v4), _) => answers.push((name, Class::IN, Ttl::ZERO, A::new(v4))),
            (Rtype::AAAA, None, IpAddr::V6(v6)) => {
                answers.push((name, Class::IN, Ttl::ZERO, Aaaa::new(*v6)))
            }
            _ => Ok(()),
        };
        pushed.expect("an address fits");
    }
    answers.finish()
}

/// The answer of one lookup, once its thread has it, and the waker of the call waiting for it.
#[derive(Default)]
struct Slot {
    answer: Option<Result<Vec<u8>, HostError>>,
    waker: Option<Waker>,
}

/// Looks the question's name up on a thread of its own; the arrival it pushes when done wakes an
/// idle system, which then hands the kernel the waiting call's answer.
pub(crate) async fn resolve(
    arrivals: Arc<Arrivals>,
    question: Question<&Name<[u8]>>,
) -> Result<NameAnswer, HostError> {
    let slot = Arc::new(Mutex::new(Slot::default()));
    let name = question.qname().to_string();
    let (qname, qtype, qclass) = (
        question.qname().to_name::<Vec<u8>>(),
        question.qtype(),
        question.qclass(),
    );
    let filled = slot.clone();
    let started = std::thread::Builder::new()
        .name(LOOKUP_THREAD.to_string())
        .spawn(move || {
            let found: Option<Vec<IpAddr>> = (name.as_str(), 0)
                .to_socket_addrs()
                .ok()
                .map(|found| found.map(|address| address.ip()).collect());
            let question = Question::new(qname.for_slice(), qtype, qclass);
            let answer = address_answer(question, found.as_deref());
            let waker = {
                let mut slot = filled.lock().unwrap_or_else(PoisonError::into_inner);
                slot.answer = Some(Ok(answer));
                slot.waker.take()
            };
            if let Some(waker) = waker {
                waker.wake();
            }
            arrivals.push(Arrival::Answered);
        });
    if started.is_err() {
        return Err(HostError::Errno(Errno::EAGAIN as i32));
    }
    let bytes = std::future::poll_fn(|context| {
        let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
        match slot.answer.take() {
            Some(answer) => Poll::Ready(answer),
            None => {
                slot.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    })
    .await?;
    NameAnswer::new(bytes)
}
