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
