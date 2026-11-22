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
pub(crate) const ST_NOATIME: u32 = 0x0400;
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

// Values from Linux's uapi headers linux-raw-sys does not generate (`linux/sockios.h`,
// `linux/ipv6_route.h`).

/// The kind (`_IOC_TYPE`) of every socket request, `SOCK_IOC_TYPE`.
pub(crate) const SOCK_IOC_TYPE: u32 = 0x89;

/// `struct in6_rtmsg`, an IPv6 route request.
#[repr(C)]
#[allow(non_camel_case_types)]
pub(crate) struct in6_rtmsg {
    pub rtmsg_dst: crate::linux::net::in6_addr,
    pub rtmsg_src: crate::linux::net::in6_addr,
    pub rtmsg_gateway: crate::linux::net::in6_addr,
    pub rtmsg_type: u32,
    pub rtmsg_dst_len: u16,
    pub rtmsg_src_len: u16,
    pub rtmsg_metric: u32,
    pub rtmsg_info: core::ffi::c_ulong,
    pub rtmsg_flags: u32,
    pub rtmsg_ifindex: i32,
}

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
/// Access control list permission bits (`ACL_READ`, `ACL_WRITE`, `ACL_EXECUTE`).
pub(crate) const ACL_PERMISSIONS: u16 = 0x04 | 0x02 | 0x01;
/// The id of an entry that names no user or group (`ACL_UNDEFINED_ID`).
pub(crate) const ACL_UNDEFINED_ID: u32 = u32::MAX;

/// `linux/sched/prio.h`'s `MIN_NICE` and `MAX_NICE`: the range of nice values.
pub(crate) const MIN_NICE: i32 = -20;
pub(crate) const MAX_NICE: i32 = 19;

/// `linux/personality.h`'s `ADDR_NO_RANDOMIZE`: a new program's layout is not placed at random.
pub const ADDR_NO_RANDOMIZE: u32 = 0x0040000;
/// `linux/personality.h`'s `ADDR_COMPAT_LAYOUT`: new mappings go up from a low base.
pub(crate) const ADDR_COMPAT_LAYOUT: u32 = 0x0200000;

/// `personality` bits `execve` clears when a program gains privileges (`PER_CLEAR_ON_SETID`, from
/// `linux/personality.h`'s `READ_IMPLIES_EXEC`, `ADDR_NO_RANDOMIZE`, `ADDR_COMPAT_LAYOUT` and
/// `MMAP_PAGE_ZERO`).
pub(crate) const PER_CLEAR_ON_SETID: u32 =
    0x0400000 | ADDR_NO_RANDOMIZE | ADDR_COMPAT_LAYOUT | 0x0100000;

