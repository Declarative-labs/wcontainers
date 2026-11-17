# Kernel status

Plan: docs/phase-1-kernel.md. Branch: phase-1-kernel.

## Where things stand

K0 to K4 are done. K5 (sockets and network) has had four runs and is not complete: what is left
is under Next. The fuller run-by-run record of K4 and of K5's first two runs is in this file at
commit fdf7001 (git history); what they built is summarised here.

K5 built (all four runs):
- Local sockets of every kind (stream, datagram, sequenced packets), names (paths and abstract),
  listening, SCM_RIGHTS and SCM_CREDENTIALS, options and buffer limits as Linux; a connect by path
  moves the socket node's access time.
- epoll in full (level, edge, one-shot, exclusive, nesting, keyed wakes), poll, select and their
  signal-mask forms over every descriptor kind; eventfd2; signalfd; inotify on the in-memory file
  systems and the backend.
- TCP and UDP over IPv4 and IPv6 between the container's own addresses, wholly in the kernel
  (syscall/socket/inet.rs): ports, TIME_WAIT, connect states, resets, shutdown, poll and options at
  every level with defaults measured on the reference. sendmmsg and recvmmsg.
- Netlink route dumps (links, addresses, routes), the interface ioctls, FIONREAD and SIOCOUTQ,
  from a made-up lo and eth0 (src/network.rs: eth0 172.17.0.2/16, gateway 172.17.0.1).
- The outside through the Host hooks (syscall/socket/outside.rs): TCP connects and UDP flows;
  bytes pulled into the kernel's own queue so poll answers locally; a name server at 127.0.0.11
  answered with simple-dns through `Host::look_up`. After a restore outside connections read as
  reset (plan section 12).
- Third run:
  - Accepting from outside: a TCP listener bound to the any address that takes IPv4 or to eth0's
    address asks `Host::listen` for its port; each `ConnectionReady` on it is taken with
    `Host::accept` into its backlog while there is room (and again after an accept makes room).
    The accepted end is at eth0's address and its peer is the gateway with a random port, as
    measured for Docker's published ports. Listeners listen again after a restore. A host that
    refuses is counted "listening outside failed" and the socket listens inside alone. The test
    host publishes ports as `docker run -p 127.0.0.1:outer:inner` does
    (`driver::Container::published`, `run_ours_published`, `reference::run_published`).
  - /proc/net (syscall/socket/procnet.rs): `/proc/net` links to `self/net`; each process
    directory has `net` with dev, if_inet6, route, tcp, tcp6, udp, udp6 and unix in Linux's
    formats (padding, states, queues, timers, inodes, local sockets' flags, states and reference
    counts). The rest of Linux's names there are counted "proc entry not built". busybox netstat,
    route and ifconfig read them.
  - splice and sendfile with sockets at either end (a pipe into a socket in one message, a socket
    into a pipe as a read; sendfile into a socket through its private pipe, a send that waits
    resuming after what was sent). SPLICE_F_NONBLOCK keeps only local stream sockets from waiting
    (measured). Socket calls take their bytes from the caller or from the kernel (`socket::Data`).
  - UDP control messages: IP_PKTINFO, IP_RECVTTL, IP_RECVTOS, IPV6_RECVPKTINFO,
    IPV6_RECVHOPLIMIT, IPV6_RECVTCLASS, in Linux's order, with eth0's index for eth0's address;
    control messages that do not fit are cut to the room as `put_cmsg` does.
  - Receive timestamps: SO_TIMESTAMP and SO_TIMESTAMPNS, old and new, set and read back as
    `sock_set_timestamp` and `sk_getsockopt` (measured); a packet is stamped as it is queued for
    a receiver that asked, a datagram that came before is stamped as it is read, TCP gives the
    last buffer's time and local streams none (measured). SO_TIMESTAMPING is still counted.
  - inotify: IN_EXCL_UNLINK (events through an unlinked open file skip such watches), an
    exchange sends two pairs of moves, a replaced target gets IN_ATTRIB then IN_DELETE_SELF at
    its end, IN_MOVE_SELF and IN_DELETE_SELF carry no IN_ISDIR (measured), renames on the backend
    send their events too, and a renamed file is told of under its new name.
  - UDP true sizes measured (the small head cache: 896 bytes for small datagrams, the payload in
    pages past SKB_MAX_ALLOC); the local sockets' model is unchanged (measured to differ).
  - A TCP connection's FIN counts in its receive queue until the end is read, as Linux shows it.

- Fourth run:
  - Urgent bytes on local streams (MSG_OOB) as Linux 7.0 with CONFIG_AF_UNIX_OOB: the last byte
    of a send is the receiver's mark (`Packet::urgent`), read out of band once (it stays in the
    queue emptied), a read stops at the mark and passes an unread urgent byte (dropping it unless
    SO_OOBINLINE; Linux's queued count keeps it, `skipped_urgent`), SIOCATMARK, POLLPRI, SIGURG
    to the file's owner (`send_urgent_signal`) and a chosen signal with POLL_PRI.
  - I/O signals from local sockets with O_ASYNC (`wake_async`): data, room after a send found
    none, a peer's shutdown and close, a connection on a listener; none for data while a reader
    waits. Internet and netlink sockets accept FASYNC and send none, counted "socket i/o signals
    not built".
  - SO_PEERPIDFD, SO_PASSPIDFD with SCM_PIDFD (a process reaped since gets a descriptor that
    names it gone), SO_PASSRIGHTS (EPERM to senders of descriptors), SO_INQ with SCM_INQ;
    SO_PASSCRED, SO_PASSPIDFD and SO_PASSRIGHTS refused on internet sockets as Linux. A message
    carries credentials only when either end passes them or the receiver is not yet accepted
    (`unix_maybe_add_creds`), else the overflow ids.
  - The `system.sockprotoname` attribute of every socket's inode, and sockfs's refusals of other
    names (a removal of a `security.` name is EAGAIN, measured).
  - Internet sends read their control messages where Linux does: TCP the socket level only, UDP
    over IPv4 also IP_TTL and IP_TOS, which the datagram carries; descriptors and credentials are
    passed over; what is not built (marks, priorities, transmit times, IPv4 options and packet
    information, IPv6 messages) is refused and counted.
  - Found by layer 1 once it reached sockets (below) and fixed, each measured: fsync on a socket
    (EINVAL), socketpair writes its numbers before it makes anything, accept4's and connect's
    order of checks, sendto and recvfrom check the buffer before the descriptor, a stream read of
    nothing from an empty queue waits (EAGAIN), epoll_pwait takes the descriptor first, sendfile
    from a socket into a non-pipe is EINVAL, a read of nothing from a socket is 0, UDP's length
    and urgent flag come first and a datagram that cannot be copied out is dropped, a local
    connect finds the listener before its own state, a datagram's bytes are taken before its
    receiver is looked up, a nonblocking read of a datagram socket shut for reading is EAGAIN, a
    send to a receiver shut for reading is EPIPE for every kind, only streams send SIGPIPE,
    every socket-level option is at least an int, and a local socket answers other levels with
    EOPNOTSUPP before reading the length. TCP's MSG_OOB on receive is EINVAL (no urgent byte can
    come), a send of nothing with it is 0.

Layer 1 before the fourth run never generated sockets, epoll, eventfd or inotify: the generated
call kind was a `u8`, so only the first 256 entries of the call list (285 now) could be chosen,
and those calls are at its end. The third run's claim that seed 5050003 had them among its calls
was wrong. At K4's end the list had 257 entries, so only its last (timerfd_gettime) was never
made. The kind is now a `u16`; seeds made before this make different sequences now.

Tests added in the fourth run (each against the reference):
- tests/unixsock.rs `urgent_bytes_and_io_signals_match_reference` (guest/unixoob.c: the mark,
  peeks, two urgent bytes, inline, descriptors with an urgent byte, epoll and poll for POLLPRI,
  SIGURG, O_ASYNC signals with and without a chosen signal, a reader waiting in a read, SIGPIPE
  by kind, a listener closed with a connection waiting) and
  `process_descriptor_and_queue_options_match_reference` (guest/sockpidfd.c: SO_PEERPIDFD of a
  pair, a child and a reaped child, SCM_PIDFD with and without room and with credentials,
  SO_PASSRIGHTS, SO_INQ, the protocol name attribute and attribute rights as root and a user).
- tests/inet.rs `urgent_flags_match_reference` (guest/inetoob.c).
- Layer 1 makes accept4, sendmsg and recvmsg (headers in the arena with destinations, vectors and
  control messages: descriptors, credentials, a bare header, IP_TTL; receives with room from none
  to 128 bytes, their control area zeroed by both runners after each call, the sender's process
  number and times masked), more socket options (SO_OOBINLINE, SO_PEEK_OFF, SO_PASSPIDFD,
  SO_PEERPIDFD, SO_PASSRIGHTS, SO_INQ, timestamps, SO_LINGER, IP_PKTINFO, IP_RECVTTL,
  IP_RECVTOS, IPV6_RECVPKTINFO). A quarter of the sequences start with a local socket pair of one
  kind and a message from one end, ended by a recvmsg at the other; an eighth with a UDP socket at
  127.0.0.1:5099 that sends itself a datagram, ended by a recvmsg. The test prints how many sends
  with control messages and receives went well.

