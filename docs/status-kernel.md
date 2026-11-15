# Kernel status

Plan: docs/phase-1-kernel.md. Branch: phase-1-kernel.

## Where things stand

K0 is done. K1 (one process) is built; five runs so far. Programs start from the file system (with
their own loader and `#!` scripts) with the credentials Linux gives them (set-user and set-group bits,
file capabilities, secure mode, dumpability), a program's own seccomp filters, rseq, membarrier,
memory calls with page protection, limits and faults, the
in-memory file systems (tmpfs with extended attributes and seals, memory files, `/proc`, `/dev`), file
descriptors, time, randomness, ids and powers, signals a process sends itself, and Docker's default
seccomp profile. Single busybox programs match the reference. Layer 1 covers every call built. K1 is not
finished: layer 1 has passed one 20000-sequence run (seed 48, third run); later 20000 runs still find
mismatches within minutes, and the one-million bar has not been run.

How to test everything: `nix develop -c cargo test -p wcontainers-kernel -p wcontainers-kernel-test`.
Known failures are `#[ignore]`d with their reason; see them with `-- --include-ignored`.
Needs Docker running; the reference uses alpine:3.22. Containers are named `wc-kernel-*` and removed.

Layer 1 long run (one command, for the orchestrator):
`LAYER1_CASES=1000000 LAYER1_SEED=<n> nix develop -c cargo test -p wcontainers-kernel-test --test layer1`
`LAYER1_CASES` is the number of sequences (default 64), `LAYER1_SEED` the seed (default 0). A failure is
shrunk and printed call by call with the seed. A sequence that runs past 60 s in the reference is
killed there (SIGKILL), which our side reports for a wait that never ends.

## What exists