/// `sched_setattr`'s and `sched_getattr`'s structure (`linux/sched/types.h`), which linux-raw-sys does
/// not generate. Its first version ends after `sched_period`.
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub struct sched_attr {
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
pub const IOPRIO_CLASS_SHIFT: u32 = 13;
pub(crate) const IOPRIO_CLASS_NONE: u32 = 0;
pub(crate) const IOPRIO_CLASS_RT: u32 = 1;
pub const IOPRIO_CLASS_BE: u32 = 2;
pub(crate) const IOPRIO_CLASS_IDLE: u32 = 3;
/// What `ioprio_get`'s `which` names: one process, a process group, or a user's processes.
pub(crate) const IOPRIO_WHO_PROCESS: u64 = 1;
pub(crate) const IOPRIO_WHO_USER: u64 = 3;

/// Process descriptors (`linux/pidfd.h`, which linux-raw-sys does not generate): one for a thread
/// rather than its process, and the scope `pidfd_send_signal` may be given.
pub(crate) const PIDFD_THREAD: u32 = crate::linux::general::O_EXCL;
pub(crate) const PIDFD_SIGNAL_THREAD: u32 = 1;
pub(crate) const PIDFD_SIGNAL_THREAD_GROUP: u32 = 2;
pub(crate) const PIDFD_SIGNAL_PROCESS_GROUP: u32 = 4;

/// `signalfd4`'s flags (`linux/signalfd.h`, which linux-raw-sys does not generate).
pub(crate) const SFD_CLOEXEC: u32 = crate::linux::general::O_CLOEXEC;
pub(crate) const SFD_NONBLOCK: u32 = crate::linux::general::O_NONBLOCK;

/// Linux 7.0's newest `fcntl` commands, delegations (`F_GETDELEG`, `F_SETDELEG`) and its newest
/// `prctl` option (`PR_RSEQ_SLICE_EXTENSION`), newer than linux-raw-sys's headers; measured in the
/// reference, which knows no command or option past them.
pub(crate) const F_GETDELEG: u32 = 1039;
pub(crate) const F_SETDELEG: u32 = 1040;
pub(crate) const PR_RSEQ_SLICE_EXTENSION: u32 = 79;

/// `rseq`'s flags (`linux/rseq.h`, which linux-raw-sys does not generate): unregister, and turn time
/// slice extensions on from the start.
pub(crate) const RSEQ_FLAG_UNREGISTER: u32 = 1;
pub(crate) const RSEQ_FLAG_SLICE_EXT_DEFAULT_ON: u32 = 2;
/// The size of the first `struct rseq`, and the part of it the kernel writes (to `end`).
pub(crate) const ORIG_RSEQ_SIZE: u32 = 32;
pub(crate) const RSEQ_WRITTEN_SIZE: u32 = 28;
/// `cpu_id`'s value in an area not registered (`RSEQ_CPU_ID_UNINITIALIZED`).
pub(crate) const RSEQ_CPU_ID_UNINITIALIZED: u32 = u32::MAX;
/// The first fields of `struct rseq`, up to the address of the current sequence.
#[repr(C)]
pub(crate) struct rseq {
    pub cpu_id_start: u32,
    pub cpu_id: u32,
    pub rseq_cs: u64,
}
/// A restartable sequence's description (`struct rseq_cs`).
#[repr(C)]
#[derive(Clone, Copy, Debug, zerocopy::FromBytes, zerocopy::Immutable, zerocopy::KnownLayout)]
pub(crate) struct rseq_cs {
    pub version: u32,
    pub flags: u32,
    pub start_ip: u64,
    pub post_commit_offset: u64,
    pub abort_ip: u64,
}

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
/// The handle type kernfs and pidfs give: a 64-bit inode number.
pub(crate) const FILEID_KERNFS: i32 = 0xfe;
/// The handle type nsfs gives (`FILEID_NSFS`, not in linux-raw-sys): a namespace's id, type and
/// number.
pub(crate) const FILEID_NSFS: i32 = 0xf1;

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

/// pidfs's requests (`linux/pidfd.h`, not generated by linux-raw-sys): a descriptor for one of the
/// process's namespaces, numbers 1 to 10 of `PIDFS_IOCTL_MAGIC`, and `PIDFD_GET_INFO`, number 11,
/// on `struct pidfd_info`, whose three published sizes are 64, 72 and 80 bytes.
pub(crate) const PIDFS_IOCTL_MAGIC: u8 = 0xff;
pub(crate) const PIDFD_GET_NAMESPACES: core::ops::RangeInclusive<u32> =
    ioc(0, PIDFS_IOCTL_MAGIC, 1, 0)..=ioc(0, PIDFS_IOCTL_MAGIC, 10, 0);
pub(crate) const PIDFD_GET_INFO_NUMBER: u8 = 11;
pub(crate) const PIDFD_INFO_SIZE_VER0: usize = 64;
pub(crate) const PIDFD_INFO_SIZE_VER2: usize = 80;
pub(crate) const PIDFD_GET_INFO: u32 = ioc(
    IOC_READ | IOC_WRITE,
    PIDFS_IOCTL_MAGIC,
    PIDFD_GET_INFO_NUMBER,
    PIDFD_INFO_SIZE_VER2,
);
/// `pidfd_info`'s mask bits, and its core dump bits.
pub(crate) const PIDFD_INFO_PID: u64 = 1 << 0;
pub(crate) const PIDFD_INFO_CREDS: u64 = 1 << 1;
pub(crate) const PIDFD_INFO_CGROUPID: u64 = 1 << 2;
pub(crate) const PIDFD_INFO_EXIT: u64 = 1 << 3;
pub(crate) const PIDFD_INFO_COREDUMP: u64 = 1 << 4;
pub(crate) const PIDFD_INFO_SUPPORTED_MASK: u64 = 1 << 5;
pub(crate) const PIDFD_INFO_COREDUMP_SIGNAL: u64 = 1 << 6;
pub(crate) const PIDFD_COREDUMP_SKIP: u32 = 1 << 1;
pub(crate) const PIDFD_COREDUMP_USER: u32 = 1 << 2;

/// The size of `struct space_resv`, which only Linux's internal `linux/falloc.h` defines: two
/// 16 bit fields, two 64 bit ones, two 32 bit ones and four 32 bit pads.
pub(crate) const SPACE_RESV_SIZE: usize = 48;

/// The preallocation requests of `linux/falloc.h` (not a uapi header), on `struct space_resv`.
pub(crate) const FS_IOC_RESVSP: u32 = ioc(IOC_WRITE, b'X', 40, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_UNRESVSP: u32 = ioc(IOC_WRITE, b'X', 41, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_RESVSP64: u32 = ioc(IOC_WRITE, b'X', 42, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_UNRESVSP64: u32 = ioc(IOC_WRITE, b'X', 43, SPACE_RESV_SIZE);
pub(crate) const FS_IOC_ZERO_RANGE: u32 = ioc(IOC_WRITE, b'X', 57, SPACE_RESV_SIZE);

/// `PROCMAP_QUERY` (`linux/fs.h`), the maps file's query for one mapping.
pub(crate) const PROCMAP_QUERY: u32 = ioc(
    IOC_READ | IOC_WRITE,
    crate::linux::general::PROCFS_IOCTL_MAGIC,
    17,
    core::mem::size_of::<crate::linux::general::procmap_query>(),
);

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

/// The largest block the page allocator gives, as an order of pages (`MAX_PAGE_ORDER` for 4 KiB pages).
pub(crate) const MAX_PAGE_ORDER: u32 = 10;

/// The largest buffer the kernel's allocator gives (`KMALLOC_MAX_SIZE`, `linux/slab.h`), which a
/// sysctl's read or write allocates for its count.
pub(crate) const KMALLOC_MAX_SIZE: u64 = wcontainers_abi::PAGE_SIZE << MAX_PAGE_ORDER;

/// `prctl` values newer than linux-raw-sys 0.12.1's headers (`linux/prctl.h`): disable transparent
/// huge pages except where advised.
pub(crate) const PR_THP_DISABLE_EXCEPT_ADVISED: u64 = 1 << 1;

/// `linux/securebits.h`'s bits (`issecure_mask` of each `SECURE_*`), which linux-raw-sys does not
/// generate: root gains no powers by running a program, a change of ids leaves the powers alone,
/// the powers are kept across a change of ids, ambient powers may not be raised, and two that
/// only ask programs to check what they run. Each has a lock bit just above it.
pub(crate) const SECBIT_NOROOT: u32 = 1 << 0;
pub(crate) const SECBIT_NO_SETUID_FIXUP: u32 = 1 << 2;
pub(crate) const SECBIT_KEEP_CAPS: u32 = 1 << 4;
pub(crate) const SECBIT_NO_CAP_AMBIENT_RAISE: u32 = 1 << 6;
pub(crate) const SECBIT_EXEC_RESTRICT_FILE: u32 = 1 << 8;
pub(crate) const SECBIT_EXEC_DENY_INTERACTIVE: u32 = 1 << 10;
/// `SECURE_ALL_BITS` and `SECURE_ALL_LOCKS`.
pub(crate) const SECURE_ALL_BITS: u32 = SECBIT_NOROOT
    | SECBIT_NO_SETUID_FIXUP
    | SECBIT_KEEP_CAPS
    | SECBIT_NO_CAP_AMBIENT_RAISE
    | SECBIT_EXEC_RESTRICT_FILE
    | SECBIT_EXEC_DENY_INTERACTIVE;
pub(crate) const SECURE_ALL_LOCKS: u32 = SECURE_ALL_BITS << 1;

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

/// `AT_MINSIGSTKSZ` on the reference's CPU: arm64's `signal_minsigstksz`, the frame's size with the
/// records that CPU has, which the made-up machine reports too (measured: 4720).
pub(crate) const SIGNAL_MINSIGSTKSZ: u64 = 4720;

// Terminal values from Linux's internal headers (`linux/tty.h`, `linux/major.h`,
// `asm-generic/termios.h`, `drivers/tty`), which are not uapi.

/// The line discipline number of `n_tty`, the only one built (`N_TTY`).
pub(crate) const N_TTY: u32 = 0;
/// The serial mouse discipline (`N_MOUSE`), whose open needs CAP_SYS_ADMIN (`serport_ldisc_open`).
pub(crate) const N_MOUSE: i32 = 2;
/// The number of line discipline slots (`NR_LDISCS`).
pub(crate) const NR_LDISCS: i32 = 31;
/// The device major of pseudo-terminal slaves (`UNIX98_PTY_SLAVE_MAJOR`).
pub(crate) const UNIX98_PTY_SLAVE_MAJOR: u32 = 136;
/// The size of a line discipline's read and echo buffers (`N_TTY_BUF_SIZE`).
pub(crate) const N_TTY_BUF_SIZE: usize = 4096;
/// The bytes a pseudo-terminal's other side may have waiting for the line discipline
/// (`tty_buffer_set_limit` in `pty_common_install`).
pub(crate) const PTY_BUFFER_LIMIT: usize = 8192;
/// The control characters a new terminal starts with (`INIT_C_CC`), by `V*` index.
pub(crate) const INIT_C_CC: [u8; 19] = [
    0o3, 0o34, 0o177, 0o25, 0o4, 0, 1, 0, 0o21, 0o23, 0o32, 0, 0o22, 0o17, 0o27, 0o26, 0, 0, 0,
];
/// How many pseudo-terminals a container's devpts may make: `kernel.pty.max` less
/// `kernel.pty.reserve`, since Docker's devpts is a new instance (`devpts_new_index`).
pub(crate) const PTY_LIMIT: u32 = 4096 - 1024;
/// The baud rates of the `B*` speed codes, by index (`baud_table` in `tty_baudrate.c`).
pub(crate) const BAUD_TABLE: [u32; 31] = [
    0, 50, 75, 110, 134, 150, 200, 300, 600, 1200, 1800, 2400, 4800, 9600, 19200, 38400, 57600,
    115200, 230400, 460800, 500000, 576000, 921600, 1000000, 1152000, 1500000, 2000000, 2500000,
    3000000, 3500000, 4000000,
];

/// What n_tty's line discipline holds at most when a pty's reader tells the other side's writers of
/// room (`TTY_THRESHOLD_UNTHROTTLE`, internal to `n_tty.c`).
pub const TTY_THRESHOLD_UNTHROTTLE: usize = 128;

// System V IPC (`linux/ipc.h`, `linux/shm.h`, `asm-generic/ipcbuf.h`, `asm-generic/shmbuf.h`), which
// linux-raw-sys does not generate.

/// The key that always makes a new object (`IPC_PRIVATE`).
pub(crate) const IPC_PRIVATE: i32 = 0;
/// `*get` flags: make the object if the key has none, and fail if it has one.
pub(crate) const IPC_CREAT: u32 = 0o1000;
pub(crate) const IPC_EXCL: u32 = 0o2000;
/// Return at once rather than wait.
pub const IPC_NOWAIT: u32 = 0o4000;
/// `*ctl` commands every kind of object takes.
pub(crate) const IPC_RMID: i32 = 0;
pub(crate) const IPC_SET: i32 = 1;
pub(crate) const IPC_STAT: i32 = 2;
pub(crate) const IPC_INFO: i32 = 3;
/// `shmctl`'s own commands.
pub(crate) const SHM_LOCK: i32 = 11;
pub(crate) const SHM_UNLOCK: i32 = 12;
pub(crate) const SHM_STAT: i32 = 13;
pub(crate) const SHM_INFO: i32 = 14;
pub(crate) const SHM_STAT_ANY: i32 = 15;
/// `shmget` flags: huge pages, and no reservation of memory.
pub(crate) const SHM_HUGETLB: u32 = 0o4000;
/// A huge page segment reserves no pages (`SHM_NORESERVE`).
pub(crate) const SHM_NORESERVE: u32 = 0o10000;
/// `shmat` flags.
pub(crate) const SHM_RDONLY: u32 = 0o10000;
pub(crate) const SHM_RND: u32 = 0o20000;
pub(crate) const SHM_REMAP: u32 = 0o40000;
pub(crate) const SHM_EXEC: u32 = 0o100000;
/// Mode bits a segment's status shows: removed once nothing is attached, locked in memory.
pub(crate) const SHM_DEST: u32 = 0o1000;
pub(crate) const SHM_LOCKED: u32 = 0o2000;
/// The smallest segment, and the largest and the most pages of all segments in a new IPC namespace
/// (`SHMMIN`, `SHMMAX`, `SHMALL`), and the most segments (`SHMMNI`).
pub(crate) const SHMMIN: u64 = 1;
pub(crate) const SHMMAX: u64 = u64::MAX - (1 << 24);
pub(crate) const SHMALL: u64 = u64::MAX - (1 << 24);
pub(crate) const SHMMNI: u64 = 4096;
/// Identifiers: the slot in the low 15 bits, the sequence number above (`IPCMNI_SHIFT`), with no
/// more than `IPCMNI` slots, and slots handed out in a cycle at least `ipc_min_cycle` long.
pub(crate) const IPCMNI_SHIFT: u32 = 15;
pub(crate) const IPCMNI: u32 = 1 << IPCMNI_SHIFT;
pub(crate) const IPC_MIN_CYCLE: u32 = 64;

/// An object's owners and mode as the `*ctl` calls show them (`struct ipc64_perm`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct ipc64_perm {
    pub key: i32,
    pub uid: u32,
    pub gid: u32,
    pub cuid: u32,
    pub cgid: u32,
    pub mode: u32,
    pub seq: u16,
    pub pad2: u16,
    pub pad3: u32,
    pub unused1: u64,
    pub unused2: u64,
}

/// A segment's status (`struct shmid64_ds`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct shmid64_ds {
    pub shm_perm: ipc64_perm,
    pub shm_segsz: u64,
    pub shm_atime: i64,
    pub shm_dtime: i64,
    pub shm_ctime: i64,
    pub shm_cpid: i32,
    pub shm_lpid: i32,
    pub shm_nattch: u64,
    pub unused4: u64,
    pub unused5: u64,
}

/// The limits `IPC_INFO` shows (`struct shminfo64`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct shminfo64 {
    pub shmmax: u64,
    pub shmmin: u64,
    pub shmmni: u64,
    pub shmseg: u64,
    pub shmall: u64,
    pub unused: [u64; 4],
}

/// What `SHM_INFO` shows (`struct shm_info`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct shm_info {
    pub used_ids: i32,
    pub pad: u32,
    pub shm_tot: u64,
    pub shm_rss: u64,
    pub shm_swp: u64,
    pub swap_attempts: u64,
    pub swap_successes: u64,
}

