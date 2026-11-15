# Kernel status

Plan: docs/phase-1-kernel.md. Branch: phase-1-kernel.

## Where things stand

K0 is done. K1 (one process) is built and tested, first run: programs start from the file system (with
their own loader and `#!` scripts), memory calls with page protection and faults, the in-memory file
systems (tmpfs, `/proc`, `/dev`), file descriptors, time, randomness, ids and powers for one process.
Single busybox programs match the reference. Layer 1 runs. What is left for K1 is under "Next".

How to test everything: `nix develop -c cargo test -p wcontainers-kernel -p wcontainers-kernel-test`.
Known failures are `#[ignore]`d with their reason; see them with `-- --include-ignored`.
Needs Docker running; the reference uses alpine:3.22. Containers are named `wc-kernel-*` and removed.

Layer 1 long run (one command, for the orchestrator):
`LAYER1_CASES=1000000 LAYER1_SEED=<n> nix develop -c cargo test -p wcontainers-kernel-test --test layer1`
`LAYER1_CASES` is the number of sequences (default 64), `LAYER1_SEED` the seed (default 0). A failure is
shrunk and printed call by call with the seed.

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
- Signals as far as one process with no delivery can go: actions and masks are kept; a signal to
  itself that is ignored does nothing, one whose default kills ends the process with that signal; one
  with a handler (or blocked, or stopping) is counted "signal handler not run" and the call gives
  ENOSYS. A fault with a handler set is counted the same way and kills.
- Test crate: `buffer.rs` (a CPU side with no instructions, for layer 1), `image.rs` (docker export of
  the image into `add_file`), `busybox.rs` (one command in both runners, comparing output, exit code and
  the files changed relative to the image), `layer1.rs` and `guest/layer1.c` (the runner: reads
  sequences from stdin, runs each in a fresh child, reports each call's value and the arena bytes it
  changed).

## System calls

Done (120): brk, capget, capset, chdir, chroot, clock_getres, clock_gettime, clock_nanosleep, close,
dup, dup3, execve, exit, exit_group, faccessat, faccessat2, fallocate, fchdir, fchmod, fchmodat,
fchmodat2, fchown, fchownat, fcntl, fdatasync, fstat, fstatfs, fsync, ftruncate, get_robust_list,
getcpu, getcwd, getdents64, getegid, geteuid, getgid, getgroups, getpgid, getpid, getppid,
getpriority, getrandom, getresgid, getresuid, getrlimit, getrusage, getsid, gettid, gettimeofday,
getuid, ioctl, kill, linkat, lseek, madvise, mkdirat, mknodat, mmap, mprotect, mremap, msync, munmap,
nanosleep, newfstatat, openat, personality, prctl, pread64, preadv, prlimit64, pwrite64, pwritev,
read, readlinkat, readv, renameat, renameat2, rt_sigaction, rt_sigpending, rt_sigprocmask,
sched_getaffinity, sched_getparam, sched_getscheduler, sched_setaffinity, sched_yield, sendfile,
set_robust_list, set_tid_address, setdomainname, setfsgid, setfsuid, setgid, setgroups, sethostname,
setpgid, setregid, setresgid, setresuid, setreuid, setrlimit, setsid, setuid, sigaltstack, statfs,
statx, symlinkat, sync, syncfs, sysinfo, tgkill, times, tkill, truncate, umask, uname, unlinkat,
utimensat, write, writev.

Partial (inside the list above):
- fcntl: no record locks (F_GETLK/SETLK/OFD: counted, ENOSYS); no leases, owners, signals.
- ioctl: FIOCLEX, FIONCLEX, FIONBIO; terminal requests give ENOTTY (no terminals yet); others ENOTTY,
  counted when the file is a device.
- mmap: no shared file mappings (counted, ENODEV); MAP_HUGETLB counted, ENOMEM. MAP_SHARED anonymous
  is exact while there is no fork.
- openat: O_TMPFILE counted, EOPNOTSUPP; named pipes counted, ENXIO; `/dev/ptmx` counted, ENODEV.
- kill, tkill, tgkill: only to the caller itself (see signals above); others ESRCH.
- execve: only from a process with one thread.
- rt_sigaction, rt_sigprocmask, sigaltstack: recorded, not delivered (K2).
- getrusage, times, CPU-time clocks: CPU time is the time since the container started.
- fallocate: mode 0 only (others counted, EOPNOTSUPP).
- prctl: SET/GET_NAME, GET/SET_DUMPABLE, GET/SET_KEEPCAPS, CAPBSET_READ, GET/SET_NO_NEW_PRIVS,
  GET_SECCOMP, GET_CHILD_SUBREAPER; others counted, EINVAL.
- statfs: block and inode counts are made up (64 MiB like Docker's tmpfs).
- ppoll: every file that exists so far is always ready (Linux's default poll mask; the host's pipes
  always have room); a wait with nothing ready sleeps until its timeout, or for ever (until K2's
  signals).
- personality: Docker's filter is modelled for this call (only 0, 8, 0x20000, 0x20008 and the query
  pass, others EPERM); the 32 bit personality gives EINVAL as on a CPU without that mode.
