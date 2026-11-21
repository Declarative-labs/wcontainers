//! What every host build shares: socket2 handles, the host's errors as Linux's, and the counted refusals.

use std::os::fd::BorrowedFd;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::io::Errno;
use socket2::SockRef;
use wcontainers_abi::{CounterReading, HostError};

/// A socket2 handle on a borrowed descriptor, through which the calls socket2 covers are made.
pub fn sock<'a>(socket: &'a BorrowedFd<'_>) -> SockRef<'a> {
    SockRef::from(socket)
}

/// The error of a call socket2 or the standard library made.
pub fn io_host_error(error: &std::io::Error) -> HostError {
    host_error(Errno::from_io_error(error).unwrap_or(Errno::IO))
}

/// A host's error as the contract gives it: a would-block, or Linux's number for it.
pub fn host_error(errno: Errno) -> HostError {
    if errno == Errno::AGAIN {
        HostError::WouldBlock
    } else {
        HostError::Errno(linux_number(errno))
    }
}

/// The error of the last libc call on this thread.
pub fn last_host_error() -> HostError {
    host_error(Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::IO))
}

#[cfg(target_os = "linux")]
fn linux_number(errno: Errno) -> i32 {
    errno.raw_os_error()
}

/// Linux's number for another system's error, whose numbers differ; an error Linux has no number for is EIO.
#[cfg(not(target_os = "linux"))]
fn linux_number(errno: Errno) -> i32 {
    use wcontainers_kernel::errno::Errno as Linux;
    const NUMBERS: &[(Errno, Linux)] = &[
        (Errno::PERM, Linux::EPERM),
        (Errno::NOENT, Linux::ENOENT),
        (Errno::INTR, Linux::EINTR),
        (Errno::IO, Linux::EIO),
        (Errno::BADF, Linux::EBADF),
        (Errno::NOMEM, Linux::ENOMEM),
        (Errno::ACCESS, Linux::EACCES),
        (Errno::FAULT, Linux::EFAULT),
        (Errno::BUSY, Linux::EBUSY),
        (Errno::INVAL, Linux::EINVAL),
        (Errno::NFILE, Linux::ENFILE),
        (Errno::MFILE, Linux::EMFILE),
        (Errno::NOTTY, Linux::ENOTTY),
        (Errno::PIPE, Linux::EPIPE),
        (Errno::NOSYS, Linux::ENOSYS),
        (Errno::NOTSOCK, Linux::ENOTSOCK),
        (Errno::DESTADDRREQ, Linux::EDESTADDRREQ),
        (Errno::MSGSIZE, Linux::EMSGSIZE),
        (Errno::PROTOTYPE, Linux::EPROTOTYPE),
        (Errno::NOPROTOOPT, Linux::ENOPROTOOPT),
        (Errno::PROTONOSUPPORT, Linux::EPROTONOSUPPORT),
        (Errno::NOTSUP, Linux::EOPNOTSUPP),
        (Errno::OPNOTSUPP, Linux::EOPNOTSUPP),
        (Errno::AFNOSUPPORT, Linux::EAFNOSUPPORT),
        (Errno::ADDRINUSE, Linux::EADDRINUSE),
        (Errno::ADDRNOTAVAIL, Linux::EADDRNOTAVAIL),
        (Errno::NETDOWN, Linux::ENETDOWN),
        (Errno::NETUNREACH, Linux::ENETUNREACH),
        (Errno::CONNABORTED, Linux::ECONNABORTED),
        (Errno::CONNRESET, Linux::ECONNRESET),
        (Errno::NOBUFS, Linux::ENOBUFS),
        (Errno::ISCONN, Linux::EISCONN),
        (Errno::NOTCONN, Linux::ENOTCONN),
        (Errno::TIMEDOUT, Linux::ETIMEDOUT),
        (Errno::CONNREFUSED, Linux::ECONNREFUSED),
        (Errno::HOSTDOWN, Linux::EHOSTDOWN),
        (Errno::HOSTUNREACH, Linux::EHOSTUNREACH),
        (Errno::ALREADY, Linux::EALREADY),
        (Errno::INPROGRESS, Linux::EINPROGRESS),
    ];
    let linux = NUMBERS
        .iter()
        .find(|(host, _)| *host == errno)
        .map_or(Linux::EIO, |&(_, linux)| linux);
    i32::from(linux as u16)
}

/// How many different reasons one value counts apart, more than any host implementation names.
const REASON_SLOTS: usize = 64;

/// Where the reasons past [`REASON_SLOTS`] are counted, in the last slot.
const MORE_REASONS: &str =
    "more reasons were counted than a host implementation's counters hold apart";

#[derive(Debug)]
struct Slot {
    reason: OnceLock<&'static str>,
    count: AtomicU64,
}

/// The requests a host implementation could not honour, counted by reason without a lock, each reason warned of at
/// its first count.
#[derive(Debug)]
pub struct Shortfalls {
    slots: [Slot; REASON_SLOTS],
}

impl Default for Shortfalls {
    fn default() -> Self {
        Self {
            slots: [const {
                Slot {
                    reason: OnceLock::new(),
                    count: AtomicU64::new(0),
                }
            }; REASON_SLOTS],
        }
    }
}

impl Shortfalls {
    pub fn count(&self, reason: &'static str) {
        let (last, named) = self.slots.split_last().expect("reason slots");
        let slot = named
            .iter()
            .find(|slot| {
                // A free slot is taken for the reason; one another thread took for it at once is the same.
                let held = *slot.reason.get_or_init(|| reason);
                core::ptr::eq(held, reason) || held == reason
            })
            .unwrap_or_else(|| {
                let _ = last.reason.set(MORE_REASONS);
                last
            });
        if slot.count.fetch_add(1, Ordering::Relaxed) == 0 {
            eprintln!("warning: {reason}");
        }
    }

    /// Counts the reason and answers that the host does not support the request.
    pub fn refuse(&self, reason: &'static str) -> HostError {
        self.count(reason);
        HostError::NotSupported
    }

    pub fn readings(&self) -> Vec<CounterReading> {
        let mut readings: Vec<CounterReading> = self
            .slots
            .iter()
            .filter_map(|slot| {
                Some(CounterReading {
                    reason: slot.reason.get()?,
                    detail: 0,
                    count: slot.count.load(Ordering::Relaxed),
                    nanoseconds: 0,
                })
            })
            .filter(|reading| reading.count > 0)
            .collect();
        readings.sort_by_key(|reading| reading.reason);
        readings
    }
}
