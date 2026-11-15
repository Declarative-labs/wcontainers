//! Linux error numbers as the kernel returns them.

use wcontainers_abi::CpuError;

use crate::linux::errno;

/// A Linux error number, such as `ENOENT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Errno(pub u32);

impl Errno {
    pub const E2BIG: Self = Self(errno::E2BIG);
    pub const EBADF: Self = Self(errno::EBADF);
    pub const EFAULT: Self = Self(errno::EFAULT);
    pub const ENOEXEC: Self = Self(errno::ENOEXEC);
    pub const ENOMEM: Self = Self(errno::ENOMEM);
    pub const ENOSYS: Self = Self(errno::ENOSYS);

    /// The value a system call returns in x0 for this error.
    pub fn to_return(self) -> u64 {
        (-i64::from(self.0)) as u64
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
