# Status: no unsafe code (track no-unsafe)

## Where it stands (2026-11-22, run 3 of milestone U1)
Milestone U1: no unsafe code in the host socket crates, the runtime, their suites and the old ARM64 crate. Not done.

Free of unsafe and inheriting the workspace's lints (their lines are gone from the root Cargo.toml's list):
- crates/udp-sockets/implementation/host (run 2): unsent bytes through `SO_MEMINFO`'s `SK_MEMINFO_WMEM_ALLOC`
  (the kernel's udp_ioctl SIOCOUTQ and sk_get_meminfo both give sk_wmem_alloc_get) read by the existing
  get_bytes; the receive's tail is an initialised per-thread buffer (`receive::with_tail`, shared); the allocation
  checks moved to tests/allocations.rs, a binary without the test harness (whose threads allocate beside a check)
  counting the whole process with stats_alloc (helpers in tests/support/mod.rs). 30 of 30 runs passed.
- crates/wcontainers-arm64: the register page's word is read through its fields.
- crates/route-resolver/implementation/host: its counter test enters its namespace and finds its link through nix.
- crates/tcp-sockets/conformance: unshare through nix; the check's own loopback is brought up through netlink
  (netlink-packet-route, netlink-sys) instead of the SIOCSIFFLAGS ioctl.

Fewer uses, still on the list (counts are the `unsafe` words left in the crate's .rs files):
- tcp host 23 -> 6 (run 2). Gone: the unit tests' counting allocator (tests/allocations.rs, harness-free, counting
  the waiting thread with allocation-counter as before; see "Open finding"), the plain receive's MaybeUninit cast (rustix's recv takes `&mut [u8]`), the zeroed in6_flowlabel_req,
  tcp_md5sig, tcp_ao_add, tcp_ao_del and tcp_ao_info_opt (struct literals, arrays by `array::from_fn`), the repair
  options' bytes (written by offset_of), CMSG_SPACE and the extension headers' control message walk (shared safe
  walker). Left: `view` (reads uapi::tcp_info and our own repr(C) structures from bytes), `bytes_of` and `zeroed`
  (tcp_ao_getsockopt, whose getsockopt reads and writes the same structure, and the MPTCP structures), the
  tcp_ao_getsockopt of holds_authentication_key (2), and the ioctl (SIOCOUTQ, SIOCOUTQNSD, SIOCATMARK).
- linux-socket-options 30 -> 21 (run 2). Gone: the control message walks (`ControlMessages` now reads the kernel's
  cmsghdr layout from linux-raw-sys by offset_of, as CMSG_NXTHDR stops), the three CMSG_SPACE (`control_space`),
  the error queue's MaybeUninit tail.
- linux-socket-options 40 -> 30 (run 1). Gone: the readiness watcher's four (polling's `add` is unsafe; mio's register is
  not), setfsuid's four (nix::unistd::setfsuid), if_nameindex's two (nix), the timestamps' read (bytes read by
  offset_of!).
- udp host 11 -> 9. Gone: the test's unshare and setns (nix).
- icmp suite 15 -> 1. Gone: unshare (nix), the packet capture's six (nix socket and recvfrom of a LinkAddr; the socket
  is no longer bound to the link, the capture keeps only packets whose ifindex is the link's), the counting
  allocator macro's seven (the raw and icmp hosts' waiting tests use stats_alloc's allocator instead).
- route-resolver suite 4 -> 2. Gone: unshare and if_nametoindex (nix).
- runtime single-kernel 7 -> 2. Gone: polling's four `add` (mio), mknodat (rustix's on Linux and the BSDs; on macOS,
  which has no mknodat in rustix or nix, a FIFO by nix's mkfifo at the directory's path from rustix's getpath, a
  socket node by binding a std UnixListener there).
- translator-test 16: not touched yet.