/// Message queues (`linux/msg.h`): `msgrcv`'s flags, `msgctl`'s own commands, and the limits of a new
/// IPC namespace (`MSGMNI`, `MSGMAX`, `MSGMNB`) with the fixed values `IPC_INFO` shows.
pub(crate) const MSG_NOERROR: u32 = 0o10000;
pub(crate) const MSG_EXCEPT: u32 = 0o20000;
pub(crate) const MSG_COPY: u32 = 0o40000;
pub(crate) const MSG_STAT: i32 = 11;
pub(crate) const MSG_INFO: i32 = 12;
pub(crate) const MSG_STAT_ANY: i32 = 13;
pub(crate) const MSGMNI: u64 = 32000;
pub(crate) const MSGMAX: u64 = 8192;
pub(crate) const MSGMNB: u64 = 16384;
pub(crate) const MSGPOOL: i32 = (MSGMNI * MSGMNB / 1024) as i32;
pub(crate) const MSGSSZ: i32 = 16;
pub(crate) const MSGSEG: u16 = 0xffff;

/// A message queue's status (`struct msqid64_ds`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct msqid64_ds {
    pub msg_perm: ipc64_perm,
    pub msg_stime: i64,
    pub msg_rtime: i64,
    pub msg_ctime: i64,
    pub msg_cbytes: u64,
    pub msg_qnum: u64,
    pub msg_qbytes: u64,
    pub msg_lspid: i32,
    pub msg_lrpid: i32,
    pub unused4: u64,
    pub unused5: u64,
}

