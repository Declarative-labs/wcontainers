# Kernel status

Plan: docs/phase-1-kernel.md. Branch: phase-1-kernel.

## Where things stand

K0 is done. K1 (one process) is complete. The seventh run closed it out; two independent checks
followed. The eighth run fixed the first check's ten problems. The ninth run (the third close-out
run) fixed the second check's six: ioctl's generic file requests, tmpfs's size and inode limits with
statfs reporting them, writes to `/proc/<pid>/comm` and the other `/proc` writes, seeks on cmdline,
environ and auxv, layer 1's narrow coverage of `/proc` files, ioctl and statfs, and sysinfo's free
memory. It also made `/proc` files' bytes at Linux's moments (not at open) and access times move on
reads. The tenth run (the fourth close-out run) fixed the third check's eight: random device reads,
getrandom and sendfile now move the whole count; sendfile of nothing succeeds before the input is
checked; `#!` lines are split as `load_script` splits them; `/proc` lists in Linux's order at Linux's
positions; fcntl and prctl gained what one process uses (owners, signals, hints, queries, the parent's
death signal, timer slack, the subreaper, huge page disabling, the saved auxiliary vector, the forms
that need powers), record locks answered as Linux without file locking (built in the eleventh run); a fault with its signal
ignored is no longer counted as a handler not run. Layer 1 now reaches the random devices. The
eleventh run (the fifth close-out run) fixed the fourth check's five blocking problems: sleeps and
waits now end as late as the thread's timer slack (seed 3141592 hung on a sleep with the largest
slack, which the reference never ends); the loader follows Linux's error rules for the named loader
(EIO, ELIBBAD, the property note) and kills the process with SIGSEGV after the point of no return; a
start with no arguments gets an empty one; argument room is `bprm_stack_limits`'s; record locks are
built (process and open file owners, joined and split ranges, let go at close); layer 1 compares the
play area after each sequence's last call. Of the minor ones, prctl's ambient powers are built; the
rest are recorded below with their reasons. The twelfth run (the sixth close-out run) found, by
reading Linux's memory calls against ours and then through layer 1, that lengths in the last page
must wrap to 0 as `PAGE_ALIGN` does (msync and mlock answered errors where Linux succeeds; an mremap
shrink by such a length unmapped the whole address space), that arm64 Linux strips a top byte tag
from the memory calls' addresses, and that `/dev/random` and the mounts files have their own poll
masks; all fixed, with layer 1 now generating those lengths and tagged addresses. See Layer 1 for the
fresh seeds. The thirteenth run (the seventh close-out run) fixed the fifth check's problems: layer 1
let machine bytes (times, inode numbers, statfs's free inodes) feed later calls and let a sequence's
filter act on the runners' own calls (both runners now zero a call's machine bytes after reporting it,
and every filter lets the runners' own calls through); mmap now follows `do_mmap` for MAP_GROWSDOWN
(refused for files and shared memory, a grows-down mapping that grows on a fault or copy below it,
mprotect's PROT_GROWSDOWN), MAP_DROPPABLE, huge page size bits under MAP_SHARED_VALIDATE and
MAP_HUGETLB on a plain file (EINVAL); madvise takes MADV_DONTNEED_LOCKED and counts the advice it does
not build (MADV_COLLAPSE, MADV_GUARD_INSTALL, MADV_GUARD_REMOVE: EINVAL, as a Linux without them);
statx reports the unique mount number when asked and each file system's own rules (devpts's as the
generic ones); newfstatat and statx take a null path with AT_EMPTY_PATH (and with a descriptor ignore
the other flags, as `vfs_fstatat` and `do_statx_fd` do, measured). Layer 1 then found a sysctl's
vectored read sized segment by segment (Linux makes one buffer of the whole request: ENOMEM at 4 MiB,
and EFAULT unless every byte made is copied out; fixed), and, once `/dev/pts` was in its paths,
devpts's root taking new names like tmpfs (Linux's has only a lookup: EPERM, and EACCES for a file
made by open or mknod; fixed), and FS_IOC_FSSETXATTR answering ENOTTY for extended flags Linux does
not know on files that keep none (Linux refuses them first, EOPNOTSUPP; and it drops the report-only
flags PREALLOC, ATOMICWRITES and HASATTR on tmpfs, measured; fixed), and sendfile from secret memory
of nothing succeeding (Linux refuses secret memory as input even then, unless the output is a pipe,
and as output at the first byte; measured; fixed). It also found a harness race under load 60 to 80:
the reference's runner counted the arena's changed ranges and then wrote them in a second pass, while
the reference kernel rewrote a registered rseq area's CPU number between the passes, which garbled
the report (it now reports from one copy). Layer 1 now generates mmap's kinds and flags (all but the
ones listed below), more advice, statx masks, null stat paths and `/dev/pts`. The alignment fault's
BUS_ADRALN and address are still not kept (only a handler can read them; K2, see Known gaps); its
visible behaviour, death by SIGBUS, matches. The one-million-sequence layer 1 run is not part of K1:
by the orchestrator's ruling it runs once at the end of the kernel track. Next is K2.

How to test everything: `nix develop -c cargo test -p wcontainers-kernel -p wcontainers-kernel-test`.
Known failures are `#[ignore]`d with their reason; see them with `-- --include-ignored`.
Needs Docker running; the reference uses alpine:3.22. Containers are named `wc-kernel-*` and removed.

Layer 1 long run (one command, for the orchestrator):
`LAYER1_CASES=1000000 LAYER1_SEED=<n> nix develop -c cargo test -p wcontainers-kernel-test --test layer1`
`LAYER1_CASES` is the number of sequences (default 64), `LAYER1_SEED` the seed (default 0). A failure is
shrunk and printed call by call with the seed; shrinking stops after 30 minutes (a step whose sequence
waits costs the reference's deadline). A sequence that runs past 60 s in the reference is killed there
(SIGKILL), which our side reports for a wait that never ends or a sleep that would end after 60 s. Seeds 100 and up are fresh
(the independent checks used 4242, 5353, 6464, 777001 to 777003 and 6060601 to 6060606).

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
- transfer `partial_copies_match_reference`: a read of a random device (or getrandom) into a buffer
  that faults part way stops up to 15 bytes short of the fault in the reference (measured: 88 where 96
  bytes were writable; 1 where fewer than 8 were), by where arm64's copy routine had stored when the
  fault came (the kernel's 64-byte block sits 8 bytes off 16-byte alignment); ours stops at the fault.
  Reads of files and `/dev/zero` stop where Linux's do (measured).
- procstatus `private_mapping_sees_later_writes`: a private file mapping is a copy made at mmap, so
  pages it has not written do not see later writes to the file, as Linux's do (counted "private file
  mapping is a copy"; procstatus `private_mapping_copy_is_counted` checks the counter).

Comparisons that are subsets rather than equality, and why:
- tests/proclist.rs `listings_are_a_subset_of_the_reference` compares listings of `/proc`,
  `/proc/self`, `/proc/1`, `/proc/self/fd` and `/proc/sys/...`: the entries we list must be the
  reference's entries that we build, in the reference's order, each with the same position after it
  (d_off), type, mode, owner, group and link count. The reference has about
  330 more entries (printed by the test): `/proc` holds a chosen subset (plan section 15). A lookup of
  any of Linux's entries that is not built is counted "proc entry not built" and warned (the list of
  Linux's names per directory is in fs/proc.rs, from the reference; procstatus
  `entries_not_built_are_counted` checks it). The link counts of `/proc` itself (Linux counts every
  process on the machine) and of `/` (the root file system's kind) are left out.
- `/dev`, `/dev/pts` and `/dev/shm` are compared whole and in order with d_off (proclist
  `device_listings_match_the_reference`), except the link count of `..` of `/dev`, which is `/`.
- tests/options.rs prints process numbers only as whether they are the caller's, its group or 0, and
  checks PR_GET_AUXV's bytes against `/proc/self/auxv` on each side (the vector itself lacks the vDSO's
  two entries here, a known gap).
- tests/sharedmap.rs sorts `/proc/self/maps` (placement differs: no address randomization here).
- tests/schedattr.rs shows time slices relative to the default (the CPU count differs).
- tests/fileioctl.rs prints FIONREAD's value on standard output only as success: what waits unread in
  the reference's output pipe depends on when Docker drained it; here the host takes bytes as they are
  written, so it is always 0. tests/tmpfsfull.rs prints inodes in use, not totals (below).

What layer 1 does not compare, and why (masked on both sides, so a difference there cannot show):
- Inode and device numbers and times in stat and statx, d_ino in getdents64: machine-specific.
- statfs and fstatfs: the inode total and the identity (fsid). tmpfs's default inode total is half
  the machine's pages, so the free inode count is compared as the count in use (total less free, on
  both sides). Block totals and free blocks are compared exactly (Docker's 64 MiB `/dev` and
  `/dev/shm`). FS_IOC_GETFSUUID's 16 bytes: tmpfs draws its identity at random.