- getpriority: nice values are not kept yet (always 0); setpriority is missing.

Missing (205, each counted "system call not implemented" and ENOSYS): io_setup, io_destroy,
io_submit, io_cancel, io_getevents, setxattr, lsetxattr, fsetxattr, getxattr, lgetxattr, fgetxattr,
listxattr, llistxattr, flistxattr, removexattr, lremovexattr, fremovexattr, lookup_dcookie, eventfd2,
epoll_create1, epoll_ctl, epoll_pwait, inotify_init1, inotify_add_watch, inotify_rm_watch,
ioprio_set, ioprio_get, flock, umount2, mount, pivot_root, nfsservctl, vhangup, pipe2, quotactl,
pselect6, signalfd4, vmsplice, splice, tee, sync_file_range, timerfd_create, timerfd_settime,
timerfd_gettime, acct, waitid, unshare, futex, getitimer, setitimer, kexec_load, init_module,
delete_module, timer_create, timer_gettime, timer_getoverrun, timer_settime, timer_delete,
clock_settime, syslog, ptrace, sched_setparam, sched_setscheduler, sched_get_priority_max,
sched_get_priority_min, sched_rr_get_interval, restart_syscall, rt_sigsuspend, rt_sigtimedwait,
rt_sigqueueinfo, rt_sigreturn, setpriority, reboot, settimeofday, adjtimex, mq_open, mq_unlink,
mq_timedsend, mq_timedreceive, mq_notify, mq_getsetattr, msgget, msgctl, msgrcv, msgsnd, semget,
semctl, semtimedop, semop, shmget, shmctl, shmat, shmdt, socket, socketpair, bind, listen, accept,
connect, getsockname, getpeername, sendto, recvfrom, setsockopt, getsockopt, shutdown, sendmsg,
recvmsg, readahead, add_key, request_key, keyctl, clone, fadvise64, swapon, swapoff, mlock, munlock,
mlockall, munlockall, mincore, remap_file_pages, mbind, get_mempolicy, set_mempolicy, migrate_pages,
move_pages, rt_tgsigqueueinfo, perf_event_open, accept4, recvmmsg, wait4, fanotify_init,
fanotify_mark, name_to_handle_at, open_by_handle_at, clock_adjtime, setns, sendmmsg,
process_vm_readv, process_vm_writev, kcmp, finit_module, sched_setattr, sched_getattr, seccomp,
memfd_create, bpf, execveat, userfaultfd, membarrier, mlock2, copy_file_range, preadv2, pwritev2,
pkey_mprotect, pkey_alloc, pkey_free, io_pgetevents, rseq, kexec_file_load, pidfd_send_signal,
io_uring_setup, io_uring_enter, io_uring_register, open_tree, move_mount, fsopen, fsconfig, fsmount,
fspick, pidfd_open, clone3, close_range, openat2, pidfd_getfd, process_madvise, epoll_pwait2,
mount_setattr, quotactl_fd, landlock_create_ruleset, landlock_add_rule, landlock_restrict_self,
memfd_secret, process_mrelease, futex_waitv, set_mempolicy_home_node, cachestat, map_shadow_stack,
futex_wake, futex_wait, futex_requeue, statmount, listmount, lsm_get_self_attr, lsm_set_self_attr,
lsm_list_modules, mseal, setxattrat, getxattrat, listxattrat, removexattrat, open_tree_attr,
file_getattr, file_setattr.

## Tests (last run 2026-11-15)

All pass except the ignored known failures. Earlier K0 tests (hello, write, loader, faults, dispatch,
standin, boundary, layouts) still pass with the new start.
- busybox: 61 single commands of the reference image (echo, printf, cat, head, tail, wc, sort, grep,
  sed, awk, cut, tr, seq, md5sum, sha256sum, base64, od, expr, basename, true, false, ls of `/dev`,
  `/dev/shm`, `/dev/pts`, mkdir, touch -d, rm, rm -r, rmdir, mv, cp -a, ln, ln -s, chmod 4755, chown,
  readlink -f, realpath, stat, dd, tee, truncate, mknod, mkfifo, test, pwd, id, whoami, hostname, env,
  sleep, getconf, cat and readlink of `/proc/self`, ls `/proc/self/fd`, ...): same stdout, stderr,
  exit code and files changed as the reference. Commands are found on the image's PATH as Docker does.