Tests added in the third run (each against the reference):
- tests/outside.rs `servers_inside_accept_clients_outside_as_linux` and
  `listeners_listen_again_after_a_restore` (guest/outsideserve.c; seed 1's save falls after the
  listens and before the clients, checked by tracing the test host's listens once).
- tests/inet.rs `proc_net_matches_reference` (guest/procnet.c: machine-specific numbers shown as
  what they are), `transfers_with_sockets_match_reference` (guest/sockxfer.c),
  `datagram_control_messages_match_reference` (guest/udpcontrol.c),
  `receive_timestamps_match_reference` (guest/timestamps.c: each time checked to fall between
  the send and the receive).
- tests/netprograms.rs `socket_and_route_tables` (busybox netstat -tulnp, netstat -lx, route -n,
  ifconfig lo).
- guest/inotify.c: unlinked files with IN_EXCL_UNLINK, exchanges, a replaced target.

Layer 2 runs repeat exactly for a seed: the test host's clock moves only by a seeded 0.5 to 1.5 us per
turn on the CPU side and by skipped waits, and the stand-in ends a turn after the slice's length in
instructions (one a nanosecond); tests/shell.rs `runs_repeat_for_a_seed` checks it.

How to test everything: `nix develop -c cargo test -p wcontainers-kernel -p wcontainers-kernel-test`.
Known failures are `#[ignore]`d with their reason; see them with `-- --include-ignored`.
Needs Docker running; the reference uses alpine:3.22. Containers are named `wc-kernel-*` and removed.

Layer 1 long run (one command, for the orchestrator):
`LAYER1_CASES=1000000 LAYER1_SEED=<n> nix develop -c cargo test -p wcontainers-kernel-test --test layer1 -- --exact sequences_match_reference`
(without the filter the two backend tests run too, at their own counts). On the backend:
`LAYER1_BACKEND_CASES=<count>` with `sequences_on_backend_match_reference`, `LAYER1_CACHED_CASES=<count>`
with `sequences_on_cached_backend_match_reference`, the same `LAYER1_SEED`.
`LAYER1_CASES` is the number of sequences (default 64), `LAYER1_SEED` the seed (default 0). A failure is
shrunk and printed call by call with the seed; shrinking stops after 30 minutes (a step whose sequence
waits costs the reference's deadline). A sequence that runs past 60 s in the reference is killed there
(SIGKILL), which our side reports for a wait that never ends or a sleep that would end after 60 s. Seeds 100 and up are fresh
(the independent checks used 4242, 5353, 6464, 777001 to 777003 and 6060601 to 6060606).

## For the independent check

Everything a reviewer should know about what is not compared, in one place.

Ignored tests (each attribute gives the reason):
- busybox `root_file_system`: `ls -la /`, `ls -ln /etc`, `stat /etc`, `find /etc`, `du /etc/apk` show
  the root file system's own kind (overlayfs over ext4 in the reference; tmpfs here, in memory or
  the backend server's directory).
- align `unaligned_exclusive_matches_reference`: unicorn faults an exclusive load from an unaligned
  address, which the reference's hardware runs. A stand-in difference, not the kernel's.
- align `misaligned_stack_pointer_matches_reference`: unicorn (QEMU) never checks stack pointer
  alignment, so the stand-in never reports `Exit::AlignmentFault` for it. The kernel's handling of
  that exit is tested through a misaligned jump in tests/faults.rs, which QEMU does report.
- memfd `huge_page_memory_files`: hugetlbfs is not built (see Partial).
- cachestat `mapped_file_pages`: the page a private mapping's read fault brings into a tmpfs file is
  counted by the reference and not known here (counted "page residency not tracked").

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
  checks PR_GET_AUXV's bytes against `/proc/self/auxv` on each side (the vector holds addresses).
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
  same calls. The bytes are then compared (as zeros) with everything else, and the count of a read
  that a fault cut short too. The test prints how many calls put a random device's bytes somewhere,
  made or opened the named pipe, opened a pseudo-terminal, and how many children were made.
- In sequences with a pseudo-terminal the reference's runner polls every descriptor (3 to 1023) twice
  after each call, which our runner does not need (its terminals move bytes at once), so ppoll is let
  through by those sequences' filters first; and writes are cut to 256 bytes, vectors to one segment,
  and every open, F_SETFL and FIONBIO keep O_NONBLOCK (see "What layer 1 does not generate").
- A child made by clone ends at once with status 7 on both sides and is waited for (not reaped)
  after the call, so waitid is let through first by those sequences' filters. clone's and wait4's
  values (process numbers), wait4's and waitid's resource use, and waitid's process number and
  times are masked and zeroed.
- Before each sequence both runners remake `/dev/null` once: the reference container's first
  `/dev/null` is held from outside the container once removed (measured: its inode is never given
  back), and the reference's runner remakes the devices after every sequence, so `/dev` always has one
  hidden inode there; ours is held by descriptor 0. Without this the inodes in use in `/dev` differ by
  one.
- Both runners' callers ignore SIGPIPE, SIGIO and the signals F_SETSIG may choose (1, 5, 8, 22, 32,
  64; guest/layer1.c and `IGNORED_SIGNALS` in layer1.rs), which Linux keeps from our caller (the
  container's first process) and not from the reference's; so what Linux does for a first process
  with these at their default is not compared, and rt_sigaction reads SIG_IGN for 1 and 64.

What layer 1 does not generate, and why:
- Terminal writes of more than 256 bytes, blocking terminal descriptors, a controlling terminal
  (every open is O_NOCTTY, TIOCSCTTY and opens of the peer without it are not generated): Linux moves
  a terminal's bytes from a work queue on another CPU, so how much a large write takes depends on when
  that work runs; a blocking read would cost the reference's deadline; and a controlling terminal
  brings job control signals that end the reference's caller and not ours. tests/terminal.rs covers
  them.
- A child that runs calls of its own (the child of clone ends at once), threads and shared tables
  (CLONE_THREAD with what it needs, CLONE_FILES, CLONE_FS: a sequence is one thread's calls, and the
  reference would run a thread on another CPU beside them; layer 2 compares them, tests/threads.rs and
  libc-test), the priority inheriting futex calls that can take a lock (they write the caller's
  thread number; tests/threads.rs compares them), CLONE_PARENT (our
  caller is the first process, which Linux refuses it to), CLONE_PARENT_SETTID
  (it writes a process number into the arena), waits without WNOHANG in sequences with a child.
- Named pipes other than the one at `p` in the work directory, made at the start of a quarter of the
  sequences; opens of it are nonblocking unless for reading and writing (a reopen through
  `/proc/self/fd` could still wait, which both runners end at the reference's deadline); descriptors 0 to 2 (the reference's runner reports
  through them); process 1 and the caller's own group negated (our caller is process 1, so -1 means
  every process); memory lengths above 256 MiB that reach memory (our programs' own mappings lie higher in a 4 GiB
  space; lengths that wrap to 0 in the last page or overflow the address are generated, and one play
  address in four carries a top byte tag); ppoll and pselect6 without a timeout; flock waits (they wait for ever).
- Signals whose default action ends or stops a process, and rt_sigaction on the signals kill sends
  (our caller is the container's first process, which Linux protects; theirs is not).
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
- Socket addresses read back (getsockname, getpeername, accept's and a receive's sender): an
  automatic local name and an ephemeral port are random. Sends to the loopback port 5099 only from
  the UDP socket a sequence starts with. tests/unixsock.rs and tests/inet.rs compare addresses.
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
- prctl's PR_GET_AUXV is not generated (the vector holds addresses);
  tests/options.rs compares it. PR_SET_TIMERSLACK only with slacks up to 1 ms: where in a long slack
  the reference's sleep ends depends on other timers' interrupts (its load), and a slack past the
  reference's 60 s deadline ends the sequence there or not by load (see Known gaps). The check's seed
  3141592 had generated the largest slack.

## What exists

- Kernel API: `Kernel::new`, `add_file(path, Attributes, NewFile)` to fill the root file system before
  the start (as unpacking an image does), `start(cpu, host, &Start { path, arguments, environment,
  working_directory, hostname, groups })`, `next`, `exit`, `event`, `counters`, `root_files()`,
  `system_calls()`.
- Exits: system calls, stops, memory faults (stack growth, SIGSEGV with SEGV_MAPERR or SEGV_ACCERR,
  SIGBUS BUS_ADRERR past a file's end), `MemoryFaultWithoutRegisters` (SIGSEGV, no handler),
  `AlignmentFault` (SIGBUS BUS_ADRALN at the address, as `do_sp_pc_abort`), illegal instructions
  (SIGILL ILL_ILLOPC at the pc), breakpoints (SIGTRAP TRAP_BRKPT at the pc). A fault's signal is forced
  (`force_sig_fault`): a blocked or ignored one goes back to the default; a handler runs with the
  fault's information and frame.
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
  `/dev/pts` (devpts, with ptmx), `/dev/shm` (tmpfs), `/dev/mqueue` (mqueue). fd 0 is `/dev/null` opened read-write, fds 1 and 2 are
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
  `fasync` (pipes, terminals, random devices) by FIOASYNC and F_SETFL, and fcntl names an owner;
  pipes and terminals send I/O signals (K2's third run); the random devices signal only when the
  generator first becomes ready (before any program), so never here. Terminals' requests are in
  src/syscall/tty.rs and pidfs's in src/syscall/pidfd.rs.
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
  end-128 MiB, reused ranges cleared, brk, mmap (anonymous, file mappings through the file's
  shared block, private ones copied page by page at the first write, `/dev/zero`), munmap, mprotect, madvise, mremap, msync, mlock family. Shared
  anonymous memory and shared `/dev/zero` show in maps as Linux's removed `/dev/zero` on its internal
  tmpfs (device 0:1, an inode each, never merged with another), a private `/dev/zero` mapping as the
  device's file.
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
  then a script's names. The auxiliary vector as the reference's, in its order,
  AT_SYSINFO_EHDR and AT_MINSIGSTKSZ included. Set-user and set-group bits, file capabilities, AT_SECURE, dumpability.
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
- Signals (src/signal.rs, src/syscall/signals.rs): see "Where things stand". The container's first
  process ignores signals it has no handler for (`SIGNAL_UNKILLABLE`); a fault's forced signal still
  kills it.
- Resource limits enforced: RLIMIT_FSIZE, RLIMIT_AS, RLIMIT_DATA (with `check_data_rlimit`),
  RLIMIT_STACK, RLIMIT_MEMLOCK, RLIMIT_NOFILE, RLIMIT_MSGQUEUE, RLIMIT_SIGPENDING, RLIMIT_NPROC,
  RLIMIT_NICE and RLIMIT_RTPRIO (the priority calls), RLIMIT_CPU (K3's fourth run); RLIMIT_CORE, RLIMIT_RSS, RLIMIT_LOCKS and RLIMIT_RTTIME are kept and bind nothing here (no
  core is written; Linux enforces neither RSS nor LOCKS; no real-time thread can exist, as Docker's
  hard RLIMIT_RTPRIO is 0).
- Docker's default seccomp profile (src/seccomp.rs), measured on the reference, and a program's own
  classic BPF filters, checked and run as Linux's.
- Record locks (src/locks.rs) as `fs/locks.c` keeps them: one list per inode, a descriptor table's
  locks (F_SETLK) and an open file's (F_OFD_SETLK) as owners, a process's own never in conflict, ranges
  joined and split, the first conflicting lock in list order reported by F_GETLK (an open file's with
  l_pid -1), let go when any descriptor of the file closes (not an O_PATH one) or, for an open file's,
  when its last reference goes; every check of `flock64_to_posix_lock`, `fcntl_getlk` and
  `fcntl_setlk` in order. A wait (F_SETLKW) ends when the lock in its way is let go or changed, or by
  a signal; one that would close a loop of holders is EDEADLK (`posix_locks_deadlock`).
- flock between open files; extended attributes on tmpfs (src/syscall/xattr.rs); memfd_create with
  seals; memfd_secret; name_to_handle_at; rseq; membarrier; protection keys as on a CPU without
  permission overlays; sched_setattr family; process_vm_readv and process_vm_writev on the caller;
  cachestat.
- Test crate: `buffer.rs` (a CPU side with no instructions, for layer 1), `standin.rs` (unicorn; a
  fetch from a pc that is not a multiple of 4 is reported as `AlignmentFault`), `image.rs`, `busybox.rs`,
  `layer1.rs` and `guest/layer1.c`, and one guest program per layer 2 test in `guest/`.

## System calls

Done (220): brk, cachestat, capget, capset, chdir, chroot, clock_getres, clock_gettime,
clock_nanosleep, clone, close, close_range, copy_file_range, dup, dup3, execve, execveat, exit,
exit_group, faccessat, faccessat2, fadvise64, fallocate, fchdir, fchmod, fchmodat, fchmodat2,
fchown, fchownat, fcntl, fdatasync, fgetxattr, flistxattr, flock, fremovexattr, fsetxattr, fstat,
fstatfs, fsync, ftruncate, futex, futex_requeue, futex_wait, futex_waitv, futex_wake, get_robust_list, getcpu, getcwd, getdents64, getegid, geteuid, getgid,
getgroups, getitimer, getpgid, getpid, getppid, getpriority, getrandom, getresgid, getresuid, getrlimit,
getrusage, getsid, gettid, gettimeofday, getuid, getxattr, ioctl, ioprio_get, ioprio_set, kill,
lgetxattr, linkat, listxattr, llistxattr, lremovexattr, lseek, lsetxattr, madvise, map_shadow_stack,
membarrier, memfd_create, memfd_secret, mincore, mq_getsetattr, mq_notify, mq_open,
mq_timedreceive, mq_timedsend, mq_unlink, mkdirat, mknodat, mlock, mlock2, mlockall, mmap,
mprotect, mremap, msgctl, msgget, msgrcv, msgsnd, msync, munlock, munlockall, munmap, name_to_handle_at, nanosleep, newfstatat,
openat, openat2, personality, pidfd_open, pidfd_send_signal, pipe2, pkey_alloc, pkey_free, pkey_mprotect, ppoll, prctl, pread64,
preadv, preadv2, prlimit64, process_vm_readv, process_vm_writev, pselect6, pwrite64, pwritev,
pwritev2, process_mrelease, read, readahead, readlinkat, readv, removexattr, renameat, renameat2, restart_syscall,
rseq, rt_sigaction, rt_sigpending, rt_sigprocmask, rt_sigqueueinfo, rt_sigreturn, rt_sigsuspend,
rt_sigtimedwait, rt_tgsigqueueinfo, sched_get_priority_max, sched_get_priority_min,
sched_getaffinity, sched_getattr, sched_getparam, sched_getscheduler, sched_rr_get_interval,
sched_setaffinity, sched_setattr, sched_setparam, sched_setscheduler, sched_yield, seccomp,
semctl, semget, semop, semtimedop, sendfile, set_robust_list, set_tid_address, setfsgid, setfsuid, setgid, setgroups, setpgid,
setitimer, setpriority, setregid, setresgid, setresuid, setreuid, setrlimit, setsid, setuid, setxattr, shmat,
shmctl, shmdt, shmget, sigaltstack, signalfd4, splice, statfs, statx, symlinkat, sync, sync_file_range, syncfs,
sysinfo, tee, tgkill, timer_create, timer_delete, timer_getoverrun, timer_gettime,
timer_settime, timerfd_create, timerfd_gettime, timerfd_settime, times, tkill, truncate, umask, uname, unlinkat, utimensat, vmsplice, wait4,
waitid, write, writev.

Partial (inside the list above), each with its reason:
- fcntl: record locks are built (above), waits between processes and EDEADLK included. Not built,
  each counted "fcntl command not built" and EINVAL (Linux's answer when the feature is not
  configured): F_SETLEASE (leases), F_NOTIFY (directory notification signals), F_GETOWNER_UIDS,
  F_GETDELEG and F_SETDELEG. Built: owners (F_SETOWN, F_GETOWN, F_SETOWN_EX, F_GETOWN_EX), F_SETSIG
  and F_GETSIG (pipes and terminals send the chosen signal), F_GET_RW_HINT and F_SET_RW_HINT,
  F_DUPFD_QUERY, F_CREATED_QUERY, F_GETLEASE (F_UNLCK: no lease can be taken), F_GETPIPE_SZ and
  F_SETPIPE_SZ on every pipe (EBADF on anything else, as Linux's).
- ioctl: terminals' requests are built (K2's second run), pidfs's (ninth run; its namespace files
  counted). FIFREEZE, FITHAW, FIBMAP and the random devices' pool requests need powers Docker's root lacks; with
  them they answer as tmpfs does (nothing to freeze or thaw, no block map) and what is given to the
  pool cannot change the host's generator.
- mmap: MAP_HUGETLB on anonymous memory counted, ENOMEM (no huge pages; on a plain file EINVAL, as
  Linux). MAP_DROPPABLE memory is never dropped (nothing presses on memory here); it is wiped at
  fork. MAP_SHARED anonymous is a shared mapping of
  an internal file. A private file mapping copies a page at its first write (above). MAP_SYNC
  EOPNOTSUPP, as tmpfs.
- openat, openat2: O_TMPFILE counted, EOPNOTSUPP; devices by Docker's rules (EPERM; the console and
  the tunnel counted, ENODEV). openat2's restricted lookups (RESOLVE_NO_XDEV, NO_MAGICLINKS,
  NO_SYMLINKS, BENEATH, IN_ROOT) are not built: counted "system call not implemented", ENOSYS, as a
  kernel before openat2 answers the whole call (Linux has had them as long as openat2); they need the
  path walk to refuse mount crossings, links and escapes, before K6's bar. RESOLVE_CACHED taken.
- getrusage, times, CPU-time clocks: CPU time is time not parked since the container started, all of
  it user time, never sampled at ticks as Linux's user and system times are (so a young process's
  profiling and user clocks are not 0 here as they may be there); with several threads (K3) a thread waiting for its turn would count. ru_maxrss is the
  mapped size, counted "page residency not tracked"; the other fields are 0 (no faults, switches or
  I/O are counted yet).
