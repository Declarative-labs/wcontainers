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
        | MAP_HUGE_2MB
        | MAP_HUGE_1GB
};

/// The extended flag for atomic writes (`FS_XFLAG_ATOMICWRITES`, newer than linux-raw-sys's headers),
/// and every extended flag Linux knows (`FS_XFLAGS_MASK` in its internal `linux/fileattr.h`): a set
/// request with any other is refused before the file is asked (measured on the reference).
pub(crate) const FS_XFLAG_ATOMICWRITES: u32 = 0x2_0000;
pub(crate) const FS_XFLAGS_MASK: u32 = {
    use crate::linux::general::*;
    FS_XFLAG_REALTIME
        | FS_XFLAG_PREALLOC
        | FS_XFLAG_IMMUTABLE
        | FS_XFLAG_APPEND
        | FS_XFLAG_SYNC
        | FS_XFLAG_NOATIME
        | FS_XFLAG_NODUMP
        | FS_XFLAG_RTINHERIT
        | FS_XFLAG_PROJINHERIT
        | FS_XFLAG_NOSYMLINKS
        | FS_XFLAG_EXTSIZE
        | FS_XFLAG_EXTSZINHERIT
        | FS_XFLAG_NODEFRAG
        | FS_XFLAG_FILESTREAM
        | FS_XFLAG_DAX
        | FS_XFLAG_COWEXTSIZE
        | FS_XFLAG_ATOMICWRITES
        | FS_XFLAG_HASATTR
};

/// Where Linux's never reused mount numbers start (`MNT_UNIQUE_ID_OFFSET` in `fs/namespace.c`), above
/// every plain mount number.
pub(crate) const MNT_UNIQUE_ID_OFFSET: u64 = 1 << 31;

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

/// The largest buffer the kernel's allocator gives (`KMALLOC_MAX_SIZE`, `linux/slab.h`: a page shifted
/// by `MAX_PAGE_ORDER`, 10 for 4 KiB pages), which a sysctl's read or write allocates for its count.
pub(crate) const KMALLOC_MAX_SIZE: u64 = 4096 << 10;

/// `prctl` values newer than linux-raw-sys 0.12.1's headers (`linux/prctl.h`): disable transparent
/// huge pages except where advised.
pub(crate) const PR_THP_DISABLE_EXCEPT_ADVISED: u64 = 1 << 1;

/// `linux/securebits.h`'s bit for keeping capabilities across a change of ids
/// (`issecure_mask(SECURE_KEEP_CAPS)`), which linux-raw-sys does not generate.
pub(crate) const SECBIT_KEEP_CAPS: u64 = 1 << 4;

/// The words Linux keeps of a program's auxiliary vector (`AT_VECTOR_SIZE`: twice arm64's 2 and the
/// 24 generic entries and the end).
pub(crate) const AT_VECTOR_SIZE: usize = 2 * (2 + 24 + 1);

/// The pages a new pipe holds (`PIPE_DEF_BUFFERS`, Linux's internal `linux/pipe_fs_i.h`), which bounds
/// one splice through a pipe.
pub(crate) const PIPE_DEF_BUFFERS: u64 = 16;

/// arm64's signal frame (`asm/sigcontext.h`, `asm/ucontext.h` and `arch/arm64/kernel/signal.c`),
/// which linux-raw-sys does not generate: `struct rt_sigframe` is a `siginfo` then a `ucontext`, whose
/// `sigcontext` ends in 4096 bytes of records.
pub(crate) mod frame {
    /// Offsets in `struct rt_sigframe`.
    pub const UC_FLAGS: u64 = 128;
    pub const UC_LINK: u64 = 136;
    pub const UC_STACK: u64 = 144;
    pub const UC_SIGMASK: u64 = 168;
    pub const FAULT_ADDRESS: u64 = 304;
    pub const REGS: u64 = 312;
    pub const SP: u64 = 560;
    pub const PC: u64 = 568;
    pub const PSTATE: u64 = 576;
    pub const RESERVED: u64 = 592;
    pub const RESERVED_SIZE: u64 = 4096;
    /// `sizeof(struct rt_sigframe)`.
    pub const SIZE: u64 = 4688;
    /// `struct frame_record`: the caller's frame pointer and link register.
    pub const RECORD_SIZE: u64 = 16;
    /// The records in `__reserved`: each starts with a magic number and its size.
    pub const FPSIMD_MAGIC: u32 = 0x4650_8001;
    pub const FPSIMD_SIZE: u32 = 528;
    pub const ESR_MAGIC: u32 = 0x4553_5201;
    pub const ESR_SIZE: u32 = 16;
    /// The exception classes and the instruction length bit of an ESR (`asm/esr.h`).
    pub const ESR_EC_SHIFT: u32 = 26;
    pub const ESR_IL: u64 = 1 << 25;
    pub const ESR_EC_IABT_LOW: u64 = 0x20;
    pub const ESR_EC_PC_ALIGN: u64 = 0x22;
    pub const ESR_EC_DABT_LOW: u64 = 0x24;
    pub const ESR_EC_SP_ALIGN: u64 = 0x26;
    /// A data abort's write bit, and the fault status codes of a level 3 translation and permission
    /// fault.
    pub const ESR_WNR: u64 = 1 << 6;
    pub const ESR_FSC_TRANSLATION_3: u64 = 0x07;
    pub const ESR_FSC_PERMISSION_3: u64 = 0x0f;
}

/// The internal answers of a call a signal interrupted, never seen by the guest
/// (`linux/errno.h`): restarted, or turned into EINTR when a handler runs.
pub(crate) const ERESTARTSYS: u32 = 512;
pub(crate) const ERESTARTNOINTR: u32 = 513;
pub(crate) const ERESTARTNOHAND: u32 = 514;
pub(crate) const ERESTART_RESTARTBLOCK: u32 = 516;

/// `AT_MINSIGSTKSZ` on the reference's CPU: arm64's `signal_minsigstksz`, the frame's size with the
/// records that CPU has, which the made-up machine reports too (measured: 4720).
pub(crate) const SIGNAL_MINSIGSTKSZ: u64 = 4720;
