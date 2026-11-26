//! The Linux error number for an error of the host the local system runs on, whose numbers may differ (macOS's do).

use rustix::io::Errno as Host;
use wcontainers_kernel::errno::Errno;

/// The host's errors and Linux's numbers for them.
const NUMBERS: &[(Host, u32)] = &[
    (Host::PERM, Errno::EPERM as u32),
    (Host::NOENT, Errno::ENOENT as u32),
    (Host::SRCH, Errno::ESRCH as u32),
    (Host::INTR, Errno::EINTR as u32),
    (Host::IO, Errno::EIO as u32),
    (Host::NXIO, Errno::ENXIO as u32),
    (Host::TOOBIG, Errno::E2BIG as u32),
    (Host::BADF, Errno::EBADF as u32),
    (Host::AGAIN, Errno::EAGAIN as u32),
    (Host::NOMEM, Errno::ENOMEM as u32),
    (Host::ACCESS, Errno::EACCES as u32),
    (Host::FAULT, Errno::EFAULT as u32),
    (Host::BUSY, Errno::EBUSY as u32),
    (Host::EXIST, Errno::EEXIST as u32),
    (Host::XDEV, Errno::EXDEV as u32),
    (Host::NODEV, Errno::ENODEV as u32),
    (Host::NOTDIR, Errno::ENOTDIR as u32),
    (Host::ISDIR, Errno::EISDIR as u32),
    (Host::INVAL, Errno::EINVAL as u32),
    (Host::NFILE, Errno::ENFILE as u32),
    (Host::MFILE, Errno::EMFILE as u32),
    (Host::NOTTY, Errno::ENOTTY as u32),
    (Host::TXTBSY, Errno::ETXTBSY as u32),
    (Host::FBIG, Errno::EFBIG as u32),
    (Host::NOSPC, Errno::ENOSPC as u32),
    (Host::SPIPE, Errno::ESPIPE as u32),
    (Host::ROFS, Errno::EROFS as u32),
    (Host::MLINK, Errno::EMLINK as u32),
    (Host::PIPE, Errno::EPIPE as u32),
    (Host::RANGE, Errno::ERANGE as u32),
    (Host::NAMETOOLONG, Errno::ENAMETOOLONG as u32),
    (Host::NOSYS, Errno::ENOSYS as u32),
    (Host::NOTEMPTY, Errno::ENOTEMPTY as u32),
    (Host::LOOP, Errno::ELOOP as u32),
    (Host::NOTSUP, Errno::EOPNOTSUPP as u32),
    (Host::OPNOTSUPP, Errno::EOPNOTSUPP as u32),
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    (Host::NOATTR, Errno::ENODATA as u32),
    #[cfg(target_os = "linux")]
    (Host::NODATA, Errno::ENODATA as u32),
    (Host::DQUOT, Errno::EDQUOT as u32),
    (Host::STALE, Errno::ESTALE as u32),
    (Host::OVERFLOW, Errno::EOVERFLOW as u32),
    (Host::CANCELED, Errno::ECANCELED as u32),
    (Host::ILSEQ, Errno::EILSEQ as u32),
    (Host::NOLCK, Errno::ENOLCK as u32),
    (Host::DEADLK, Errno::EDEADLK as u32),
    (Host::TIMEDOUT, Errno::ETIMEDOUT as u32),
    (Host::CONNREFUSED, Errno::ECONNREFUSED as u32),
    (Host::CONNRESET, Errno::ECONNRESET as u32),
    (Host::CONNABORTED, Errno::ECONNABORTED as u32),
    (Host::NETUNREACH, Errno::ENETUNREACH as u32),
    (Host::NETDOWN, Errno::ENETDOWN as u32),
    (Host::HOSTUNREACH, Errno::EHOSTUNREACH as u32),
    (Host::ADDRINUSE, Errno::EADDRINUSE as u32),
    (Host::ADDRNOTAVAIL, Errno::EADDRNOTAVAIL as u32),
    (Host::AFNOSUPPORT, Errno::EAFNOSUPPORT as u32),
    (Host::NOTCONN, Errno::ENOTCONN as u32),
    (Host::MSGSIZE, Errno::EMSGSIZE as u32),
    (Host::NOBUFS, Errno::ENOBUFS as u32),
];

/// Returns Linux's number for a host error; none for one Linux has no number for.
pub(crate) fn linux(error: Host) -> Option<u32> {
    NUMBERS
        .iter()
        .find(|(host, _)| *host == error)
        .map(|&(_, number)| number)
}

/// Returns Linux's number for a standard library error; none for one Linux or the host has no number for.
pub(crate) fn of_io(error: &std::io::Error) -> Option<u32> {
    error
        .raw_os_error()
        .and_then(|raw| linux(Host::from_raw_os_error(raw)))
}