- fallocate: plain and keep-size allocation and punching holes (what tmpfs has); others EOPNOTSUPP.
- prctl: built: SET/GET_NAME, GET/SET_DUMPABLE, GET/SET_KEEPCAPS, CAPBSET_READ, CAPBSET_DROP,
  GET/SET_NO_NEW_PRIVS, GET/SET_SECCOMP, CAP_AMBIENT (eleventh run: is-set, raise, lower, clear, with
  `cap_task_prctl`'s checks), SET/GET_CHILD_SUBREAPER (orphans go to the nearest subreaper),
  SET/GET_PDEATHSIG (kept per thread, cleared on a change of ids or a secure exec; sent when the parent
  thread ends, even while its process goes on), GET/SET_TIMERSLACK (kept; real-time threads keep theirs), GET/SET_THP_DISABLE (kept; no
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
- ppoll, pselect6: they park until a file's readiness may have changed (any change to a pipe,
  terminal, queue, process descriptor or signal descriptor, any close), their deadline or a signal
  (one watching no descriptor until its deadline or a signal only),
  with their signal mask applied while they wait. Pipes, terminals, named pipes, message queues and
  process and signal descriptors have their own poll; other files are always ready: Linux's default
  poll mask, readable only for `/dev/random` (`random_poll`, the generator ready from the start) and
  the mounts files (`mounts_poll`), invalid for O_PATH (in every select set, as `select_poll_one`);
  the host's pipes always have room.
- sched_setscheduler, sched_setparam, sched_setattr: kept per thread with Linux's checks. The fair
  scheduler weighs nice values and SCHED_BATCH and SCHED_IDLE; the real-time and deadline policies
  are kept but scheduled as normal threads (K3). Custom slices kept; no utilisation clamps
  (EOPNOTSUPP, as the reference).
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
- seccomp: no listener (counted, EINVAL as a kernel without listeners). TRAP sends SIGSYS with the
  call, its address and the filter's data.
- rseq: the time slice extension flag is counted and refused with EINVAL; the reference takes it.
- process_vm_readv, process_vm_writev: only the caller's own process; another process's memory needs
  `ptrace_may_access`, counted "call on another process not built", ENOSYS.
- clone: namespaces are refused by Docker's profile as in the reference.
- futex: NUMA futexes (FUTEX2_NUMA, FUTEX2_MPOL) are counted "call flag not built", EINVAL.
- process_mrelease never frees anything: there is no dying process to free.
- pipe2: O_NOTIFICATION_PIPE counted "call flag not built", EINVAL (watch queues not built).
- cachestat, stat's blocks: pages a mapped file's faults bring in are not known (counted).
- shmget: SHM_HUGETLB counted "call flag not built", ENOMEM (the reference, with no huge pages
  reserved, answers ENOMEM). shmctl SHM_INFO's resident pages are those the kernel wrote (counted "page
  residency not tracked"). A call that both adds and removes mappings of one segment (munmap of the
  middle of an attachment: two pieces made, one closed) sets only the attach time; Linux sets both.
- vmsplice: copies the pages when called (counted "vmsplice pages copied"); Linux's pipe holds the
  caller's pages, so a later change to them shows in what is read.
- Core dumps: not written (counted "core dump not written"; Linux writes `core` and sets 0x80 when the
  limit is above zero; the reference's default limit is 0).

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

K5's first run built (not yet moved into the lists above): eventfd2, epoll_create1, epoll_ctl,
epoll_pwait, epoll_pwait2, socket, socketpair, bind, listen, accept, accept4, connect,
getsockname, getpeername, sendto, recvfrom, sendmsg, recvmsg, shutdown, setsockopt, getsockopt
(sockets of AF_UNIX only; the rest counted "socket family not built").

Missing before K5 (40, each counted "system call not implemented" and ENOSYS), with what each needs:
- Processes: ptrace (tracing is out of phase one).
- Waiting on many things (K5): epoll_create1, epoll_ctl, epoll_pwait, epoll_pwait2, eventfd2,
  inotify_init1, inotify_add_watch, inotify_rm_watch, fanotify_mark.
- Sockets (K5): socket, socketpair, bind, listen, accept, accept4, connect, getsockname, getpeername,
  sendto, recvfrom, setsockopt, getsockopt, shutdown, sendmsg, recvmsg, recvmmsg, sendmmsg.
- Asynchronous I/O (K5 or later, needs the waiting design for many requests): io_setup, io_destroy,
  io_submit, io_cancel, io_getevents, io_pgetevents.
- Single-process calls not built, each before K6's bar: adjtimex and clock_adjtime (reading the
  clock's adjustment state: the made-up machine's values; setting needs CAP_SYS_TIME, which Docker's
  root lacks), remap_file_pages (deprecated in Linux, emulated there with mmap over a shared mapping),
  landlock_create_ruleset, landlock_add_rule and landlock_restrict_self (Landlock's rules applied in the
  path walk).

## Known gaps (with reasons)

- Lock waiters wait on one channel for all files: a change to any owner's record locks, or the last
  close of an open file holding locks, wakes every lock waiter, where Linux wakes only those blocked
  on the lock that changed. A waiter woken for nothing is out of `posix_locks_deadlock`'s loops until
  its turn comes and it waits again, so a deadlock asked about in that window is not seen.
- Poll and select waiters that watch descriptors share one channel: any object's change wakes them
  all to look again, where Linux wakes only those on the changed file's wait queue. A waiter woken
  for nothing that does not block a process signal may take it before the thread Linux's
  `complete_signal` chose (seen only with `pause`, now fixed; one watching descriptors would need
  per-file wait queues).
- CPU timers and RLIMIT_CPU are looked at whenever a thread comes back from running (and its turn
  ends when one is due); Linux looks at its timer tick, so there a limit already passed when it is set
  takes up to a tick. tests/cpulimit.rs prints nothing in that window.

- `/proc/<pid>/stat`: the start time, rss, rsslim, the code, data, stack, argument and environment
  addresses and the pending signals are 0 (the loader knows the addresses but does not keep them).

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
- `/proc/mounts` does not list Docker's read-only and masked `/proc` mounts, `/dev/pts` or `/sys` (counted "proc fields not tracked", detail 3).
- xattr space limits; huge page memory files; O_TMPFILE.
- Running a program does not move its access time (Linux's `kernel_read` of the program does).
- An madvise range that reaches `[vvar]`: Linux's is VM_PFNMAP, so it is EINVAL there (measured);
  whether ours answers the same since the vDSO was built is not checked.
- I/O signals not sent: a random device's (nothing is ever waited for there). At a terminal's hangup
  Linux's `__tty_hangup` takes each file's `FASYNC` away before the line discipline's wakeup, so none
  is expected there; not measured.
  A fallback to SIGIO when a chosen real-time signal cannot be queued is not made.
- The disciplines the reference has registered besides `n_tty` (3, 14, 27; 2 for a caller with
  CAP_SYS_ADMIN) are not built: refused with EINVAL and counted, where the reference accepts them.
- The reference has four CPUs, so a forked child runs at once beside its parent; here it waits for
  the CPU under the fair scheduler. Tests wait for what a person would see (a job's program
  running) rather than for time.
- A `seq_file` with many records (maps, mounts) is made whole at the read from the start; Linux makes
  each record as the reader reaches it, so a change between two reads of one open file can show
  partly there and not here.
- FIONREAD on the standard output pipes is always 0: the host takes bytes as they are written.
- sendfile waiting for a terminal's room reads the rest of its round from the input again when it
  runs again; Linux keeps the round in its private pipe, so a change to the input file during the
  wait shows here and not there. splice from a pipe into a terminal holds the pipe's lock in Linux
  while it waits, so other readers of that pipe wait too; here they do not.
- Where the kernel's source of a copy lies within 16 bytes decides what a buffer that faults part
  way receives; it is measured for the calls tests/partialcopy.rs makes and assumed aligned to 8
  elsewhere (Linux's stack layout of each call is its compiler's). Calls Linux copies field by field
  with `put_user` that are not listed under "Where things stand" are copied whole here.
- sendfile into the standard stream pipes moves at most an empty pipe's worth (the host drains them);
  a reference pipe Docker has not drained holds less room.
- busybox `su` fails here (`can't set groups: Function not implemented`): musl's `initgroups` asks
  nscd through a socket, and sockets are K5; `adjtimex` fails (missing, above).
- A memory fault's syndrome in a handler's frame: whether the page was present and the level of the
  missing table are Linux's page tables' own; given as a level 3 translation fault for a page that
  allows nothing or is unmapped, a level 3 permission fault otherwise (counted "fault syndrome
  approximated"; measured: the reference reported level 2 for an address in an empty 2 MiB region).
  tests/sigframe.rs prints a translation fault's syndrome without its level.
- PSTATE.BTYPE: the reference's CPU has BTI and its frames show BTYPE after an indirect branch
  (measured 0x800 after a jump to a misaligned address); the made-up CPU has no BTI (HWCAP2 0) and the
  register block keeps only the flags, so ours shows 0. tests/sigframe.rs prints only the flags.
- Shared anonymous memory is an internal tmpfs file whose bytes are a `Vec` the size of the mapping,
  as every tmpfs file here; a large shared reservation costs that much kernel memory at once.
- A memory file's or tmpfs file's pages are not counted against user pipe limits; pipes' per-user
  page limits (`pipe-user-pages-soft`) are not modelled.
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

## Tests (counts and the last whole-suite run under Measurements)

All pass except the ignored known failures listed above. Added in K4's fifth run: tests/backend.rs
`truncations_and_appends_match_reference`, `transfers_match_reference` and
`attributes_read_as_a_user_match_reference` (Built in K4's fifth run). Added in K3's fifth run: timers
`cpu_clock_sleeps_match_reference` (guest/cpusleep.c, with 2 saves a seed: sleeps on the process's
clock, another thread's and another process's, relative and absolute, passed and zero times, a
handled signal with what is left, a bad address for it, a stop and continue); threads
`priority_inheriting_futexes_match_reference` (guest/futexpi.c, with 3 saves a seed: every PI
call's checks and their order, waiters on a condition requeued onto a held lock and handed it in
turn, a free lock taken from the requeue, plain wakes and requeues on PI waiters, an owner ending
with a waiter); guest/futexes.c a vector wait naming one futex twice, woken once and twice. Added in
K3's fourth run: cpulimit
(guest/cpulimit.c, RLIMIT_CPU's SIGXCPU and SIGKILL against the reference); terminal's ttylock now
starts with `cat /dev/tty | cat`; clonevm checks the shared dumpable flag. Added in K3's first run (seeded ones run
under `SCHEDULE_SEEDS` seeds, 3 by default; 4 to 6 were run by hand):
- threads: guest/threads.c (pthread creation and joining, mutex counting, condition variables in
  turn, barriers, read-write locks, semaphores, thread ids, robust locks after a thread's and a
  child process's end, a priority inheriting mutex, process and thread directed signals with
  per-thread masks and alternate stacks, the first thread leaving), the same saved 6 times per seed,
  guest/futexes.c (every futex operation's checks, waits ended by wakes, time-outs on both clocks
  and a signal, wakes by bits, requeue, wake-op, PI locks alone and handed between threads,
  futex2, the thread id address) and guest/threadlife.c (clone flag combinations, the group's
  status, exit_group from a thread, execve from a thread, stop and continue with a busy thread,
  SIGTERM).
- timers: guest/timers.c (interval timers on the real and CPU clocks, alarm, every POSIX timer
  check, notifications, values, overruns, signals dropped after delete, a thread's timer, CPU clock
  timers, timer descriptors with reads, poll, ticks and absolute times).
- rwmodes (RWF_NOWAIT on every kind of file, fallocate modes 0 to 255), procsignals `continued`,
  terminal `splice_holds_the_pipe`, rseqabort `preempted_sections_match_reference` (on one CPU).

Added in K2's tenth run (each guest's
reference output was read and checked against Linux's source):
- zombie: guest/zombie.c: a child that has ended and is not yet reaped: stat's fields we know
  (state, parent, group, session, terminal, priority, nice, threads, sizes, rsslim, masks, policy,
  exit code), status lines, every process file's open and read, the three links and opens through
  them, the `fd` directory, its CPU clocks, kill, tkill, tgkill and sigqueue, getpgid, getsid and
  setpgid (it moves), prlimit get and set, affinity, priority, pidfd_open, process_vm_readv,
  children's times before the reap; after the reap its clock and stat; a group whose only member has
  ended (kill, joining it, setsid refused while it is unreaped).
- turns: guest/turns.c against the reference on one CPU: a parent spinning on sched_yield until a
  child has run (within 100 yields), yields until a blocked child is seen asleep, and a child that
  only makes system calls beside one that must still run. The old kernel failed it.
- userlimits: guest/userlimits.c: RLIMIT_SIGPENDING of 5 with sigqueue, kill and tkill of real-time
  signals, a standard sigqueue, SIGCHLD, every signal taken with its code and value, the place given
  back; RLIMIT_NPROC for root, after setuid, with another task of the user, over it at setuid (execve
  refused twice), and beside a task that has ended and is not yet reaped.
- unknown: fcntl commands and prctl options Linux does not know or has only for other architectures
  (plain EINVAL, nothing counted), and two Linux has that are counted.
- saving: `processes`, `deadlines` and guest/clockwatch.c (four processes sleeping, polling and
  computing while the monotonic and boot clocks are checked) saved 8 times for each of 3 seeds.

Added in K2's ninth run (each guest's reference output was read and checked against Linux's
source):
- deadlines: guest/deadlines.c: nanosleep, clock_nanosleep with an absolute time, ppoll, pselect6,
  sigtimedwait, semtimedop and mq_timedreceive of 20 ms each while a child computes for ever (with
  and without system calls in its loop), a sleep beside two such children, and a short sleep that
  ends before a computing child does. Ours hung for ever before the fix.
- lockwaits: guest/lockwaits.c: a record lock waiter woken by an unlock, two readers by a change to
  read (a writer not), an unlock elsewhere leaving the waiter waiting, a close of another descriptor
  and the holder's end waking it, loops of two and three processes refused with EDEADLK (and the
  later try EAGAIN), a wait behind an open file's lock, an open file's wait, signals with and without
  SA_RESTART; flock waiters woken by a conversion to shared, an unlock, the last close, and a signal.
  Waiters are seen asleep through `/proc/<pid>/stat`'s state, so nothing depends on time.
- orphans: guest/orphans.c: a group orphaned by its leader's end with a stopped member (SIGCONT and
  SIGHUP reach it), a group whose outside parent is the first process (SIGTSTP stops its member), an
  orphan made with exit signal SIGUSR1 reported to its new parent by SIGCHLD.