/// What `IPC_INFO` and `MSG_INFO` show of message queues (`struct msginfo`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct msginfo {
    pub msgpool: i32,
    pub msgmap: i32,
    pub msgmax: i32,
    pub msgmnb: i32,
    pub msgmni: i32,
    pub msgssz: i32,
    pub msgtql: i32,
    pub msgseg: u16,
    pub pad: u16,
}

/// Semaphores (`linux/sem.h`, not generated by linux-raw-sys): `semop`'s undo flag, `semctl`'s own
/// commands, and the limits of a new IPC namespace (`SEMMSL`, `SEMMNS`, `SEMOPM`, `SEMMNI`) with the
/// fixed values `IPC_INFO` shows.
pub(crate) const SEM_UNDO: i16 = 0x1000;
pub(crate) const GETPID: i32 = 11;
pub(crate) const GETVAL: i32 = 12;
pub(crate) const GETALL: i32 = 13;
pub(crate) const GETNCNT: i32 = 14;
pub(crate) const GETZCNT: i32 = 15;
pub(crate) const SETVAL: i32 = 16;
pub(crate) const SETALL: i32 = 17;
pub(crate) const SEM_STAT: i32 = 18;
pub(crate) const SEM_INFO: i32 = 19;
pub(crate) const SEM_STAT_ANY: i32 = 20;
pub(crate) const SEMMNI: u64 = 32000;
pub(crate) const SEMMSL: i32 = 32000;
pub(crate) const SEMMNS: u64 = SEMMNI * SEMMSL as u64;
pub(crate) const SEMOPM: u32 = 500;
pub(crate) const SEMVMX: i32 = 32767;
pub(crate) const SEMAEM: i32 = SEMVMX;
pub(crate) const SEMUSZ: i32 = 20;

