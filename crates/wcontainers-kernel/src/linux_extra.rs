//! The few Linux values linux-raw-sys does not carry: C headers define them as casts (`SIG_DFL`) or only
//! in the C library's headers (`ST_*`). Each is part of Linux's ABI and fixed.

/// The default action for a signal, `((__sighandler_t)0)`.
pub(crate) const SIG_DFL: u64 = 0;
/// Ignore the signal, `((__sighandler_t)1)`.
pub(crate) const SIG_IGN: u64 = 1;

/// `statfs`'s `f_flags` bits, as Linux's `calculate_f_flags` sets them.
pub(crate) const ST_RDONLY: u32 = 0x0001;
pub(crate) const ST_NOSUID: u32 = 0x0002;
pub(crate) const ST_NODEV: u32 = 0x0004;
pub(crate) const ST_NOEXEC: u32 = 0x0008;
/// Always set: the flags are meaningful.
pub(crate) const ST_VALID: u32 = 0x0020;
pub(crate) const ST_RELATIME: u32 = 0x1000;

// Values from Linux's uapi headers newer than the ones linux-raw-sys 0.12.1 was generated from.

/// `preadv2` and `pwritev2` flags past `RWF_APPEND` (`linux/fs.h`).
pub(crate) const RWF_NOAPPEND: u32 = 0x20;
pub(crate) const RWF_ATOMIC: u32 = 0x40;
pub(crate) const RWF_DONTCACHE: u32 = 0x80;
pub(crate) const RWF_NOSIGNAL: u32 = 0x100;
/// `close_range` flags (`linux/close_range.h`).
pub(crate) const CLOSE_RANGE_UNSHARE: u32 = 1 << 1;
pub(crate) const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;

// Values from Linux's uapi headers linux-raw-sys does not generate (`linux/posix_acl.h`,
// `linux/posix_acl_xattr.h`).

/// The version an access control list's attribute value starts with.
pub(crate) const POSIX_ACL_XATTR_VERSION: u32 = 0x0002;
/// Access control list entry tags.
pub(crate) const ACL_USER_OBJ: u16 = 0x01;
pub(crate) const ACL_USER: u16 = 0x02;
pub(crate) const ACL_GROUP_OBJ: u16 = 0x04;
pub(crate) const ACL_GROUP: u16 = 0x08;
pub(crate) const ACL_MASK: u16 = 0x10;
pub(crate) const ACL_OTHER: u16 = 0x20;

/// `personality` bits `execve` clears when a program gains privileges (`PER_CLEAR_ON_SETID`, from
/// `linux/personality.h`'s `READ_IMPLIES_EXEC`, `ADDR_NO_RANDOMIZE`, `ADDR_COMPAT_LAYOUT` and
/// `MMAP_PAGE_ZERO`).
pub(crate) const PER_CLEAR_ON_SETID: u32 = 0x0400000 | 0x0040000 | 0x0200000 | 0x0100000;

/// `sched_setattr`'s and `sched_getattr`'s structure (`linux/sched/types.h`), which linux-raw-sys does
/// not generate. Its first version ends after `sched_period`.
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct sched_attr {
    pub size: u32,
    pub sched_policy: u32,
    pub sched_flags: u64,
    pub sched_nice: i32,
    pub sched_priority: u32,
    pub sched_runtime: u64,
    pub sched_deadline: u64,
    pub sched_period: u64,
    pub sched_util_min: u32,
    pub sched_util_max: u32,
}
/// The size of the first version of `sched_attr` (`SCHED_ATTR_SIZE_VER0`).
pub(crate) const SCHED_ATTR_SIZE_VER0: u32 = 48;
/// The size of the version with utilisation clamps (`SCHED_ATTR_SIZE_VER1`).
pub(crate) const SCHED_ATTR_SIZE_VER1: u32 = 56;
/// A flag only the kernel's own frequency governor threads may use (`SCHED_FLAG_SUGOV`).
pub(crate) const SCHED_FLAG_SUGOV: u64 = 0x1000_0000;

/// I/O priorities (`linux/ioprio.h`, which linux-raw-sys does not generate): the class sits above 13
/// bits, the level in the lowest three.
pub(crate) const IOPRIO_CLASS_SHIFT: u32 = 13;
pub(crate) const IOPRIO_CLASS_NONE: u32 = 0;
pub(crate) const IOPRIO_CLASS_RT: u32 = 1;
pub(crate) const IOPRIO_CLASS_BE: u32 = 2;
pub(crate) const IOPRIO_CLASS_IDLE: u32 = 3;
/// What `ioprio_get`'s `which` names: one process, a process group, or a user's processes.
pub(crate) const IOPRIO_WHO_PROCESS: u64 = 1;
pub(crate) const IOPRIO_WHO_USER: u64 = 3;