- one_cpu: nproc says 1 (the plan's one-CPU difference).
- memory: guest/memory.c, 10 cases against the reference: mprotect then write (SIGSEGV), PROT_NONE
  read, unmapped hole, fresh memory is zero after reuse, MADV_DONTNEED, a private file mapping past
  the file's end (SIGBUS), private file mapping copy semantics, stack growth by 4 MB of frames, stack
  overflow (SIGSEGV), brk grow and shrink, running code from a page made executable and then not.
- script: `#!` scripts with and without an argument, a script whose interpreter is a busybox applet,
  and a missing interpreter (ENOENT).
- layer1: see the layer 1 section.

Known failures (ignored, with the reason in the attribute):
- busybox root_file_system: `ls -la /`, `ls -ln /etc`, `stat /etc`, `find /etc`, `du /etc/apk` show
  the root file system's own kind (overlayfs over ext4 in the reference, tmpfs here until K4).
- The three K0 ones (seccomp gaps, two unicorn alignment differences), unchanged.

## Layer 1

Calls covered (95 kinds, plus mkdirat and chdir at the start of every sequence): openat, close, read,
write, pread64, pwrite64, readv, writev, preadv, pwritev, lseek, getdents64, dup, dup3, fcntl, ioctl,
fsync, fdatasync, mkdirat, mknodat, symlinkat, linkat, unlinkat, renameat, renameat2, readlinkat,
newfstatat, fstat, statx, faccessat, faccessat2, fchmod, fchmodat, fchmodat2, fchown, fchownat,
utimensat, truncate, ftruncate, fallocate, getcwd, chdir, fchdir, umask, statfs, fstatfs, sendfile,
ppoll, mmap, munmap, mprotect, madvise, uname, getuid, getpid, getppid, gettid, getpgid, getsid,
getresuid, getresgid, getgroups, setuid, setgid, setreuid, setregid, setresuid, setresgid, setfsuid,
setfsgid, setgroups, prlimit64, getrlimit, setrlimit, getrusage, times, clock_gettime, clock_getres,
gettimeofday, nanosleep, clock_nanosleep, getrandom, sysinfo, getcpu, rt_sigprocmask, rt_sigaction,
rt_sigpending, sigaltstack, set_robust_list, prctl, personality, sched_getaffinity, sched_yield,
getpriority, capget. Arguments come from pools with bad pointers, bad descriptors, huge counts,
unknown flags and invalid values. 1 to 15 calls after the two setup calls. Everything happens under
`/dev/shm/w` (tmpfs in both) and the devices; never on the root file system. After each sequence the
runner puts back what a sequence can change outside its directory (`/dev/shm` and the three devices).

Masked on both sides: inode and device numbers and times in stat and statx, d_ino in getdents64,
statfs counts and fsid, the values of clock_gettime, gettimeofday, times, sysinfo and getrusage,
getrandom's bytes, uname's release and version, the CPU set and CPU number (one CPU is a declared
difference), the time ppoll writes back, process numbers (the reference's caller is a child of its
runner), and the address mmap returns without MAP_FIXED.

Not generated, and why: named pipes (opening one waits; pipes are K2); fd 0 (a pipe in the reference
runner, /dev/null in our container); process 1 as an argument; memory lengths above 256 MiB (our
programs' own mappings lie higher in a 4 GiB space); ppoll without a timeout.

Not yet covered by layer 1 (built, tested only through programs): brk, execve, exit, exit_group,
mremap, msync, kill/tkill/tgkill, setpgid, setsid, chroot, sethostname, setdomainname, sync, syncfs,
get_robust_list, set_tid_address, sched_getparam, sched_getscheduler, sched_setaffinity.

Cases run (all pass at the end of this run): 50 (seed 0), 300 (seed 1), 3000 (seed 2) on the first
59 calls; 500 (seed 3) on all 95. See measurements for the last long run.

## Measurements

All on this shared Mac; noisy, not benchmarks.
- Layer 1, 50 sequences, seed 0: 2.2 s including starting the reference container (load about 30).
- Layer 1, 3000 sequences, seed 2, 59 call kinds: 86 s (load 27.75 32.08 54.80 at the start).
- Layer 1, 500 sequences, seed 3, 95 call kinds: 15.4 s (load about 14).
- busybox test binary (4 tests in parallel, about 67 commands, each in both runners): 9 to 54 s
  (load 14 to 50).
- A failing layer 1 case takes one to two minutes to shrink.

## Departures from the plan

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
- Docker's default seccomp profile is still not modelled (see K0), except for `personality`, whose
  argument filter is modelled because layer 1 reaches it.
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

## Changes to the shared crate (wcontainers-abi)

- K0: `Host::output(stream, bytes)` with `Stream`; `CounterReading::detail`.
- None in K1 so far.

## Questions for the orchestrator (boundary)

- A load or store through a misaligned stack pointer is SIGBUS (BUS_ADRALN) on ARM64 Linux. `Exit` has no
  alignment fault, and the translator plan says only that unaligned access matches. Open.
- The plan's shared machine description: should release, memory size and system name move into
  wcontainers-abi's machine.rs? Not done, to avoid changing a file this side does not own.

## Known issues

- Building the test crate prints a linker warning from unicorn-engine-sys (a missing lib64 directory).
- `/proc/<pid>/fd` links of files opened by a path keep that path; a rename afterwards is not seen.

## Next (K1, in order)

1. Run layer 1 for many more cases with several seeds (the orchestrator's long run) and fix what it
   finds; extend it to the calls in "Not yet covered".
2. More busybox single programs.
3. close_range, copy_file_range, preadv2/pwritev2, mincore, mlock family, memfd_create, openat2, flock
   (single-process semantics), fadvise64, sched_* remaining, setpriority, getitimer.
4. Docker's default seccomp profile (gaps and blocked calls give EPERM).
5. Then K2.
