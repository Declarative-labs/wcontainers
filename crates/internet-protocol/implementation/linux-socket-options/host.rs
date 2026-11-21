//! What every host build shares: socket2 handles, the host's errors as Linux's, and the counted refusals.

use std::collections::BTreeMap;
use std::os::fd::BorrowedFd;
use std::sync::Mutex;

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

/// The requests a host implementation could not honour, counted by reason, each reason warned of at its first count.
#[derive(Debug, Default)]
pub struct Shortfalls {
    counts: Mutex<BTreeMap<&'static str, u64>>,
}

impl Shortfalls {
    pub fn count(&self, reason: &'static str) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = counts.entry(reason).or_default();
        *count += 1;
        if *count == 1 {
            eprintln!("warning: {reason}");
        }
    }

    /// Counts the reason and answers that the host does not support the request.
    pub fn refuse(&self, reason: &'static str) -> HostError {
        self.count(reason);
        HostError::NotSupported
    }

    pub fn readings(&self) -> Vec<CounterReading> {
        let counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        counts
            .iter()
            .map(|(&reason, &count)| CounterReading {
                reason,
                detail: 0,
                count,
                nanoseconds: 0,
            })
            .collect()
    }
}