## What is left, by call, and why no vetted library offers it
- Raw `setsockopt`/`getsockopt` with option bytes (linux-socket-options set_raw, get_bytes, get_option: 4 uses;
  route-resolver suite: IP_OPTIONS and IP_UNICAST_IF/IPV6_UNICAST_IF, 2; icmp suite: IPV6_ADDR_PREFERENCES, 1).
  rustix, socket2 and nix wrap typed options only; none offers raw bytes, rightly, as some options (SO_ATTACH_FILTER)
  carry pointers. Options the code sets that no vetted crate wraps include TCP_REPAIR_QUEUE, TCP_QUEUE_SEQ,
  TCP_REPAIR_WINDOW, TCP_REPAIR_OPTIONS, TCP_AO_*, TCP_MD5SIG, TCP_SAVED_SYN, TCP_CC_INFO, MPTCP_INFO, SO_MEMINFO,
  MCAST_MSFILTER, MCAST_JOIN_GROUP, IP_LOCAL_PORT_RANGE, SO_BUF_LOCK, SO_TXREHASH, SO_WIFI_STATUS,
  TCP_FASTOPEN_KEY, TCP_FASTOPEN_NO_COOKIE, IP_MINTTL, IP_ROUTER_ALERT, IP_OPTIONS, IP_UNICAST_IF,
  IPV6_ADDR_PREFERENCES. Removing them: a typed wrapper per option in a vetted crate (upstream to rustix), or the
  two raw calls stay as the one marked place.
- `recvmsg` with control messages (linux-socket-options receive_message, datagram/receive.rs; walking them is safe
  now). rustix's recvmsg parses only SCM_RIGHTS and SCM_CREDENTIALS;
  socket2's gives no MSG_CTRUNC; nix's parses most into ControlMessageOwned but allocates a Vec for each message it
  does not know (IPv6 extension headers, IP_RETOPTS, ...), which the zero-allocation receive cannot take. Cost: move
  the receive path to nix and accept an allocation for the rare unknown messages, or read the raw control bytes
  some other safe way (none found).
- uapi structures built and passed as bytes (`zeroed`, `as_bytes`, `storage`, the in6_addr union read, the group
  filter, the send control buffer: addresses.rs, memberships.rs, send.rs, exchange.rs; the tcp host's
  tcp_ao_getsockopt, read and written by one getsockopt). Structures only passed in can be built by literal, as
  the tcp host's now are, but giving them to the kernel as bytes still takes the unsafe `set_option<T>`. linux-raw-sys's and libc's structures derive no zerocopy traits. Cost: our own
  `#[repr(C)]` zerocopy mirrors of each structure, each checked against the uapi's size and offsets at compile time.
- `ioctl` SIOCOUTQ, SIOCOUTQNSD and SIOCATMARK (tcp host ioctl_count): rustix's safe ioctls are FIONREAD and a few
  others, not these; its generic `ioctl` is unsafe. tcp_info gives no exact equivalent of SIOCOUTQ (write_seq -
  snd_una). Cost: stays, or a typed wrapper upstream in rustix.
- tcp_info read from bytes (tcp host `view`, 3 uses through one helper): linux-raw-sys's tcp_info derives no
  zerocopy traits. Cost: our own `#[repr(C)]` zerocopy mirror of its 69 fields used, with its two bitfield bytes,
  checked against linux-raw-sys's size and offsets at compile time; the MPTCP and congestion structures are ours
  already and need only the derives.
