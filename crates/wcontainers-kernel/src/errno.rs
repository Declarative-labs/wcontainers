//! Linux error numbers as the kernel returns them.

use wcontainers_abi::CpuError;

use crate::linux::errno;

/// A Linux error number, such as `ENOENT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Errno(pub u32);

impl Errno {
    pub const E2BIG: Self = Self(errno::E2BIG);
    pub const EACCES: Self = Self(errno::EACCES);
    pub const EAGAIN: Self = Self(errno::EAGAIN);
    pub const EBADF: Self = Self(errno::EBADF);
    pub const EBUSY: Self = Self(errno::EBUSY);
    pub const EDEADLK: Self = Self(errno::EDEADLK);
    pub const ECHILD: Self = Self(errno::ECHILD);
    pub const EEXIST: Self = Self(errno::EEXIST);
    pub const EFAULT: Self = Self(errno::EFAULT);
    pub const EFBIG: Self = Self(errno::EFBIG);
    pub const EIDRM: Self = Self(errno::EIDRM);
    pub const EINTR: Self = Self(errno::EINTR);
    pub const EINVAL: Self = Self(errno::EINVAL);
    pub const EIO: Self = Self(errno::EIO);
    pub const EISDIR: Self = Self(errno::EISDIR);
    pub const ELIBBAD: Self = Self(errno::ELIBBAD);
    pub const ELOOP: Self = Self(errno::ELOOP);
    pub const EMFILE: Self = Self(errno::EMFILE);
    pub const EMLINK: Self = Self(errno::EMLINK);
    pub const ENAMETOOLONG: Self = Self(errno::ENAMETOOLONG);
    pub const ENFILE: Self = Self(errno::ENFILE);
    pub const ENODEV: Self = Self(errno::ENODEV);
    pub const ENOENT: Self = Self(errno::ENOENT);
    pub const ENOEXEC: Self = Self(errno::ENOEXEC);
    pub const ENOMEM: Self = Self(errno::ENOMEM);
    pub const EMSGSIZE: Self = Self(errno::EMSGSIZE);
    pub const ENOMSG: Self = Self(errno::ENOMSG);
    pub const ENOSPC: Self = Self(errno::ENOSPC);
    pub const ENOSYS: Self = Self(errno::ENOSYS);
    pub const ENOTDIR: Self = Self(errno::ENOTDIR);
    pub const ENOTEMPTY: Self = Self(errno::ENOTEMPTY);
    pub const ENOTSOCK: Self = Self(errno::ENOTSOCK);
    pub const ENOTTY: Self = Self(errno::ENOTTY);
    pub const ENXIO: Self = Self(errno::ENXIO);
    pub const EOPNOTSUPP: Self = Self(errno::EOPNOTSUPP);
    pub const EOVERFLOW: Self = Self(errno::EOVERFLOW);
    pub const EPERM: Self = Self(errno::EPERM);
    pub const EPIPE: Self = Self(errno::EPIPE);
    pub const ERANGE: Self = Self(errno::ERANGE);
    pub const EROFS: Self = Self(errno::EROFS);
    pub const ESPIPE: Self = Self(errno::ESPIPE);
    pub const ESRCH: Self = Self(errno::ESRCH);
    pub const ETIMEDOUT: Self = Self(errno::ETIMEDOUT);
    pub const ETXTBSY: Self = Self(errno::ETXTBSY);
    pub const EXDEV: Self = Self(errno::EXDEV);
    pub const ENODATA: Self = Self(errno::ENODATA);
    /// A call a signal interrupted: restarted unless a handler without `SA_RESTART` runs.
    pub const ERESTARTSYS: Self = Self(crate::linux_extra::ERESTARTSYS);
    /// Restarted whatever the handler.
    pub const ERESTARTNOINTR: Self = Self(crate::linux_extra::ERESTARTNOINTR);
    /// Restarted only when no handler runs.
    pub const ERESTARTNOHAND: Self = Self(crate::linux_extra::ERESTARTNOHAND);
    /// Continued by `restart_syscall` when no handler runs.
    pub const ERESTART_RESTARTBLOCK: Self = Self(crate::linux_extra::ERESTART_RESTARTBLOCK);

    /// Not an error: the call parked its thread and runs again when woken.
    pub const PARKED: Self = Self(u32::MAX);

    /// The value a system call returns in x0 for this error.
    pub fn to_return(self) -> u64 {
        (-i64::from(self.0)) as u64
    }

    /// The restart answer a call's return value carries, if it is one.
    pub fn restart_of(value: u64) -> Option<Self> {
        use crate::linux_extra::*;
        let code = (value as i64).checked_neg()?;
        match u32::try_from(code).ok()? {
            ERESTARTSYS | ERESTARTNOINTR | ERESTARTNOHAND | ERESTART_RESTARTBLOCK => {
                Some(Self(code as u32))
            }
            _ => None,
        }
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