/// One operation of `semop` (`struct sembuf`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct sembuf {
    pub sem_num: u16,
    pub sem_op: i16,
    pub sem_flg: i16,
}

/// A semaphore set's status (`struct semid64_ds`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct semid64_ds {
    pub sem_perm: ipc64_perm,
    pub sem_otime: i64,
    pub sem_ctime: i64,
    pub sem_nsems: u64,
    pub unused3: u64,
    pub unused4: u64,
}

/// What `IPC_INFO` and `SEM_INFO` show of semaphores (`struct seminfo`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct seminfo {
    pub semmap: i32,
    pub semmni: i32,
    pub semmns: i32,
    pub semmnu: i32,
    pub semmsl: i32,
    pub semopm: i32,
    pub semume: i32,
    pub semusz: i32,
    pub semvmx: i32,
    pub semaem: i32,
}

/// POSIX message queues (`linux/mqueue.h`, `ipc_namespace.h`, `mqueue.c`): the most priorities, the
/// limits and defaults of a new IPC namespace, the hard limits for `CAP_SYS_RESOURCE` and the
/// least ones a sysctl may set, the sizes of
/// the kernel's message (`struct msg_msg`) and priority node (`struct posix_msg_tree_node`) on
/// arm64 that a queue's charge counts, the size a queue file shows, the cookie a thread
/// notification reads, the file system's magic and `struct sigevent`'s size.
pub(crate) const MQ_PRIO_MAX: u64 = 32768;
pub(crate) const MQ_QUEUES_MAX: u64 = 256;
pub(crate) const MQ_MSG_MAX: i64 = 10;
pub(crate) const MQ_MSG_DEFAULT: i64 = 10;
pub(crate) const MQ_MSGSIZE_MAX: i64 = 8192;
pub(crate) const MQ_MSGSIZE_DEFAULT: i64 = 8192;
pub(crate) const HARD_MSGMAX: i64 = 65536;
pub(crate) const MIN_MSGMAX: i32 = 1;
pub(crate) const MIN_MSGSIZEMAX: i32 = 128;
pub(crate) const HARD_MSGSIZEMAX: i64 = 16 << 20;
pub(crate) const MSG_MSG_SIZE: u64 = 48;
pub(crate) const TREE_NODE_SIZE: u64 = 48;
pub(crate) const FILENT_SIZE: u64 = 80;
pub(crate) const NOTIFY_COOKIE_LEN: u64 = 32;
/// What the last byte of a SIGEV_THREAD cookie says (`linux/mqueue.h`).
pub(crate) const NOTIFY_WOKENUP: u8 = 1;
pub(crate) const NOTIFY_REMOVED: u8 = 2;
pub(crate) const MQUEUE_MAGIC: u32 = 0x1980_0202;
pub(crate) const SIGEVENT_SIZE: u64 = 64;
/// The highest signal number (`_NSIG`).
pub(crate) const SIGNALS: u32 = 64;

