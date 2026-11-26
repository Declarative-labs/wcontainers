//! What a stub holds at fixed numbers, and how its threads are made and stopped.

use linux_raw_sys::general as linux;

/// The stub's socket's fixed descriptor number, above the few the zygote holds when it forks.
pub const SOCKET_FD: u32 = 8;
/// The memory memfd's fixed descriptor number.
pub const MEMORY_FD: u32 = 9;
/// The number a shared block's memfd gets in the stub while it is mapped: the lowest, as the stub holds only the two
/// above.
pub const BLOCK_FD: u32 = 0;

/// How a stub's threads are made: sharing everything a thread of one process shares, the new thread's exit-clear word
/// written before the call returns and cleared, with a wake, at its end.
pub const THREAD_FLAGS: u32 = linux::CLONE_VM
    | linux::CLONE_FS
    | linux::CLONE_FILES
    | linux::CLONE_SIGHAND
    | linux::CLONE_THREAD
    | linux::CLONE_SYSVSEM
    | linux::CLONE_PARENT_SETTID
    | linux::CLONE_CHILD_CLEARTID;

/// The signal that stops a stub's thread: a standard one, so stops sent while one is pending are that one, and a
/// stub holds no C library that claims it.
pub const STOP_SIGNAL: u32 = linux::SIGUSR1;

/// [`STOP_SIGNAL`] alone, as a kernel signal set.
pub const STOP_SET: u64 = 1 << (STOP_SIGNAL - 1);