- pidfd: PIDFD_GET_INFO of the caller in sizes 64, 72, 80 and 96 and with each mask bit, of a
  running, ended, reaped and killed child, another user's child and one that may not dump; sizes,
  directions and numbers refused, null and bad pointers, FS_IOC_GETVERSION; the namespace requests
  with an argument, on ended and reaped children, another user's and an undumpable one. The caller's
  own namespace requests are made but not printed (they give a descriptor in the reference); the test
  checks they are counted.
- posixmq: fstatfs of a queue from mq_open and of one opened by path.

Added in K2's eighth run:
- terminal `making_pairs`: TIOCSETD with 2 (EPERM), 4, -1 and 31 on both sides; `waiting_for_room`:
  sendfile and splice into a full blocking terminal finish once the master is read, and a signal ends
  a waiting sendfile with the part written (how much fits is not printed, see Departures).
- sigio: close_range over a writer and two readers in both orders, dup2 over the writer, closes after
  it; the terminal echo is read until whole (the reference's buffer work handed it over in two parts
  once under load).
- posixmq: preadv2 of a queue with RWF_HIPRI and with RWF_NOWAIT. signalfd and pidfd: an O_PATH
  descriptor given to signalfd4, pidfd_send_signal and waitid.

Added in K2's seventh run (each guest's
reference output was read and checked against Linux's source):
- sysvsem: semget's keys, sizes and limits, semop whole or not at all, operations naming a semaphore
  twice, IPC_NOWAIT on the operation that blocks, the order waiting operations finish in (simple and
  on several semaphores), waits for zero, GETNCNT and GETZCNT, timeouts, signals, stops, kills and
  removal during waits, undo at exit (held to the range, cleared by SETVAL, shared by CLONE_SYSVSEM,
  dropped with the set, left by a failed call), every semctl command, another user's permissions.
- posixmq: mq_open's names, flags, modes and limits, the queue files and directory and what the file
  calls do with them, sends and receives by priority, sizes, nonblocking and timed calls, waiting
  receivers and senders handed messages in order, signals with and without SA_RESTART, a killed
  waiter, notification by signal and its rules, attributes, unlinking, RLIMIT_MSGQUEUE (6 default
  queues fit beside the others), another user.

Before: Added in K2's sixth run (each guest's
reference output was read and checked against Linux's source):
- priorities: nice values and I/O priorities of one process, a group with a running child, an ended
  child and another user's child, a user's processes, refusals and their order, inheritance by fork.
- pidfd: pidfd_open's flags and refusals, the file (status, flags, link, file system, handle), poll
  through a process's life, pidfd_send_signal in every scope and with information, waitid on a
  descriptor (nonblocking, stops, continues), CLONE_PIDFD, process_mrelease, and every generic call
  (attributes, truncation, fallocate, sync, mmap, reopen, exec, locks, splice) on a process descriptor
  and a signal descriptor.
- signalfd: the file, flags, sizes and refusals, reads of every information layout, several at once,
  into a bad buffer, waits and polls woken by another process, the mask changed, inheritance by fork.
- sysvmsg: msgget, msgsnd and msgrcv with every search and flag, limits, IPC_SET, the info commands,
  three receivers handed messages in the order they waited (one E2BIG), a sender waiting for room, a
  removal waking a receiver (EIDRM), a signal ending a wait (EINTR even with SA_RESTART), another
  user's refusals.

Added in K2's fourth run:
- partialcopy: guest/partialcopy.c makes 47 kinds of call (stat family, statfs, uname, the clocks,
  sysinfo, rusage, times, limits, signal masks and actions, pipe2, getcwd, scheduling, readlink of a
  `/proc` link and a tmpfs link, getdents64, reads of a tmpfs file, a pipe, a terminal, `/proc` seq
  files, cmdline, a sysctl, the random devices, getrandom, getresuid, getgroups, getcpu, wait4,
  ppoll, pselect6, prctl, FIONREAD, sendfile's offset) with its buffer ending 0 to N bytes before a
  read-only page, and prints the end of what each stored and whether it failed. Same output.
- transfer `partial_copies_match_reference` is no longer ignored (it passes).
- sigio: a terminal writer that found no room hears of it when the reader empties the other side,
  once per write that found none.

Added in K2's third run (each guest's reference output was read and checked against Linux's source):
- privatemap: guest/privatemap.c: later writes by pwrite and by a shared mapping seen by pages not
  written, the kernel's read into a page, MADV_DONTNEED, mprotect, MAP_POPULATE writable and read
  only, MADV_POPULATE_WRITE and READ, mlock, truncation (SIGBUS for copies past the end, the pages
  showing the file after growing), a punched hole, mremap, fork. procstatus
  `private_mapping_sees_later_writes` now passes.
- fifo: guest/fifo.c: nonblocking opens, read and write opens, ENXIO, polls before and after writers,
  the buffer and its size dropped at the last close, stat, lseek, the `/proc` link, a reader waiting
  for a forked writer, a nonblocking writer let in by a waiting reader, a signal ending a wait
  (EINTR) and restarting it (SA_RESTART).
- devopen: guest/devopen.c: device nodes made with mknod (EPERM outside Docker's rules); the tunnel's
  counted ENODEV against the reference's working device.
- rseqabort: guest/rseqabort.c (above).
- sigio: guest/sigio.c: pipe readers and writers with SIGIO and F_SETSIG, a duplicate closed, the last
  writer closed, SIGCHLD chosen (SI_SIGIO), O_ASYNC at open, a terminal's partial line, line end and
  raw byte, a master taking the caller as owner (each terminal check polls first, as Linux's buffer
  work runs later; the master's echo is read so its poll takes in the next input).

Added in K2's second run:
- terminal: guest/tty.c (one part per test): making pairs (the locked slave, stat of the slave and
  the master, `/proc` links, TIOCGDEV, numbering, devpts's listing, TIOCGPTPEER, default settings,
  window size, and the answers of TIOCSTI, TIOCEXCL, TIOCSETD, TIOCSIG, TIOCSCTTY, TIOCNOTTY,
  TIOCVHANGUP, lseek and bad arguments); line editing (every editing key with each ECHO* setting,
  control characters, tabs, high bytes, small reads, input flags, every output flag, echo columns
  after output); non-canonical reads (raw settings, the switch back to canonical, VMIN and VTIME,
  flushes, queue sizes); flow control and size (stop and start keys, TCOOFF/TCOON/TCIOFF/TCION, IXANY,
  TIOCSWINSZ, packet mode's status bytes); closing (the master reading EIO after the slave closed and
  again after it reopened, the slave hung up after the master closed, its node gone). guest/jobs.c:
  sessions and controlling terminals, background reads and writes (SIGTTIN, SIGTTOU with TOSTOP and
  for settings, ignored, blocked, orphaned), signal keys to the foreground group and its flush,
  SIGWINCH, TIOCSIG, a hangup, a session leader's end, TIOCNOTTY. guest/shpty.c: `sh -i` with job
  control on a terminal (commands come through a pipe so the shell does no line editing of its own;
  busybox's line editing asks the terminal for the cursor and drops input read together with the
  answer). Each guest was run three or four times in the reference with the same output; the
  reference's buffer work is asynchronous, so the guests read the slave before the master (a read
  that finds nothing waits for the work) and wait for echoes or polls where a race was seen.

Added in K2's first run:
- processes: guest/processes.c: memory after fork (shared anonymous memory, a shared file mapping,
  private memory, MADV_DONTFORK and MADV_WIPEONFORK, vfork sharing), fork and exit statuses (exit,
  SIGTERM, SIGSEGV, abort), wait with no children and a stranger, WNOHANG, waitid with WNOWAIT, vfork,
  exec of `/bin/sh` in a child, five children reaped; pipes (sizes, FIONREAD, nonblocking ends,
  filling, poll, F_SETPIPE_SZ, a child writing 300000 bytes, no reader with SIGPIPE ignored and
  default, packet mode, bad flags); signals (a handler's code, SIGCHLD's information, pending while
  blocked, a sleep interrupted with the time left, a read restarted with SA_RESTART and interrupted
  without, stop and continue seen by waitpid, a group kill, sigtimedwait and sigqueue); an orphan
  handed to the first process. Same output.
- sigframe: guest/sigframe.c: the vDSO (magic, type, machine, maps lines), AT_MINSIGSTKSZ, frames for
  a raised signal and for faults (write and read of an unmapped page, a read-only page, a page that
  allows nothing, an undefined instruction, a breakpoint, a misaligned jump): code, fault address,
  flags, records and syndromes, si_addr, the mask in the handler and in the frame, the alternate
  stack, a one-shot handler, a re-entrant one, a handler that moves the pc past a fault, a handler
  with no restorer returning through the vDSO, floating point kept. Same output.
- saving: see "Where things stand".
- shell `runs_repeat_for_a_seed`: four background pipelines run twice with one seed give the same
  output and the same counters, down to the measured wake-to-run nanoseconds; two other seeds give the
  same lines.
- shell: busybox sh with pipelines, exit statuses through pipes, redirections, subshells, command
  substitution, background jobs and wait, traps, kill, descriptors opened by exec. Same output, exit
  code and files.
Added in the thirteenth run:
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

K3's fourth run: with woken calls run at their turn and the lock wake change in (head 39cd8a2),
fresh seeds 3006, 3007 and 3008 passed 20000 sequences each (333 s each, three side by side beside
the whole suite, load 12 at the start, 17 at the end). Then, with every fix of the milestone in
(head 82981eb, after the dumpable and RLIMIT_CPU changes, which layer 1's prctl, setresuid and
setrlimit reach), the milestone's three fresh seeds 3009, 3010 and 3011 passed 20000 sequences each
(318 s each, three side by side beside the whole suite, load 48 at the start, 39 at the end). Seeds
used: up to 737, the checks' seeds, 3001 to 3011.

K3's third run, with its clone and credential changes in (before the deferred rerun, which was taken
back): fresh seeds 3004 and 3005 passed 20000 sequences each (324 s each, side by side with 3003,
load 23 at the start, 14 at the end). Seed 3003 found readv on a nonblocking timer descriptor with
two buffers (4 and 4 bytes) answering EINVAL where Linux answers EAGAIN: a timer descriptor's read
needs eight bytes in all and writes them across the buffers (measured with readv: 4+4, 0+8, 2+10
and 3+5 read 8 bytes; 7+0 and 0+7 EINVAL); ours wanted the first buffer to hold all eight. Fixed
(`signalfd::store` writes across buffers for both); 3003 then passed 20000 (307 s, load 18). These
three count for K3 only if nothing after them changes what layer 1 reaches; the deferred rerun
would (see Next).

K2's ninth run: with the check's fixes in (head after the scheduler indexes), the check's seed 91003
passed 30000 sequences (507 s, beside the others, load 35 at the start, 20 at the end), so the
mqueue fstatfs fix holds. Fresh seeds 730 and 731 passed 20000; 729 found copy_file_range into a
process descriptor at an output position past 2^31 answering 0 where Linux answers EFBIG
(`generic_write_check_limits` runs even for nothing to copy, and a file opened without O_LARGEFILE
is held to `MAX_NON_LFS`; ours skipped both). Fixed for every write path. With every fix in (head 1152610), seed 729 and the fresh seeds 732, 733 and 734 passed
20000 sequences each (303 s each, four side by side beside 91003, load 19 at the start, 22 at the
end). Seeds up to 734 are used.

K2's eighth run: TIOCSETD's disciplines come from a table (see "Where things stand"). Seed 711 passed
20000 after that fix (345 s alone, load 31 to 44). Then, with the terminal, pipe release and process
state work in (head 54a857f, before the queue fix), seeds 721 and 723 passed 20000 (334 s and
393 s, three side by side beside the whole suite, load 39 to 44); 722 found preadv2 with RWF_NOWAIT on a
message queue (Linux's queue files have no iterating read, so `do_loop_readv_writev` refuses every
flag but RWF_HIPRI before copying: EOPNOTSUPP; ours copied into a bad buffer: EFAULT). After that fix
722 and 724 passed 20000 (330 s and 329 s, three side by side, load 20 to 36); 725 found signalfd4
given an O_PATH descriptor (Linux's `fdget` passes over it: EBADF; ours EINVAL), and the process
descriptor calls and mq_notify had the same fault; fixed (head b01b1de).
With every fix in (head b01b1de), the milestone's three fresh seeds 726, 727 and 728 passed 20000
sequences each (335 s each, four side by side with 725, which also passed, beside the whole suite;
load 23 at the start, 52 at the end).

K2's sixth run: with the fixes for 516, 517 and 518 in (head 603f689), those seeds passed 20000 each
(335 to 344 s, three side by side, load 82 at the start, 40 to 48 at the end). Then process
descriptors, signal descriptors and message queues are generated (see "Where things stand"). Seeds
600, 601 and 602 (3000 each) found writev, fsync, fallocate and truncate on a process descriptor
(Linux: no write, no fsync, pidfs's setattr refuses), signalfd4 reading its mask before its flags, and
mmap of a process descriptor with PROT_EXEC (pidfs is `SB_I_NOEXEC`: EPERM); 603 to 605 (3000 each)
found MSG_COPY loading the caller's buffer before the queue (EFAULT) and sendfile from a process
descriptor (EINVAL even for nothing). All fixed; then 600 to 605 passed 3000 each (49 to 51 s, three
side by side, load 14 to 40). At 20000 (head 1f9cffe, four side by side, load 16 to 37): 608 passed
(339 s); 511, 606 and 607 found extended attributes on descriptors (Linux's security layer passes over
private inodes, so `security.` names reach "no attributes": EOPNOTSUPP, measured). After that fix 511
and 606 passed 20000 (330 s each, three side by side, load 9 at the start, 56 at the end); 607 then
found a pipe's reading end showing O_DIRECT (only the writing end takes it, `create_pipe_files`;
fixed); 607 then passed 20000 (350 s alone, load 62 at the start, 33 at the end; head 7d54a79). Seed 511's SHM_HUGETLB fix from the fifth run is confirmed by its pass.

K2's fifth run: shmget, shmat, shmdt, shmctl, splice (twice), tee and vmsplice are generated.
Segment identifiers differ between the runners (the reference's slots and sequence numbers carry
over between sequences), so an argument `SEGMENT` is replaced by each runner with its sequence's last
identifier from a successful shmget (4000, a slot neither reaches, before one), as `SELF` is; shmget's
value, SHM_STAT's, IPC_INFO's and SHM_INFO's (the highest slot) and an unplaced shmat's are not
compared, nor a status's sequence number, times and processes or SHM_INFO's resident pages. After
each sequence the reference's runner removes every segment, each as its creator (and makes itself
dumpable again, which the change of user took away and later sequences would inherit; seed 500
found that). Sizes stay within our address space. splice, tee and vmsplice always carry
SPLICE_F_NONBLOCK (vmsplice ignores O_NONBLOCK, and a named pipe opened for reading and writing is
its own writer), and splice into a terminal is capped like write. Seed 500 (3000) also found our
runner taking a failed shmget's error as an identifier. Seeds 500, 501 and 502 passed 3000 each
(52 to 79 s, load 50 to 125). At 20000: seed 512 passed (466 s, beside the whole suite and two
other runs, load 50 to 67); 513 passed (395 s, five side by side, load 25 to 95). Found and fixed:
sendfile answers EOVERFLOW past the largest file of either side's file system (`MAX_NON_LFS` for
pipefs, devpts and secret memory, whose `s_maxbytes` stays as `alloc_super` sets it; seed 511),
TIOCGPTPEER with O_PATH gives a file even for a locked slave (no open is run; seed 510), shmat and
shmdt keep an address's tag (the tagged address ABI leaves them out: ENOMEM, EINVAL; seeds 514,
515), and the runners' links to pipes name each machine's own inode numbers, now not compared
(seed 510). Then seeds 514 and 515 passed 20000 (335 s each, six side by side, load 30 to 85);
511 found shmget's SHM_HUGETLB answering EPERM to a caller outside group 0 without CAP_IPC_LOCK
(`can_do_hugetlb_shm`; fixed, not rerun). Open, not looked into yet: seed 516, madvise of a tagged
address 0x5a00000071006000 with length 0x10000000 and advice 17: ours ENOMEM, theirs EINVAL; seed
517, sendfile with the same descriptor as input and output and an offset from the arena: ours
EINVAL, theirs EOVERFLOW; seed 518, ioprio_get of the caller after earlier calls: ours 24576, theirs
16388. Each is replayable with `LAYER1_CASES=20000 LAYER1_SEED=<seed>`.

