//! One `sendmsg`, and a socket's error queue read as the contract's received packets and transmit reports.

use std::collections::VecDeque;
use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::net::SocketAddr;
use std::os::fd::BorrowedFd;

use libc::c_int;
use linux_raw_sys::net as uapi;
use socket2::{MsgHdr, SockAddr};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    ByteCount, MaximumTransmissionUnit, Received, Report, SendError, TransmitEvent, TransmitReport,
    TransmitReportKey, TransmitReportRequest, TransmitTimeError,
};

use crate::datagram::receive::{self, Ancillary, ControlBuffer};
use crate::{Shortfalls, io_host_error, sock};

pub const TRANSMIT_TIME_REJECTION_WITHOUT_KEY: &str =
    "linux reports a rejected transmit time with the time, not the key of its send";
pub const ERROR_OF_NO_REPORT: &str =
    "an error queue entry of an origin the contract has no report for";
pub const NO_ARRIVAL_INTERFACE: &str =
    "an ICMP error came without the packet information that names its arrival interface";

/// The most an error queue entry is read of: a whole IP packet.
const LARGEST_ENTRY: usize = u16::MAX as usize;

/// One `sendmsg` of a payload to a name, or to the connected peer when there is none.
pub fn send_message(
    fd: BorrowedFd<'_>,
    name: Option<SocketAddr>,
    payload: &[u8],
    control: &[u8],
    flags: c_int,
) -> Result<(), HostError> {
    let payload = [IoSlice::new(payload)];
    let message = MsgHdr::new().with_buffers(&payload).with_control(control);
    let name = name.map(SockAddr::from);
    let message = match &name {
        Some(name) => message.with_addr(name),
        None => message,
    };
    sock(&fd)
        .sendmsg(&message, flags)
        .map(|_| ())
        .map_err(|error| io_host_error(&error))
}

/// One entry of the error queue: the bytes it holds, the address it names and its control messages.
pub struct ErrorEntry {
    pub data: Vec<u8>,
    pub source: Option<SocketAddr>,
    pub control: Vec<u8>,
}

impl ErrorEntry {
    /// Reads the next entry of the socket's error queue, `WouldBlock` when it is empty.
    pub fn read(fd: BorrowedFd<'_>, shortfalls: &Shortfalls) -> Result<Self, HostError> {
        let mut control = ControlBuffer::new();
        // Read on the stack, so that only the entry's own bytes are allocated, and nothing for an empty queue.
        let mut data = [const { MaybeUninit::<u8>::uninit() }; LARGEST_ENTRY];
        let message = receive::receive_with_tail(fd, &mut [], &mut data, &mut control, true)?;
        if message.control_truncated {
            shortfalls.count(receive::CONTROL_TRUNCATED);
        }
        let length = message.length.min(data.len());
        // SAFETY: the kernel wrote the first `length` bytes.
        let data = unsafe { core::slice::from_raw_parts(data.as_ptr().cast::<u8>(), length) };
        Ok(Self {
            data: data.to_vec(),
            source: message.source,
            control: message.control.to_vec(),
        })
    }
}

/// The entries of one socket's error queue that a send read while it looked for its own, kept for receives, and
/// the sends that asked for transmit time errors, which Linux reports by the time and not the key.
#[derive(Default)]
pub struct ErrorQueue {
    kept: VecDeque<ErrorEntry>,
    transmit_times: VecDeque<TransmitTimeSend>,
    /// Whether receives read the host's queue first, for reports a send asked for.
    first: bool,
}

/// A send that asked for transmit time errors: the time it gave, in nanoseconds of its clock, and its key.
struct TransmitTimeSend {
    at: u64,
    key: TransmitReportKey,
    packet: bool,
}

/// The transmit time of a rejection, which Linux splits between the error's data and information.
fn rejected_time(error: &libc::sock_extended_err) -> u64 {
    (u64::from(error.ee_data) << u32::BITS) | u64::from(error.ee_info)
}

impl ErrorQueue {
    /// Whether entries a send read wait here for receives.
    pub fn holds_entries(&self) -> bool {
        !self.kept.is_empty()
    }

    /// Has receives read the host's queue first, for the reports a send asked for, until the host holds no send.
    pub fn read_first(&mut self) {
        self.first = true;
    }

    /// Notes a send that asked for transmit time errors, so that a rejection of its time gives its key.
    pub fn expect_transmit_time_errors(&mut self, at: u64, key: TransmitReportKey, packet: bool) {
        self.transmit_times
            .push_back(TransmitTimeSend { at, key, packet });
    }

    /// Whether the host holds none of the socket's sends; if so, every report or rejection of one is on the error
    /// queue, which this reads into the entries kept, and the sends noted without a rejection there are forgotten.
    pub fn settle(
        &mut self,
        fd: BorrowedFd<'_>,
        shortfalls: &Shortfalls,
    ) -> Result<bool, HostError> {
        if unsent_bytes(fd)?.0 != usize::MIN {
            return Ok(false);
        }
        self.keep_host_entries(fd, shortfalls);
        let rejected: Vec<u64> = self
            .kept
            .iter()
            .filter_map(|entry| Ancillary::parse(&entry.control).error)
            .filter(|(error, _)| error.ee_origin == receive::SO_EE_ORIGIN_TXTIME)
            .map(|(error, _)| rejected_time(&error))
            .collect();
        self.transmit_times
            .retain(|send| rejected.contains(&send.at));
        Ok(true)
    }

