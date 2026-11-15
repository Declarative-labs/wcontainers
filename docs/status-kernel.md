# Kernel status

Plan: docs/phase-1-kernel.md. Branch: phase-1-kernel.

## Where things stand

K0 is done. K1 (one process) is complete as the orchestrator ruled it (seventh run, the close-out):
every item of the K1 list is built, layer 1 covers every call built, and with every fix in three
fresh seeds passed 20000 sequences each (see Layer 1). The one-million-sequence layer 1 run is not
part of K1: by the orchestrator's ruling it runs once at the end of the kernel track. Next is K2.

How to test everything: `nix develop -c cargo test -p wcontainers-kernel -p wcontainers-kernel-test`.
Known failures are `#[ignore]`d with their reason; see them with `-- --include-ignored`.
Needs Docker running; the reference uses alpine:3.22. Containers are named `wc-kernel-*` and removed.

Layer 1 long run (one command, for the orchestrator):
`LAYER1_CASES=1000000 LAYER1_SEED=<n> nix develop -c cargo test -p wcontainers-kernel-test --test layer1`
`LAYER1_CASES` is the number of sequences (default 64), `LAYER1_SEED` the seed (default 0). A failure is
shrunk and printed call by call with the seed. A sequence that runs past 60 s in the reference is
killed there (SIGKILL), which our side reports for a wait that never ends. Seeds 94 and up are fresh.

## For the independent check

Everything a reviewer should know about what is not compared, in one place.

Ignored tests (each attribute gives the reason):
- busybox `root_file_system`: `ls -la /`, `ls -ln /etc`, `stat /etc`, `find /etc`, `du /etc/apk` show
  the root file system's own kind (overlayfs over ext4 in the reference, tmpfs here until K4).
- align `unaligned_exclusive_matches_reference`: unicorn faults an exclusive load from an unaligned
  address, which the reference's hardware runs. A stand-in difference, not the kernel's.
- align `misaligned_stack_pointer_matches_reference`: unicorn (QEMU) never checks stack pointer
  alignment, so the stand-in never reports `Exit::AlignmentFault` for it. The kernel's handling of
  that exit is tested through a misaligned jump in tests/faults.rs, which QEMU does report.
- memfd `huge_page_memory_files`: hugetlbfs is not built (see Partial).
- cachestat `mapped_file_pages`: the page a private mapping's read fault brings into a tmpfs file is
  counted by the reference and not known here (counted "page residency not tracked").
- proclist `thread_listings_are_a_subset_of_the_reference`: `/proc/<pid>/task` is not built (threads,
  K3).

Comparisons that are subsets rather than equality, and why:
- tests/proclist.rs compares listings of `/proc`, `/proc/self`, `/proc/1`, `/proc/self/fd`,
  `/proc/sys/...`, `/dev`, `/dev/pts` and `/dev/shm`: every entry we list must be in the reference's
  listing with the same type, mode, owner, group and link count. The reference has 331 more entries
  (printed by the test): `/proc` holds a chosen subset (plan section 15). The link counts of `/proc`
  itself (Linux counts every process on the machine) and of `/` (the root file system's kind) are left
  out of that comparison.
- tests/sharedmap.rs sorts `/proc/self/maps` (placement differs: no address randomization here).
- tests/schedattr.rs shows time slices relative to the default (the CPU count differs).

What layer 1 does not compare, and why (masked on both sides, so a difference there cannot show):
- Inode and device numbers and times in stat and statx, d_ino in getdents64, statfs counts and fsid:
  machine-specific.
- The values of clock_gettime, gettimeofday, times, sysinfo and getrusage, getrandom's bytes, uname's
  release and version, the CPU set and number (one CPU is declared), the time ppoll, pselect6 and
  sched_rr_get_interval write back, sched_getattr's runtime, mincore's vector (residency).
- Process numbers, set_tid_address's value, getcwd inside `/proc/<pid>`, a link naming a process
  directory by number, the address mmap or a moving mremap returns, `/proc`'s own link count, the CPU
  fields of rseq areas, name_to_handle_at's handle words and mount number.
- getdents64 and lseek on `/dev`, `/dev/shm`, `/proc` itself and process directories (the reference's
  runner asks after each call; "unshared"): `/dev` and `/dev/shm` outlive a sequence in the reference
  and keep tmpfs's position counter across sequences; `/proc` and process directories list the
  machine's processes and the chosen subset of entries. These listings are compared in layer 2 by
  tests/proclist.rs (above) instead. Listings of the sequence's own directories are compared in full.
- A call's random or machine bytes are kept out of later calls by writing them only to areas no later
  call reads (getrandom, uname, name_to_handle_at).

What layer 1 does not generate, and why:
- Named pipes (opening one waits; pipes are K2); descriptors 0 to 2 (the reference's runner reports
  through them); process 1 and the caller's own group negated (our caller is process 1, so -1 means
  every process); memory lengths above 256 MiB (our programs' own mappings lie higher in a 4 GiB
  space); ppoll and pselect6 without a timeout; flock waits (they wait for ever).