/// `rseq`'s flags (`linux/rseq.h`, which linux-raw-sys does not generate): unregister, and turn time
/// slice extensions on from the start.
pub(crate) const RSEQ_FLAG_UNREGISTER: u32 = 1;
pub(crate) const RSEQ_FLAG_SLICE_EXT_DEFAULT_ON: u32 = 2;
/// The size of the first `struct rseq`, and the part of it the kernel writes (to `end`).
pub(crate) const ORIG_RSEQ_SIZE: u32 = 32;
pub(crate) const RSEQ_WRITTEN_SIZE: u32 = 28;
/// `cpu_id`'s value in an area not registered (`RSEQ_CPU_ID_UNINITIALIZED`).
pub(crate) const RSEQ_CPU_ID_UNINITIALIZED: u32 = u32::MAX;

/// The `mmap` flags Linux knew before `MAP_SHARED_VALIDATE` (`LEGACY_MAP_MASK` in Linux's internal
/// `linux/mman.h`): a validated shared mapping refuses any other.
pub(crate) const LEGACY_MAP_MASK: u32 = {
    use crate::linux::general::*;
    MAP_SHARED
        | MAP_PRIVATE
        | MAP_FIXED
        | MAP_ANONYMOUS
        | MAP_DENYWRITE
        | MAP_EXECUTABLE
        | MAP_UNINITIALIZED
        | MAP_GROWSDOWN
        | MAP_LOCKED
        | MAP_NORESERVE
        | MAP_POPULATE
        | MAP_NONBLOCK
        | MAP_STACK
        | MAP_HUGETLB
};

/// File handles (Linux's internal `linux/exportfs.h`): the largest handle, and the types tmpfs and the
/// generic inode encoding give, and the one for a handle that did not fit.
pub(crate) const MAX_HANDLE_SZ: u32 = 128;
pub(crate) const FILEID_INO32_GEN: i32 = 1;
pub(crate) const FILEID_INO64_GEN: i32 = 0x81;
pub(crate) const FILEID_INVALID: i32 = 0xff;

// The encoding of CPU clock ids below zero, from Linux's `linux/posix-timers_types.h`, which is not a
// uapi header: the C library's `clock_getcpuclockid` and `pthread_getcpuclockid` make these ids.

/// Which time a CPU clock counts: `CPUCLOCK_PROF`, `CPUCLOCK_VIRT`, `CPUCLOCK_SCHED`; 3 is past them.
pub(crate) const CPUCLOCK_CLOCK_MASK: i32 = 3;
pub(crate) const CPUCLOCK_SCHED: i32 = 2;
pub(crate) const CPUCLOCK_MAX: i32 = 3;
/// Set for a thread's clock, clear for a process's.
pub(crate) const CPUCLOCK_PERTHREAD_MASK: i32 = 4;
/// The low bits of a clock id naming a clock device by descriptor (`CLOCKFD`, under `CLOCKFD_MASK`).
pub(crate) const CLOCKFD: i32 = 3;
pub(crate) const CLOCKFD_MASK: i32 = CPUCLOCK_PERTHREAD_MASK | CPUCLOCK_CLOCK_MASK;

/// `times`'s structure (`linux/times.h`), which linux-raw-sys does not generate.
#[repr(C)]
#[derive(Clone, Copy, Debug, zerocopy::IntoBytes, zerocopy::Immutable)]
pub(crate) struct tms {
    pub tms_utime: i64,
    pub tms_stime: i64,
    pub tms_cutime: i64,
    pub tms_cstime: i64,
}

/// An `_IOR` or `_IOW` request number (`asm-generic/ioctl.h`, whose macros bindgen does not expand).
const fn ioc(direction: u32, kind: u8, number: u8, size: usize) -> u32 {
    (direction << 30) | ((size as u32) << 16) | ((kind as u32) << 8) | number as u32
}
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

/// The size of `struct space_resv`, which only Linux's internal `linux/falloc.h` defines: two
/// 16 bit fields, two 64 bit ones, two 32 bit ones and four 32 bit pads.
pub(crate) const SPACE_RESV_SIZE: usize = 48;

/// The preallocation requests of `linux/falloc.h` (not a uapi header), on `struct space_resv`.
pub(crate) const FS_IOC_RESVSP: u32 = ioc(IOC_WRITE, b'X', 40, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_UNRESVSP: u32 = ioc(IOC_WRITE, b'X', 41, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_RESVSP64: u32 = ioc(IOC_WRITE, b'X', 42, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_UNRESVSP64: u32 = ioc(IOC_WRITE, b'X', 43, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_ZERO_RANGE: u32 = ioc(IOC_WRITE, b'X', 57, SPACE_RESV_SIZE);

/// `linux/fs.h`'s file system identity requests, which linux-raw-sys's ioctl list lacks.
pub(crate) const FS_IOC_GETFSUUID: u32 = ioc(
    IOC_READ,
    0x15,
    0,
    size_of::<crate::linux::general::fsuuid2>(),
);
pub(crate) const FS_IOC_GETFSSYSFSPATH: u32 = ioc(
    IOC_READ,
    0x15,
    1,
    size_of::<crate::linux::general::fs_sysfs_path>(),
);