K2's fourth run: seed 410 (statfs into a buffer that faults part way) passed 20000 after the copy
fix (337 s, load about 60 to 90). Terminals generated (see "Where things stand"): seed 420 (1000)
found O_DIRECT on `/dev/tty` and then a reference runner that did not put back `/dev/tty`, `/dev/ptmx`
and `/dev/pts/ptmx` after a sequence changed them (the runner now remakes them); seed 423 (2000)
found sendfile from a terminal and RWF_NOWAIT on one; all fixed, then 423 (2000) and 424, 425, 426
(5000 each, 83 s, three side by side, load 50 to 90) passed with about 2500 terminal opens each.
Fork generated: seed 427 (2000) found the reference's runner keeping the orphans of earlier
sequences, which `kill(-1)` then reached (the runner now reaps them); seed 428 (5000) found a wait
for stops alone answered 0 with only an ended child (Linux: ECHILD), and that the runner's wait for a
child with no exit signal needs __WALL; seed 431 (5000) found sendfile into a terminal. All fixed;
428, 429, 430, 431 and 432 passed 5000 each (82 to 86 s, load 50 to 90), with 100 to 300 children
made per 5000 sequences. Then fresh seeds at 20000 (clone weighted three times): 440 found chroot
into `/proc/sys` reaching `/proc/sys/dev/tty` (now built: `ldisc_autoload` and `legacy_tiocsti`,
both 1, measured), 441 found sendfile from a terminal into a pipe (it reads the terminal as
`copy_splice_read` does: EAGAIN when nonblocking and empty) and then a wait for process 3, a child's
number here and no one's there (sequences with children now wait for 4000000 instead), 442 found
`kill(-1)` refused for permission everywhere answering EPERM (Linux: 0, `kill_something_info`) and
then a filter answering clone with ERRNO 0, which the reference's runner took for being the child
(it now asks getpid). All fixed. Seed 440 passed 20000 before the last runner fix; with every fix in
(head 99e83eb) seeds 441, 442 and the fresh 443 passed 20000 each (331 s each, three side by side,
load 25 at the start, 63 at the end; about 10000 terminal opens and 1100 children each), and then the
fresh 444 and 445 passed 20000 each (393 s and 333 s, side by side, load 104 at the start, 34 at the
end). So the fresh 443, 444 and 445 passed with every fix in on head 99e83eb. Seeds up to 445 are used.

K2's third run: with named pipes generated, seed 409 found that a named pipe opened with O_DIRECT
answers its open's own error first (ENXIO) and EINVAL only after the open (`do_dentry_open`); seed
410 found that locking droppable memory that allows nothing is ENOMEM (measured; readable droppable
memory locks with no error and is not counted), then that the file memfd_create makes refuses
O_DIRECT in F_SETFL (it is not opened through tmpfs's `open`, so lacks FMODE_CAN_ODIRECT; the same
file opened again takes it). All fixed. Seeds 409, 412, 413 and 414 passed 20000 each before the last
fix (memfd). With every fix in, the fresh seeds 415, 416 and 417 passed 20000 each (319 s each, four
side by side, load 48 at the start, 12 at the end). Seed 410 then found one more difference, not
fixed (see Next): statfs into a buffer that crosses into an unmapped page fails EFAULT in both, but
Linux's `copy_to_user` has written the bytes before the fault and ours writes none (`Guest::write`
is all or nothing). Seeds up to 417 are used.

K2's second run: the milestone's fresh 20000-sequence seeds. Seeds 400 and 402 passed; 401 found that
rt_sigqueueinfo read the whole 128-byte `siginfo` and checked the target first, where Linux copies
the kernel's 48 bytes, then for a code whose layout it does not know (`known_siginfo_layout`) checks
the other 80 bytes are zero (E2BIG) before anything else. Fixed. With every fix in, seed 401 and the
fresh seeds 403, 404 and 405 passed 20000 each (320 s each, four side by side, load 84 at the start,
31 at the end). Layer 1 still generates no terminal calls (see Next).

K2's first run: with the new calls, seeds 300 (2000), 301 and 302 (3000 each) found three things:
shared anonymous memory took mmap's offset (Linux starts it at 0), rt_tgsigqueueinfo compared the
caller with the group, not the thread (EPERM first), and waitid writes the information's fields (zeros
when nothing is found or it fails, EFAULT for a bad pointer) as its wrapper does; seed 303 found a
pipe mapped with PROT_EXEC refused ENODEV where pipefs's SB_I_NOEXEC gives EPERM first. All fixed; 300
(2000, 42 s, load 55 to 62), 301 and 302 (3000, 50 s each, three side by side, load 63 to 20) passed
after their fixes. With every fix in (head 10645dd), seeds 303, 304 and 305 passed 3000 each (49 s
each, three side by side, load 16 at the start, 10 at the end). The milestone's three fresh 20000
seeds are not run yet (they run once at the milestone's end).

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

- K4, fifth run, final, at head deb0e5c (every fix in): plain layer 1 seeds 4040510, 4040511,
  4040512 each passed 20000 (328.1 s, 388.0 s, 327.3 s) and the harness's failing seed 4040506
  passed 20000 (326.7 s), four side by side with the whole suite, load 17.8 at the start and 18.5 to
  20.5 at the end. Whole suite (`RUST_TEST_THREADS=3 cargo test --no-fail-fast -p
  wcontainers-kernel -p wcontainers-kernel-test`): 151 passed, 0 failed, 5 ignored, 6 min 53 s,
  load 17.8 at the start and 17.4 at the end. Backend seed 4040116 and caching backend seed 4040315
  each passed 2000 (116.8 s, 117.1 s, side by side, load 19.4 to 15.8).
- K4, fifth run, at heads f7cf6fc to 717998b: backend seeds 4040113 to 4040115 and caching backend
  seeds 4040312 (the directory seek seed, after its fix) to 4040314 each passed 2000 (139 to 192 s,
  load 12 to 50). Plain 4040504 and 4040505 passed 20000 (328.0 s, 328.3 s) before the layer 1
  harness fix. A whole-suite run at 717998b beside five other jobs (load 12 to 46) had two failures
  of the reference's known races: terminal's `making_pairs` (`exec /wc/tty: no such file`, the
  shared guest directory) and futexpi's print order (the woken waiter printed before the unlocking
  thread); the final suite run above passed both.
- K4, fifth run, at head aecc367 (the fourth run's end): plain layer 1 seeds 4040401, 4040402,
  4040403 each passed 20000 (330.0 s, 330.7 s, 329.6 s; side by side with each other and the
  suite; load 16.2 at the start, 28.6 at the end). Backend seed 4040111 passed 2000 (144.3 s,
  load 15 to 25). Caching backend seed 4040311 failed (the access control list finding above).
- K4, fifth run, at head c5b36f8 (lock, splice and list fixes in): plain layer 1 seeds 4040501,
  4040502, 4040503 each passed 20000 (322.0 s, 322.4 s, 323.0 s; side by side with each other, the
  suite and two backend runs; load 23.3 at the start, 30.6 at the end). Caching backend 4040311
  passed 2000 (164.7 s); backend 4040112 passed 2000 (191.0 s); caching backend 4040312 failed (the
  directory seek above). Whole suite (`RUST_TEST_THREADS=3 cargo test --no-fail-fast -p
  wcontainers-kernel -p wcontainers-kernel-test`): 150 passed, 0 failed, 5 ignored, 8 min 18 s,
  load 23.3 at the start and 17.1 at the end.
- K4, third run: layer 1 on the backend seed 4040103 (failing before on statx) passed 2000
  (1723.7 s; docker execs were slow then, load 14 to 18). On the backend that opens files for the
  page cache (`sequences_on_cached_backend_match_reference`): seed 0 passed 32, 4040301 passed
  2000 (1784.6 s, same time and load). With pipes, splice, tee and cachestat added to the backend
  calls (head 3cad455): direct 4040104 passed 300 (20.3 s), 4040105 passed 2000 (123.5 s); cached
  4040302 passed 300 (20.3 s), 4040303 passed 2000 (123.4 s); load 14 to 17.
- K4, third run: the milestone's fresh plain layer 1 seeds with every fix in (kernel at a2b3fcf,
  the same at 3cad455): 4040205, 4040206 and 4040207 each passed 20000 (326.6 s, 326.4 s,
  326.2 s), run side by side with each other and with the whole suite, load 14.5 at the start
  and 34.2 at the end.
- K4, third run: whole suite (`RUST_TEST_THREADS=3 cargo test --no-fail-fast -p
  wcontainers-kernel -p wcontainers-kernel-test`) at head 3cad455 beside the three layer 1 runs:
  144 passed, 0 failed, 5 ignored, 6 min 47 s, load 15.0 at the start and 16.3 at the end.
- K4, second run: layer 1 on the backend (`sequences_on_backend_match_reference`): seed 0 passed
  300 (18.1 s); seed 4040101 passed 2000 (113.7 s), 4040102 passed 2000 (118.1 s), load 11 to 15.
  Seed 4040103 failed at 2000 on statx with an empty mask (Known issues). Fresh plain layer 1 seed
  4040201 passed 20000 (333.4 s, load about 11 to 15) at head 49f67c3 (before the direct IO read and
  ACL read fixes, which touch only backend files).
- K4, second run: the milestone's fresh layer 1 seeds at head ee59eab, every fix in: 4040202
  passed 20000 (331.2 s), 4040203 passed 20000 (331.0 s), run side by side with the whole suite
  (load 13.2 at the start, 17.0 at the end), and 4040204 passed 20000 (346.0 s, load 13.5 to 16.0).