- The values of clock_gettime, gettimeofday, times, sysinfo (memory sizes are the made-up machine's;
  free memory here is the size less every mapped and tmpfs page, counted "page residency not
  tracked") and getrusage, getrandom's bytes, uname's
  release and version, the CPU set and number (one CPU is declared), the time ppoll, pselect6 and
  sched_rr_get_interval write back, sched_getattr's runtime, mincore's vector (residency).
- Process numbers, set_tid_address's value, getcwd inside `/proc/<pid>`, a link naming a process
  directory by number, the address mmap or a moving mremap returns, `/proc`'s own link count, the CPU
  fields of rseq areas, name_to_handle_at's handle words and mount number.
- getdents64 and lseek on `/dev`, `/dev/shm`, `/proc` itself and process directories (the reference's
  runner asks after each call; "unshared"): `/dev` and `/dev/shm` outlive a sequence in the reference
  and keep tmpfs's position counter across sequences; `/proc` and process directories list the
  machine's processes and the chosen subset of entries. These listings are compared in layer 2 by
  tests/proclist.rs (above) instead: `/dev` in full and in order, `/proc` as a subset. Listings of the
  sequence's own directories are compared in full.
- A call's machine bytes never reach a later call: after each call is reported, both runners zero
  what it wrote that two machines cannot share (`machine_bytes` in layer1.rs, `clear_machine` in the
  runner, by process_vm_writev): the masked ranges above plus a stat's link count and a whole
  listing, link target, working directory and CPU set. After a failed call only arena ranges are
  zeroed, so no play page the call did not touch is brought in; a call that fails part way through a
  play page (EFAULT at a page edge) leaves its machine bytes there masked but not zeroed. getrandom,
  uname and name_to_handle_at also write only where no call reads. Before this, a clock_gettime result
  read as a later call's segment list made the check's seed 6060602 fail.
- A sequence's seccomp filters let through, first, every call the runners make themselves (write,
  exit_group, name_to_handle_at, readlinkat, fstat (made bare, as musl's fstat may also call fcntl),
  lseek, pwrite64, process_vm_readv and process_vm_writev): their arguments (process numbers, the runners' own buffers) differ between the
  runners, so a filter could treat them differently (the check's seed 6060601). The sequence's own
  calls of those numbers are therefore never filtered; filters are still tested on every other call.
- statfs's free inode count in the play area (where it cannot be turned into the count in use) is
  masked, and zeroed after the call; a failed stat or statfs call's machine fields are masked too
  (a copy that met a fault leaves part of them).
- The link count of devpts's root (inode 1 with 1024 byte blocks, as `/proc`'s root, by which the
  mask finds it); tests/proclist.rs and tests/mapflags.rs compare it. statx's unique mount number
  (Linux counts every mount the machine made).
- The play area (the 16 pages at 0x71000000 that sequences map, protect, move and advise, and that 1 in
  8 buffers point into) is compared once, after the last call: which pages process_vm_readv can read,
  and their bytes, with the same masks as the arena. Not after every call: reading a page brings it in
  (on tmpfs a read fault of a hole takes a page, which statfs and stat's blocks show), so reading it
  between calls would change what later calls see. What a call put there and a later call overwrote
  is not seen. A sequence that ends early (a fault, a filter's kill) has no play area compared. Seed
  152 (1000 sequences, mmap chosen four times as often as other calls): 2% of sequences ended with a
  readable play page, none with bytes other than zeros there, so the bytes are compared thinly.
- fcntl's F_GETLK and F_OFD_GETLK: the l_pid a test writes when it finds a process's lock (each
  runner's own process number); -1 (an open file's lock) and 0 are compared.
- The random devices are in the path pool. After a read, pread64, readv, preadv or preadv2 of one,
  both runners zero the whole range the call was given (up to the first page they cannot write, by
  process_vm_writev), and after a sendfile from one into a regular file they zero what it put there
  (pwrite at the output's old position); the reference's runner says when it did, and ours does the
  same calls. The bytes are then compared (as zeros) with everything else. A read of one that moved less
  than it was asked for (a fault part way) has its count not compared: where the copy stops differs (the
  known failure above). The test prints how many calls put a random device's bytes somewhere.
- Before each sequence both runners remake `/dev/null` once: the reference container's first
  `/dev/null` is held from outside the container once removed (measured: its inode is never given
  back), and the reference's runner remakes the devices after every sequence, so `/dev` always has one
  hidden inode there; ours is held by descriptor 0. Without this the inodes in use in `/dev` differ by
  one.

What layer 1 does not generate, and why:
- Named pipes (opening one waits; pipes are K2); descriptors 0 to 2 (the reference's runner reports
  through them); process 1 and the caller's own group negated (our caller is process 1, so -1 means
  every process); memory lengths above 256 MiB that reach memory (our programs' own mappings lie higher in a 4 GiB
  space; lengths that wrap to 0 in the last page or overflow the address are generated, and one play
  address in four carries a top byte tag); ppoll and pselect6 without a timeout; flock waits (they wait for ever).
- Signals whose default action ends or stops a process, and rt_sigaction on the signals kill sends
  (our caller is the container's first process, which Linux protects; theirs is not; handlers are K2).
- RLIMIT_CPU and RLIMIT_FSIZE limits that can be reached, process limits that can be reached (Linux
  counts the user's processes machine-wide), stack limits above 1 GiB (our stack could grow into the
  play area), core size limits above zero (core dumps are not written, counted).
- setpriority and getpriority by user, ioprio by user (they reach the reference's runner).
- openat2's restricted lookups (not built) and RESOLVE_CACHED (depends on Linux's caches); huge page
  memory files and anonymous huge pages (MAP_HUGETLB without a file: hugetlbfs is not built; with a
  file it is generated, EINVAL); MAP_POPULATE and MAP_LOCKED on file mappings (which pages a file
  mapping's faults bring in is not known here, and statfs shows them); madvise's MADV_COLLAPSE,
  MADV_GUARD_INSTALL and MADV_GUARD_REMOVE (not built, counted; tests/mapflags.rs checks the counter); seccomp listeners and the rseq time slice flag (not built); filter loads of the
  instruction pointer (the runners' differ); alternate signal stacks that could hold either caller's
  stack pointer; rseq areas where other calls write.
- brk (its values are addresses that differ by layout; tests/memory.rs and tests/limits.rs test it).
- CPU clock ids of process 1 (our caller is process 1, the reference's is not); those of the
  caller's own process and thread (encoded pid 0), of a missing process, and clock devices are
  generated. Sleeps on the profiling and user time CPU clocks: Linux samples those times at ticks, so
  a young process's may still be 0 and a sleep until a past time may never end there (seed 777002);
  here they are always the time not parked. The scheduler time clocks are slept on. tests/clocks.rs compares every kind of clock id, process 1's and the caller's own
  number included, in a program that is process 1 on both sides.
- Paths reach `/dev/shm/w` (the sequence's directory), the three zero devices, the two random devices
  (their bytes zeroed, above), `/proc/self/...` and
  five `/proc` files whose bytes are the same in both runners (`/proc/self/comm`, `cmdline`,
  `cgroup`, `limits` and `/proc/sys/kernel/ostype`: a seq_file, one read from memory, a sysctl), so
  every call is made on them; never the root file system, and no `/proc` file whose bytes differ by
  machine (status, stat, maps, auxv, environ, cpuinfo and the like are compared in layer 2 only where a
  test reads them: procwrite, procstatus, procsignals, the busybox commands, execveat, memfd).
- ioctl requests: every request of Linux's `do_vfs_ioctl` and `file_ioctl`, tmpfs's flags, two
  terminal requests, two random device requests and two unknown ones, with arguments from a pool of
  flags, extended flags, clone, deduplication and preallocation ranges, bad pointers and descriptors.
- fcntl's F_SETOWN only with numbers in use in both runners or in neither (0, 1, -1, the smallest int,
  4000000): the runners' other process numbers differ. F_SETOWN_EX is not generated (its number comes
  from memory); tests/options.rs compares it. Record locks are generated (F_GETLK, F_SETLK and the
  open file forms, from eight requests in the arena that tests rewrite), but not F_SETLKW and
  F_OFD_SETLKW: in one process they wait only on another open file's lock, for ever, which costs the
  reference's 60 s deadline; tests/recordlocks.rs runs them where they do not wait.
- prctl's PR_GET_AUXV is not generated (the vector holds addresses and lacks the vDSO's entries here);
  tests/options.rs compares it. PR_SET_TIMERSLACK only with slacks up to 1 ms: where in a long slack
  the reference's sleep ends depends on other timers' interrupts (its load), and a slack past the
  reference's 60 s deadline ends the sequence there or not by load (see Known gaps). The check's seed
  3141592 had generated the largest slack.

## What exists

- Kernel API: `Kernel::new`, `add_file(path, Attributes, NewFile)` to fill the root file system before
  the start (as unpacking an image does), `start(cpu, host, &Start { path, arguments, environment,
  working_directory, hostname, groups })`, `next`, `exit`, `event`, `counters`, `root_files()`,
  `system_calls()`.
- Exits: system calls, stops, memory faults (stack growth, SIGSEGV, SIGBUS past a file's end),
  `MemoryFaultWithoutRegisters` (SIGSEGV), `AlignmentFault` (SIGBUS as Linux's `do_sp_pc_abort`;
  BUS_ADRALN and the address reach only a handler, and handlers run in K2), illegal instructions
  (SIGILL), breakpoints (SIGTRAP). With no handler, or the signal ignored (a forced signal resets it), the
  process dies of the signal as on Linux; with a handler it dies too, counted "signal handler not run".
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
  null, zero, full, random, urandom, tty and Docker's links, made in runc's order so listings match),
  `/dev/pts` (devpts, with ptmx), `/dev/shm` (tmpfs), `/dev/mqueue` as a plain directory. fd 0 is `/dev/null` opened read-write, fds 1 and 2 are
  pipes the host reads (as Docker's; no O_LARGEFILE, as Linux's `create_pipe_files`, measured).
- Open files keep O_LARGEFILE from open as a 64 bit kernel sets it, except with O_PATH
  (`build_open_how`); F_GETFL reports the file's own flags. open, openat and openat2 take the
  descriptor right after checking the flags (`do_sys_openat2`'s `FD_ADD`), so at the limit EMFILE
  comes before any path error; a trailing slash on an existing name asks for a directory (ENOTDIR).
- tmpfs's limits (`shmem_sb_info`): pages and inode space are counted per mount: each inode, each
  extra hard link and each extended attribute (40 bytes plus name and value, not the ACLs) take inode
  space, each page a file holds (written or reserved) and each link target longer than 127 bytes takes
  a page. Writes stop at the last page that fits (ENOSPC if none), fallocate refuses a range larger
  than the mount and gives back what it took when it runs out, creating anything fails with ENOSPC
  when no inode space is left. Docker's `/dev` and `/dev/shm` hold 64 MiB; `/` and the internal mount
  of memory files have tmpfs's defaults (half the machine's pages, in pages and inodes; the internal
  mount has no limit). statfs reports them as `shmem_statfs` does (no limit: zeros); other file
  systems report zeros (`simple_statfs`). Pages a mapping's faults bring in are not known, so statfs
  on a file system with a mapped file is counted "page residency not tracked".
- ioctl (src/syscall/ioctl.rs): Linux's `do_vfs_ioctl` and `file_ioctl` for every file (FIOCLEX,
  FIONCLEX, FIONBIO, FIOASYNC, FIOQSIZE, FIFREEZE, FITHAW, FS_IOC_FIEMAP, FIGETBSZ, FICLONE,
  FICLONERANGE, FIDEDUPERANGE, FIONREAD, the inode flag and extended flag requests, FS_IOC_GETFSUUID,
  FS_IOC_GETFSSYSFSPATH, FIBMAP and the preallocation requests), then each file's own: pipes'
  FIONREAD, the random devices' `random_ioctl`; files with none answer ENOTTY as Linux does. tmpfs
  keeps its inode flags (no-dump and no-access-time can be set; immutable and append-only need
  CAP_LINUX_IMMUTABLE, which Docker's root lacks; statx shows them). FASYNC is set through a file's
  `fasync` (pipes, random devices) by FIOASYNC and F_SETFL, and fcntl names an owner; no signal is
  sent yet: the random devices signal only when the generator first becomes ready (before any
  program), and the standard stream pipes would signal when Docker reads them (pipes and signal
  delivery, K2; a known gap).
- Access times: reads, listings, `readlink` and mappings of tmpfs files move the access time as
  `touch_atime` does (`relatime` on `/`, `/dev/shm`; `strictatime` on `/dev`), except with O_NOATIME
  or the no-access-time flag.
- Devices: reads and writes leave a device's position alone; `/dev/null`, `/dev/zero` and `/dev/full`
  always seek to 0 (`null_lseek`), the random devices keep their position (`noop_llseek`).
- copy_file_range needs one superblock (EXDEV otherwise; no file system here has its own copy).
- `/proc`: self, `<pid>/` with cmdline, comm, environ, auxv, maps, stat, statm, status, mounts, limits,
  cgroup, exe, cwd, root, fd/ (magic links); cpuinfo, meminfo, uptime, loadavg, stat, version,
  filesystems, mounts, sys/kernel/{hostname, osrelease, ostype, pid_max, ngroups_max, cap_last_cap,
  random/boot_id, random/uuid}, sys/fs/nr_open, sys/vm/overcommit_memory (each with the mode of Linux's
  sysctl table: 0644 for the writable ones). Bytes are made as Linux makes them: cmdline and environ
from the process's memory and auxv from the saved vector at each read, sysctls at each read, every
other file as a `seq_file` at a read from the start or from anywhere but where the last read stopped,
or a seek elsewhere, and kept between. They seek as Linux's (`generic_file_llseek` with proc's
largest offset for cmdline, environ, auxv and directories; `default_llseek` for sysctls; `seq_lseek`
for the rest) and take writes as Linux's: comm names the process (`comm_write`), the other process
files have no write (EINVAL), the files outside process directories EIO (`pde_write`); seq files take
no positioned writes (ESPIPE); truncation succeeds and changes nothing (`proc_setattr`); a sysctl
read of 4 MiB or more is ENOMEM (`proc_sys_call_handler`'s buffer). Process files and `/dev/null`
cannot be spliced from, nor `/dev/full` to (sendfile EINVAL, measured). Sysctl
permissions follow the table's bits with no power over them (`proc_sys_permission`). Rules: a process directory refuses
  writes and changes (EPERM), its own `fd` directory is always usable (0500), `/proc/sys` is Docker's
  read-only mount (EROFS), owners follow `task_dump_owner`, link counts (9 for a process directory, 1
  for sysctl directories), an `fd` directory's size is its number of descriptors, its listing
  positions are a descriptor's number plus 2 (`proc_readfd_common`). status and stat show the blocked,
  ignored and caught signals (stat only the first 31, as `do_task_stat`); status shows FDSize as the
  table's size (`max_fds`) and the groups as `task_state` prints them. `/proc` itself, `self` and the
  entries outside the process directories keep a change of owner, mode or times (kept by inode
  number, as Linux keeps them in the root's inode and in each `proc_dir_entry`). A lookup of an entry
  Linux has that is not built is counted "proc entry not built"; opening stat, statm, status or
  mounts is counted "proc fields not tracked" (zeroed or missing fields, below).
- Memory (src/memory.rs): ranges with what the program asked for, top-down placement below
  end-128 MiB, reused ranges cleared, brk, mmap (anonymous, private file copies, `/dev/zero`, shared
  file mappings through shared blocks), munmap, mprotect, madvise, mremap, msync, mlock family. Shared
  anonymous memory and shared `/dev/zero` show in maps as Linux's removed `/dev/zero` on its internal
  tmpfs (device 0:1, an inode each, never merged with another), a private `/dev/zero` mapping as the
  device's file. A change to a file a private mapping copied is counted "private file mapping is a
  copy".
  Pages wholly past a file's end are inaccessible (SIGBUS). The stack and MAP_GROWSDOWN memory grow
  on a fault (or a copy) below them as `expand_downwards` allows: 1 MiB clear of an accessible mapping
  below that does not grow down, within the stack, address space and locked memory limits; new
  mappings are placed clear of that gap (`vm_start_gap`). Pages that may only run cannot be read (measured: the
  reference's CPU has execute-only pages).
- Shared file mappings (src/shared.rs): a shared tmpfs file's first pages live in one block of shared
  pages that every such mapping shows and the file's reads, writes, truncation and holes reach; with
  Linux's rules for VM_MAYWRITE, VM_MAYEXEC, MAP_SHARED_VALIDATE, seals, MADV_REMOVE, mremap and merging.
- Loader: ELF with PT_INTERP (the named loader placed where mmap would put it, AT_BASE), ET_DYN at 2/3
  of the address space, `#!` scripts (one argument, 4 levels, 256 byte line), execve and execveat,
  close on exec, handlers reset. Errors in `load_elf_binary`'s order before the point of no return:
  the program's magic, kind, machine and program headers (ENOEXEC; class, byte order and version are
  not checked, as Linux's are not), the loader's name (ENOEXEC for its size or a missing zero, EIO for
  a short read, EINVAL for an offset no file position holds), the loader's header (EIO when short), its
  magic, machine and program headers (ELIBBAD), then the GNU property note (`parse_elf_properties` and
  arm64's FEATURE_1_AND size). An empty loader name opens the working directory (EACCES). After the
  point of no return (`begin_new_exec`) a failure to place the program, its loader or the stack kills
  the process with SIGSEGV (`bprm_execve`); for the first process the container ends with it. A start
  with no arguments gets an empty first one (`do_execveat_common`). The strings' room is
  `bprm_stack_limits`'s (a quarter of RLIMIT_STACK, at most 6 MiB, at least ARG_MAX, less a pointer
  each), taken in Linux's order: both lists counted, the file name, the environment, the arguments,
  then a script's names. The auxiliary vector as the reference's, in its order, except
  AT_SYSINFO_EHDR and AT_MINSIGSTKSZ (counted and warned at each start; they come with the vDSO and
  signal frames in K2). Set-user and set-group bits, file capabilities, AT_SECURE, dumpability.
- Credentials: uid/euid/suid/fsuid and gids, supplementary groups (Docker's), Docker's 14 powers,
  Linux's rules for changing ids and what that does to powers and `dumpable`, capget/capset. Each
  change Linux commits (`commit_creds`: the id setters unless a no-op, setgroups, capset, keepcaps,
  dropping a bounding power, execve) gives the credentials a new serial, and an open file keeps the
  serial it was opened with (`f_cred`): linkat with AT_EMPTY_PATH walks from a descriptor only with
  the same credentials or CAP_DAC_READ_SEARCH (which Docker's root lacks), else ENOENT.
- Time: clocks from the host, coarse clocks at 1 ms resolution (measured), sleeps park the thread and
  the kernel goes idle until the time; the test host jumps its clocks instead of waiting. Sleeps
  (nanosleep, clock_nanosleep) end at the time plus the thread's timer slack, and ppoll and pselect6
  at the timeout plus `select_estimate_accuracy`'s slack (0.1% of the wait, 0.5% for a nice thread, at
  most 100 ms, at least the thread's slack); real-time policies have none, and leaving them brings the
  default back (`__setscheduler_params`). CPU time is
  the time since the container started less the time the process's threads spent parked. CPU clock
  ids below zero (`clock_getcpuclockid`, `pthread_getcpuclockid`) as Linux's `pid_for_clock`: own
  process and thread, any process, a thread of the caller's; profiling and user time at 1 ms
  resolution, scheduler time at 1 ns (measured); clock devices EINVAL, and EOPNOTSUPP for sleeps.
  Alarm clock sleeps refuse unknown flags and need CAP_WAKE_ALARM (EPERM in Docker), as measured.
- Signals within one process (src/signal.rs): sent to itself, dropped if ignored, kept pending while
  blocked, acted on when let through. The container's first process ignores signals it has no handler
  for (`SIGNAL_UNKILLABLE`); faults still kill it. A signal that would need its handler run, or would
  stop the process, is refused with ENOSYS and counted.
- Resource limits enforced: RLIMIT_FSIZE, RLIMIT_AS, RLIMIT_DATA (with `check_data_rlimit`),
  RLIMIT_STACK, RLIMIT_MEMLOCK, RLIMIT_NOFILE. RLIMIT_CPU is kept, not enforced (needs timers, K3).
- Docker's default seccomp profile (src/seccomp.rs), measured on the reference, and a program's own
  classic BPF filters, checked and run as Linux's.
- Record locks (src/locks.rs) as `fs/locks.c` keeps them: one list per inode, a descriptor table's
  locks (F_SETLK) and an open file's (F_OFD_SETLK) as owners, a process's own never in conflict, ranges
  joined and split, the first conflicting lock in list order reported by F_GETLK (an open file's with
  l_pid -1), let go when any descriptor of the file closes (not an O_PATH one) or, for an open file's,
  when its last reference goes; every check of `flock64_to_posix_lock`, `fcntl_getlk` and
  `fcntl_setlk` in order. A wait (F_SETLKW) for another holder in the same process lasts until a signal
  (K2), as on Linux.
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
- fcntl: record locks are built (above); between processes (K2) a wait must also wake when a holder
  lets go, and `posix_locks_deadlock` must refuse waits that close a loop (EDEADLK); within one process
  neither can happen. Not built, each counted "fcntl command not built" and EINVAL (Linux's answer when
  the feature is not configured): F_SETLEASE (leases: K2), F_NOTIFY (directory
  notification signals: K2), F_GETOWNER_UIDS, F_GETDELEG and F_SETDELEG, and F_GETPIPE_SZ and
  F_SETPIPE_SZ on the standard stream pipes (pipes: K2; on anything else they are EBADF as Linux's).
  Built in the tenth run: owners (F_SETOWN, F_GETOWN, F_SETOWN_EX, F_GETOWN_EX), F_SETSIG and
  F_GETSIG (no file here has an `fasync`, so no signal is ever sent, as on Linux for these files),
  F_GET_RW_HINT and F_SET_RW_HINT, F_DUPFD_QUERY, F_CREATED_QUERY, F_GETLEASE (F_UNLCK: no lease can
  be taken).
- ioctl: terminals are K2 (no terminal can be opened yet, so their requests reach no file).
  FIFREEZE, FITHAW, FIBMAP and the random devices' pool requests need powers Docker's root lacks; with
  them they answer as tmpfs does (nothing to freeze or thaw, no block map) and what is given to the
  pool cannot change the host's generator.
- mmap: MAP_HUGETLB on anonymous memory counted, ENOMEM (no huge pages; on a plain file EINVAL, as
  Linux). MAP_DROPPABLE memory is never dropped (nothing presses on memory here) and must be wiped at
  fork (K2). MAP_SHARED anonymous is private memory here,
  exact while there is no fork (K2). A private file mapping is a copy made at mmap, so pages not yet
  written do not see later changes to the file: counted "private file mapping is a copy" when such a
  file is written, truncated or allocated, or is mapped both shared and private (the CPU side does not
  say which pages were written, so the count includes pages a program had already written). MAP_SYNC
  EOPNOTSUPP, as tmpfs.
- openat, openat2: O_TMPFILE counted, EOPNOTSUPP; named pipes counted, ENXIO (pipes, K2); `/dev/ptmx`
  counted, ENODEV (terminals, K2). openat2's restricted lookups (RESOLVE_NO_XDEV, NO_MAGICLINKS,
  NO_SYMLINKS, BENEATH, IN_ROOT) are not built: counted "system call not implemented", ENOSYS, as a
  kernel before openat2 answers the whole call (Linux has had them as long as openat2); they need the
  path walk to refuse mount crossings, links and escapes, before K6's bar. RESOLVE_CACHED taken.
- kill, tkill, tgkill: only to the caller itself; others ESRCH (no other process in K1).
- execve, execveat: only from a process with one thread (threads, K3).
- rt_sigaction, rt_sigprocmask, sigaltstack: handlers recorded, not run (K2).
- getrusage, times, CPU-time clocks: CPU time is time not parked since the container started, all of
  it user time, never sampled at ticks as Linux's user and system times are (so a young process's
  profiling and user clocks are not 0 here as they may be there); with several threads (K3) a thread waiting for its turn would count. ru_maxrss is the
  mapped size, counted "page residency not tracked"; the other fields are 0 (no faults, switches or
  I/O are counted yet).
- fallocate: plain and keep-size allocation and punching holes (what tmpfs has); others EOPNOTSUPP.
- prctl: built: SET/GET_NAME, GET/SET_DUMPABLE, GET/SET_KEEPCAPS, CAPBSET_READ, CAPBSET_DROP,
  GET/SET_NO_NEW_PRIVS, GET/SET_SECCOMP, CAP_AMBIENT (eleventh run: is-set, raise, lower, clear, with
  `cap_task_prctl`'s checks), SET/GET_CHILD_SUBREAPER (kept; it acts on orphans, K2),
  SET/GET_PDEATHSIG (kept per thread, cleared on a change of ids or a secure exec; sent when the parent
  ends, K2), GET/SET_TIMERSLACK (kept; real-time threads keep theirs), GET/SET_THP_DISABLE (kept; no
  huge pages here), GET_MDWE (0: none can be set), GET_AUXV, GET_SECUREBITS (the keep-capabilities bit;
  the others cannot be set), SET_MM (EPERM without CAP_SYS_RESOURCE, which Docker's root lacks; the map
  size form), SET/GET_IO_FLUSHER (EPERM without CAP_SYS_RESOURCE). Not built, each counted "prctl
  option not built" and EINVAL (Linux's answer for an option it does not know): SET_MDWE (needs
  mmap and mprotect to refuse writable and executable memory), SET/GET_TAGGED_ADDR_CTRL (needs every
  every call that copies user memory to strip pointer tags; EINVAL is Linux's answer without the tagged
  address ABI; the memory calls and process_vm's other side strip tags already, as arm64 Linux always does),
  SET_SECUREBITS (needs the no-root, no-setuid-fixup, no-ambient-raise and exec bits applied in
  execve's and the id setters' power rules), GET/SET_SPECULATION_CTRL (the made-up machine's answers), SET_MM's map
  form and its other forms with CAP_SYS_RESOURCE, SET_VMA, the timer, MCE, FP mode, SVE, SME, pointer
  authentication, syscall user dispatch, memory merging, shadow stack and the other arch options. All
  before K6's bar.
- sendfile: secret memory is refused as input (EINVAL, even for nothing unless the output is a pipe)
  and as output (at the first byte). Moves the whole count in rounds of a pipe's 16 pages, as `splice_direct_to_actor` does,
  or one round into the standard stream pipes (`splice_file_to_pipe`); nothing to move succeeds before
  the input is checked; a round that falls short ends it, the position following Linux's.
- statfs, sysinfo, `/proc/meminfo`: pages a mapping's faults bring in are not known (counted); free
  memory is the made-up machine's size less every mapped and tmpfs page (counted "page residency not
  tracked"); `/proc/meminfo` has only some of Linux's lines (counted "proc fields not tracked", 4).
- ppoll, pselect6: every file is always ready: Linux's default poll mask, readable only for
  `/dev/random` (`random_poll`, the generator ready from the start) and the mounts files
  (`mounts_poll`), invalid for O_PATH (in every select set, as `select_poll_one`); the host's pipes
  always have room; a wait with nothing ready sleeps until its timeout, or for ever (until K2's signals). The
  signal mask they take is read, not applied (K2).
- flock: a wait for a conflicting lock never ends (one process).
- sched_setscheduler, sched_setparam, sched_setattr: kept per thread with Linux's checks; nothing is
  scheduled by them yet (K3). Custom slices kept; no utilisation clamps (EOPNOTSUPP, as the reference).
- mlock family: lock state and limits are kept; pages are not brought in (nothing to bring).
- madvise: MADV_COLLAPSE (making huge pages now: needs huge pages and knowing which pages a mapping
  holds), MADV_GUARD_INSTALL and MADV_GUARD_REMOVE (guard markers: pages that fault whatever their
  protection, kept through mprotect and MADV_DONTNEED, moved by mremap, gone with munmap; glibc falls
  back to mprotect when they are refused) are counted "madvise advice not built" and EINVAL, as a Linux
  without them answers. Before K6's bar.
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
  statm shows only the size (resident pages are not known; text and data need the loader's addresses).
  `/proc/<pid>/status` has only some of Linux's lines (no memory, SigQ, SigPnd, ShdPnd or context
  switch lines). Each open of these is counted "proc fields not tracked". Built with K2's processes,
  before K6's bar.
- About 330 of Linux's `/proc` entries are not built (mountinfo, mem, fdinfo, task, oom_score_adj,
  uid_map, smaps, entropy_avail and more; proclist prints them): ENOENT, counted "proc entry not
  built" with the entry in the detail. Those programs commonly read come before K6's bar.
- `/proc/<pid>/fd` links of files opened by a path keep that path; a rename afterwards is not seen
  (directories are followed live).
- Inodes of `/proc` are made at each lookup. chown of a process's entries succeeds with no effect, as
  on Linux; utimensat on them succeeds with no effect (Linux keeps the times while the inode is
  cached). Entries outside the process directories keep their changes (above).
- `/proc/mounts` does not list Docker's read-only and masked `/proc` mounts, `/dev/pts`, `/dev/mqueue`
  or `/sys` (counted "proc fields not tracked", detail 3).
- RLIMIT_CPU is not enforced (timers, K3); xattr space limits; huge page memory files; private file
  mappings that see later file changes; O_TMPFILE.
- K2 must abort rseq critical sections when it delivers signals, and run seccomp TRAP handlers.
- Running a program does not move its access time (Linux's `kernel_read` of the program does).
- No vDSO or vvar mapping yet (K2): Linux's vvar is VM_PFNMAP, so an madvise range that reaches it is
  EINVAL there and ENOMEM or success here (measured in the reference).
- A `seq_file` with many records (maps, mounts) is made whole at the read from the start; Linux makes
  each record as the reader reaches it, so a change between two reads of one open file can show
  partly there and not here.
- FIONREAD on the standard output pipes is always 0: the host takes bytes as they are written.
- A copy of the random devices' bytes (read, getrandom) to a buffer that faults part way stops at the
  fault here and up to 15 bytes short of it in the reference (arm64's copy routine; an ignored test
  shows it). Files and `/dev/zero` match.
- sendfile into the standard stream pipes moves at most an empty pipe's worth (the host drains them);
  a reference pipe Docker has not drained holds less room.
- busybox `su` fails here (`can't set groups: Function not implemented`): musl's `initgroups` asks
  nscd through a socket, and sockets are K5; `adjtimex` fails (missing, above).
- An alignment fault's BUS_ADRALN and address, and a memory fault's code and address, are not kept:
  only a handler can read them, and they belong in the pending signal's information, which comes with
  K2's signal queue and frames.
- Two opens of one `/proc` file are two inodes here (made at each lookup), so their open file locks
  (and flock locks) do not conflict as Linux's do.
- An open file's record locks go at its last close here; Linux keeps them while a mapping still holds
  the file.
- Loader: a segment whose file bytes lie past the file's end kills the process here (SIGSEGV); Linux
  maps it, and touching those pages is SIGBUS. PT_GNU_STACK's executable stack is not applied (the
  stack is never executable here). A program that can be placed anywhere and names no loader goes at
  2/3 of the address space here; Linux places it where mmap would. Running a program does not move its
  access time.
- Timer slack: a sleep ends at the end of its slack here; Linux's timer ends at the first timer
  interrupt on its CPU after the requested time, which on an idle machine is the slack's end
  (measured by the check: a 50 s slack slept 49.964 s; the largest slack never ended) and on a busy
  one much sooner (the options test once saw four 2 s slacks end early under load 22).

## Tests (last run 2026-11-15, thirteenth run: 63 passed, 8 ignored known failures)

All pass except the ignored known failures listed above. Added in the thirteenth run:
- mapflags: guest/mapflags.c: every kind of mmap with validated sharing and huge page sizes, MAP_SYNC,
  MAP_FIXED_NOREPLACE, an unknown bit, MAP_GROWSDOWN and MAP_HUGETLB on a file, `/dev/zero` and
  anonymous memory, droppable memory (locked, grows-down, populated, mlocked, dropped); a grows-down
  mapping grown by a read below it and not past its reach, mprotect with PROT_GROWSDOWN, PROT_GROWSUP,
  both and a high bit, a hint inside the guard gap refused, maps; MADV_DONTNEED_LOCKED on locked
  memory and a hole, advice 12, 13 and unknown ones; statx of eight files on five file systems with
  eight masks (mask, attributes, link count, which mount number); newfstatat and statx with null,
  empty and bad paths, junk flags, bad and O_PATH descriptors; a sysctl read split at a fault, too
  large in all, and across two segments; making, linking, renaming and removing names in devpts's
  root, as root and not, and its attributes; FS_IOC_FSSETXATTR of every bit on a tmpfs file and of
  known and unknown flags on `/dev/null`, secret memory and a `/proc` file; sendfile between secret
  memory, a tmpfs file, `/dev/zero` and the output pipe, of nothing and of a byte, with and without an
  offset. Same output. A second case checks that MADV_COLLAPSE and
  the guard advice are counted.
Added in the twelfth run:
- pollmask: guest/pollmask.c polls (twice, every event bit) and selects each kind of file one process
  opens: the five devices, a tmpfs file (read, write, O_PATH) and directory, `/dev`, `/dev/pts`, the
  built `/proc` files and directories, a sysctl, a memory file, secret memory. Same output.
- memory: two more cases. "wrap": msync, mlock (aligned and not), mlock2, munlock, mprotect, madvise,
  mincore, munmap, mmap and mremap (old and new length) with lengths -1, -4095, -4096, -8192 and 2^62,
  then the mapping is still there. "tagged": the memory calls and process_vm_readv (as the other side
  and as the local one) with addresses tagged 0x5a, 0xff and a bit-55 address, then a tagged munmap.
  Same output.
Added in the eleventh run:
- loaderrors: guest/loaderrors.c makes ELF programs and loaders: missing, a script, 5 bytes,
  unreadable, mode 0644, only a header, zeros, x86-64, no or short program headers, another kind with a
  bad note; loader names of sizes 0, 1, 4097, past the file, at offsets no file position holds, without
  a final zero, empty, cut by a zero; programs with bad magic, kind, machine, header size, count, offset,
  another class, and bad property notes; argument room at its edges (pointers, strings, one long one).
  A second case places a fixed loader at 0 (SIGSEGV after the point of no return, 139); a third starts
  itself with no arguments (argc 1, an empty argv[0], cmdline of one zero). Same output.
- recordlocks: guest/recordlocks.c: ranges joined, split, cut and bridged, to the end; own locks and
  F_GETLK; open file locks against the process's and each other's; F_OFD_GETLK of an unlock; waits
  that need not wait; release by closing another descriptor, not an O_PATH one, and an open file's at
  its last copy; SEEK_CUR and SEEK_END; every range, kind, pid, access and pointer error; locks on
  `/dev/null`, the output pipe, a `/proc` file, a directory and a memory file. Same output.
- options: ambient powers; four sleeps with a 2 s timer slack end within it (no longer than 3 s).
- busybox: `setpriv --dump`, and with an ambient power raised.
Added in the tenth run:
- transfer: guest/transfer.c: reads of 64 MiB, 70000, 100000 and 2 MiB (vectored) from the random
  devices, reads and writes of them at a fault, getrandom of 64 MiB, sendfile from `/dev/zero` and
  `/dev/urandom` to a file and `/dev/null` with positions, from a file across pages, and with counts of
  0 and 1 from a directory, `/dev/null`, `/proc/self/stat`, `/proc/self/cmdline`, to `/dev/full`, the
  stdout pipe and an append-only file. Same output; partial copies are the ignored known failure.
- scriptline: guest/scriptline.c: execve of `#!` lines with no name, blanks only, a zero first, a zero
  after the name or argument, a directory as interpreter, and names cut at 256 bytes. Same errors.
  script.rs also runs four lines with zeros and blanks. Same output.
- options: guest/options.c: every fcntl and prctl option built in this run with good and bad
  arguments, and a change of ids clearing the parent's death signal. Same output.
- faults: a read fault with SIGSEGV ignored dies of it with no counter.
- proclist: compares order and d_off (above).
Added in the ninth run:
- fileioctl: guest/fileioctl.c: every generic request, both bad and good arguments, on a tmpfs file,
  directory, memory file, secret memory, `/dev/null` as fd 0, the stdout pipe, `/dev/null`, `/dev/zero`,
  both random devices, `/proc/self/comm`, `/proc/cpuinfo`, a sysctl, `/proc/self` and `/dev/pts`;
  tmpfs's inode flags and extended flags set and refused, by owner and not, with statx's attributes,
  and an access time kept under the flag. Same output.
- procwrite: guest/procwrite.c: writes to comm (long, with newline, empty, bad buffer, huge count,
  vectored, positioned), writes to 18 other `/proc` files, seeks of every kind on eight files, when
  status, comm, cmdline and a sysctl make their bytes, preadv2 and pwritev2 flags on six files. Same
  output.
- tmpfsfull: guest/tmpfsfull.c: `/dev/shm` filled by writes to ENOSPC, fallocate past the limit and
  part way, truncation, holes, long and short links, hard links and extended attributes, all with
  statfs's blocks and inodes in use; `/dev`'s own limit; statfs of `/proc`, `/dev/pts`, a pipe and a
  memory file. Same output.
- atime: guest/atime.c: which reads move the access time on `/dev/shm` (relatime) and `/dev`
  (strictatime). Same output.
Added in the eighth run:
- opencheck: guest/opencheck.c: opens of "f/", "lf/", "ld/", "/dev/null/", "/etc/passwd/" and
  `/proc` entries with a slash, with O_PATH, O_NOFOLLOW and O_CREAT; copy_file_range between memory
  files, secret memory and `/dev/shm`; linkat with AT_EMPTY_PATH before and after no-op and real
  credential changes; reads, writes and seeks of the five devices; chmod and chown of
  `/proc`, `/proc/self` and `/proc/cpuinfo`; at RLIMIT_NOFILE 6 with every slot used, open of a bad
  descriptor, a bad pointer, a missing path, O_TMPFILE, openat2, memfd_create and memfd_secret. Same
  output.
- clocks: guest/clocks.c: gettime, getres (with and without a buffer) and four sleeps on every fixed
  clock id 0 to 12 and on CPU clock ids of pid 0, self, process 1, a missing process and process 2,
  for all eight low-bit values. Same output.
- procstatus: FDSize and Groups after 70 opens, dup2 to 300, close, setgroups of 3, 1 and 0 groups;
  maps lines of shared anonymous memory before and after splitting it with mprotect and merging it
  back, shared and private `/dev/zero`. Same output (inode numbers and `/dev`'s device left out:
  machine-specific); a process directory's owner after setuid, and a file's. Also: lookups of
  mountinfo and entropy_avail are counted, a missing name is not;
  the private mapping copy is counted; and the ignored known failure above.
- proclist `device_listings_match_the_reference`: `/dev`, `/dev/pts`, `/dev/shm` whole and in order.
- busybox: `cat /etc/hostname/`, `cat /dev/null/`, `find /dev`, `ls -f /dev`.
Added in the seventh run:
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

Thirteenth run (seventh close-out), after the fifth check: with the harness fixes and the sysctl
fix (head after 321c3cd's kernel work, before the generator was widened) the check's seeds 6060601 and
6060602 passed 20000 each (303 s, two side by side, load 10 to 17); 6060606 had passed 20000 on the
old head once the masked bytes were zeroed. With the wider generator, seeds 200, 202 and 6060601
passed and 201, 6060602 and 6060606 found devpts's root taking names; after that fix 201, 6060602,
6060606, 211 and 212 passed (322 s each, six side by side, load 12 to 15) and 210 found the extended
flags; after that, 210, 221 and 222 passed (340 s each, four side by side, load 18 to 59) and 220
found sendfile from secret memory; after that, 220, 231 and 232 passed (384 s each, load 70 to 64)
and 230 found the report race. On the final head 9955ebd, seed 230 and the fresh seeds 240, 241 and
242 passed 20000 each (334 s each, four side by side, load 34 at the start, 83 at the end).
Eleventh run (fifth close-out): the check's seed 3141592 (failing at case 9982 on a sleep after
PR_SET_TIMERSLACK of the largest value) passed 10000 after the slack fix (235 s, load 7 to 8). Record
locks and ambient powers are generated, mmap is chosen four times as often, and the play area is
compared after each sequence's last call. Seeds 140 (1500), 141 to 143 (3000 each, 44 s side by side,
load 17) and 160 to 162 (5000 each, 81 s side by side, load 14 to 15) passed on the way; 160 to 162
first found our runner reading the play area after a sequence's filter had killed it (a test runner
fix). Seed 174 (3000) then found, through the play area, mprotect over a mapped page and a hole
leaving the page as it was; Linux's `do_mprotect_pkey` changes each mapping up to the hole before
ENOMEM (fixed: mapping by mapping, also for EACCES and the data limit). The options test once failed
under load 22: the reference's 2 s slack sleeps ended early (other timers' interrupts), so it now shows
only that they end within the slack, and layer 1 generates slacks up to 1 ms. openat is now chosen
four times as often (most calls need an open file) and one open in eight names a random device:
seeds 174 to 177 (5000 each, 77 s side by side, load 22 to 23) passed with 1, 1, 1 and 0 calls
putting a random device's bytes somewhere (thin still; tests/transfer.rs covers those reads).
Fresh seeds 170 to 172 were started at the end of the eleventh run and stopped with it, unfinished.
Twelfth run (sixth close-out): on head c156ebd the fresh seeds 180, 181 and 182 passed 20000 each
(297 s each, three side by side, load 20 at the start, 14 at the end; 10, 8 and 6 calls putting a
random device's bytes somewhere). Reading Linux's msync, mlock, madvise, munmap and mremap against
ours then showed lengths in the last page wrapping to 0 under `PAGE_ALIGN`, which layer 1 never
generated: the play lengths now include -1, -4095 and -8192 (fixed: msync and mlock succeed as
Linux's; an mremap shrink by such a length is refused by munmap's rule instead of unmapping
everything; layer 2 case memory "wrap"). arm64 Linux strips a top byte tag from those calls'
addresses, so one play address in four is now tagged (fixed; memory "tagged"). Seed 190 (3000)
passed with the lengths, 191 (3000, 49 s, load 16 to 14) with the tags. On head e87afdc seeds 184 and
185 (20000) found ppoll of `/dev/random` reporting it writable (Linux's `random_poll`: readable only);
a poll test of every kind of file then also found the mounts files (`mounts_poll`) and O_PATH in
select's exception set (fixed; pollmask). With every fix in (head 01761aa), seeds 184 and 185 and the
fresh seeds 186, 187 and 188 passed 20000 each: 320 s each, five side by side, load 22 at the start,
16 at the end; 3, 7, 4, 5 and 5 calls putting a random device's bytes somewhere.
Tenth run (fourth close-out): layer 1 now reaches the random devices (bytes zeroed by both runners),
fcntl's owners, signals, hints, queries and pipe sizes, and prctl's parent death signal, timer slack,
subreaper, huge page, deny-write-execute, secure bits, SET_MM and I/O flusher options. Seeds 130 and
131 passed 1500 each. With every fix in (head e16c7ee), the fresh seeds 132, 133 and 134 passed 20000
each (318 s each, three side by side, load 4.6 at the start, 10.2 at the end). They reached a random
device's bytes in only 1, 1 and 0 calls: a sequence must open one of the two random paths readable
and then read from the descriptor it got, which the uniform choice of calls and paths makes rare
(about one in 20000 sequences; about 50 expected in the million-sequence run). tests/transfer.rs
covers those reads directly.
Ninth run (third close-out): layer 1 now opens five `/proc` files, makes every file ioctl request and
compares statfs's blocks and inodes in use (see "For the independent check"). Seed 100 at 1500 found
truncate of a sysctl checking the mount before the permission (Linux checks the permission first),
truncate of a `/proc` file refused (Linux takes it), a rename between `/proc/sys` and `/proc` no
longer EXDEV (a change of mine), and `/dev`'s hidden inode in the reference (above); all fixed. Seeds
100 to 103 then passed 4000 each. At 20000: 110 found sendfile moving a device's given offset, and
then sendfile from `/dev/null` (Linux: EINVAL, no `splice_read`); 111 found RWF_NOWAIT taken on a
memory file (Linux: EOPNOTSUPP); 112 found a write whose buffer faults leaving no page (Linux keeps
the page `shmem_write_begin` took); 114 found an open with both access bits asking for no permission
(Linux's `ACC_MODE` asks for reading and writing), then execve of "a/" (a link to a file) giving
EACCES (Linux: ENOTDIR); 118 found layer 1 cloning from descriptor 0, which differs between the
runners (a generator fix); 121 found a sysctl read of more than the kernel's largest buffer taken
(Linux: ENOMEM). All fixed, with layer 2 cases added (procwrite, opencheck). Along the way 111, 112,
113, 115 to 120 and 122 passed 20000 on earlier heads. With every fix in (head 817ac6e), seed 121 and
the fresh seeds 123, 124 and 125 passed 20000 each (316 s each, four side by side, load 8.5 at the
start, 10.5 at the end).
Eighth run (second close-out, after the independent check): the check's failing seeds were fixed
first (4242: open took its descriptor last; 777001: chmod of `/proc` refused; 777002: copy across
superblocks). Then at 20000 each: 777001 found fstat of an open `/proc/<pid>` directory showing the
owner from before setuid (fixed: `pid_getattr` reports it live); 777002 found a sleep until 1 us on
the user time CPU clock that never ends in the reference (Linux samples user time at ticks; such
sleeps left out of layer 1, see above); 94 and 4242 found linkat with AT_EMPTY_PATH walking from a
descriptor opened before a credential change (fixed: credential serials); 96 found MADV_PAGEOUT on
locked memory (fixed: EINVAL before a gap's ENOMEM). 777001 and 777002 then passed 20000 each (305 s,
load 8 to 17). With every fix in (head e46ef66), seeds 4242, 94, 96 and the fresh seeds 97, 98 and 99
passed 20000 each: 303 s each, six side by side, load 20 at the start and 18 at the end.

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
each sequence the runner puts back what a sequence can change outside its directory (`/dev/shm`, `/dev`,
`/` and the three zero devices, and `/proc`'s owner and mode, which `/proc` keeps).

## Measurements

All on this shared Mac; noisy, not benchmarks.
- Thirteenth run: the whole test suite of both crates 88 s wall (load 13 to 18), 181 s (load 59 to
  69); layer 1 20000 sequences: two side by side 303 s each (load 10 to 17), six side by side 322 s
  (load 12 to 15), four side by side 334 to 384 s (load 34 to 83); 1500 sequences alone 23 s (load
  15).
- Twelfth run, layer 1 20000 sequences: three side by side 297 s each (load 20 to 14); five side by
  side 320 s each (load 22 to 16); 3000 sequences alone 48 s (load 16 to 22). The whole test suite of
  both crates: 88 s wall (load 10 to 20).
- Tenth run, layer 1 20000 sequences, three side by side: 318 s each (load 4.6 to 10.2); 1500
  sequences alone: 24 to 25 s (load about 5). The whole test suite of both crates: 77 s wall (load 9).
- Ninth run, layer 1 20000 sequences, four side by side: 294 to 312 s each (load 8 to 15); three
  side by side: 298 to 305 s (load 10 to 45). 4000 sequences, three side by side: 60 s each (load 27
  to 44). The whole test suite of both crates passed under load 100 to 157 (not timed).
- Eighth run, layer 1 20000 sequences, six side by side: 303 s each (load 18 to 20); two side by
  side: 305 s each (load 8 to 17).
- The whole test suite of both crates, eighth run: 69 s wall (load about 9).
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
  sizes, RWF_NOAPPEND, RWF_ATOMIC, RWF_DONTCACHE, RWF_NOSIGNAL, CLOSE_RANGE_UNSHARE, CLOSE_RANGE_CLOEXEC,
  PIPE_DEF_BUFFERS, PR_THP_DISABLE_EXCEPT_ADVISED, SECBIT_KEEP_CAPS, AT_VECTOR_SIZE (arm64's 2 and
  the generic 24, which gives the 432 bytes PR_GET_AUXV reports, measured).
- A sleep ends at the end of its timer slack (the timer's hard end), since the made-up machine has no
  other interrupt to end it sooner (see Known gaps).
- `/proc`'s listing order and positions come from the reference's getdents listing (names and their
  order depend on the configuration); `.reset` in `/proc`'s list is the reference's.
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
- The names of Linux's `/proc` entries that are not built (for counting lookups of them) are taken
  from the reference's listing of its own kernel (Linux 7.0 with linuxkit's configuration), since which
  entries exist depends on the configuration; a lookup of a name not in that list is a plain ENOENT.
- `/dev` is filled in runc's order (Docker's mount points, runc's default devices, the `ptmx` link,
  the standard links), measured from the reference's listing and matching runc's source.
- The internal tmpfs (memory files, shared anonymous memory) has device 0:1, as Linux's `shm_mnt`
  mounted first at boot (measured in the reference's maps).
- tmpfs's identity (FS_IOC_GETFSUUID), which Linux draws at random at each mount, is a keyed hash of
  the mount's device number with the key drawn at random when the container starts (siphasher).
- FS_XFLAG_ATOMICWRITES and the set of extended flags Linux knows (`FS_XFLAGS_MASK`, internal) are
  typed in linux_extra.rs: newer than linux-raw-sys's headers; the set was measured bit by bit.
- The preallocation requests (`FS_IOC_RESVSP` and the rest) and `struct space_resv`'s size are typed
  in linux_extra.rs: they are in Linux's internal `linux/falloc.h`, not a uapi header. The file system
  identity requests are built there with `_IOR`'s rule from linux-raw-sys's structures. Layer 1 types
  the same numbers (the test crate cannot see the kernel's internal constants).
- devpts reports 1024 byte blocks (`devpts_fill_super`; measured through FIGETBSZ), and numbers its
  root 1 and `ptmx` 2 as Linux's does (measured); other inode numbers come from one counter.
- The unique mount number (`mnt_id_unique`) is `MNT_UNIQUE_ID_OFFSET` (2^31) plus the plain mount
  number: Linux counts every mount the machine has made since boot, which the made-up machine has not.
- `struct tms` (`linux/times.h`) is typed in linux_extra.rs (linux-raw-sys does not generate it);
  `sysinfo` and `stack_t` have padding, so their fields are placed by `offset_of!` on linux-raw-sys's
  structures instead of zerocopy.

## Changes to the shared crate (wcontainers-abi)

- K0: `Host::output(stream, bytes)` with `Stream`; `CounterReading::detail`.
- K1: none by this side. The seventh run merged origin/phase-1, which added `Exit::AlignmentFault {
  address }` (the translator's side) and moved a layout assertion in registers.rs to a constant
  assertion; the kernel handles the new exit (SIGBUS) and the stand-in reports it for misaligned
  fetches. Cargo.lock was regenerated by cargo for the merge (the kernel's lock plus the translator's
  packages, only additions). The eighth to thirteenth runs changed nothing in the shared crate
  (origin/phase-1 had nothing new to merge in the ninth to thirteenth).

## Questions for the orchestrator (boundary)

None open. (Answered: the alignment fault exit exists now; the machine description stays split.)

## Known issues

- Building the test crate prints a linker warning from unicorn-engine-sys (a missing lib64 directory).
- Once in the third run (load about 20 to 50) the stand-in test binary hung for over 12 minutes and was
  killed; three reruns passed at once. Cause not found; not seen since.
- Test binaries running side by side build guest programs into one directory; the dispatch test read
  that directory while another test was writing it and failed once (fixed in the eleventh run: a file
  gone since the listing is skipped).
- The object crate is built with its `unaligned` feature, so headers are read from any byte offset.

## Next

1. K2: processes, fork, wait, pipes, signal delivery with Linux's frames and a vDSO (then
   AT_SYSINFO_EHDR and AT_MINSIGSTKSZ), terminals, job control, shared memory between processes; abort
   rseq critical sections and run seccomp TRAP handlers on delivery; a fault's signal information
   (BUS_ADRALN and the address for `Exit::AlignmentFault`, the code and address of memory faults);
   droppable memory wiped at fork.
2. The missing calls above by their milestone; the known gaps above before K6's bar.
3. The one-million-sequence layer 1 run once at the end of the kernel track (orchestrator).
