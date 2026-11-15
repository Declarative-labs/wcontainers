//! Docker's default seccomp profile, which the reference applies before Linux sees a call. What it
//! refuses was measured on the reference (Docker 29.9.3) by making every call with and without the
//! profile, and the argument rules by making the filtered calls with chosen arguments. Its arguments
//! are compared as full 64 bit values, as seccomp does.

use crate::Errno;
use crate::linux::general::*;
use crate::linux::net::{AF_MAX, AF_VSOCK};

/// The highest call number the profile knows; it answers anything above as a kernel without the
/// call would, so that programs fall back.
const LAST_KNOWN: u32 = __NR_futex_requeue;

/// Calls the profile refuses whatever their arguments.
const REFUSED: &[u32] = &[
    __NR_lookup_dcookie,
    __NR_umount2,
    __NR_mount,
    __NR_pivot_root,
    __NR_nfsservctl,
    __NR_quotactl,
    __NR_unshare,
    __NR_kexec_load,
    __NR_delete_module,
    __NR_clock_settime,
    __NR_settimeofday,
    __NR_add_key,
    __NR_request_key,
    __NR_keyctl,
    __NR_swapon,
    __NR_mbind,
    __NR_get_mempolicy,
    __NR_set_mempolicy,
    __NR_migrate_pages,
    __NR_move_pages,
    __NR_perf_event_open,
    __NR_open_by_handle_at,
    __NR_setns,
    __NR_kcmp,
    __NR_bpf,
    __NR_userfaultfd,
    __NR_kexec_file_load,
    __NR_io_uring_setup,
    __NR_io_uring_enter,
    __NR_io_uring_register,
    __NR_open_tree,
    __NR_fsconfig,
    __NR_pidfd_getfd,
    __NR_process_madvise,
    __NR_mount_setattr,
    __NR_quotactl_fd,
    __NR_set_mempolicy_home_node,
];

/// The personalities the profile lets through; the last only asks.
const PERSONALITIES: [u64; 5] = [0, 8, 0x20000, 0x20008, 0xffff_ffff];

/// The `clone` flags that make namespaces, which need powers a default container lacks.
const NAMESPACE_FLAGS: u64 = (CLONE_NEWNS
    | CLONE_NEWCGROUP
    | CLONE_NEWUTS
    | CLONE_NEWIPC
    | CLONE_NEWUSER
    | CLONE_NEWPID
    | CLONE_NEWNET) as u64;

/// What the profile answers in place of Linux, if it stops the call. `known` says whether Linux has a
/// call with this number.
#[allow(non_upper_case_globals, reason = "Linux's names for call numbers")]
pub(crate) fn docker_default(number: u32, arguments: &[u64; 6], known: bool) -> Option<Errno> {
    if number > LAST_KNOWN || number == __NR_clone3 {
        return Some(Errno::ENOSYS);
    }
    let refused = match number {
        _ if !known => true,
        __NR_personality => !PERSONALITIES.contains(&arguments[0]),
        __NR_socket => arguments[0] == u64::from(AF_VSOCK) || arguments[0] >= u64::from(AF_MAX),
        __NR_clone => arguments[0] & NAMESPACE_FLAGS != 0,
        _ => REFUSED.contains(&number),
    };
    refused.then_some(Errno::EPERM)
}