    /// Reads every entry of the host's error queue into the entries kept, as a send must when it may have taken
    /// the socket error that tells a receive an ICMP error waits there.
    pub fn keep_host_entries(&mut self, fd: BorrowedFd<'_>, shortfalls: &Shortfalls) {
        while let Ok(entry) = ErrorEntry::read(fd, shortfalls) {
            self.kept.push_back(entry);
        }
    }

    /// What an entry gives, as `error_entry` says, with a transmit time rejection given the key of its send.
    pub fn received(
        &mut self,
        shortfalls: &Shortfalls,
        entry: &ErrorEntry,
        into: &mut [u8],
        write_icmp_error: impl FnOnce(
            &mut [u8],
            usize,
            &Ancillary<'_>,
            &libc::sock_extended_err,
            Option<SocketAddr>,
        ) -> Option<usize>,
    ) -> Option<Received> {
        let ancillary = Ancillary::parse(&entry.control);
        let Some((error, _)) = ancillary
            .error
            .filter(|(error, _)| error.ee_origin == receive::SO_EE_ORIGIN_TXTIME)
        else {
            return error_entry(shortfalls, entry, into, write_icmp_error);
        };
        let at = rejected_time(&error);
        let Some(send) = self
            .transmit_times
            .iter()
            .position(|send| send.at == at)
            .and_then(|position| self.transmit_times.remove(position))
        else {
            shortfalls.count(TRANSMIT_TIME_REJECTION_WITHOUT_KEY);
            return None;
        };
        let reason = match error.ee_code {
            receive::SO_EE_CODE_TXTIME_INVALID_PARAM => TransmitTimeError::InvalidParameter,
            receive::SO_EE_CODE_TXTIME_MISSED => TransmitTimeError::Missed,
            _ => {
                shortfalls.count(ERROR_OF_NO_REPORT);
                return None;
            }
        };
        let packet = send.packet.then(|| {
            let copied = entry.data.len().min(into.len());
            into[..copied].copy_from_slice(&entry.data[..copied]);
            ByteCount(entry.data.len())
        });
        // Linux puts the frame's length where a report's interface would be.
        Some(Received::TransmitReport(TransmitReport {
            key: send.key,
            interface: None,
            event: TransmitEvent::TransmitTimeRejected(reason),
            packet_length: packet,
        }))
    }

    /// The error of a send Linux refused as too long: the MTU of the local error it queued, else `EMSGSIZE`.
    pub fn message_too_long(&mut self, fd: BorrowedFd<'_>, shortfalls: &Shortfalls) -> SendError {
        loop {
            let Ok(entry) = ErrorEntry::read(fd, shortfalls) else {
                return SendError::Host(HostError::Errno(libc::EMSGSIZE));
            };
            match Ancillary::parse(&entry.control).error {
                Some((error, _))
                    if error.ee_origin == libc::SO_EE_ORIGIN_LOCAL
                        && error.ee_errno == libc::EMSGSIZE as u32 =>
                {
                    return SendError::MessageTooLong {
                        maximum_transmission_unit: MaximumTransmissionUnit(error.ee_info),
                    };
                }
                _ => self.kept.push_back(entry),
            }
        }
    }

    /// The next entry, a kept one first, `WouldBlock` when there is none.
    pub fn next(
        &mut self,
        fd: BorrowedFd<'_>,
        shortfalls: &Shortfalls,
    ) -> Result<ErrorEntry, HostError> {
        match self.kept.pop_front() {
            Some(entry) => Ok(entry),
            None => ErrorEntry::read(fd, shortfalls),
        }
    }
}

/// Whether a send asks for any transmit report, each of which Linux queues on the error queue.
pub fn reports_requested(reports: &TransmitReportRequest) -> bool {
    [
        reports.scheduled,
        reports.software,
        reports.hardware,
        reports.acknowledgement,
        reports.completion,
    ]
    .contains(&Report::Requested)
}

pub const PACKET_WISH_CHANGED: &str = "a send changed whether transmit reports hold their packet while the \
     host held earlier sends, whose reports linux makes by the new wish (it reads SOF_TIMESTAMPING_OPT_TSONLY when \
     it takes a timestamp)";

/// Counts a send that asks for reports with or without the packet unlike the socket's earlier sends, while the
/// host still holds some of them; `held` is whether the socket's reports hold the packet before this send.
pub fn note_packet_wish(
    fd: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
    held: bool,
    reports: &TransmitReportRequest,
) {
    if reports_requested(reports)
        && (reports.packet == Report::Requested) != held
        && unsent_bytes(fd).is_ok_and(|bytes| bytes.0 != usize::MIN)
    {
        shortfalls.count(PACKET_WISH_CHANGED);
    }
}