- Kernel API: `Kernel::new`, `add_file(path, Attributes, NewFile)` to fill the root file system before
  the start (as unpacking an image does), `start(cpu, host, &Start { path, arguments, environment,
  working_directory, hostname, groups })`, `next`, `exit`, `event`, `counters`, `root_files()` (the
  root file system's files now, for comparing what a container left behind), `system_calls()`.
- File layer (src/fs): inodes shared by `Rc`, directories listing newest first with tmpfs's offsets
  (measured in the reference: `.`=1, `..`= newest entry's offset, entries from 3, end 2^31-1), mounts,
  path walk with symlinks (40 at most), `..` across mounts, `d_path` style paths, permission checks
  (`generic_permission` with DAC_OVERRIDE and DAC_READ_SEARCH), sticky directories, set-group
  directories, the set-user and set-group bits dropped on write, truncate and chown.
- Mounts as `docker run` makes them: `/` (tmpfs here, see departures), `/proc`, `/dev` (tmpfs with
  null, zero, full, random, urandom, tty and Docker's links), `/dev/pts` (devpts, with ptmx), `/dev/shm`
  (tmpfs), `/dev/mqueue` as a plain directory. fd 0 is `/dev/null` opened read-write, fds 1 and 2 are
  pipes the host reads (as Docker's).
- `/proc`: self, `<pid>/` with cmdline, comm, environ, auxv, maps, stat, statm, status, mounts, limits,
  cgroup, exe, cwd, root, fd/ (magic links); cpuinfo, meminfo, uptime, loadavg, stat, version,
  filesystems, mounts, sys/kernel/{hostname, osrelease, ostype, pid_max, ngroups_max, cap_last_cap,
  random/boot_id, random/uuid}, sys/fs/nr_open, sys/vm/overcommit_memory. Bytes are made at open.
- Memory (src/memory.rs): ranges with what the program asked for, top-down placement below
  end-128 MiB, reused ranges cleared before use (never-used ones are not touched), brk, mmap (anonymous,
  private file copies, `/dev/zero`), munmap, mprotect, madvise (DONTNEED restores zero or file bytes),
  mremap, msync. Pages wholly past a file's end are kept inaccessible and touching one is SIGBUS. The
  stack grows on a fault below it up to 8 MiB, keeping Linux's 1 MiB guard gap. Faults: unmapped or
  forbidden is SIGSEGV, past a file's end SIGBUS; with no handler the process dies of it.
- Loader: ELF with PT_INTERP (the named loader is placed where mmap would put it, AT_BASE set), ET_DYN
  at 2/3 of the address space, `#!` scripts (one argument, 4 levels, 256 byte line), execve, close on
  exec, handlers reset. Auxiliary vector as the reference's, in its order, except AT_SYSINFO_EHDR and
  AT_MINSIGSTKSZ (counted and warned at each start; they come with signal frames in K2).
- Credentials: uid/euid/suid/fsuid and gids, supplementary groups (Docker's: root's group, then
  `/etc/group` entries naming root, computed by the test driver as Docker does), Docker's 14 powers,
  Linux's rules for changing ids and what that does to powers and to `dumpable`, capget/capset.
- Time: clocks from the host, coarse clocks at 1 ms resolution (measured), sleeps park the thread and
  the kernel goes idle until the time; the test host jumps its clocks instead of waiting.
- Signal actions and masks are kept; see the signals bullet below for sending. A fault with a handler
  set is counted "signal handler not run" and kills.
- Test crate: `buffer.rs` (a CPU side with no instructions, for layer 1), `image.rs` (docker export of
  the image into `add_file`), `busybox.rs` (one command in both runners, comparing output, exit code and
  the files changed relative to the image), `layer1.rs` and `guest/layer1.c` (the runner: reads
  sequences from stdin, runs each in a fresh child, reports each call's value and the arena bytes it
  changed).
- Signals within one process (src/signal.rs): a signal sent to itself is dropped if ignored, kept
  pending while blocked (`rt_sigpending` shows it, `SIG_IGN` flushes it), and acted on when it is let
  through on the way back from any call. The container's first process ignores every signal it has no
  handler for, as Linux protects a pid namespace's init (`SIGNAL_UNKILLABLE`); faults still kill it.
  A signal that would need its handler run, or would stop the process, is refused with ENOSYS and
  counted ("signal handler not run", "process stop not implemented") where it would become deliverable
  (`kill`, or `rt_sigprocmask` unblocking it).
- Resource limits enforced: RLIMIT_FSIZE (writes shortened, EFBIG, SIGXFSZ; truncate and fallocate),
  RLIMIT_AS and RLIMIT_DATA (mmap, brk, mprotect to writable, mremap growth; Linux's `may_expand_vm`),
  RLIMIT_STACK (stack growth), RLIMIT_MEMLOCK (mlock, MAP_LOCKED, MCL_FUTURE), RLIMIT_NOFILE. RLIMIT_CPU
  is kept but not enforced (needs CPU time and timers, K3).
- Docker's default seccomp profile (src/seccomp.rs), measured on the reference: 37 calls refused with
  EPERM, numbers Linux does not have up to the profile's last known call (futex_requeue, 456) EPERM,
  above it ENOSYS, clone3 ENOSYS, and the argument rules for personality, socket (AF_VSOCK and families
  from AF_MAX refused) and clone (namespace flags refused), compared as 64 bit values.
- `/proc` follows the reference's rules: a process directory refuses writes and changes (EPERM), a
  process may always use its own `fd` directory (mode 0500), `/proc/sys` is Docker's read-only mount
  (EROFS, a mount root), owners follow Linux's `task_dump_owner` (root while not dumpable, except
  world-readable directories), link counts (9 for a process directory, 1 for sysctl directories), and
  an `fd` directory's size is its number of open descriptors. Removing or making names there gives the
  path and type errors first, then EPERM.
- File locks with `flock` between open files (a refused change loses the old lock, as on Linux). A
  conflicting wait parks for ever: only this process holds locks.
- Extended attributes (src/syscall/xattr.rs) on tmpfs inodes, as Linux's xattr layer and tmpfs's
  handlers give them: `user.` (regular files and directories only, sticky directories), `trusted.`
  (needs CAP_SYS_ADMIN: EPERM to write, ENODATA to read, hidden from lists), `security.` (needs
  CAP_SYS_ADMIN to write), listed greatest name first (measured). `security.capability` is converted
  as `cap_convert_nscap` and `cap_inode_getsecurity` do and is removed by a write, truncate or chown
  when not empty. POSIX ACL names are checked and refused with EOPNOTSUPP: the reference's tmpfs has no
  ACLs (measured). Other file systems refuse with EOPNOTSUPP; a process directory is immutable (EPERM).
- execve applies set-user and set-group bits (not on `nosuid` mounts or with no_new_privs) and file
  capabilities, with root's powers, as Linux's `bprm_fill_uid` and `cap_bprm_creds_from_file` (for a
  process nobody traces); AT_SECURE, dumpability (unreadable programs too), the personality bits a
  gain of privilege clears, and the stack limit of a program in secure mode follow.
- memfd_create (src/syscall/memfd.rs) makes unlinked tmpfs files on an internal tmpfs, with
  `MFD_ALLOW_SEALING`, `MFD_NOEXEC_SEAL` and `MFD_EXEC` (`vm.memfd_noexec` 0). Seals (F_ADD_SEALS,
  F_GET_SEALS) on every tmpfs file, enforced by write (page by page, after the times change, as tmpfs),
  truncate, open with O_TRUNC, fallocate and chmod.
- sched_setattr and sched_getattr in the order of `__sched_setscheduler` (shared with
  sched_setscheduler), with custom time slices (clamped to 0.1 to 100 ms); no utilisation clamps
  (EOPNOTSUPP, as the reference's kernel); sched_rr_get_interval in whole ticks.
- A program's own seccomp filters (src/seccomp.rs, src/syscall/seccomp.rs): classic BPF programs
  checked as Linux's `bpf_check_classic`, `check_load_and_stores` and `seccomp_check_filter` do
  (every refusal EINVAL), and run on each call of the thread with Docker's profile as the oldest
  filter; the lowest action wins as in Linux (ties to the newest). ERRNO (at most 4095), ALLOW, LOG,
  TRACE and USER_NOTIF (ENOSYS: no tracer or listener can exist), TRAP (SIGSYS; a handler cannot run
  yet, counted), KILL_THREAD, KILL_PROCESS and unknown actions (the process dies of SIGSYS, whatever
  its handler). Needs no_new_privs (Docker's root lacks CAP_SYS_ADMIN, so EACCES); the instruction
  limit counts Docker's profile as 1032 instructions (measured). Filters are per thread and kept
  across execve; `SECCOMP_FILTER_FLAG_TSYNC` checks and copies to other threads as Linux. The
  instruction pointer is read from the registers only for filters that load it.
  `PR_GET_SECCOMP` and `/proc/<pid>/status` say filter mode with Docker's filter plus the thread's
  own; `NoNewPrivs` there is now real.
- rseq: registration checks in the order of Linux 7.0 (measured: an area of other than 32 bytes must
  be 64 byte aligned), writes the fields Linux writes (CPU 0, no critical section, node 0, concurrency
  id 0) and unregistration writes them back; cleared at execve. membarrier: Linux's checks and
  registrations (per address space); the barriers need nothing (one CPU, threads take turns).
- Protection keys as on the reference's CPU, which has no permission overlays: only key 0, allocated
  from the start; pkey_alloc is ENOSPC, pkey_free can free key 0, pkey_mprotect is mprotect with
  Linux's key check. map_shadow_stack is EOPNOTSUPP (no guarded control stacks, as the reference).
- Calls whose first step in Linux is to check a power no container process can hold (acct, vhangup,
  swapoff, init_module, finit_module, reboot, syslog with `dmesg_restrict` 1 as measured, fsopen,
  fspick, fsmount, move_mount): EPERM. If a process held the power the call would be counted and
  ENOSYS, which cannot happen in a default container.
- execveat: by path, from a directory descriptor, or a descriptor itself (`AT_EMPTY_PATH`), named
  `/dev/fd/<n>[/path]` as Linux names it; a script through a descriptor closed at exec is ENOENT;
  `AT_SYMLINK_NOFOLLOW` and `AT_EXECVE_CHECK` (a check only). Flags are checked before the path
  (measured). A process is named after the file asked for (a script, not its interpreter), and when
  run from a descriptor after the file run, as Linux 7.0.
- `..` in `/proc` reaches the directory above even after the walk that made it is gone (proc
  directories hold their parent).
- mlock fails with ENOMEM after locking when a page cannot be brought in (no access, or wholly past
  its file's end), as Linux's `__mm_populate`.
- process_vm_readv and process_vm_writev (fifth run) on the caller's own threads, in the order of
  Linux's `process_vm_rw`: the other side is reached as `get_user_pages` reaches it (by the mapping's
  own read or write right, never past a file's end, never growing a stack), a page at a time, and the
  caller's side faults part way as `copy_to_user` and `copy_from_user`. Overlapping ranges end up as
  ARM64's `copy_template.S` leaves them (its loads and stores are modelled; 1620 overlap cases match
  the reference). Another process's memory is counted "system call not implemented" and ENOSYS; no
  other process can exist in K1.
- cachestat (fifth run): tmpfs pages written or allocated, as the reference counts them (never dirty,
  under writeback or evicted, measured); devices, `/proc`, directories and pipes have none. A file that
  has been mapped is counted "page residency not tracked", since its faults bring in pages here unseen.
- The zero devices clear buffers as ARM64's `clear_user` does (eight bytes at a time; `/dev/full`
  retries from where it stopped, measured), with no limit on the length.


## System calls

Done (181): acct, brk, cachestat, capget, capset, chdir, chroot, clock_getres, clock_gettime, clock_nanosleep,
close, close_range, copy_file_range, dup, dup3, execve, execveat, exit, exit_group, faccessat,
faccessat2, fadvise64, fallocate, fchdir, fchmod, fchmodat, fchmodat2, fchown, fchownat, fcntl,
fdatasync, fgetxattr, finit_module, flistxattr, flock, fremovexattr, fsetxattr, fsmount, fsopen, fspick,
fstat, fstatfs, fsync, ftruncate, get_robust_list, getcpu, getcwd, getdents64, getegid, geteuid, getgid,
getgroups, getpgid, getpid, getppid, getpriority, getrandom, getresgid, getresuid, getrlimit, getrusage,
getsid, gettid, gettimeofday, getuid, getxattr, init_module, ioctl, ioprio_get, ioprio_set, kill,
lgetxattr, linkat, listxattr, llistxattr, lremovexattr, lseek, lsetxattr, madvise, map_shadow_stack,
membarrier, memfd_create, mincore, mkdirat, mknodat, mlock, mlock2, mlockall, mmap, move_mount,
mprotect, mremap, msync, munlock, munlockall, munmap, nanosleep, newfstatat, openat, openat2,
personality, pkey_alloc, pkey_free, pkey_mprotect, ppoll, prctl, pread64, preadv, preadv2, prlimit64,
process_vm_readv, process_vm_writev, pselect6, pwrite64, pwritev, pwritev2, read, readahead, readlinkat, readv, reboot, removexattr,
renameat, renameat2, rseq, rt_sigaction, rt_sigpending, rt_sigprocmask, sched_get_priority_max,
sched_get_priority_min, sched_getaffinity, sched_getattr, sched_getparam, sched_getscheduler,
sched_rr_get_interval, sched_setaffinity, sched_setattr, sched_setparam, sched_setscheduler,
sched_yield, seccomp, sendfile, set_robust_list, set_tid_address, setdomainname, setfsgid, setfsuid,
setgid, setgroups, sethostname, setpgid, setpriority, setregid, setresgid, setresuid, setreuid,
setrlimit, setsid, setuid, setxattr, sigaltstack, statfs, statx, swapoff, symlinkat, sync,
sync_file_range, syncfs, sysinfo, syslog, tgkill, times, tkill, truncate, umask, uname, unlinkat,
utimensat, vhangup, write, writev.

Partial (inside the list above):
- fcntl: no record locks (F_GETLK/SETLK/OFD: counted, ENOSYS); no leases, owners, signals, seals.
- ioctl: FIOCLEX, FIONCLEX, FIONBIO; terminal requests give ENOTTY (no terminals yet); others ENOTTY,
  counted when the file is a device.
- mmap: no shared file mappings (counted, ENODEV); MAP_HUGETLB counted, ENOMEM. MAP_SHARED anonymous
  is exact while there is no fork.
- openat, openat2: O_TMPFILE counted, EOPNOTSUPP; named pipes counted, ENXIO; `/dev/ptmx` counted,
  ENODEV. openat2's restricted lookups (RESOLVE_NO_XDEV, NO_MAGICLINKS, NO_SYMLINKS, BENEATH, IN_ROOT)
  are counted and refused with ENOSYS, as by a kernel without them; RESOLVE_CACHED is taken (everything
  is in memory).
- kill, tkill, tgkill: only to the caller itself (see signals above); others ESRCH.
- execve, execveat: only from a process with one thread.
- rt_sigaction, rt_sigprocmask, sigaltstack: handlers recorded, not run (K2).
- getrusage, times, CPU-time clocks: CPU time is the time since the container started.
- fallocate: plain and keep-size allocation and punching holes (what tmpfs has); others EOPNOTSUPP.
- prctl: SET/GET_NAME, GET/SET_DUMPABLE, GET/SET_KEEPCAPS, CAPBSET_READ, GET/SET_NO_NEW_PRIVS,
  GET_SECCOMP, GET_CHILD_SUBREAPER; others counted, EINVAL.
- statfs: block and inode counts are made up (64 MiB like Docker's tmpfs).
- ppoll, pselect6: every file that exists so far is always ready (Linux's default poll mask; the host's
  pipes always have room); a wait with nothing ready sleeps until its timeout, or for ever (until K2's
  signals). The signal mask they take is read, not applied.
- flock: a wait for a conflicting lock never ends (one process).
- sched_setscheduler, sched_setparam: kept per thread with Linux's checks for an unprivileged caller
  (no CAP_SYS_NICE, RLIMIT_RTPRIO and RLIMIT_NICE zero); nothing is scheduled by them yet.
  `/proc/<pid>/stat` still shows nice 0.
- mlock family: lock state and limits are kept; pages are not brought in (nothing to bring).
- brk: RLIMIT_DATA is checked on the heap alone, not heap plus data segment as Linux does.
- memfd_create: MFD_HUGETLB counted ("memory call flag not implemented") and refused with ENOSYS, as by
  a kernel without hugetlbfs; the reference makes such files. Shared mappings of memory files are
  shared file mappings, not built (counted, ENODEV), which is most of what programs use them for.
- mincore: every mapped page is reported in memory, counted "page residency not tracked" (the CPU side
  does not say which pages a program touched).
- Extended attributes: tmpfs's space limit for them (ENOSPC) is not modelled.
- ioprio_get, ioprio_set: the value is kept per thread; nothing does I/O by it. A group or user is
  answered with the implied class and level for no class (measured).
- seccomp: no listener (`SECCOMP_FILTER_FLAG_NEW_LISTENER` counted "seccomp listener not
  implemented", EINVAL as a kernel without listeners); TRAP with a handler set kills (handlers, K2).
- rseq: a critical section is never aborted yet; signal delivery (K2) and preemption between threads
  (K3) must do that. The time slice extension flag is counted and refused with EINVAL (not built);
  the reference takes it.
- process_vm_readv, process_vm_writev: only the caller's own process (the only one in K1).
- cachestat: pages a mapped file's faults bring in are not known (counted, see above).
- sigaltstack: refuses a change while the caller runs on its alternate stack (Linux's `on_sig_stack`,
  from the thread's stack pointer); handlers still need K2.

Refused by Docker's profile as in the reference (51; never reach the kernel there, so nothing to
build): lookup_dcookie, umount2, mount, pivot_root, nfsservctl, quotactl, unshare, kexec_load,
delete_module, clock_settime, settimeofday, add_key, request_key, keyctl, swapon, mbind, get_mempolicy,
set_mempolicy, migrate_pages, move_pages, perf_event_open, open_by_handle_at, setns, kcmp, bpf,
userfaultfd, kexec_file_load, io_uring_setup, io_uring_enter, io_uring_register, open_tree, fsconfig,
clone3, pidfd_getfd, process_madvise, mount_setattr, quotactl_fd, set_mempolicy_home_node, statmount,
listmount, lsm_get_self_attr, lsm_set_self_attr, lsm_list_modules, mseal, setxattrat, getxattrat,
listxattrat, removexattrat, open_tree_attr, file_getattr, file_setattr.

Missing (93, each counted "system call not implemented" and ENOSYS): io_setup, io_destroy, io_submit,
io_cancel, io_getevents, eventfd2, epoll_create1, epoll_ctl, epoll_pwait, inotify_init1,
inotify_add_watch, inotify_rm_watch, pipe2, signalfd4, vmsplice, splice, tee, timerfd_create,
timerfd_settime, timerfd_gettime, waitid, futex, getitimer, setitimer, timer_create, timer_gettime,
timer_getoverrun, timer_settime, timer_delete, ptrace, restart_syscall, rt_sigsuspend, rt_sigtimedwait,
rt_sigqueueinfo, rt_sigreturn, adjtimex, mq_open, mq_unlink, mq_timedsend, mq_timedreceive, mq_notify,
mq_getsetattr, msgget, msgctl, msgrcv, msgsnd, semget, semctl, semtimedop, semop, shmget, shmctl, shmat,
shmdt, socket, socketpair, bind, listen, accept, connect, getsockname, getpeername, sendto, recvfrom,
setsockopt, getsockopt, shutdown, sendmsg, recvmsg, clone, remap_file_pages, rt_tgsigqueueinfo, accept4,
recvmmsg, wait4, fanotify_init, fanotify_mark, name_to_handle_at, clock_adjtime, sendmmsg,
io_pgetevents, pidfd_send_signal, pidfd_open, epoll_pwait2,
landlock_create_ruleset, landlock_add_rule, landlock_restrict_self, memfd_secret, process_mrelease,
futex_waitv, futex_wake, futex_wait, futex_requeue. Most belong to K2 (processes, pipes,
signals, futex), K3 (timers) or K5 (sockets, epoll).

## Tests (last run 2026-11-15: 33 passed, 5 ignored known failures)

All pass except the ignored known failures.
- processvm (new, fifth run): guest/processvm.c checks process_vm_readv and process_vm_writev's
  refusals in order, whole copies by process and thread number, partial copies at pages with no
  access, read-only, write-only and execute-only pages, a gap, past a mapped file's end and below the
  stack, faults on the caller's side, and 1620 overlapping copies (sizes 3 to 1000, distances -70 to
  100, three alignments, both directions). Same output.
- cachestat (new, fifth run): guest/cachestat.c on written, grown, allocated, read, shrunk and
  punched tmpfs files, ranges (wrapping, past the end), devices, `/proc`, a directory, an `O_PATH`
  descriptor, a memory file, a pipe, bad pointers and flags, and the write-permission rule after
  giving up root. Same output. Known failure (ignored): a file read through a private mapping.
- execveat (new, fourth run): guest/execveat.c checks execveat's refusals and AT_EXECVE_CHECK, then
  runs itself by path, from a directory descriptor, from its own descriptor and through a script's
  descriptor (closed at exec first: ENOENT), printing arguments, name and program each time. Same
  output. (Reading AT_EXECFN with getauxval in a program run from a descriptor stopped the program
  in both runners alike; the test prints `/proc/self/exe` instead.)
- filter (new, fourth run): guest/filter.c in nine runs, each compared with the reference: the
  seccomp operations and their checks, 26 kinds of refused program, accepted ones, arithmetic and
  jumps on the call's data (157 cases), the architecture, high halves, the instruction pointer,
  stacked filters with Docker's profile, then ending by trap (also with SIGSYS ignored and blocked),
  kill process, kill thread, an unknown action, a division by zero, execve (the filter stays), and
  the instruction limit (ENOMEM). Same output and exit status (159 for SIGSYS).
- busybox: 85 single commands of the reference image (added this run: nice, renice, setpriv to another
  user then id, linux32 and linux64 uname, sysctl read and write, pwdx, ionice (four ways),
  fallocate, install, readahead, fsync, cmp, xxd): same stdout, stderr, exit code and files changed as the reference.
- xattr (new): guest/xattr.c sets, gets, lists and removes every kind of name on tmpfs, devices, `/proc`,
  devpts and pipes, ACLs, file capabilities in both forms, their removal by write, chown and truncate;
  then execs copies of itself: plain, set-group, set-user, after giving up root, unreadable, with file
  capabilities (granted, too many), set-user-root with capabilities, set-user-root. Same output.
- memfd (new): guest/memfd.c: names, flags, modes, paths, seals and their effects, reopening through
  `/proc/self/fd`, and reads of the zero devices ending at an unmapped page. Same output.
- schedattr (new): guest/schedattr.c: sched_getattr sizes, sched_setattr checks, nice, policies,
  deadline, flags, slices, clamps; sched_rr_get_interval; mincore's values. Slices are shown relative to
  the default (the CPU count differs). Same output.
- memory (10 cases), script, one_cpu, dispatch, loader, faults, standin, boundary, layouts: as before.
- limits (new): guest/limits.c as the container's first process in both: RLIMIT_FSIZE on write,
  pwrite, ftruncate and fallocate with SIGXFSZ ignored (first process) and blocked (pending);
  kill/tkill of itself with SIGTERM, SIGKILL and SIGSTOP (all ignored by the first process), kill(0),
  kill(-1), unknown signals, pending signals, flushing by SIG_IGN; RLIMIT_AS, RLIMIT_DATA (mmap,
  mprotect, mremap, sbrk); RLIMIT_STACK (stack growth past the limit is SIGSEGV).
- seccomp (new): guest/seccomp.c makes the profile's refused calls and argument rules; same output.
- procaccess (new): guest/procaccess.c checks access, rmdir, unlink, mkdir and create on `/proc`
  entries as root and after setreuid(0, 1000); same output.
- dispatch: the seccomp gap case is no longer a known failure.
- layer1: see the layer 1 section.

Known failures (ignored, with the reason in the attribute):
- busybox root_file_system: `ls -la /`, `ls -ln /etc`, `stat /etc`, `find /etc`, `du /etc/apk` show
  the root file system's own kind (overlayfs over ext4 in the reference, tmpfs here until K4).
- The two unicorn alignment differences from K0, unchanged.
- memfd huge_page_memory_files: hugetlbfs is not built (see Partial).
- cachestat mapped_file_pages: the page a private mapping's read fault brings into a tmpfs file is
  counted by the reference and not known here (counted "page residency not tracked").

## Layer 1

Fifth run: added process_vm_readv and process_vm_writev (the caller itself, by process number, 0 or a
missing one; segment lists from the vector pool, arbitrary arena bytes or bad addresses; never from the
play area, whose contents the comparison does not know) and cachestat. A byte copied by them from a
masked byte is masked too; the masks now also cover the play area. Directories that outlive a sequence:
the reference's `/dev/shm` and `/dev` keep tmpfs's offset counters across sequences (each sequence
makes `/dev/shm/w`, and the devices are remade; measured: a million mkdir and rmdir leave the next
entry at offset 1000003), which a fresh kernel cannot share. The reference's runner now asks, after
each getdents64 and lseek, whether the descriptor is on one of them (by `name_to_handle_at`, which
every generated filter lets through and the generator never makes); such a call's value and the
listing it wrote are not compared, on both sides. Found by seed 62 after under 1500 sequences.

Fourth run: seccomp (three times in the list of kinds, as most of its sequences need several filters
to show anything) with four filters per sequence made from the generated values: each first lets
through write and exit_group (which the reference's runner reports with) and every call of the
sequence that names a process (the runners' numbers differ), then up to eight instructions, one in
sixteen of a kind Linux refuses, then allows. Half the sequences start with
`prctl(PR_SET_NO_NEW_PRIVS, 1)`; prctl now also makes PR_SET_SECCOMP, PR_GET_SECCOMP and
PR_SET_NO_NEW_PRIVS with 1. Also added: rseq, membarrier, pkey_mprotect, pkey_alloc, pkey_free,
map_shadow_stack, the eleven calls refused for powers, O_DIRECT and O_NOATIME in open flags. The
generated values grew from 72 to 125, so seeds before the fourth run make different sequences now.
Not generated: seccomp listeners and the rseq time slice flag (not built), filter loads of the
instruction pointer (the runners' differ), rseq areas where other calls write. Masked: the CPU
fields of the rseq areas (the reference rewrites them whenever its caller changes CPU).

Calls covered (157 kinds in the third run, with the xattr calls, memfd_create, F_ADD_SEALS and F_GET_SEALS,
sched_setattr, sched_getattr, sched_rr_get_interval, mincore, ioprio_get and ioprio_set added this
run; ioprio not by user, which names the reference's runner too): every call in "Done"
except brk (its values are addresses that differ by layout; it is
tested by guest/memory.c and guest/limits.c), plus mkdirat and chdir at the start of every sequence.
Arguments come from pools with bad pointers, bad descriptors, huge counts, unknown flags and invalid
values. 1 to 15 calls after the two setup calls. Everything happens under `/dev/shm/w` (tmpfs in both),
the devices and `/proc`; never on the root file system. After each sequence the runner puts back what a
sequence can change outside its directory (`/dev/shm` and the three devices).

How the runner works now: the reference child reports into a pipe its parent drains; the parent sends
the whole report with a length, then the wait status, so a child that dies part way through a report
cannot desynchronise the stream (the cause of the earlier stalls: a child killed mid-record by its own
10 s alarm). The child first makes itself a session leader, as our caller (the container's first
process) is. An argument SELF is replaced on each side by the caller's own process number, so kill,
tkill, tgkill, setpgid, getpgid, getsid, prlimit64, get_robust_list and the sched_* calls can name their
own process.

Masked on both sides: inode and device numbers and times in stat and statx, d_ino in getdents64,
statfs counts and fsid, the values of clock_gettime, gettimeofday, times, sysinfo and getrusage,
getrandom's bytes, uname's release and version, the CPU set and CPU number (one CPU is a declared
difference), the time ppoll and pselect6 write back, process numbers (the reference's caller is a child
of its runner), set_tid_address's value, getcwd inside `/proc/<pid>` (it names the process number),
and the address mmap or a moving mremap returns. Added this run: the link count of `/proc` itself
(Linux counts every process on the machine, and the reference shares its machine), mincore's vector
(residency), the time sched_rr_get_interval writes and sched_getattr's runtime (both depend on the CPU
count).

Not generated, and why: named pipes (opening one waits; pipes are K2); descriptors 0 to 2 (the
reference's caller reports through them, and fd 0 differs); process 1 as an argument and the caller's
own group negated (our caller is process 1, so -1 would mean every process); memory lengths above
256 MiB (our programs' own mappings lie higher in a 4 GiB space); ppoll and pselect6 without a timeout;
signals whose default action ends or stops a process, and rt_sigaction on the signals kill sends (our
caller is the container's first process, which Linux keeps from such signals, theirs is not; handlers
need K2); RLIMIT_CPU and RLIMIT_FSIZE limits that can be reached (the same reason, and CPU time depends
on the machine's speed); setpriority and getpriority by user (they reach the reference's runner,
which has no counterpart here); flock waits (they
would wait for ever); openat2 restricted lookups (not built) and RESOLVE_CACHED (its result in Linux
depends on what is cached); huge page memory files (not built); alternate signal stacks that could hold
either caller's stack pointer (the two differ; the stacks are now chosen from a pool in the play area). Process limits that can be
reached (Linux counts the user's processes on the whole machine, which the reference shares with other
layer 1 runs; found by seed 48 while seed 53 ran beside it). Stack limits above 1 GiB:
in our 4 GiB address space nothing lies between the stack and the play area, so the stack could grow
down into it, which the reference's layout never allows.

Cases run in the fourth run: seed 54: 3000 passed after the O_DIRECT fix (97 s, load 14 to 24), then
20000 failed after 430 s (ioprio kept in 16 bits by Linux; fixed). Seeds 55 and 56: 1000 each passed
(several times while seccomp was added to the generator). Seed 57: 20000 failed after 180 s (mlock
of a PROT_NONE range is ENOMEM; fixed), then 3000 failed (descriptor links in `/proc` have size 64;
fixed), then 3000 passed (86 s, load 6 to 10). Seed 58: 20000 failed after 155 s (`..` from
`/proc/self/fd` must reach `/proc/<pid>`; fixed), then 3000 passed (93 s). Seed 59: 2000 failed
(execveat checks its flags before its path; fixed), then 2000 passed (61 s). Seed 60: 20000 failed
after 83 s (a descriptor link's mode in `/proc` has execute only with read or write, so an `O_PATH`
one has none; fixed), then 3000 passed (85 s, load 16 to 23). Seed 61: 20000 failed after 526 s,
the longest yet (load 6 to 80): utimensat with no path refuses an `O_PATH` descriptor (EBADF); fixed,
then 3000 passed (93 s, load 11 to 30).

Cases run in the third run (earlier ones are in git history): seed 45: 3000 passed; seed 47: 3000
passed; seed 49: 3000 passed (after fixes); seed 50: 3000 passed; seed 51: 3000 passed;
seed 52: 3000 passed (90 s, load 9 to 17); seed 53: 3000 passed (90 s, load 10 to 16). After the
last fixes seed 48 passed 20000 sequences: the first 20000-sequence pass (599 s, load 4 to 42). Seed
54's 20000 run then failed twice within 100 to 150 s: F_SETFL with O_DIRECT on a device (EINVAL; only
tmpfs's regular files take direct I/O; O_NOATIME needs the owner or CAP_FOWNER), and SEEK_DATA over
pages fallocate reserved (tmpfs treats pages not yet written as holes). Both fixed; seed 54 then passed
3000 (90 s, load 25 to 52). Not yet checked: open with O_DIRECT on a file without direct I/O (Linux
refuses with EINVAL in `do_dentry_open`; here it is taken). 20000-sequence runs: seed 44 (twice), 46 and 48 (four
times) each found a mismatch within 1 to 5 minutes, which was fixed: madvise MADV_REMOVE on shared
anonymous memory, EROFS before the name lookup in unlink and rename, `/proc`'s link count (now masked,
see above), reads of `/dev/full` into a buffer ending at an unmapped page (ARM64's `clear_user`), rename
onto "." with RENAME_NOREPLACE (EEXIST), sendfile's order (offset first, written back always),
sigaltstack while on the stack (layer 1's stacks now from a pool), ioprio_get for a group, renaming a
`/proc` directory below itself (EINVAL), access checks walking with the checked credentials, chown
naming neither owner nor group in `/proc`, fsync on `/proc` files (EINVAL: no sync operation), and
seeking `/proc` directories and `/proc/sys` files (Linux's `generic_file_llseek` with size 0), opening
the own `fd` directory after the ids change (open now uses `/proc`'s permission rules), and
(a layout difference, excluded) a stack limit large enough for our stack to grow to the play area. Seeds 50
and 51 (after the fsync fix): 3000 each passed. One was the runner's fault: a descriptor on `/dev/shm` and
".." reach `/dev` and `/`, whose mode a sequence could change for the rest of the run; the runner now
puts back their owner, mode, attributes and pool names after each sequence. No run stalled.

## Measurements

All on this shared Mac; noisy, not benchmarks.
- Fourth run, layer 1 with 175 call kinds: 3000 sequences, seed 57: 85.6 s (load 5.8 to 9.5); 1000
  sequences, seed 56: 30 s (load 5 to 14).
- Fourth run, the filter test (nine programs in both runners): 2.0 s (load about 10).
- Third run, layer 1 with 171 call kinds: 3000 sequences, seed 45: 86 s (load 11 to 17); seed 47:
  85 s (load 7 to 11); seed 49: 88 s (load 28 to 49).
- Third run, layer 1 20000 sequences, seed 48: 599 s (load 4 to 42).
- Third run, busybox system test (one test, about 30 commands in both runners): 10.8 s (load about 10).
- Layer 1, 3000 sequences, seed 32, all call kinds: 89 s (load about 8 to 15).
- Layer 1, 1500 sequences, seed 30: 46 s (load about 8 to 15).
- Layer 1, 500 sequences, seed 10: 15.4 s (load 4.6).
- Earlier (first run): 3000 sequences, seed 2, 59 call kinds: 86 s (load 27 to 55).
- busybox test binary (4 tests in parallel, about 67 commands, each in both runners): 11 to 18 s this
  run (load about 7 to 15); 9 to 54 s in the first run (load 14 to 50).
- A failing layer 1 case takes one to two minutes to shrink.

## Departures from the plan

- Added in the fifth run: ARM64 Linux's user copy routine (`copy_template.S`) is modelled in
  src/user.rs, only for what overlapping copies within one process leave behind (process_vm_readv and
  process_vm_writev on oneself); no crate does this. cachestat on the root file system counts tmpfs
  pages here, where the reference's overlayfs file has none of its own (part of the root file
  system's known difference until K4).

- Added in the fourth run: the classic BPF checker and interpreter for seccomp filters are written
  here (src/seccomp.rs), following Linux's: the few crates that interpret classic BPF are small,
  little used, not no_std, or do not make Linux's checks, which are part of the behaviour. Docker's
  profile is modelled as a table (third run), so its length as a program, which counts towards the
  instruction limit, is the measured constant 1032. The made-up machine has, like the reference's
  CPU, no permission overlays (protection keys) and no guarded control stacks. Typed by hand (not in
  linux-raw-sys): rseq's flags, sizes and `RSEQ_CPU_ID_UNINITIALIZED`, in src/linux_extra.rs; the
  generator now also copies linux-raw-sys's `ptrace` module (seccomp's and BPF's definitions).

- Added in the third run: the made-up machine has one CPU, so its default time slice is 0.7 ms (the
  reference, with more CPUs, reports 2.8 ms) and sched_rr_get_interval reports it in whole 1 ms ticks
  (the reference's tick, inferred from 2.8 ms reported as 2 ms). mincore reports every mapped page as
  in memory. `/proc`'s link count follows Linux's formula with this container's processes. Huge page
  memory files are refused (hugetlbfs not built). The reference's tmpfs has no POSIX ACLs and its
  kernel no utilisation clamps; both are refused as there.
- Values typed by hand because linux-raw-sys lacks them (in src/linux_extra.rs): the ACL tags and
  version, `sched_attr` and its sizes, SCHED_FLAG_SUGOV, the ioprio classes and `which` values, and
  PER_CLEAR_ON_SETID.

- The root file system is in memory (tmpfs) in K1: the file backend is K4. Tests fill it from
  `docker export` of the reference image with `Kernel::add_file`. What shows the file system's own kind
  (directory sizes, link counts, listing order, blocks) differs from the reference's overlayfs; those
  cases are known failures, and layer 1 stays off the root.
- Docker's `/etc/hostname` is given the container's name; `/etc/hosts` and `/etc/resolv.conf` keep the
  image's empty placeholders (their contents come with the network, K5) but get Docker's mode 0644.
  The test programs' directory `/wc` is a plain directory here, a read-only bind mount in the reference.
- `/sys` is the image's empty directory (Docker mounts sysfs read-only there). `/dev/mqueue` is a
  plain directory.
- No address randomization: the break starts right after the program, mmap goes top-down from
  end-128 MiB, the stack is at the top. The reference randomizes all three.
- `uname` says release "7.0.0-wcontainers", version "#1 SMP PREEMPT"; the reference says
  7.0.14-linuxkit. The machine's memory is reported as the host's address space cap. These are part of
  the made-up machine of section 15. The plan says the machine description is shared with the
  translator; the abi's machine.rs holds only CPU features, so release and memory size live in the
  kernel (`Machine` in lib.rs) for now.
- A few Linux values are not in linux-raw-sys: `SIG_DFL` and `SIG_IGN` (casts in C) and the `ST_*`
  statfs flags (C library headers). They are in src/linux_extra.rs, with that reason. Also typed there
  by need, from Linux's source: MAX_RW_COUNT, UIO_MAXIOV, MAX_ARG_STRLEN, BINPRM_BUF_SIZE,
  MAXSYMLINKS, MINSIGSTKSZ, DIR_OFFSET values, BOGO_DIRENT_SIZE (each next to its use, named as Linux
  names it).
- The generator now also copies linux-raw-sys's `system`, `ioctl` and `prctl` modules, and gives a named
  list of structures zerocopy's byte derives (which prove they have no padding).
- Mount flags are modelled per file system (`/dev` nosuid; `/dev/shm` and `/proc` nosuid, nodev,
  noexec, relatime; `/dev/pts` nosuid, noexec, relatime; `/` relatime): statfs reports them, `nodev`
  refuses opening devices (EACCES), `noexec` refuses execve (EACCES) and executable file mappings
  (EPERM).
- `fs.protected_hardlinks` and `fs.protected_symlinks` are on, as in the reference (measured: 1 and 1;
  protected_regular and protected_fifos are 0).
- Many checks were reordered to match the order Linux makes them in (found by layer 1): which error
  comes first is part of the behaviour.
- Layer 1 uses a long-lived reference container that runs each sequence in a forked child (so state does
  not carry over), instead of one container per sequence, so a case costs about a millisecond and
  proptest can shrink.
- Layer 1's mmap lengths stay below 4 GiB: refusing larger reservations is the plan's declared
  small-address-space difference.
- Docker's default seccomp profile is modelled from measurement of the reference (Docker 29.9.3), not
  from the profile's source, because the reference is the specification. A program run on a different
  Docker version could see a different profile.
- `/proc/sys/{abi,debug,dev,net,sunrpc,user}` exist as empty directories so that paths through them
  resolve as in the reference; their contents are not part of the made-up machine yet.
- statfs reports a block size of 4096 for every file system, `/proc` included (measured); stat still
  reports 1024 for `/proc` files (measured).
- The generator now also copies linux-raw-sys's `net` module (for the socket families the profile
  checks). Values newer than linux-raw-sys 0.12.1 (RWF_NOAPPEND, RWF_ATOMIC, RWF_DONTCACHE,
  CLOSE_RANGE_UNSHARE, CLOSE_RANGE_CLOEXEC) are in src/linux_extra.rs, with that reason.


## Changes to the shared crate (wcontainers-abi)

- K0: `Host::output(stream, bytes)` with `Stream`; `CounterReading::detail`.
- None in K1 so far (checked again in the fourth run).

## Questions for the orchestrator (boundary)

- A load or store through a misaligned stack pointer is SIGBUS (BUS_ADRALN) on ARM64 Linux. `Exit` has no
  alignment fault, and the translator plan says only that unaligned access matches. Open.
- The plan's shared machine description: should release, memory size and system name move into
  wcontainers-abi's machine.rs? Not done, to avoid changing a file this side does not own.

## Known issues

- Building the test crate prints a linker warning from unicorn-engine-sys (a missing lib64 directory).
- Once in the third run (load about 20 to 50) the stand-in test binary (tests/standin.rs, which uses no
  kernel) hung for over 12 minutes and was killed; three reruns passed at once. Cause not found.
- `/proc/<pid>/fd` links of files opened by a path keep that path; a rename afterwards is not seen.
- Inodes of `/proc` are made at each lookup, so changes to them do not last: chown of a process's
  entries succeeds with no effect, as on Linux; chmod or chown of entries outside the process
  directories (which Linux keeps) are refused with EPERM and counted ("proc attribute change not
  kept"); utimensat on them succeeds with no effect.
- `/proc` writes to `/proc/sys` files give EACCES or EROFS depending on our file modes; the reference
  gives EROFS for the writable ones (not compared yet).
- `/proc/mounts` does not list Docker's read-only and masked `/proc` mounts.
- `/proc/<pid>/status` has only some of Linux's lines (the reference has many more, such as the
  memory and context switch lines); the seccomp and NoNewPrivs lines are now right.

## Next (K1, in order)

1. Run layer 1 with new seeds (20000 or more each) and fix what it finds, until long runs pass; then
   the million (the orchestrator's command at the top). In the fourth run every 20000 run (seeds 54,
   57, 58, 60, 61) found a mismatch within 1.5 to 9 minutes, each fixed. Rerun seed 61 at 20000 next;
   seeds 62 and up are fresh.
2. Missing calls a single program can use without other processes, threads, timers or sockets:
   name_to_handle_at (tmpfs has handles; the reference's root, overlayfs, refuses them),
   process_vm_readv and process_vm_writev on the caller itself, cachestat, memfd_secret (needs shared
   file mappings), seccomp listeners, rseq's time slice extension. The rest need K2 (pipe2, fork,
   futex, wait, signal delivery: rt_sigreturn, rt_sigsuspend, rt_sigtimedwait), K3 (timers,
   getitimer and setitimer) or K5 (sockets, epoll, eventfd, inotify).
3. Known gaps above (RLIMIT_CPU, brk's data limit, `/proc/mounts`, `/proc/sys` writes, nice in
   `/proc/<pid>/stat`, `/proc/<pid>/status` lines, xattr space, huge page memory files, shared file
   mappings).
4. K2 must abort rseq critical sections when it delivers signals, and run seccomp TRAP handlers.
5. Then K2.