/// A queue's attributes (`struct mq_attr`).
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable,
)]
pub(crate) struct mq_attr {
    pub mq_flags: i64,
    pub mq_maxmsg: i64,
    pub mq_msgsize: i64,
    pub mq_curmsgs: i64,
    pub reserved: [i64; 4],
}

/// The longest name a FUSE backend may answer with (`FUSE_NAME_MAX`, Linux's internal
/// `fs/fuse/fuse_i.h`).
pub(crate) const FUSE_NAME_MAX: usize = 1024;

// Values from Linux's kernel-side `linux/net.h`, which no uapi header carries.

/// One more than the largest socket kind (`SOCK_PACKET` is 10).
pub(crate) const SOCK_MAX: u32 = 11;

/// `recvmmsg`'s flag to wait only for the first message (`linux/socket.h`, not in uapi).
pub(crate) const MSG_WAITFORONE: u32 = 0x10000;

/// epoll's requests for busy polling's parameters, `_IOW` and `_IOR` of an 8 byte
/// `struct epoll_params` (`linux/eventpoll.h`, which linux-raw-sys does not generate).
pub(crate) const EPIOCSPARAMS: u32 = 0x4008_8a01;
pub(crate) const EPIOCGPARAMS: u32 = 0x8008_8a02;

/// The control message carrying a sender's process descriptor (`linux/socket.h`, not in uapi).
pub(crate) const SCM_PIDFD: u32 = 0x04;

/// The one extended attribute a socket's inode has, its protocol's name (`linux/xattr.h`, not in
/// uapi).
pub(crate) const XATTR_NAME_SOCKPROTONAME: &[u8] = b"system.sockprotoname";

/// TCP's states as `/proc/net/tcp` shows them (`net/tcp_states.h`, which only the kernel's own
/// headers hold); UDP shows the first and seventh.
pub(crate) const TCP_ESTABLISHED: u8 = 1;
pub(crate) const TCP_SYN_SENT: u8 = 2;
pub(crate) const TCP_SYN_RECV: u8 = 3;
pub(crate) const TCP_FIN_WAIT2: u8 = 5;
pub(crate) const TCP_TIME_WAIT: u8 = 6;
pub(crate) const TCP_CLOSE: u8 = 7;
pub(crate) const TCP_CLOSE_WAIT: u8 = 8;
pub(crate) const TCP_LISTEN: u8 = 10;