/// Receives in the order Linux keeps for a socket with `IP_RECVERR` or `IPV6_RECVERR`: the error queue first while
/// reports a send asked for may wait there or entries a send took off the host are kept, else the packet queue, and
/// the error queue when a receive is answered with the socket error that Linux sets for every ICMP error it queues
/// (and sets again when an entry read off the queue is followed by another ICMP error).
pub fn receive_in_order<S, T>(
    fd: BorrowedFd<'_>,
    state: &mut S,
    shortfalls: &Shortfalls,
    into: &mut [u8],
    errors: impl Fn(&mut S) -> &mut ErrorQueue,
    mut take_error_entry: impl FnMut(&mut S, &mut [u8]) -> Result<T, HostError>,
    mut receive_packet: impl FnMut(&mut S, &mut [u8]) -> Result<T, HostError>,
) -> Result<T, HostError> {
    let queue = errors(state);
    if queue.first || queue.holds_entries() {
        let settled = !queue.first || queue.settle(fd, shortfalls)?;
        match take_error_entry(state, into) {
            Err(HostError::WouldBlock) => errors(state).first = !settled,
            taken => return taken,
        }
    }
    match receive_packet(state, into) {
        Err(HostError::WouldBlock) => Err(HostError::WouldBlock),
        Err(error) => match take_error_entry(state, into) {
            Err(HostError::WouldBlock) => Err(error),
            taken => taken,
        },
        received => received,
    }
}

/// What an error queue entry gives: an ICMP error that `write_icmp_error` writes whole into `into` from its
/// ancillary data, error and offender, returning its length; a transmit report; or nothing, for an entry the
/// send that made it answered or one the contract has no report for.
pub fn error_entry(
    shortfalls: &Shortfalls,
    entry: &ErrorEntry,
    into: &mut [u8],
    write_icmp_error: impl FnOnce(
        &mut [u8],
        usize,
        &Ancillary<'_>,
        &libc::sock_extended_err,
        Option<SocketAddr>,
    ) -> Option<usize>,
) -> Option<Received> {
    let ancillary = Ancillary::parse(&entry.control);
    let Some((error, offender)) = ancillary.error else {
        shortfalls.count(ERROR_OF_NO_REPORT);
        return None;
    };
    let copied = entry.data.len().min(into.len());
    into[..copied].copy_from_slice(&entry.data[..copied]);
    match error.ee_origin {
        libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 => {
            let length = write_icmp_error(into, copied, &ancillary, &error, offender);
            match length.map(|length| ancillary.packet_fields(length, u32::MIN)) {
                None => {
                    shortfalls.count(ERROR_OF_NO_REPORT);
                    None
                }
                Some(Err(_)) => {
                    shortfalls.count(NO_ARRIVAL_INTERFACE);
                    None
                }
                Some(Ok(fields)) => Some(Received::Packet(fields)),
            }
        }
        libc::SO_EE_ORIGIN_TIMESTAMPING => {
            let packet = (!entry.data.is_empty()).then_some(entry.data.len());
            let report = ancillary.transmit_report(&error, packet);
            if report.is_none() {
                shortfalls.count(ERROR_OF_NO_REPORT);
            }
            report.map(Received::TransmitReport)
        }
        // The send that made a local error answered it.
        libc::SO_EE_ORIGIN_LOCAL => None,
        receive::SO_EE_ORIGIN_TXTIME => {
            shortfalls.count(TRANSMIT_TIME_REJECTION_WITHOUT_KEY);
            None
        }
        _ => {
            shortfalls.count(ERROR_OF_NO_REPORT);
            None
        }
    }
}

/// One count of the socket's memory information, `SO_MEMINFO`, by its `SK_MEMINFO_*` index.
fn memory_information(fd: BorrowedFd<'_>, count: c_int) -> Result<u32, HostError> {
    const COUNTS: usize = libc::SK_MEMINFO_DROPS as usize + 1;
    let mut words = [u32::MIN; COUNTS];
    // SAFETY: the words are plain data, written by the kernel as their bytes.
    let bytes = unsafe {
        core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), size_of_val(&words))
    };
    crate::get_bytes(fd, uapi::SOL_SOCKET, uapi::SO_MEMINFO, bytes)?;
    Ok(words[count as usize])
}

/// The bytes the socket's sends hold in the host until they leave, which is what `SIOCOUTQ` gives a UDP socket.
pub fn unsent_bytes(fd: BorrowedFd<'_>) -> Result<ByteCount, HostError> {
    memory_information(fd, libc::SK_MEMINFO_WMEM_ALLOC).map(|bytes| ByteCount(bytes as usize))
}

/// The packets the socket has dropped since it was made, for a socket Linux gives no `SO_RXQ_OVFL` count.
pub fn dropped_packets(fd: BorrowedFd<'_>) -> Result<u32, HostError> {
    memory_information(fd, libc::SK_MEMINFO_DROPS)
}
