//! One `sendmsg`, and a socket's error queue read as the contract's received packets and transmit reports.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::os::fd::{AsRawFd, BorrowedFd};

use libc::c_int;
use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    ByteCount, InterfaceIndex, MaximumTransmissionUnit, Received, SendError, TransmitEvent,
    TransmitReport, TransmitReportKey, TransmitTimeError,
};

use crate::Shortfalls;
use crate::datagram::receive::{self, Ancillary, ControlBuffer};

pub const TRANSMIT_TIME_REJECTION_WITHOUT_KEY: &str =
    "linux reports a rejected transmit time with the time, not the key of its send";
pub const ERROR_OF_NO_REPORT: &str =
    "an error queue entry of an origin the contract has no report for";

/// One `sendmsg` of a payload to a name, or to the connected peer when there is none.
pub fn send_message(
    fd: BorrowedFd<'_>,
    name: Option<SocketAddr>,
    payload: &[u8],
    control: &[u8],
    flags: c_int,
) -> Result<(), HostError> {
    let send = |name: *const libc::c_void, length: libc::socklen_t| {
        let mut vector = libc::iovec {
            iov_base: payload.as_ptr() as *mut _,
            iov_len: payload.len(),
        };
        let mut header = crate::datagram::addresses::zeroed::<libc::msghdr>();
        header.msg_name = name as *mut _;
        header.msg_namelen = length;
        header.msg_iov = &mut vector;
        header.msg_iovlen = 1;
        if !control.is_empty() {
            header.msg_control = control.as_ptr() as *mut _;
            header.msg_controllen = control.len() as _;
        }
        // SAFETY: every pointer in the header describes a buffer that outlives the call.
        unsafe { libc::sendmsg(fd.as_raw_fd(), &header, flags) }
    };
    let sent = match name {
        // SAFETY: rustix gives a pointer to `length` bytes of the socket address for the duration of the call.
        Some(name) => unsafe {
            rustix::net::addr::SocketAddrArg::with_sockaddr(&name, |pointer, length| {
                send(pointer.cast(), length)
            })
        },
        None => send(core::ptr::null(), libc::socklen_t::MIN),
    };
    if sent < 0 {
        Err(crate::last_host_error())
    } else {
        Ok(())
    }
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
        let mut data = vec![u8::MIN; usize::from(u16::MAX)];
        let message = receive::receive(fd, &mut data, &mut control, true)?;
        if message.control_truncated {
            shortfalls.count(receive::CONTROL_TRUNCATED);
        }
        data.truncate(message.length);
        Ok(Self {
            data,
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
        while let Ok(entry) = ErrorEntry::read(fd, shortfalls) {
            self.kept.push_back(entry);
        }
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
        Some(Received::TransmitReport(TransmitReport {
            key: send.key,
            interface: InterfaceIndex(ancillary.interface.unwrap_or(u32::MIN)),
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
            if length.is_none() {
                shortfalls.count(ERROR_OF_NO_REPORT);
            }
            length.map(|length| Received::Packet(ancillary.packet_fields(length, u32::MIN)))
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