/// Where a queued socket error came from (`linux/errqueue.h`, which linux-raw-sys does not
/// generate).
pub(crate) const SO_EE_ORIGIN_ICMP: u8 = 2;
pub(crate) const SO_EE_ORIGIN_ICMP6: u8 = 3;
pub(crate) const SO_EE_ORIGIN_TIMESTAMPING: u8 = 4;
/// Which send timestamp an error queue entry carries (`linux/errqueue.h`).
pub(crate) const SCM_TSTAMP_SND: u32 = 0;
pub(crate) const SCM_TSTAMP_SCHED: u32 = 1;
/// The ICMP and ICMPv6 type and code of an unreachable port (`linux/icmp.h`, `linux/icmpv6.h`).
pub(crate) const ICMP_DEST_UNREACH: u8 = 3;
pub(crate) const ICMP_PORT_UNREACH: u8 = 3;
pub(crate) const ICMPV6_DEST_UNREACH: u8 = 1;
pub(crate) const ICMPV6_PORT_UNREACH: u8 = 4;

/// UDP's own options (`linux/udp.h`, which linux-raw-sys does not generate).
pub(crate) const UDP_CORK: u32 = 1;
pub(crate) const UDPLITE_SEND_CSCOV: u32 = 10;
pub(crate) const UDPLITE_RECV_CSCOV: u32 = 11;
pub(crate) const UDP_ENCAP: u32 = 100;
pub(crate) const UDP_NO_CHECK6_TX: u32 = 101;
pub(crate) const UDP_NO_CHECK6_RX: u32 = 102;
pub(crate) const UDP_SEGMENT: u32 = 103;
pub(crate) const UDP_GRO: u32 = 104;

/// `rtm_flags` of a route answer (`in_route.h`): the route is to this host, a broadcast or a
/// multicast group.
pub(crate) const RTCF_LOCAL: u32 = 0x8000_0000;
pub(crate) const RTCF_BROADCAST: u32 = 0x1000_0000;
pub(crate) const RTCF_MULTICAST: u32 = 0x2000_0000;

/// Native asynchronous I/O (`linux/aio_abi.h`, not in linux-raw-sys): the request kinds, the
/// flags of `struct iocb`, the key Linux writes into each request, and the ring's header
/// (`struct aio_ring` in `fs/aio.c`: its magic, features and size). `struct iocb` and `struct
/// io_event` are 64 and 32 bytes.
pub(crate) const IOCB_CMD_PREAD: u16 = 0;
pub(crate) const IOCB_CMD_PWRITE: u16 = 1;
pub(crate) const IOCB_CMD_FSYNC: u16 = 2;
pub(crate) const IOCB_CMD_FDSYNC: u16 = 3;
pub(crate) const IOCB_CMD_POLL: u16 = 5;
pub(crate) const IOCB_CMD_PREADV: u16 = 7;
pub(crate) const IOCB_CMD_PWRITEV: u16 = 8;
pub(crate) const IOCB_FLAG_RESFD: u32 = 1;
pub(crate) const KIOCB_KEY: u32 = 0;
pub(crate) const AIO_RING_MAGIC: u32 = 0xa10a10a1;
pub(crate) const AIO_RING_COMPAT_FEATURES: u32 = 1;
pub(crate) const AIO_RING_HEADER: u64 = 32;
pub(crate) const IOCB_SIZE: u64 = 64;
pub(crate) const IO_EVENT_SIZE: u64 = 32;

/// `<linux/oom.h>`'s scales of how the out-of-memory killer weighs a process, which linux-raw-sys
/// does not carry.
pub(crate) const OOM_SCORE_ADJ_MIN: i32 = -1000;
pub(crate) const OOM_SCORE_ADJ_MAX: i32 = 1000;
pub(crate) const OOM_DISABLE: i32 = -17;
pub(crate) const OOM_ADJUST_MAX: i32 = 15;

/// A core file's notes of a process and its threads (`linux/elfcore.h`, kernel-internal, so not
/// in linux-raw-sys), as ARM64 lays them out: `struct elf_prstatus` with its signal (`struct
/// elf_siginfo`), its registers (`elf_gregset_t`, `user_pt_regs`) and padding made explicit.
#[derive(Clone, Copy, zerocopy::FromZeros, zerocopy::IntoBytes, zerocopy::Immutable)]
#[repr(C)]
pub(crate) struct elf_prstatus {
    pub si_signo: i32,
    pub si_code: i32,
    pub si_errno: i32,
    pub pr_cursig: i16,
    pub pad0: u16,
    pub pr_sigpend: u64,
    pub pr_sighold: u64,
    pub pr_pid: i32,
    pub pr_ppid: i32,
    pub pr_pgrp: i32,
    pub pr_sid: i32,
    /// `pr_utime`, `pr_stime`, `pr_cutime` and `pr_cstime`, each seconds then microseconds.
    pub pr_times: [[i64; 2]; 4],
    pub pr_reg: [u64; 34],
    pub pr_fpvalid: i32,
    pub pad1: u32,
}

