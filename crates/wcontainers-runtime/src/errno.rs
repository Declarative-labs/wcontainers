//! The Linux error number for an error of the host the local system runs on, whose numbers may differ
//! (macOS's do).

use rustix::io::Errno as Host;
use wcontainers_kernel::linux::errno as linux;

/// The host's errors and Linux's numbers for them.
const NUMBERS: &[(Host, u32)] = &[
    (Host::PERM, linux::EPERM),
    (Host::NOENT, linux::ENOENT),
    (Host::SRCH, linux::ESRCH),
    (Host::INTR, linux::EINTR),
    (Host::IO, linux::EIO),
    (Host::NXIO, linux::ENXIO),
    (Host::TOOBIG, linux::E2BIG),
    (Host::BADF, linux::EBADF),
    (Host::AGAIN, linux::EAGAIN),
    (Host::NOMEM, linux::ENOMEM),
    (Host::ACCESS, linux::EACCES),
    (Host::FAULT, linux::EFAULT),
    (Host::BUSY, linux::EBUSY),
    (Host::EXIST, linux::EEXIST),
    (Host::XDEV, linux::EXDEV),
    (Host::NODEV, linux::ENODEV),
    (Host::NOTDIR, linux::ENOTDIR),
    (Host::ISDIR, linux::EISDIR),
    (Host::INVAL, linux::EINVAL),
    (Host::NFILE, linux::ENFILE),
    (Host::MFILE, linux::EMFILE),
    (Host::NOTTY, linux::ENOTTY),
    (Host::TXTBSY, linux::ETXTBSY),
    (Host::FBIG, linux::EFBIG),
    (Host::NOSPC, linux::ENOSPC),
    (Host::SPIPE, linux::ESPIPE),
    (Host::ROFS, linux::EROFS),
    (Host::MLINK, linux::EMLINK),
    (Host::PIPE, linux::EPIPE),
    (Host::RANGE, linux::ERANGE),
    (Host::NAMETOOLONG, linux::ENAMETOOLONG),
    (Host::NOSYS, linux::ENOSYS),
    (Host::NOTEMPTY, linux::ENOTEMPTY),
    (Host::LOOP, linux::ELOOP),
    (Host::NOTSUP, linux::EOPNOTSUPP),
    (Host::OPNOTSUPP, linux::EOPNOTSUPP),
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    (Host::NOATTR, linux::ENODATA),
    #[cfg(target_os = "linux")]
    (Host::NODATA, linux::ENODATA),
    (Host::DQUOT, linux::EDQUOT),
    (Host::STALE, linux::ESTALE),
    (Host::OVERFLOW, linux::EOVERFLOW),
    (Host::CANCELED, linux::ECANCELED),
    (Host::ILSEQ, linux::EILSEQ),
    (Host::NOLCK, linux::ENOLCK),
    (Host::DEADLK, linux::EDEADLK),
    (Host::TIMEDOUT, linux::ETIMEDOUT),
    (Host::CONNREFUSED, linux::ECONNREFUSED),
    (Host::CONNRESET, linux::ECONNRESET),
    (Host::CONNABORTED, linux::ECONNABORTED),
    (Host::NETUNREACH, linux::ENETUNREACH),
    (Host::NETDOWN, linux::ENETDOWN),
    (Host::HOSTUNREACH, linux::EHOSTUNREACH),
    (Host::ADDRINUSE, linux::EADDRINUSE),
    (Host::ADDRNOTAVAIL, linux::EADDRNOTAVAIL),
    (Host::AFNOSUPPORT, linux::EAFNOSUPPORT),
    (Host::NOTCONN, linux::ENOTCONN),
    (Host::MSGSIZE, linux::EMSGSIZE),
    (Host::NOBUFS, linux::ENOBUFS),
];

/// Linux's number for a host error; none for one Linux has no number for.
pub(crate) fn linux(error: Host) -> Option<u32> {
    NUMBERS
        .iter()
        .find(|(host, _)| *host == error)
        .map(|&(_, number)| number)
}

/// Linux's number for a host error from the standard library; none for one Linux has no number
/// for, or one the host gave no number for.
pub(crate) fn of_io(error: &std::io::Error) -> Option<u32> {
    error
        .raw_os_error()
        .and_then(|raw| linux(Host::from_raw_os_error(raw)))
}
