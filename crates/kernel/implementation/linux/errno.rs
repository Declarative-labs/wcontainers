//! Linux error numbers as the kernel carries them inside: any number a call or the backend gives,
//! and the kernel's own marker of a parked call.

use wcontainers_abi::CpuError;
use wcontainers_kernel::errno::Errno as Linux;

/// A Linux error number, such as `ENOENT`, or [`Errno::PARKED`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Errno(pub u32);

/// Names every error number of the interface on [`Errno`], and the way back to the interface's.
macro_rules! numbers {
    ($($name:ident),* $(,)?) => {
        #[allow(dead_code, reason = "every number Linux names, whether the kernel returns it or not")]
        impl Errno {
            $(pub const $name: Self = Self(Linux::$name as u32);)*

            /// The interface's error for this number, if Linux has one.
            pub fn to_linux(self) -> Option<Linux> {
                [$(Linux::$name),*].into_iter().find(|linux| *linux as u32 == self.0)
            }
        }
    };
}

numbers!(
    EPERM,
    ENOENT,
    ESRCH,
    EINTR,
    EIO,
    ENXIO,
    E2BIG,
    ENOEXEC,
    EBADF,
    ECHILD,
    EAGAIN,
    ENOMEM,
    EACCES,
    EFAULT,
    ENOTBLK,
    EBUSY,
    EEXIST,
    EXDEV,
    ENODEV,
    ENOTDIR,
    EISDIR,
    EINVAL,
    ENFILE,
    EMFILE,
    ENOTTY,
    ETXTBSY,
    EFBIG,
    ENOSPC,
    ESPIPE,
    EROFS,
    EMLINK,
    EPIPE,
    EDOM,
    ERANGE,
    EDEADLK,
    ENAMETOOLONG,
    ENOLCK,
    ENOSYS,
    ENOTEMPTY,
    ELOOP,
    ENOMSG,
    EIDRM,
    ECHRNG,
    EL2NSYNC,
    EL3HLT,
    EL3RST,
    ELNRNG,
    EUNATCH,
    ENOCSI,
    EL2HLT,
    EBADE,
    EBADR,
    EXFULL,
    ENOANO,
    EBADRQC,
    EBADSLT,
    EBFONT,
    ENOSTR,
    ENODATA,
    ETIME,
    ENOSR,
    ENONET,
    ENOPKG,
    EREMOTE,
    ENOLINK,
    EADV,
    ESRMNT,
    ECOMM,
    EPROTO,
    EMULTIHOP,
    EDOTDOT,
    EBADMSG,
    EOVERFLOW,
    ENOTUNIQ,
    EBADFD,
    EREMCHG,
    ELIBACC,
    ELIBBAD,
    ELIBSCN,
    ELIBMAX,
    ELIBEXEC,
    EILSEQ,
    ERESTART,
    ESTRPIPE,
    EUSERS,
    ENOTSOCK,
    EDESTADDRREQ,
    EMSGSIZE,
    EPROTOTYPE,
    ENOPROTOOPT,
    EPROTONOSUPPORT,
    ESOCKTNOSUPPORT,
    EOPNOTSUPP,
    EPFNOSUPPORT,
    EAFNOSUPPORT,
    EADDRINUSE,
    EADDRNOTAVAIL,
    ENETDOWN,
    ENETUNREACH,
    ENETRESET,
    ECONNABORTED,
    ECONNRESET,
    ENOBUFS,
    EISCONN,
    ENOTCONN,
    ESHUTDOWN,
    ETOOMANYREFS,
    ETIMEDOUT,
    ECONNREFUSED,
    EHOSTDOWN,
    EHOSTUNREACH,
    EALREADY,
    EINPROGRESS,
    ESTALE,
    EUCLEAN,
    ENOTNAM,
    ENAVAIL,
    EISNAM,
    EREMOTEIO,
    EDQUOT,
    ENOMEDIUM,
    EMEDIUMTYPE,
    ECANCELED,
    ENOKEY,
    EKEYEXPIRED,
    EKEYREVOKED,
    EKEYREJECTED,
    EOWNERDEAD,
    ENOTRECOVERABLE,
    ERFKILL,
    EHWPOISON,
    ERESTARTSYS,
    ERESTARTNOINTR,
    ERESTARTNOHAND,
    ERESTART_RESTARTBLOCK,
);

impl Errno {
    /// Not an error: the call parked its thread and runs again when woken.
    pub const PARKED: Self = Self(u32::MAX);

    /// The interface's error for one, which every number an embedder can give is.
    pub fn from_linux(linux: Linux) -> Self {
        Self(linux as u32)
    }

    /// The interface's error for a number the kernel chose itself, which is always one of Linux's.
    pub fn to_known(self) -> Linux {
        self.to_linux()
            .expect("the kernel's own errors are Linux's error numbers")
    }

    /// The value a system call returns in x0 for this error.
    pub fn to_return(self) -> u64 {
        (-i64::from(self.0)) as u64
    }

    /// Whether a call's return value is an error (Linux's `IS_ERR_VALUE`: the last 4095 values).
    pub fn is_error_return(value: u64) -> bool {
        value >= (-4095i64) as u64
    }

    /// The restart answer a call's return value carries, if it is one.
    pub fn restart_of(value: u64) -> Option<Self> {
        let code = Self(u32::try_from((value as i64).checked_neg()?).ok()?);
        [
            Self::ERESTARTSYS,
            Self::ERESTARTNOINTR,
            Self::ERESTARTNOHAND,
            Self::ERESTART_RESTARTBLOCK,
        ]
        .contains(&code)
        .then_some(code)
    }
}

/// What a failed request to the CPU side means to the guest.
pub(crate) fn from_cpu(error: CpuError) -> Errno {
    match error {
        CpuError::OutOfMemory => Errno::ENOMEM,
        CpuError::BadAddress => Errno::EFAULT,
        CpuError::NotSupported => Errno::ENOSYS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_number_goes_back_to_the_interface_and_others_do_not() {
        assert_eq!(Errno::EAGAIN.to_linux(), Some(Linux::EAGAIN));
        assert_eq!(Errno::ERESTARTSYS.to_linux(), Some(Linux::ERESTARTSYS));
        assert_eq!(Errno::PARKED.to_linux(), None);
        assert_eq!(Errno::from_linux(Linux::EHWPOISON), Errno::EHWPOISON);
    }
}
