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