- K4, second run: whole suite (`RUST_TEST_THREADS=3 cargo test --no-fail-fast -p wcontainers-kernel
  -p wcontainers-kernel-test`) at head ee59eab beside two layer 1 runs: 141 passed, 0 failed, 5
  ignored, 6 min 29 s, load 13.8 at the start and 14.5 at the end.
- K4, second run: `messages_per_call` gave the same counts as the first run's (below) with FLUSH at
  every close.
- K4, first run: layer 1 seed 9393002 (the check's failing seed) passed 30000 with the SIGPIPE fix
  (478.4 s, load 13.5 at the start, 32.2 at the end). Fresh seed 4040001 passed 20000 at head
  52212a9 (312.1 s, load 18.7 at the start, 12.7 at the end).
- K4, first run: whole suite (`RUST_TEST_THREADS=3 cargo test --no-fail-fast -p wcontainers-kernel
  -p wcontainers-kernel-test`) at head c4e2bc3, beside one layer 1 run: 135 passed, 0 failed, 6
  ignored (one new: backend file mappings), 7 min 10 s, load 13.6 at the start and 14.0 at the end.
- K4, first run: messages each kind of call sends to the backend, 100 of each less the setup,
  measured by tests/backend.rs `messages_per_call` (seed 1; opcodes: 1 LOOKUP, 2 FORGET, 3 GETATTR,
  9 MKDIR, 10 UNLINK, 11 RMDIR, 12 RENAME, 14 OPEN, 15 READ, 16 WRITE, 18 RELEASE, 25 FLUSH,
  27 OPENDIR, 28 READDIR, 29 RELEASEDIR, 35 CREATE). Paths are `d/f` from the working directory, the
  backend's root:
  - stat: 2 LOOKUP, 1 GETATTR (the working directory's attributes for the search check).
  - fstat: 1 GETATTR.
  - open and close: 2 LOOKUP, 1 GETATTR, 1 OPEN, 1 FLUSH, 1 RELEASE.
  - pread of 4 KiB: 1 READ. pwrite of 4 KiB: 1 WRITE.
  - opendir, readdir to the end, closedir of 12 entries: 1 LOOKUP, 1 GETATTR, 1 OPENDIR, 2 READDIR,
    1 FLUSH, 1 RELEASEDIR (no FLUSH since the fourth run, as Linux; measured again then, the other
    kinds unchanged).
  - create, close and unlink: 4 LOOKUP, 2 GETATTR, 1 CREATE, 1 FLUSH, 1 RELEASE, 1 UNLINK, 2 FORGET.
  - mkdir and rmdir: 4 LOOKUP, 2 GETATTR, 1 MKDIR, 1 RMDIR, 2 FORGET.
  - rename: 4 LOOKUP, 1 GETATTR, 1 RENAME.
  The backend test (453 messages a run) takes about 1.1 s a seed including the reference container
  (load about 15).

- K3, sixth run: layer 1 fresh seeds 3030, 3031, 3032 at 20000 each, side by side (load 13 to 32):
  3031 passed (318.4 s), 3032 passed (317.8 s), 3030 failed at its timer_create (fixed above).
  Seed 3023 passed 20000 after the harness fix (313.8 s, load 13.8 to 14.3); the checker's seed
  5150002 passed 25000 after the splice fix (384.7 s, load 36 to 120).
- K3, sixth run, with every fix in (head after the timer charge): fresh layer 1 seeds 3033 and
  3034 passed 20000 each (306.4 s and 306.3 s, side by side with the suite; load 31 to 144), and
  3030 passed 20000 on rerun (317.0 s, load 67 to 32). Whole suite, same command as below: 129
  passed, 1 failed, 5 ignored, 6 min 18 s, load 44.1 at the start and 25.0 at the end; the failure
  was `process_signal_takers_match_reference`, where the one-CPU reference chose thread 1 (Known
  issues).
- K3, sixth run: whole suite (`RUST_TEST_THREADS=3 cargo test --no-fail-fast -p wcontainers-kernel
  -p wcontainers-kernel-test`) at head 1712fe7, beside three layer 1 runs: 129 passed, 1 failed, 5
  ignored, 6 min 6 s, load 15.8 at the start and 67.8 at the end. The failure was terminal's
  `splice_holds_the_pipe` (ours printed `cat /dev/tty | cat`'s line once, the reference twice); it
  passed 3 reruns alone (load 55). See Known issues.

- K3, fifth run: layer 1 with every fix in, fresh seeds 3025, 3026 and 3027 passed 20000 sequences
  each (304 to 307 s, four side by side with seed 3023; load 17 to 22). Along the way, 20000 each:
  3013 and 3014 passed (before the sendfile fix); 3015, 3016, 3018, 3020, 3021 failed and were
  fixed (above), then passed on rerun; 3017, 3019, 3022, 3024 passed; 3023 fails in a whole run
  (Known issues). Seed 3012 passed 4000 (60.6 s, load 13.6) with the PI calls first generated.
- K3, fifth run: the whole suite (`RUST_TEST_THREADS=4 cargo test --no-fail-fast -p
  wcontainers-kernel -p wcontainers-kernel-test`) at head 9bd0ea1: 124 passed, 1 failed, 5 ignored,
  6 min 22 s, load 13.64 at the start and 17.08 at the end. The failure was the new futexpi guest's
  print order (the reference ran the woken waiter before the unlocking thread printed); fixed in the
  guest and passed 3 reruns. At head 96d35e3 (`RUST_TEST_THREADS=3`, beside layer 1 runs): 124
  passed, 0 failed, 5 ignored, 6 min 28 s, load 17.26 at the start and 17.50 at the end.
- K3, fifth run, libc-test at head a3b3e5e (with the varied speed of turns): the 43 thread, signal,
  timer and IPC tests under seeds 300 to 309 (`SCHEDULE_SEEDS=10 SCHEDULE_SEED=300`), with and
  without saves, and the rest under one seed, all matched the reference (3 tests passed); 501.8 s,
  load 14.70 at the start, 26.65 at the end.
- K3, fifth run: guest/cpusleep.c matched the reference under seeds 1 and 2 (harness, load 10 to
  15); the check's timermix.c matched under seed 1 and differed under seed 2 in one line, "alarm
  after 1us itimer" (a 1 µs timer read back by the next call: whether it has fired depends on the
  time between two calls, 0.5 to 1.5 µs a turn on the repeatable clock; not in the suite).
- K3, fourth run: the whole suite (`RUST_TEST_THREADS=4 cargo test --no-fail-fast -p
  wcontainers-kernel -p wcontainers-kernel-test`) with the deferred rerun in (head 39cd8a2): 119
  passed, 1 failed, 5 ignored, 5 min 47 s, load 12.08 at the start, 16.20 at the end; the failure was
  clonevm, whose new dumpable line was built before the kernel change (the reference shares the
  flag). At the end (head 82981eb, `RUST_TEST_THREADS=3`, beside three layer 1 runs): 120 passed, 1
  failed (sigio, the reference's known race: it counted 0 signals for the raw byte and 2 for the
  output to the master; three reruns passed), 5 ignored; 6 min 1 s, load 47.08 at the start, 32.96
  at the end.
- K3, fourth run, libc-test at head 82981eb: the 43 thread, signal, timer and IPC tests under seeds
  200 to 209 (`SCHEDULE_SEEDS=10 SCHEDULE_SEED=200`), without saves and with saves (430 runs each),
  all matched the reference; 486 s, load 36.61 at the start, 57.72 at the end.
- K3, fourth run: guest/cpulimit.c takes about 15 s here (2 s of guest CPU time at one instruction
  a nanosecond, load 37 to 45); 2.3 s in the reference.
- K3, third run, at its end (timerfd fix in): the whole suite passed but for sigio (119 passed, 1
  failed, 5 ignored; 4 min 26 s, load 19.83 at the start, 13.74 at the end); sigio's reference
  counted no signal for output to the master where ours, and three reruns of both, counted one (see
  Known issues).
- K3, third run: the whole suite (`RUST_TEST_THREADS=4 cargo test --no-fail-fast -p
  wcontainers-kernel -p wcontainers-kernel-test`, default case counts) with the clone and
  credential changes: 119 passed, 1 failed (pidfd: PIDFD_GET_INFO gave the caller's ids, fixed and
  pidfd run again alone, passed), 5 ignored; 4 min 42 s, load 35.10 at the start and 30.40 at the
  end.
- K3, second run: the whole suite (`RUST_TEST_THREADS=4 cargo test --no-fail-fast -p
  wcontainers-kernel -p wcontainers-kernel-test`, default case counts) passed: 118 tests, 0 failed,
  5 ignored, about 212 s of test time, load 24.26 at the start and 16.56 at the end; again at the
  end of the run with CLONE_VFORK threads: 118 passed, 0 failed, 5 ignored, about 244 s, load 24.85
  at the start and 26.23 at the end.
- K3, second run, libc-test: the 97 other tests under seed 1 and the 43 thread, signal, timer and
  IPC tests under seeds 1 to 3 with and without 4 saves (129 runs each) matched (72.5 s for all
  three, load about 33). Seeds 100 to 109 (430 runs each): without saves all matched (load about
  33); with saves 11 differed before the wall clock fix (above) and all 430 matched after it (84 s,
  load 24.26 at the start, 22.66 at the end). Typical times here: 3 to 30 ms a test, 1 to 3 s with
  saves; raise-race 2.6 s, daemon-failure 5 s (2^20 dups).
- K3, second run, guest/threadsched.c on the reference pinned to one CPU, three runs: identical
  output each time, the weighted share and yield turns both within their bounds.

- K3, first run: layer 1 seed 93103 (the K2 check's) passed 25000 sequences with the RWF_NOWAIT fix,
  378.88 s beside other work (load 36.75 at the start, 29.12 at the end).
- K3, first run: the whole suite (`cargo test --no-fail-fast -p wcontainers-kernel -p
  wcontainers-kernel-test`, default case counts) passed with the stand-in fix: 111 tests passed, 0
  failed, 6 ignored (the known failures), about two minutes, load 34.30 at the start and 17.43 at
  the end. The run before it, without the fix, also passed (110 and 7 ignored).
- K3, first run: the reference showed SIGCHLD already pending right after kill(SIGCONT) in one run
  of guest/continued.c (the child ran on another CPU first), and not in earlier ones, so that line is
  no longer compared; the test now checks the wait and the one CLD_CONTINUED signal only, and does
  not tell the old behaviour (the signal sent inside kill) from the new. guest/procsig.c of the K2
  check showed the difference by hand (above).
- K3, first run, with the new futex and timer calls in layer 1: seed 3001 passed 2000 after three
  fixes (31 s, load about 40); seed 3002 passed 6000 (93.17 s, load 18.56 at the start, 33.00 at
  the end).

All on this shared Mac; noisy, not benchmarks.
- K2 and earlier: in this file at commit aecc367 (git history).
- A failing layer 1 case takes one to two minutes to shrink.

## Departures from the plan

- K5, second run, the made-up network (plan section 15 allows it; these are its choices): lo
  (1) and eth0 (2) only, where the reference's kernel (Docker Desktop's linuxkit) also shows
  eight tunnel devices and numbers eth0 11; eth0 is 172.17.0.2/16 with hardware address
  02:42:ac:11:00:02 and MTU 65535 (the reference's), where Docker gives each container its own
  address and a random hardware address; traffic counters read zero (counted). The name server
  is 127.0.0.11, as Docker's own for user networks, where the reference's default bridge names
  Docker Desktop's 192.168.65.7. Tests hide what Docker chooses per container.
- K5, second run, TCP between the container's own addresses: how much a connection holds is the
  sender's SO_SNDBUF of unread bytes (counted "tcp room modelled" when a send finds none), where
  Linux's depends on timing (windows, buffer growth); writable at two thirds empty as
  `__sk_stream_is_writeable`. A connect to a full backlog waits and is completed by the accept
  that makes room (counted "tcp connect waits for backlog room"), where Linux drops the SYN and
  retries after 1, 3, 7 seconds and so on. Nagle and delayed acknowledgements are not modelled:
  every send arrives at once. An end that closes first holds its port for TIME_WAIT from the
  moment both FINs are sent, or from its release if the other end's FIN never came (Linux:
  FIN_WAIT2, then TIME_WAIT). UDP true sizes are near Linux's, not equal (92 of 1000-byte
  datagrams fill the reference's receive buffer).
- K5, second run, the outside: a UDP socket has one host connection per outside destination
  (the hooks carry no source address). What an outside connection's bytes look like to poll comes
  from the kernel's own queue, pulled on `ConnectionReady` and at each read. Name queries are
  answered with TTL 0 (the lookup hook gives none) and only for A and AAAA (others "not
  implemented", counted). The test host maps `host.docker.internal` and its address to this
  machine's loopback, so one server serves the reference and ours.
- K5, second run: inotify's directory for a call by path is where the inode was made or last
  moved to (inodes keep no other dentries), so after a hard link Linux may name the other link;
  IN_DELETE_SELF waits for the last open file (descriptor tables are searched), not for
  mappings.
- K5, first run: a backend directory's later pages are listed with plain READDIR. Linux also
  uses READDIRPLUS for them when a name in the directory was looked up afresh, or first
  revalidated after READDIRPLUS made it, since its last listing (`FUSE_I_ADVISE_RDPLUS`); which
  names Linux's dentry cache holds is not modelled. INIT asks for no lock flags and the kernel
  sends no COPY_FILE_RANGE, IOCTL or POLL (above).

- K4, fifth run: a backend inode holds its access control lists between reads, as Linux's
  `i_acl` does, although the plan says the kernel caches nothing in phase one: without it, which
  reads answer ENODATA and which EOPNOTSUPP differs from Linux (a permission check by another
  user holds none). They are let go of at every LOOKUP, GETATTR and INVAL_INODE, as Linux does.

- K4, fourth run: a backend file's writes hold its lock alone always; Linux shares it among
  writes inside the file when the backend opened it with FOPEN_PARALLEL_DIRECT_WRITES (the first
  backend does not). A shared file's position lock is taken whatever the file's count of holders
  (Linux takes it only when shared); unshared, nothing else can wait for it, so nothing differs.
- K4, fourth run: open with O_CREAT through a link to a missing file on the backend sends one
  READLINK and one LOOKUP more than Linux (the link is read again and the target looked up again).

- K4, second run: the image is unpacked into the backend's directory by the server's container
  (busybox tar) before the server starts, not as FUSE messages from the kernel; the plan's section 1
  says tests unpack an image into a directory.
- K4, third run: a mapped backend file's pages are read whole at mmap (from the file's start to the
  mapping's end), and a program on the backend whole at exec, where Linux reads each page at its
  first fault (counted "backend file read whole when mapped" since the fourth run, with a warning); a change to the file through a
  direct-IO descriptor after the mmap and before a page is first touched shows on Linux and not
  here. Pages are written back whole, not only those written (counted). The plan's "keeps no
  cache" is kept apart from these pages, which correct mappings need. INIT does not ask for
  FUSE_DIRECT_IO_ALLOW_MMAP or FUSE_SETXATTR_EXT (flags2 is 0), which Linux asks for; the first
  backend grants neither, so the reference behaves the same.
- K4, second run (changed in the third): Linux's FUSE keeps a file's attributes until the next
  jiffy even with an attribute timeout of 0; the kernel asks again in every call that has not had
  them. statx with no basic field or with AT_STATX_DONT_SYNC takes the held attributes, as Linux
  does with no sync (seed 4040103); within a jiffy Linux may still answer other stats from held
  attributes where ours asks.
- K4, second run: FLUSH for a descriptor closed by anything but `close` is sent with nothing
  waiting for it; Linux's `filp_close` waits for it (and ignores its error).

- K4: the reference for the backend runs the server as the container's first process and the program as
  its child, in a container with /dev/fuse and CAP_SYS_ADMIN (dropped from the program's sets), so
  Docker's seccomp profile there is the one for CAP_SYS_ADMIN; the program is not the first
  process there.
- K4: a call waiting for the backend is not interrupted (no FUSE_INTERRUPT); Linux sends INTERRUPT
  on a signal and goes on waiting. A fatal signal ends the process at once here, and the answer
  that comes later gives back what it handed out.
- K4: an open file's path for `/proc/<pid>/fd` on the backend is the name it was opened by, not a
  followed link's target (walking again would ask the backend again).
- K4: requests the backend took before a save are answered after it by the host that goes on with
  the backend (the test host's copy); a host that cannot must bring the backend back with them.

- K3, sixth run: setting the wall clock is not compared with the reference. Docker gives no
  CAP_SYS_TIME, and adding it would set the clock of every container on this shared machine (time
  namespaces do not cover CLOCK_REALTIME). The test's expected lines come from Linux's source
  (`clock_was_set`, `timerfd_clock_was_set`, `timerfd_read_iter`, `common_hrtimer_arm`). The kernel
  sees a setting only at a restore (as Linux at resume); the host gives no sign of a step of its own
  wall clock during a run, so then timers on the wall clock keep their monotonic time and nothing is
  canceled.

- libc-test runs each test through its own runner with a 600 s time limit (`runtest -t 600`), not
  its default 5 s: the stand-in CPU on a loaded machine is far slower than the reference, and only a
  test that never ends should time out.

- A fatal signal ends a process at once here, where Linux ends it when it next runs. So
  process_mrelease never finds a dying process with memory to free: a running process is EINVAL and an
  ended one ESRCH; Linux answers 0 in the window before the victim runs (a race in the reference).

- The root file system is in memory (tmpfs), or on the backend (K4, the server's tmpfs directory in
  tests). Tests fill it from `docker export` of the reference image. What shows the file system's
  own kind differs from the reference's overlayfs over ext4 (busybox `root_file_system`, ignored as
  a known failure); layer 1 stays off it.
- Docker's `/etc/hostname` is given the container's name; `/etc/hosts` and `/etc/resolv.conf` keep the
  image's empty placeholders (contents come with the network, K5) with Docker's mode 0644. The test
  programs' directory `/wc` is a plain directory here, a read-only bind mount in the reference. `/sys`
  is the image's empty directory (Docker mounts sysfs there).
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
- Saving natively copies the whole test process (fork) rather than only the kernel's memory: the
  kernel's state is a graph of Rust heap objects shared through `Rc`, which only a copy at the same
  addresses keeps, as copying a WebAssembly kernel's memory into a new instance will (K7). The copy
  is a new kernel instance in a new process; it goes on with a new CPU side built from the saved
  memories, blocks and registers alone and a new host whose clocks read differently, so what is
  proven is that the kernel needs nothing else and that `restore` carries its clock over. Deep
  copying the kernel inside one process was not done: it needs a copy routine for every type that
  shares (`Rc` inodes, files, terminals, counts), which no crate makes for `no_std`.

- Layer 2's clock is not the wall clock: time passes only by a seeded step per turn and by skipped
  waits, so CPU times and the scheduler's choices repeat for a seed. Layer 1 keeps the wall clock (its
  reference runs beside it).
- What one terminal side writes is taken into the other side's line discipline at once, as if
  Linux's buffer work (`flush_to_ldisc`, run later from a work queue) ran straight away; what does not
  fit waits in the other side's buffer, counted in bytes against `tty_buffer_set_limit`'s 8192
  (Linux counts allocated pieces of 256 bytes and more, so how much a write is taken while nobody
  reads differs by up to a few hundred bytes; in Linux it also depends on when the work runs).
  `n_tty_lookahead_flow_ctrl` (stop and start keys seen while the line discipline is full) is not
  built.
- A non-canonical read waiting for VMIN bytes takes them only once enough have come (Linux copies
  each as it comes and waits holding them): another reader or TIOCINQ sees them in the meantime.
- Layer 2's clock moves a nanosecond for each instruction a turn ran (whole blocks entered are
  counted) plus the seeded 0.5 to 1.5 us, so CPU times and the fair scheduler see work in
  proportion to its length.
- AT_MINSIGSTKSZ is the reference's measured 4720 (typed in linux_extra.rs): arm64 computes it from
  the CPU's records at boot.
- The vDSO is laid out by hand with `object`'s ELF structures (src/vdso.rs): one function, its
  version and soname as Linux's; Linux's own also has the time functions, which programs find
  missing here and fall back to system calls for (musl and glibc both do).
- A background job's output order in tests is sorted: which child runs first after fork is a race in
  the reference's four CPUs, while here the scheduler decides it the same way every time.
- The made-up machine has one CPU (default time slice 0.7 ms, sched_rr_get_interval in whole 1 ms
  ticks), no permission overlays and no guarded control stacks, like the reference's CPU for the last
  two.
- sched_yield puts the caller behind every thread waiting for its turn (its virtual runtime raised to
  the run queue's greatest). The plan's scheduler is CFS, whose yield (a skip buddy) keeps a yielder
  running while the others are more than the wakeup granularity ahead, which for a new child is
  about 2 ms of yields; the reference (Linux 7.0, EEVDF) forfeits the yielder's deadline so the
  others run at once, and programs that spin on sched_yield waiting for another process behave as
  there only with the reference's way (measured with the reference on one CPU).
- A mapped file keeps its first pages in a block of shared pages while any mapping shows it, private
  mappings included (their written pages are their own copies); a block covers each mapping's whole
  length, past the file's end too. A tmpfs inode's generation is a keyed hash of its number
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

- Docker's device rules (which devices a container may open) are modelled from runc's default list
  and measured in the reference (mknod then open: 1:1, 4:1, 200:7 and block devices EPERM, a missing
  pty slave EIO, the tunnel opens); with cgroup v2 they are an eBPF program in Linux, a table here.
- What a buffer that faults part way receives depends on where the reference kernel's own copy lies
  within 16 bytes (its compiler's stack layout); that is measured for the calls tests/partialcopy.rs
  makes (uname's lies 1 past alignment, the random devices' block 8 past) and taken as aligned to 8
  elsewhere.
- Layer 1's reference runner polls every descriptor twice after each call of a sequence with a
  pseudo-terminal (Linux's buffer work runs later from a work queue; ours at once), and such
  sequences write at most 256 bytes a call. A child made by clone in layer 1 ends at once.
- A message queue's absolute real time timeout becomes a deadline on the monotonic clock when the
  call starts (the made-up machine's real time clock cannot be set).
- A futex wait's record is a slot in the thread's entry (`Thread::futex`: the futex each of its
  call's entries is queued on, inline for one, and a bit per entry a wake took), as the plan says;
  the queue of each futex is an index beside it, a `VecDeque` of waiters in a `BTreeMap` by key
  (Linux's are intrusive lists in hash buckets), so the first waiter on a futex allocates its queue
  and a vector wait of more than one futex allocates its list of keys (Linux allocates that vector
  too). Before K3's fifth run every wait also allocated a list of keys in a map by thread, cloned
  it at each look and searched its queues for itself. The cost was not measured either way.
- A named pipe's open waiting for the other end keeps the file it makes in the thread's `opening`
  slot beside the call slot (the plan's one fixed slot holds only plain values): the file must count
  as an end while it waits, as Linux's does. It is given up when the call ends any other way or the
  thread ends.
- K5, third run: an outside connection accepted through `Host::accept` shows the gateway
  (172.17.0.1) and a random port as its peer and eth0's address as its own, as measured for
  Docker's published ports; `Host::accept` gives no peer address, so the real client is not seen.
- K5, third run: /proc/net shows the made-up network (lo and eth0 only; the reference also has
  tunl0, gre0 and other tunnel devices and eth0's traffic). Interface counters read zero (counted
  "interface counters not kept"). Kernel pointers are a number unique to each socket (Linux
  prints a hashed address), UDP lines are numbered by port modulo 16384 (Linux mixes in a random
  value per network), TCP lines list listeners first then connections in port order (Linux:
  hash order), TCP's reference count is 2 with a file and 1 without (Linux holds one more while a
  timer is pending), and the delayed acknowledgement fields (ato, quick, pingpong) are a model
  (4, 30 and pingpong once data came and was answered; Linux's depend on timing). Local sockets
  are those open in some process and those waiting in a backlog (Linux also lists sockets in
  flight in SCM_RIGHTS). A write to a /proc/net file is EIO here; the reference refuses the open
  for writing with EACCES even for root (cause not found).
- K5, third run: the TTL and type of service of datagrams from outside and of the name server's
  answers are not reported by the host: 64 and 0 are given when asked for (counted "outside
  datagram header made up").

## Not built, by counter

Every answer to something Linux has and this kernel does not build goes through `Counters::refuse`
or `refuse_flag` under one of `reason::NOT_BUILT`; the detail says which. The names:
- "system call not implemented": the Missing list above (detail: the call number).
- "call flag not built": a flag or form of a built call (detail: call number times 2^32 plus the
  flags): mmap MAP_HUGETLB (ENOMEM), memfd_create MFD_HUGETLB (ENOSYS), openat O_TMPFILE
  (EOPNOTSUPP), openat2's restricted lookups (ENOSYS), rseq's time slice flag (EINVAL), NUMA
  futexes (EINVAL), pipe2's O_NOTIFICATION_PIPE (EINVAL),
  shmget's SHM_HUGETLB
  (ENOMEM).
- "fcntl command not built": F_SETLEASE, F_NOTIFY, F_GETOWNER_UIDS, F_GETDELEG and F_SETDELEG (EINVAL,
  as Linux without them). Commands Linux does not know are a plain EINVAL, uncounted.
- "prctl option not built": options Linux has on ARM64 that are not built (EINVAL; see Partial):
  PR_SET_MM's forms, PR_GET_TIMING, PR_SET_TIMING, the TSC, perf event, MCE, SVE, SME, pointer
  authentication, tagged address, speculation, syscall user dispatch, memory merge, MDWE setting,
  shadow stack, timer id, futex hash and rseq slice options, PR_GET_TID_ADDRESS, PR_SET_SECUREBITS
  and PR_SET_VMA. Unknown options and those Linux has only for other architectures are a plain
  EINVAL, uncounted.
- "line discipline not built": TIOCSETD with a discipline other than `n_tty` (detail: its number).
- "ioctl request not built": the watch queue requests on pipes; TIOCCONS and TIOCSLCKTRMIOS for a
  caller with the powers they need (Docker gives neither, so they answer EPERM as in the reference);
  pidfs's ten namespace requests once their checks pass (nsfs is not built; EOPNOTSUPP).
- "madvise advice not built": MADV_COLLAPSE, MADV_GUARD_INSTALL, MADV_GUARD_REMOVE.
- "device not built": the console (5:1) and `/dev/net/tun` (10:200), which Docker's rules allow
  (ENODEV; detail the device number).
- "proc entry not built": the `/proc` entries not built (detail: directory and name index).
- "seccomp listener not implemented": SECCOMP_FILTER_FLAG_NEW_LISTENER.
- "call on another process not built": process_vm_readv and process_vm_writev on another process.
- "socket family not built": socket and socketpair of a family other than AF_UNIX, AF_INET,
  AF_INET6 and AF_NETLINK, such as AF_PACKET (detail: the family; EAFNOSUPPORT).
- "socket protocol not built": raw sockets (EPERM), ping sockets (EACCES), SCTP
  (ESOCKTNOSUPPORT for SOCK_SEQPACKET, EPROTONOSUPPORT in a stream), MPTCP and UDP-Lite
  (EPROTONOSUPPORT), netlink protocols other than NETLINK_ROUTE (EPROTONOSUPPORT), and netlink
  messages to another socket's port (ECONNREFUSED); detail family times 2^32 plus kind times 2^16
  plus protocol.
- "socket option not built": socket options not built (detail: level times 2^32 plus option;
  ENOPROTOOPT), among them TCP_INFO, IP_PKTINFO and the other IP and IPv6 options that add
  control messages, and since K5's second run SO_PASSPIDFD and the SO_TIMESTAMP* options (their
  control messages are not built; before, they were kept and did nothing).
- "netlink request not built": rtnetlink requests other than RTM_GETLINK, RTM_GETADDR and
  RTM_GETROUTE dumps and RTM_GETLINK of one link (EOPNOTSUPP; detail the type plus 2^16 for a
  dump), such as `ip route get`, neighbours, rules and queue disciplines.
- "ioctl request not built" also covers SIOCGSTAMP and its forms on internet sockets (ENOENT),
  and the requests that change interfaces for a caller with CAP_NET_ADMIN (Docker gives none,
  so EPERM as in the reference).
Notices the backend sends that Linux refuses are counted "backend notice refused" (not a
refusal of the guest).
Approximations that are not refusals have their own counters: "tcp room modelled", "tcp
connect waits for backlog room", "interface counters not kept", "name query not answered",
"recvmmsg returned early", "listening outside failed" (a host that cannot accept from outside
or whose port is taken), "outside datagram header made up",
"page residency not tracked", "proc
fields not tracked", "fault syndrome approximated", "core dump not
written", "vmsplice pages copied", "backend pages written whole", "backend change not seen by
mapped pages". "wake to run" is a measurement (count and total nanoseconds), not a slow path. "host cannot do
this" counts a CPU side that answered it cannot make a memory, thread or shared block (ENOSYS to the
guest).

## Changes to the shared crate (wcontainers-abi)

- K5, third run (host.rs): documentation only. `Host::listen` now says its port is the
  container's TCP port, that `ConnectionReady` comes on the listener once after the call and again
  after `accept` answered `WouldBlock`, and that `close` on the listener stops it; `Host::accept`
  says the new connection is then used as one made by `connect`, and that `WouldBlock` means none
  waits. No signature changed.

- K5, second run (host.rs, which the kernel owns): added `Host::finish(connection)` (no more
  bytes will be sent on a TCP connection; the program's SHUT_WR), a required method, so any
  other `Host` implementor must add it. `Event::LookedUp`'s `addresses` are now 16 bytes each,
  IPv4 ones mapped (the old mix of 4 and 16 byte addresses could not be split). Documented what
  `error` means in `LookedUp` and `Connected`, when `ConnectionReady` comes, that a UDP connection
  is usable at once with one datagram per send and receive, and that `receive`'s
  `HostError::Closed` is a reset.

- K5, first run: none. (The network hooks and `Event::Connected`, `ConnectionReady` and
  `LookedUp` exist from K0 and are not used yet.)

- K4, fifth run: none.
- K4, fourth run: none.
- K4, third run: none.
- K4, second run: none.
- K4, first run: none. (`Host::backend_send`, `Event::BackendReply` and `Event::BackendNotice` are
  used as they were.)

- K3, sixth run: none.
- K3, fifth run: none.
- K3, fourth run: none.
- K3, third run: none.
- K3, second run: none.
- K3, first run: none.

- K2, tenth run: none.
- K2, ninth run: none.

- K2, eighth run: none.
- K2, seventh run: none.
- K2, sixth run: none.
- K2, fifth run: none.
- K2, fourth run: none.
- K2, third run: none.

- K2, second run: none. (`Event::TerminalInput` and `TerminalResized`, for a container's own
  terminal, are still counted "event with no waiter": the container's first process has pipes, as
  `docker run` without `-t` gives it.)

- K0: `Host::output(stream, bytes)` with `Stream`; `CounterReading::detail`.
- K1: none by this side. The seventh run merged origin/phase-1, which added `Exit::AlignmentFault {
  address }` (the translator's side) and moved a layout assertion in registers.rs to a constant
  assertion; the kernel handles the new exit (SIGBUS) and the stand-in reports it for misaligned
  fetches. Cargo.lock was regenerated by cargo for the merge (the kernel's lock plus the translator's
  packages, only additions). The eighth to thirteenth runs changed nothing in the shared crate
  (origin/phase-1 had nothing new to merge in the ninth to thirteenth).
- K2, first run: none. (Event, Host and the rest are used as they were.)

## Questions for the orchestrator (boundary)

None open. (Answered: the alignment fault exit exists now; the machine description stays split.)

## Known issues

- K5, second run (not reproduced in the third): tests/saving.rs once failed for guest/unixsock.c
  seed 1 ("after connect atime moved 0" where the reference prints 1). In the third run the
  program alone under saves passed 3 times with 6 seeds each and the whole saving test passed
  twice (load 28 to 40). The access time check is right for any wall clock after 1970; the likely
  cause is test binaries building the same guest program into one shared directory while another
  runs it (see the K2 entries below). Left as it is.

- K4, second run (fixed in the third): layer 1 on the backend seed 4040103 differed on
  statx(AT_FDCWD, "", AT_EMPTY_PATH, mask 0): Linux's FUSE does not sync attributes for a mask with
  no basic field and shows what it held; ours asked the backend. Ours now takes the held
  attributes too; the seed passed 2000.
- K4, second run (narrowed in the fourth): getxattr of `security.capability` on the reference's
  FUSE mount answers EINVAL (with size 0 and with a buffer, set or not). Traced with strace at the
  server in the fourth run: no GETXATTR reaches it for this name, while `security.other` does and
  answers ENODATA; files on tmpfs (nosuid or not) answer ENODATA. So the EINVAL comes from the
  reference kernel before FUSE is asked; which line of 7.0.14 gives it is not found (scratch:
  kernel/capcause). Ours answers EINVAL and sends nothing, the same. Exec: the reference's mount is
  nosuid (`rw,nosuid,nodev,noatime`), so exec there asks for no capabilities (traced: none sent);
  ours mounts the volume the same and sends none. On a backend root ours reads them at exec; there
  is no FUSE root in the reference to compare.
- K4, first run (from the K3 check): terminal's `job_control` (guest/jobs.c `leader_exit`) can fail
  from a race in the reference: Linux's `disassociate_ctty` sends SIGHUP to the foreground group
  before `session_clear_tty`, and on a loaded multi-CPU reference the job can open `/dev/tty` in that
  window (theirs "job /dev/tty: 4", ours always the final ENXIO). Seen once in a whole-suite run at
  load 40 to 60; three reruns alone passed. The test is left as it is.

- K3, sixth run: layer 1 seed 3023 (open since the fifth run) was the harness: the IPC status calls
  (IPC_STAT, SEM_STAT, SEM_STAT_ANY and the shm and msg ones) write sequence numbers and times
  that the comparison skips but nothing cleared, and a later call read them (process_vm_readv took
  a set's sequence number as a vector length: the reference's is not 0 after earlier sequences).
  `machine_bytes` now clears them after the call; seed 3023 passed 20000 (313.8 s, load 13.8 to
  14.3). Reproduced first with the old harness (failed at the same call, load 23 to 15).
- K3, sixth run: sigpick's reference (tests/threads.rs `process_signal_takers_match_reference`)
  sometimes gives the process signal to thread 1 instead of the first thread that does not block
  it: once in about ten multi-CPU runs at load 30 to 90, and once on one CPU in a whole-suite run
  beside two layer 1 runs (load up to 144); 8 of 8 one-CPU runs alone gave thread 0, as ours does.
  Cause in the reference not found (`complete_signal` should pick the first thread while it wants
  the signal). The test is left as it is.
- K3, sixth run: terminal's `splice_holds_the_pipe` (ttylock) failed once in a whole-suite run
  beside three layer 1 runs (ours showed `abc` once where the reference echoed and printed it);
  three reruns alone passed. Not looked into further.
- K3, fourth run: sigio's reference race came again once in a whole-suite run beside three layer 1
  runs (load 47): it counted the raw byte's signal with the next section's (0 and 2 where ours, and
  every rerun, has 1 and 1).
- K3, third run: sigio failed once in a whole-suite run beside a layer 1 run (load about 20): the
  reference counted 0 signals for "output to the master" (ours 1, as three reruns of both). The
  same family as the ninth run's sigio entry below: the reference's terminal buffer work races
  under load.
- K3, first run: the busybox shell test `processes` failed once in a run beside layer 1 at load
  about 50 (standard error differed); four reruns passed. Not looked into further.

- The stand-in counts instructions by whole blocks, so a turn whose slice runs out in a block that
  ends in `svc` comes back as a system call, not a stop (K2 check). The kernel's tick at every return
  (tenth run) gives way either way, so nothing depends on it.

- Building the test crate prints a linker warning from unicorn-engine-sys (a missing lib64 directory).
- K2's ninth run: sigio failed once in a whole-suite run beside four layer 1 runs (load 36): the
  reference counted no signal for the raw byte a master wrote after the switch to raw mode (ours,
  as every other run of the reference, one). Three reruns passed. Cause not found; the guest takes in
  the byte with a poll of the slave (`n_tty_poll` flushes Linux's buffer work), so this may be a race
  in the reference's buffer work under load.

- K2's seventh run: terminal's `making_pairs` failed once in a whole-suite run because the reference
  found no `/wc/tty` (test binaries build guest programs into one shared directory while others run
  containers from it); it passed alone. Not fixed yet: each test binary could build into its own
  directory.
- The dispatch test failed twice in whole-suite runs beside layer 1 runs: the reference container
  found no `/wc/syscall` (its five tests each built the program and renamed it over the others while
  a container started it, through Docker's shared directory). It now builds the program once.
- Once in the third run (load about 20 to 50) the stand-in test binary hung for over 12 minutes and was
  killed; three reruns passed at once. Cause not found; not seen since.
- Test binaries running side by side build guest programs into one directory; the dispatch test read
  that directory while another test was writing it and failed once (fixed in the eleventh run: a file
  gone since the listing is skipped).
- The object crate is built with its `unaligned` feature, so headers are read from any byte offset.

## Next

K5, in this order (what the four runs built is under Where things stand):
1. Layer 1 now reaches sockets, epoll, eventfd and inotify for the first time (see Where things
   stand). Run fresh seeds (5050414 and up) at 5000 to 20000 until they pass, fixing what they
   find; then the three end-of-milestone seeds of 20000 (`LAYER1_CASES=20000 LAYER1_SEED=<n>`,
   one command each, see Layer 1) and the whole suite.
2. Not built, counted "socket option not built": SO_TIMESTAMPING (old and new), TCP_INFO, and the
   internet send control messages listed under Where things stand. TCP's urgent data (a send
   with MSG_OOB and bytes is refused, counted). I/O signals of internet and netlink sockets
   (counted "socket i/o signals not built").
3. Local sockets left: collecting sockets passed over themselves (such a cycle stays open here),
   a socket's `/proc/<pid>/fdinfo` (fdinfo is not built at all). A SCM_PIDFD or SO_PEERPIDFD for a
   reaped process whose number was given again would name the new one (numbers are kept, not
   Linux's `struct pid`).
4. /proc/net's other files (netstat, snmp, sockstat, protocols, packet, raw and the rest) are
   counted "proc entry not built". fanotify_mark is still "system call not implemented"
   (fanotify_init is refused by Docker's profile, so no group can exist; Linux's flag checks come
   before EBADF and EINVAL, and linux-raw-sys has no FAN_MARK_* to check them with).
5. inotify gaps: IN_UNMOUNT is never sent; a mapping does not keep an unlinked inode's watches
   alive (Linux sends IN_DELETE_SELF at the last unmap); events of files changed on the backend
   from outside are not seen (plan section 15's kind).

K4's parts are built, the check's findings are fixed, and the fifth run's end-of-milestone seeds
found and fixed two more (Built in K4's fifth run). Left, none of it blocking:
1. Notices about mapped files are not tested with a program that sees them: STORE and INVAL_INODE
   reach mapped files only by chance (node numbers 1 to 11), and a STORE that does changes what the
   program reads, which the reference cannot be made to match. RETRIEVE is sent and answered.
2. Calls that move a position without Linux's `f_pos_lock` (sendfile, splice and copy_file_range
   with no offset given) can still see it move while they wait for the backend, as on Linux; ours
   then asks again (counted "backend question asked again") where Linux's would race.
3. Directory changes (create, unlink, rename, mkdir, rmdir on the backend) do not hold the parent
   directory's lock across waits as Linux's `inode_lock` on the parent does; writes, positions,
   truncation and fallocate do. Two threads changing one backend directory at once can interleave
   their questions where Linux's cannot; not seen in any test.
4. Held access control lists that are not none are not reached: the test server's directory has
   no lists. Layer 1 on the backend did not reach a pipe spliced into a backend file.

Left open from K3: poll and select waiters share one channel; a host wall clock step during a run is
not seen as a setting; futex queues allocate when a futex first gets waiters; the one-million-
sequence layer 1 run at the end of the kernel track (orchestrator). Layer 1 seeds used so far: up to
737, 3023 to 3034, 4040001, 4040201 to 4040207, 4040401 to 4040403, 4040501 to 4040512, 5050001, 5050003 and
5050002 (K5, with sockets), and the checks' 91001 to 91003, 8181801 to 8181803, 5150002 and 9393001 to 9393003. Layer 1 on the backend:
0, 4040101 to 4040105, 4040111 to 4040116; on the caching backend: 0, 4040301 to 4040303, 4040311
to 4040315, and the check's 7171002.