- Signals whose default action ends or stops a process, and rt_sigaction on the signals kill sends
  (our caller is the container's first process, which Linux protects; theirs is not; handlers are K2).
- RLIMIT_CPU and RLIMIT_FSIZE limits that can be reached, process limits that can be reached (Linux
  counts the user's processes machine-wide), stack limits above 1 GiB (our stack could grow into the
  play area), core size limits above zero (core dumps are not written, counted).
- setpriority and getpriority by user, ioprio by user (they reach the reference's runner).
- openat2's restricted lookups (not built) and RESOLVE_CACHED (depends on Linux's caches); huge page
  memory files; seccomp listeners and the rseq time slice flag (not built); filter loads of the
  instruction pointer (the runners' differ); alternate signal stacks that could hold either caller's
  stack pointer; rseq areas where other calls write.
- brk (its values are addresses that differ by layout; tests/memory.rs and tests/limits.rs test it).
- Paths reach `/dev/shm/w` (the sequence's directory), the three zero devices and `/proc/self/...`
  through a fixed pool; never the root file system, never `/proc` files' contents (those are compared
  in layer 2 only where a test reads them: procsignals, the busybox commands, execveat, memfd).

## What exists

- Kernel API: `Kernel::new`, `add_file(path, Attributes, NewFile)` to fill the root file system before
  the start (as unpacking an image does), `start(cpu, host, &Start { path, arguments, environment,
  working_directory, hostname, groups })`, `next`, `exit`, `event`, `counters`, `root_files()`,
  `system_calls()`.
- Exits: system calls, stops, memory faults (stack growth, SIGSEGV, SIGBUS past a file's end),
  `MemoryFaultWithoutRegisters` (SIGSEGV), `AlignmentFault` (SIGBUS as Linux's `do_sp_pc_abort`;
  BUS_ADRALN and the address reach only a handler, and handlers run in K2), illegal instructions
  (SIGILL), breakpoints (SIGTRAP). With no handler the process dies of the signal as on Linux; with a
  handler it dies too, counted "signal handler not run".
- File layer (src/fs): inodes shared by `Rc`, mounts, path walk with symlinks (40 at most), `..`
  across mounts, `d_path` paths (a removed directory gets " (deleted)", one outside the caller's root
  its path from the top; getcwd's "(unreachable)" is separate), permission checks
  (`generic_permission` with DAC_OVERRIDE and DAC_READ_SEARCH), sticky and set-group directories, the
  set-user and set-group bits dropped on write, truncate and chown.
- tmpfs directories as Linux 7.0 keeps them (`simple_offset_*` and the dentry list): each entry has a
  position (a new one for each new name; a rename onto a name keeps that name's position, an exchange
  swaps them) and a place in the list `getdents` follows (newest first; a rename moves the entry to the
  head, the whiteout of RENAME_WHITEOUT goes in before the move). Listing from a position starts at the
  head at 2 or below, otherwise at the entry with the greatest position at or below it, or at the head
  when there is none. `.`=0, `..`=1, end 2^31-1.
- Mounts as `docker run` makes them: `/` (tmpfs here, see departures), `/proc`, `/dev` (tmpfs with
  null, zero, full, random, urandom, tty and Docker's links), `/dev/pts` (devpts, with ptmx), `/dev/shm`
  (tmpfs), `/dev/mqueue` as a plain directory. fd 0 is `/dev/null` opened read-write, fds 1 and 2 are
  pipes the host reads (as Docker's; no O_LARGEFILE, as Linux's `create_pipe_files`, measured).
- Open files keep O_LARGEFILE from open as a 64 bit kernel sets it, except with O_PATH
  (`build_open_how`); F_GETFL reports the file's own flags.
- `/proc`: self, `<pid>/` with cmdline, comm, environ, auxv, maps, stat, statm, status, mounts, limits,
  cgroup, exe, cwd, root, fd/ (magic links); cpuinfo, meminfo, uptime, loadavg, stat, version,
  filesystems, mounts, sys/kernel/{hostname, osrelease, ostype, pid_max, ngroups_max, cap_last_cap,
  random/boot_id, random/uuid}, sys/fs/nr_open, sys/vm/overcommit_memory (each with the mode of Linux's
  sysctl table: 0644 for the writable ones). Bytes are made at open. Rules: a process directory refuses
  writes and changes (EPERM), its own `fd` directory is always usable (0500), `/proc/sys` is Docker's
  read-only mount (EROFS), owners follow `task_dump_owner`, link counts (9 for a process directory, 1
  for sysctl directories), an `fd` directory's size is its number of descriptors, its listing
  positions are a descriptor's number plus 2 (`proc_readfd_common`). status and stat show the blocked,
  ignored and caught signals (stat only the first 31, as `do_task_stat`).
- Memory (src/memory.rs): ranges with what the program asked for, top-down placement below
  end-128 MiB, reused ranges cleared, brk, mmap (anonymous, private file copies, `/dev/zero`, shared
  file mappings through shared blocks), munmap, mprotect, madvise, mremap, msync, mlock family.
  Pages wholly past a file's end are inaccessible (SIGBUS). The stack grows on a fault below it up to
  its limit, keeping Linux's 1 MiB guard gap. Pages that may only run cannot be read (measured: the
  reference's CPU has execute-only pages).
- Shared file mappings (src/shared.rs): a shared tmpfs file's first pages live in one block of shared
  pages that every such mapping shows and the file's reads, writes, truncation and holes reach; with
  Linux's rules for VM_MAYWRITE, VM_MAYEXEC, MAP_SHARED_VALIDATE, seals, MADV_REMOVE, mremap and merging.
- Loader: ELF with PT_INTERP (the named loader placed where mmap would put it, AT_BASE), ET_DYN at 2/3
  of the address space, `#!` scripts (one argument, 4 levels, 256 byte line), execve and execveat,
  close on exec, handlers reset. The auxiliary vector as the reference's, in its order, except
  AT_SYSINFO_EHDR and AT_MINSIGSTKSZ (counted and warned at each start; they come with the vDSO and
  signal frames in K2). Set-user and set-group bits, file capabilities, AT_SECURE, dumpability.
- Credentials: uid/euid/suid/fsuid and gids, supplementary groups (Docker's), Docker's 14 powers,
  Linux's rules for changing ids and what that does to powers and `dumpable`, capget/capset.
- Time: clocks from the host, coarse clocks at 1 ms resolution (measured), sleeps park the thread and
  the kernel goes idle until the time; the test host jumps its clocks instead of waiting. CPU time is
  the time since the container started less the time the process's threads spent parked.
- Signals within one process (src/signal.rs): sent to itself, dropped if ignored, kept pending while
  blocked, acted on when let through. The container's first process ignores signals it has no handler
  for (`SIGNAL_UNKILLABLE`); faults still kill it. A signal that would need its handler run, or would
  stop the process, is refused with ENOSYS and counted.
- Resource limits enforced: RLIMIT_FSIZE, RLIMIT_AS, RLIMIT_DATA (with `check_data_rlimit`),
  RLIMIT_STACK, RLIMIT_MEMLOCK, RLIMIT_NOFILE. RLIMIT_CPU is kept, not enforced (needs timers, K3).
- Docker's default seccomp profile (src/seccomp.rs), measured on the reference, and a program's own
  classic BPF filters, checked and run as Linux's.
- flock between open files; extended attributes on tmpfs (src/syscall/xattr.rs); memfd_create with
  seals; memfd_secret; name_to_handle_at; rseq; membarrier; protection keys as on a CPU without
  permission overlays; sched_setattr family; process_vm_readv and process_vm_writev on the caller;
  cachestat.
- Test crate: `buffer.rs` (a CPU side with no instructions, for layer 1), `standin.rs` (unicorn; a
  fetch from a pc that is not a multiple of 4 is reported as `AlignmentFault`), `image.rs`, `busybox.rs`,
  `layer1.rs` and `guest/layer1.c`, and one guest program per layer 2 test in `guest/`.

## System calls

Done (170): brk, cachestat, capget, capset, chdir, chroot, clock_getres, clock_gettime,
clock_nanosleep, close, close_range, copy_file_range, dup, dup3, execve, execveat, exit, exit_group,
faccessat, faccessat2, fadvise64, fallocate, fchdir, fchmod, fchmodat, fchmodat2, fchown, fchownat,
fcntl, fdatasync, fgetxattr, flistxattr, flock, fremovexattr, fsetxattr, fstat, fstatfs, fsync,
ftruncate, get_robust_list, getcpu, getcwd, getdents64, getegid, geteuid, getgid, getgroups,
getpgid, getpid, getppid, getpriority, getrandom, getresgid, getresuid, getrlimit, getrusage,
getsid, gettid, gettimeofday, getuid, getxattr, ioctl, ioprio_get, ioprio_set, kill, lgetxattr,
linkat, listxattr, llistxattr, lremovexattr, lseek, lsetxattr, madvise, map_shadow_stack,
membarrier, memfd_create, memfd_secret, mincore, mkdirat, mknodat, mlock, mlock2, mlockall, mmap,
mprotect, mremap, msync, munlock, munlockall, munmap, name_to_handle_at, nanosleep, newfstatat,
openat, openat2, personality, pkey_alloc, pkey_free, pkey_mprotect, ppoll, prctl, pread64, preadv,
preadv2, prlimit64, process_vm_readv, process_vm_writev, pselect6, pwrite64, pwritev, pwritev2, read,
readahead, readlinkat, readv, removexattr, renameat, renameat2, rseq, rt_sigaction, rt_sigpending,
rt_sigprocmask, sched_get_priority_max, sched_get_priority_min, sched_getaffinity, sched_getattr,
sched_getparam, sched_getscheduler, sched_rr_get_interval, sched_setaffinity, sched_setattr,
sched_setparam, sched_setscheduler, sched_yield, seccomp, sendfile, set_robust_list,
set_tid_address, setfsgid, setfsuid, setgid, setgroups, setpgid, setpriority, setregid, setresgid,
setresuid, setreuid, setrlimit, setsid, setuid, setxattr, sigaltstack, statfs, statx, symlinkat,
sync, sync_file_range, syncfs, sysinfo, tgkill, times, tkill, truncate, umask, uname, unlinkat,
utimensat, write, writev.

Partial (inside the list above), each with its reason:
- fcntl: no record locks (F_GETLK/SETLK/OFD: counted, ENOSYS: they matter between processes, K2); no
  leases, owners or signals (K2).
- ioctl: FIOCLEX, FIONCLEX, FIONBIO; terminal requests ENOTTY (terminals are K2); others ENOTTY,
  counted when the file is a device.
- mmap: MAP_HUGETLB counted, ENOMEM (no huge pages). MAP_SHARED anonymous is private memory here,
  exact while there is no fork (K2). A private file mapping is a copy made at mmap, so pages not yet
  written do not see later changes to the file (no counter: the CPU side does not say which pages were
  written). MAP_SYNC EOPNOTSUPP, as tmpfs.
- openat, openat2: O_TMPFILE counted, EOPNOTSUPP; named pipes counted, ENXIO (pipes, K2); `/dev/ptmx`
  counted, ENODEV (terminals, K2). openat2's restricted lookups counted, ENOSYS, as a kernel without
  them; RESOLVE_CACHED taken.
- kill, tkill, tgkill: only to the caller itself; others ESRCH (no other process in K1).
- execve, execveat: only from a process with one thread (threads, K3).
- rt_sigaction, rt_sigprocmask, sigaltstack: handlers recorded, not run (K2).
- getrusage, times, CPU-time clocks: CPU time is time not parked since the container started, all of
  it user time; with several threads (K3) a thread waiting for its turn would count. ru_maxrss is the
  mapped size, counted "page residency not tracked"; the other fields are 0 (no faults, switches or
  I/O are counted yet).
- fallocate: plain and keep-size allocation and punching holes (what tmpfs has); others EOPNOTSUPP.
- prctl: SET/GET_NAME, GET/SET_DUMPABLE, GET/SET_KEEPCAPS, CAPBSET_READ, GET/SET_NO_NEW_PRIVS,
  GET_SECCOMP, SET_SECCOMP, GET_CHILD_SUBREAPER; others counted, EINVAL.
- statfs: block and inode counts are made up (64 MiB like Docker's tmpfs).
- ppoll, pselect6: every file is always ready (Linux's default poll mask; the host's pipes always have
  room); a wait with nothing ready sleeps until its timeout, or for ever (until K2's signals). The
  signal mask they take is read, not applied (K2).
- flock: a wait for a conflicting lock never ends (one process).
- sched_setscheduler, sched_setparam, sched_setattr: kept per thread with Linux's checks; nothing is
  scheduled by them yet (K3). Custom slices kept; no utilisation clamps (EOPNOTSUPP, as the reference).
- mlock family: lock state and limits are kept; pages are not brought in (nothing to bring).
- memfd_create: MFD_HUGETLB counted, ENOSYS (no hugetlbfs); the reference makes such files.
- mincore: every mapped page reported in memory, counted "page residency not tracked".
- Extended attributes: tmpfs's space limit for them (ENOSPC) is not modelled.
- ioprio_get, ioprio_set: kept per thread; nothing does I/O by it.
- seccomp: no listener (counted, EINVAL as a kernel without listeners); TRAP with a handler kills (K2).
- rseq: a critical section is never aborted yet (signal delivery K2, preemption K3). The time slice
  extension flag is counted and refused with EINVAL; the reference takes it.
- process_vm_readv, process_vm_writev: only the caller's own process (the only one in K1).
- cachestat, stat's blocks: pages a mapped file's faults bring in are not known (counted).
- Core dumps: not written (counted "core dump not written"; Linux writes `core` and sets 0x80 when the
  limit is above zero; the reference's default limit is 0).
- sigaltstack: refuses a change while on the alternate stack; handlers need K2.

Refused by Docker's profile as in the reference (65; they never reach the kernel there, so nothing to
build): lookup_dcookie, umount2, mount, pivot_root, nfsservctl, quotactl, unshare, kexec_load,
delete_module, clock_settime, settimeofday, add_key, request_key, keyctl, swapon, mbind,
get_mempolicy, set_mempolicy, migrate_pages, move_pages, perf_event_open, open_by_handle_at, setns,
kcmp, bpf, userfaultfd, kexec_file_load, io_uring_setup, io_uring_enter, io_uring_register,
open_tree, fsconfig, clone3, pidfd_getfd, process_madvise, mount_setattr, quotactl_fd,
set_mempolicy_home_node, statmount, listmount, lsm_get_self_attr, lsm_set_self_attr,
lsm_list_modules, mseal, setxattrat, getxattrat, listxattrat, removexattrat, open_tree_attr,
file_getattr, file_setattr, acct, vhangup, swapoff, init_module, finit_module, reboot, syslog, fsopen,
fspick, fsmount, move_mount, sethostname, setdomainname, fanotify_init. (sethostname and
setdomainname keep handlers the profile keeps out of reach.)

Missing (90, each counted "system call not implemented" and ENOSYS), with what each needs:
- Processes and waiting (K2): clone, wait4, waitid, ptrace (tracing is out of phase one), pidfd_open,
  pidfd_send_signal, process_mrelease.
- Signals (K2: handler frames and the vDSO return path): rt_sigreturn, rt_sigsuspend,
  rt_sigtimedwait, rt_sigqueueinfo, rt_tgsigqueueinfo, restart_syscall, signalfd4.
- Pipes and splicing (K2): pipe2, vmsplice, splice, tee.
- Futexes (K3, with threads): futex, futex_waitv, futex_wake, futex_wait, futex_requeue.
- Timers (K3, through the kernel's one timer queue): getitimer, setitimer, timer_create,
  timer_gettime, timer_getoverrun, timer_settime, timer_delete, timerfd_create, timerfd_settime,
  timerfd_gettime.
- Waiting on many things (K5): epoll_create1, epoll_ctl, epoll_pwait, epoll_pwait2, eventfd2,
  inotify_init1, inotify_add_watch, inotify_rm_watch, fanotify_mark.
- Sockets (K5): socket, socketpair, bind, listen, accept, accept4, connect, getsockname, getpeername,
  sendto, recvfrom, setsockopt, getsockopt, shutdown, sendmsg, recvmsg, recvmmsg, sendmmsg.
- Asynchronous I/O (K5 or later, needs the waiting design for many requests): io_setup, io_destroy,
  io_submit, io_cancel, io_getevents, io_pgetevents.
- System V IPC and message queues (K2, shared between processes): mq_open, mq_unlink, mq_timedsend,
  mq_timedreceive, mq_notify, mq_getsetattr, msgget, msgctl, msgrcv, msgsnd, semget, semctl,
  semtimedop, semop, shmget, shmctl, shmat, shmdt.
- Single-process calls not built, each before K6's bar: adjtimex and clock_adjtime (reading the
  clock's adjustment state: the made-up machine's values; setting needs CAP_SYS_TIME, which Docker's
  root lacks), remap_file_pages (deprecated in Linux, emulated there with mmap over a shared mapping),
  landlock_create_ruleset, landlock_add_rule and landlock_restrict_self (Landlock's rules applied in the
  path walk).

## Known gaps (with reasons)

- `/proc/<pid>/task` is not built (threads, K3); listing it is ENOENT here (an ignored test shows it).
- `/proc/<pid>/stat`: the start time, rss, rsslim, the code, data, stack, argument and environment
  addresses and the pending signals are 0 (the loader knows the addresses but does not keep them;
  pending signals are one set per thread here while Linux keeps shared and thread pending apart, K2).
  `/proc/<pid>/status` has only some of Linux's lines (no memory, SigQ, SigPnd, ShdPnd or context
  switch lines).
- `/proc/<pid>/fd` links of files opened by a path keep that path; a rename afterwards is not seen
  (directories are followed live).
- Inodes of `/proc` are made at each lookup, so changes to them do not last: chown of a process's
  entries succeeds with no effect, as on Linux; chmod or chown of entries outside the process
  directories are refused with EPERM and counted ("proc attribute change not kept"); utimensat on them
  succeeds with no effect (Linux keeps the times while the inode is cached).
- `/proc/mounts` does not list Docker's read-only and masked `/proc` mounts.
- RLIMIT_CPU is not enforced (timers, K3); xattr space limits; huge page memory files; private file
  mappings that see later file changes; O_TMPFILE.
- K2 must abort rseq critical sections when it delivers signals, and run seccomp TRAP handlers.

## Tests (last run 2026-11-15: 43 passed, 6 ignored known failures)

All pass except the ignored known failures listed above. Added in the seventh run:
- dirorder: guest/dirorder.c: tmpfs listings whole, one entry per call and from every position, after
  creating, renaming over a name, renaming to a new name, exchanging, linking, removing, renaming a
  directory over an empty one, moving out and back, refused renames, whiteouts, lseek to 2, 3, 5, 100,
  2^31-2, 2^31, 2^40, a removed entry at the position, and the oldest entry removed (the listing
  starts again from the head). Same output.
- openflags: F_GETFL of fds 0 to 2, files opened with many flags, O_PATH (also through openat2),
  `/proc/self/fd`, memory files, after F_SETFL and dup. Same output.
- proclist: see "For the independent check".
- procsignals: SigBlk, SigIgn and SigCgt in status and fields 32 to 34 of stat after blocking, ignoring
  and catching signals. Same output.
- dpath: `/proc/self/cwd` and fd links of removed directories, renamed directories, getcwd in a removed
  directory and outside a chroot. Same output.
- cputime: process and thread CPU clocks, getrusage and times after nanosleep, clock_nanosleep and
  ppoll of 2 s each, and a sleep on the CPU clock that is already over. Same output.
- faults: now also a jump to an address that is not a multiple of 4 (SIGBUS, 135, through
  `Exit::AlignmentFault`).
Earlier tests: sharedmap, handle, secret, processvm, cachestat, execveat, filter, busybox (85 single
commands: same stdout, stderr, exit code and files changed), xattr, memfd, schedattr, memory, script,
one_cpu, dispatch, loader, faults, standin, boundary, layouts, limits, seccomp, procaccess, layer1.

## Layer 1

Seventh run (close-out): seeds 82 to 84 at 20000 (side by side, load 6 to 18): 82 and 84 passed
(301 s each); 83 failed after 311 s: F_GETFL on an O_PATH descriptor showed O_LARGEFILE, which
F_GETFL added to every file (fixed: open keeps it, never with O_PATH). Seeds 85 to 87 (load 5 to 11):
86 and 87 passed (294 s each); 85 failed: readlink of `/proc/self/cwd` after the working directory was
removed gave nothing where Linux's `d_path` gives the path and " (deleted)" (fixed). Seeds 88 to 90
then passed 20000 each (297 s each, side by side, load 11 at the start, 15 at the end), before the CPU
time change. With every fix in (head afe0a86), seeds 91, 92 and 93 passed 20000 each (301 s each,
side by side, load 14.5 at the start, 10 at the end, a full test run beside them for part of it).

Sixth run: added shared file mappings, name_to_handle_at and memfd_secret; seeds 70 to 81 found and
fixed about twenty mismatches; at its end seeds 79 to 81 passed 20000 each. Fifth run: added
process_vm_readv, process_vm_writev and cachestat; `/dev` and `/dev/shm` listings made "unshared".
Fourth run: seccomp filters (four per sequence, from generated values), rseq, membarrier, protection
keys and the calls refused for powers. Earlier runs and every case are in git history.

How the runner works: the reference child reports into a pipe its parent drains; the parent sends the
whole report with a length, then the wait status. The child first makes itself a session leader, as
our caller (the container's first process) is. An argument SELF is replaced on each side by the
caller's own process number. 1 to 15 calls follow mkdirat and chdir into `/dev/shm/w`; arguments come
from pools with bad pointers, bad descriptors, huge counts, unknown flags and invalid values. After
each sequence the runner puts back what a sequence can change outside its directory.

## Measurements

All on this shared Mac; noisy, not benchmarks.
- Seventh run, layer 1 20000 sequences, three side by side: 294 to 311 s each (load 5 to 18).
  Seeds 91 to 93: 301 s each (load 10 to 15).
- Sixth run, layer 1 20000 sequences, three side by side: about 290 to 330 s each (load 6 to 15).
- Fourth run, layer 1, 3000 sequences, seed 57: 85.6 s (load 5.8 to 9.5); the filter test: 2.0 s
  (load about 10).
- Third run, layer 1 20000 sequences, seed 48: 599 s (load 4 to 42).
- busybox test binary (4 tests in parallel, about 85 commands, each in both runners): 11 to 18 s
  (load about 7 to 15).
- The whole test suite of both crates: 57 s (load 18 at the start, seventh run).
- A failing layer 1 case takes one to two minutes to shrink.

## Departures from the plan

- The root file system is in memory (tmpfs) in K1: the file backend is K4. Tests fill it from
  `docker export` of the reference image with `Kernel::add_file`. What shows the file system's own kind
  differs from the reference's overlayfs; those cases are known failures, and layer 1 stays off it.
- Docker's `/etc/hostname` is given the container's name; `/etc/hosts` and `/etc/resolv.conf` keep the
  image's empty placeholders (contents come with the network, K5) with Docker's mode 0644. The test
  programs' directory `/wc` is a plain directory here, a read-only bind mount in the reference. `/sys`
  is the image's empty directory (Docker mounts sysfs there). `/dev/mqueue` is a plain directory.
- No address randomization: the break starts right after the program, mmap goes top-down from
  end-128 MiB, the stack is at the top.
- `uname` says release "7.0.0-wcontainers", version "#1 SMP PREEMPT"; the reference says
  7.0.14-linuxkit. The machine's memory is reported as the host's address space cap. These are part of
  the made-up machine of section 15. The plan says the machine description is shared with the
  translator; by the orchestrator's ruling release, system name and memory size stay in the kernel
  (`Machine` in lib.rs) and the abi's machine.rs holds only CPU features.
- Values not in linux-raw-sys are typed by hand in src/linux_extra.rs, each with its reason: `SIG_DFL`,
  `SIG_IGN`, the `ST_*` statfs flags, MAX_RW_COUNT, UIO_MAXIOV, MAX_ARG_STRLEN, BINPRM_BUF_SIZE,
  MAXSYMLINKS, MINSIGSTKSZ, the tmpfs directory positions, BOGO_DIRENT_SIZE, the ACL tags and version,
  `sched_attr` and its sizes, SCHED_FLAG_SUGOV, the ioprio classes, PER_CLEAR_ON_SETID, rseq's flags and
  sizes, RWF_NOAPPEND, RWF_ATOMIC, RWF_DONTCACHE, RWF_NOSIGNAL, CLOSE_RANGE_UNSHARE, CLOSE_RANGE_CLOEXEC.
- The generator copies linux-raw-sys's `system`, `ioctl`, `prctl`, `net` and `ptrace` modules, and gives
  a named list of structures zerocopy's byte derives.
- Mount flags are modelled per file system (`/dev` nosuid; `/dev/shm` and `/proc` nosuid, nodev,
  noexec, relatime; `/dev/pts` nosuid, noexec, relatime; `/` relatime).
- `fs.protected_hardlinks` and `fs.protected_symlinks` are on, as in the reference (measured).
- Layer 1 uses a long-lived reference container that runs each sequence in a forked child, instead of
  one container per sequence, so a case costs about a millisecond and proptest can shrink.
- Layer 1's mmap lengths stay below 4 GiB: refusing larger reservations is the declared
  small-address-space difference.
- Docker's default seccomp profile is modelled from measurement of the reference (Docker 29.9.3), not
  from the profile's source; a different Docker version could have a different profile. The classic
  BPF checker and interpreter are written here, following Linux's (no crate makes Linux's checks).
- `/proc/sys/{abi,debug,dev,net,sunrpc,user}` exist as empty directories so paths through them resolve.
- statfs reports a block size of 4096 for every file system, `/proc` included (measured); stat reports
  1024 for `/proc` files (measured).
- The made-up machine has one CPU (default time slice 0.7 ms, sched_rr_get_interval in whole 1 ms
  ticks), no permission overlays and no guarded control stacks, like the reference's CPU for the last
  two.
- A file mapped shared keeps its first pages in a block of shared pages while any mapping shares it;
  private file mappings stay copies. A tmpfs inode's generation is a keyed hash of its number
  (siphasher, no_std). The stand-in maps a block's host memory into unicorn wherever it is shown.
- ARM64 Linux's user copy routine (`copy_template.S`) is modelled in src/user.rs for what overlapping
  copies within one process leave behind.
- cachestat on the root file system counts tmpfs pages, where the reference's overlayfs file has none.
- Many checks were reordered to match the order Linux makes them in: which error comes first is part of
  the behaviour.

## Changes to the shared crate (wcontainers-abi)

- K0: `Host::output(stream, bytes)` with `Stream`; `CounterReading::detail`.
- K1: none by this side. The seventh run merged origin/phase-1, which added `Exit::AlignmentFault {
  address }` (the translator's side) and moved a layout assertion in registers.rs to a constant
  assertion; the kernel handles the new exit (SIGBUS) and the stand-in reports it for misaligned
  fetches. Cargo.lock was regenerated by cargo for the merge (the kernel's lock plus the translator's
  packages, only additions).

## Questions for the orchestrator (boundary)

None open. (Answered: the alignment fault exit exists now; the machine description stays split.)

## Known issues

- Building the test crate prints a linker warning from unicorn-engine-sys (a missing lib64 directory).
- Once in the third run (load about 20 to 50) the stand-in test binary hung for over 12 minutes and was
  killed; three reruns passed at once. Cause not found; not seen since.
- The test driver prints a counter's detail with a call name even for counters whose detail is not a
  call number (cosmetic).

## Next

1. K2: processes, fork, wait, pipes, signal delivery with Linux's frames and a vDSO (then
   AT_SYSINFO_EHDR and AT_MINSIGSTKSZ), terminals, job control, shared memory between processes; abort
   rseq critical sections and run seccomp TRAP handlers on delivery.
2. The missing calls above by their milestone; the known gaps above before K6's bar.
3. The one-million-sequence layer 1 run once at the end of the kernel track (orchestrator).