/// `struct elf_prpsinfo`, the process's note in a core file.
#[derive(Clone, Copy, zerocopy::IntoBytes, zerocopy::Immutable)]
#[repr(C)]
pub(crate) struct elf_prpsinfo {
    pub pr_state: u8,
    pub pr_sname: u8,
    pub pr_zomb: u8,
    pub pr_nice: i8,
    pub pad0: u32,
    pub pr_flag: u64,
    pub pr_uid: u32,
    pub pr_gid: u32,
    pub pr_pid: i32,
    pub pr_ppid: i32,
    pub pr_pgrp: i32,
    pub pr_sid: i32,
    pub pr_fname: [u8; 16],
    pub pr_psargs: [u8; ELF_PRARGSZ],
}

/// The length of `pr_psargs`.
pub(crate) const ELF_PRARGSZ: usize = 80;

/// Task flags a core file's process note shows (`linux/sched.h`'s `PF_*`).
pub(crate) const PF_EXITING: u64 = 0x4;
pub(crate) const PF_FORKNOEXEC: u64 = 0x40;
pub(crate) const PF_SUPERPRIV: u64 = 0x100;
pub(crate) const PF_DUMPCORE: u64 = 0x200;
pub(crate) const PF_SIGNALED: u64 = 0x400;
pub(crate) const PF_RANDOMIZE: u64 = 0x0040_0000;

/// `coredump_filter`'s bits (`MMF_DUMP_*` less `MMF_DUMP_FILTER_SHIFT`).
pub(crate) const DUMP_ANON_PRIVATE: u32 = 1 << 0;
pub(crate) const DUMP_ANON_SHARED: u32 = 1 << 1;
pub(crate) const DUMP_MAPPED_PRIVATE: u32 = 1 << 2;
pub(crate) const DUMP_MAPPED_SHARED: u32 = 1 << 3;
pub(crate) const DUMP_ELF_HEADERS: u32 = 1 << 4;

/// `struct __kernel_timex` (`linux/timex.h`), which linux-raw-sys does not carry, with its
/// unnamed padding fields named.
#[derive(Clone, Copy, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable)]
#[repr(C)]
pub(crate) struct kernel_timex {
    pub modes: u32,
    pub pad0: u32,
    pub offset: i64,
    pub freq: i64,
    pub maxerror: i64,
    pub esterror: i64,
    pub status: i32,
    pub pad1: u32,
    pub constant: i64,
    pub precision: i64,
    pub tolerance: i64,
    pub time_sec: i64,
    pub time_usec: i64,
    pub tick: i64,
    pub ppsfreq: i64,
    pub jitter: i64,
    pub shift: i32,
    pub pad2: u32,
    pub stabil: i64,
    pub jitcnt: i64,
    pub calcnt: i64,
    pub errcnt: i64,
    pub stbcnt: i64,
    pub tai: i32,
    pub pad3: [u32; 11],
}

/// `adjtimex`'s mode bits as the kernel names them (`linux/timex.h`, kernel side).
pub(crate) const ADJ_OFFSET_SINGLESHOT: u32 = 0x0001;
pub(crate) const ADJ_SETOFFSET: u32 = 0x0100;
pub(crate) const ADJ_OFFSET_READONLY: u32 = 0x2000;
pub(crate) const ADJ_TICK: u32 = 0x4000;
pub(crate) const ADJ_ADJTIME: u32 = 0x8000;
/// The clock is not synchronised (`STA_UNSYNC`), and the state that reports (`TIME_ERROR`).
pub(crate) const STA_UNSYNC: i32 = 0x0040;
pub(crate) const TIME_ERROR: u64 = 5;
/// A never-adjusted clock's error bounds (`NTP_PHASE_LIMIT`), time constant, tick in
/// microseconds (`USER_TICK_USEC`) and tolerance (`MAXFREQ_SCALED / PPM_SCALE`).
pub(crate) const NTP_PHASE_LIMIT: i64 = 16_000_000;
pub(crate) const NTP_TIME_CONSTANT: i64 = 2;
pub(crate) const USER_TICK_USEC: i64 = 10_000;
pub(crate) const NTP_TOLERANCE: i64 = 32_768_000;

/// Linux's `USER_HZ`, the clock ticks a second that `times`, `/proc` and `AT_CLKTCK` count in,
/// and the length of one.
pub(crate) const USER_HZ: u64 = 100;
pub(crate) const NANOSECONDS_PER_TICK: u64 = wcontainers_abi::NANOSECONDS_PER_SECOND / USER_HZ;

/// The most one read or write moves (`MAX_RW_COUNT`).
pub const MAX_RW_COUNT: u64 = 0x7fff_f000;