- Runtime StopFlag `entered`/`raise` (2): the flag word is in the wasmtime executor's guest memory and the runtime
  holds its address. Needs an API change with the executor (another track's crate), see For the owner.
- translator-test (not started): wasmtime SharedMemory is `&[UnsafeCell<u8>]` with no safe read or write
  (runner.rs, tests/test_guest.rs), the untyped hooks' `MaybeUninit<ValRaw>`, the x86 intrinsics' transmutes in
  tests/reciprocals.rs (zerocopy's `simd` feature could replace those). It is the translator's harness and runs
  generated code as an executor does.

## Next
1. tcp host (6): a zerocopy mirror of tcp_info and tcp_ao_getsockopt, zerocopy derives on the MPTCP and congestion
   structures; then only the ioctl is left, for the owner.
2. linux-socket-options (21): the same mirrors for its uapi structures (in_pktinfo, in6_pktinfo, sock_txtime,
   group_req, group_filter, ipv6_mreq, in6_flowlabel_req) and `set_option`/`get_option` bounded by zerocopy's
   traits; what then remains is the raw setsockopt/getsockopt and recvmsg calls, for the owner.
3. translator-test.

## Open finding (run 2)
Counted over the whole process (stats_alloc, no test harness), the tcp check "a warm wait allocates nothing" found
one allocation in 100 waits in 3 of 30 runs after 10 warm-up cycles (and in cycles 0 to 4 with one), never on the
waiting thread. The only other thread is the shared readiness watcher (linux-socket-options readiness.rs), so it
very likely allocates now and then while waking tcp waits; the udp check over the whole process passed 30 of 30. Not
yet traced. The tcp check counts the waiting thread, as it did before this track, through allocation-counter.

## Dependencies added (all MIT or MIT/Apache; each contains unsafe code inside)
- allocation-counter 0.8.1 (MIT or Apache-2.0; 8.9 million downloads, 7.8 million of them in the last 90 days),
  dev-dependency of the tcp host only: its allocation test's per-thread count, replacing our own GlobalAlloc.
  Linked into that test binary alone.
- stats_alloc 0.1.10 also became a dev-dependency of the udp host (run 2).
- mio 1.2.5 (MIT; 1.3 billion downloads, tokio's event loop), default features off, `os-ext`: the readiness
  watcher of linux-socket-options and the runtime's network. Replaces polling 3.11.0 (Apache/MIT, 300 million),
  whose `add` is unsafe; polling, concurrent-queue and hermit-abi left Cargo.lock.
- nix 0.31 (MIT; 1 billion; already a workspace dependency): setfsuid, if_nameindex, if_nametoindex, unshare,
  setns, the packet socket and recvfrom, and mkfifo on macOS, in more crates than before.
- stats_alloc 0.1.10 (MIT; 22 million): the allocator of the raw and icmp hosts' waiting tests (dev-dependency only).
- netlink-packet-core 0.9.0, netlink-packet-route 0.34.0, netlink-sys 0.9.0 (MIT; 47 million each; already used by
  the route resolver host): the tcp suite's loopback.

## Measurements (ARM64 machine, Graviton4, 96 cores, Linux 7.0.0-1018-aws)
Run 2 (measured in run 3, 2026-11-22 02:45, load 18 falling to 8; log speed-compare-run3.log): the code before run 2
against run 2's, alternated three times, two rounds each, same test as below. TCP send 384-386 ns before, 382-384
after; TCP receive 404-407 before, 397-399 after; UDP receive 774-784 both; UDP send 1.482-1.502 us before,
1.507-1.522 after, about 1% slower in all six rounds. Run 2 did not change the send path (the receive tail, the
unsent count and the tests did); the cause of that 1% is not found.

Run 1:
Speed of one send and one receive over loopback, `tests/speed.rs` of each host (median of 4000 batches of 32,
64-byte payloads, release build, root), before (HEAD copied before the change) and after, run alternately at load
5 to 10: TCP send 386-388 ns both, receive 408-411 ns both; UDP send 1.546-1.569 us before, 1.529-1.565 us after;
UDP receive 781-785 ns before, 776-785 ns after. No difference measured. An earlier single before run at load 23
gave UDP send 1.45 us; the alternating runs are the comparison.

## Suites (ARM64 machine, after the changes, 2026-11-23)
Run 2: udp conformance 78 + readiness 3 + the harness-free allocation check; tcp conformance 140 + unit 2 + the
harness-free allocation check (speed ignored); raw, icmp, route resolver, configurable, negative-route-advisable
and linux-socket-options as in run 1. All passed; each allocation check passed 30 of 30 runs.
Clippy with warnings denied: the socket crates on Linux (ARM) and macOS (the Mac, niced).

Run 1: udp conformance 78 + readiness 5; tcp 140 + 4; the raw, icmp and route resolver hosts, the configurable and
negative-route-advisable suites and linux-socket-options (binaries of 2, 67, 5, 1, 1, 7, 53, 2, 1, 38 and 1 checks);
runtime 13, and its 2 new network tests run on their own; arm64 9. All passed, as before the change. The two new runtime tests (a
datagram wakes its wait once and later datagrams are not told until it is waited on again; a connection and its
bytes wake the listener's and the stream's waits) pass on the code before the change too.

The ban is on: with `pub fn ban_probe() { unsafe {} }` added to a copy on the ARM machine, arm64, the route
resolver host and the tcp suite each fail with "usage of an `unsafe` block" (`-F unsafe-code`).

## How to run
- Scripts in pyro-parked/wcontainers-scratch/no-unsafe, run as
  `cloud/arm-run <checkout> --name no-unsafe -- "$(cat <scratch>/<script>)"`:
  `clippy.sh` (warnings denied, the track's crates and the socket crates that share them), `suites.sh` (SUITES picks
  udp tcp small runtime arm64), `speed.sh` (ROUNDS of the two speed tests, as root).
- macOS build of the changed crates, the only Mac cargo run: `CARGO_TARGET_DIR=$PWD/target/no-unsafe nice -n 19 nix
  develop -c cargo clippy -q -j 4 --no-deps --all-targets -p wcontainers-runtime-single-kernel -p
  wcontainers-internet-protocol-linux-socket-options -p wcontainers-tcp-sockets-host -p wcontainers-udp-sockets-host
  -p wcontainers-tcp-sockets-conformance -- -D warnings` (passed).
- Cargo.lock: commit this track's lines only, through a private index (`GIT_INDEX_FILE`, `git read-tree HEAD`,
  `git update-index --cacheinfo`), then `git reset -q -- Cargo.lock` in the shared one.

## Departures from the plan
- The readiness watcher and the runtime moved from polling to mio, which the owner agreed to earlier beside polling:
  polling has no safe registration. mio has no one-shot mode and no empty interest: the runtime takes a socket out
  of the poller when its change arrives (the one-shot polling gave), and a wait for neither reading nor writing
  registers urgent data on Linux (readable elsewhere), which a datagram socket has only beside an error.
- The icmp suite's packet capture is not bound to its link; it filters by the link's index.
- Run 2 changed the icmp host (not this track's crate) in two lines: its receive takes the shared per-thread tail
  (`receive::with_tail`) now that `receive_with_tail` takes initialised bytes.
- The udp and tcp hosts' allocation checks count the whole process (stats_alloc, one test in its own binary) where
  they counted the receiving thread: stronger, and no per-thread counting crate is added.
- The icmp suite's `counting_allocator!` macro is gone; `wait_allocations` takes what the caller counts. The raw and
  icmp hosts' waiting tests (not this track's crates) changed with it: they now count the whole process's
  allocations, stronger than the waiting thread's alone, as each binary holds that one test.

## For the owner
- The runtime's `StopFlag::entered(&self, flag: &AtomicU32)` is an `unsafe fn` and `raise` dereferences the stored
  address: the word is in the wasmtime executor's state memory. Removing it needs the executor to give something
  safe to call, for example `entered(&self, raise: Arc<dyn Fn() + Send + Sync>)` kept per thread by the executor
  (whose closure writes the word with the executor's own marked unsafe), one Arc clone per turn and no allocation.
  This touches crates/executor/implementation/wasmtime (another track's), so it is not done.
- Raw socket options: see "What is left". The smallest honest end state for linux-socket-options is likely two raw
  calls (set and get option bytes) under `#[expect(unsafe_code)]`, which the root manifest does not allow for a
  crate that is not an executor or a graphics crate.
