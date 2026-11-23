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
- tcp host 23 -> 6 (run 2) -> 1 (run 3). Left: the one `ioctl` (SIOCOUTQ, SIOCOUTQNSD, SIOCATMARK, FIONREAD).
  Run 3: `TCP_INFO` is read into `InformationBytes { info: TcpInfo, ecn: AccurateEcnInformation }`, zerocopy types;
  `TcpInfo` mirrors linux-raw-sys's tcp_info (bit fields read by linux-raw-sys's own `__BindgenBitfieldUnit::get`);
  `TCP_CC_INFO` forms, `MPTCP_INFO`, `MPTCP_FULL_INFO` (subflow addresses as `StorageBytes`), `TCP_AO_GET_KEYS`
  (`KeyQuery`, one builder for both readers), `TCP_AO_INFO` (mirrored, then given as linux-raw-sys's structure for
  its bit field accessors), `TCP_AO_REPAIR` and `TCP_REPAIR_WINDOW` all read through zerocopy.
  Run 2: allocation-counter for the test's count, rustix recv, struct literals for what is only passed in.
- linux-socket-options 40 -> 30 (run 1) -> 21 (run 2) -> 6 (run 3). Left: the raw `setsockopt` and `getsockopt`
  (set_raw, get_bytes), and in each of the two `recvmsg` paths (linux.rs receive_message, datagram/receive.rs
  receive_with_tail) the zeroed `libc::msghdr` and the call itself.
  Run 3: `uapi_mirror!` (mirror.rs, exported): declares a `#[repr(C)]` zerocopy copy of a uapi structure and asserts
  at compile time its size and every field's offset against the uapi's. Through it: sockaddr_in and sockaddr_in6
  (`storage_bytes`, `address_of_storage`, `kernel_storage` builds `__kernel_sockaddr_storage` by a union literal,
  which is safe), in_pktinfo and in6_pktinfo (sent and received), scm_ts_pktinfo, sock_extended_err (read, then
  given as libc's), so_timestamping, sock_txtime. The send control buffer writes the kernel's cmsghdr fields by
  offset_of; receive buffers, SO_MEMINFO words and `get_option` (now `T: FromBytes + IntoBytes`) go through
  zerocopy; the group filter is built as bytes; `as_bytes`, `read_address` (rustix's unsafe SocketAddrAny::read)
  and `from_in6_addr` (a union read) are gone; ipv6_mreq, group_req and in6_flowlabel_req are struct literals.
  `set_option<T: Copy>` still takes any plain uapi structure and hands its bytes to the raw call (counted in set_raw).
  Run 2: control message walks and sizes. Run 1: mio, nix's setfsuid and if_nameindex.
- icmp suite 15 -> 1, route-resolver suite 4 -> 2 (run 1): raw setsockopt only, see below.
- runtime single-kernel 7 -> 2 (run 1): the StopFlag, see For the owner.
- translator-test 16: not touched.

## What is left, by call, and why no vetted library offers it
- Raw `setsockopt`/`getsockopt` with option bytes (linux-socket-options set_raw and get_bytes, 2; route-resolver
  suite: IP_OPTIONS and IP_UNICAST_IF/IPV6_UNICAST_IF, 2; icmp suite: IPV6_ADDR_PREFERENCES, 1). rustix, socket2
  and nix wrap typed options only (nix's SetSockOpt cannot be implemented without the raw call); none offers raw
  bytes, rightly, as some options (SO_ATTACH_FILTER) carry pointers. Options the code sets that no vetted crate
  wraps include TCP_REPAIR_QUEUE, TCP_QUEUE_SEQ, TCP_REPAIR_WINDOW, TCP_REPAIR_OPTIONS, TCP_AO_*, TCP_MD5SIG,
  TCP_SAVED_SYN, TCP_CC_INFO, MPTCP_INFO, SO_MEMINFO, MCAST_MSFILTER, MCAST_JOIN_GROUP, IP_LOCAL_PORT_RANGE,
  SO_BUF_LOCK, SO_TXREHASH, SO_WIFI_STATUS, TCP_FASTOPEN_KEY, TCP_FASTOPEN_NO_COOKIE, IP_MINTTL, IP_ROUTER_ALERT,
  IP_OPTIONS, IP_UNICAST_IF, IPV6_ADDR_PREFERENCES. Cost of removing: a typed wrapper per option in a vetted crate
  (upstream to rustix); otherwise these two calls are the one marked place.
- `recvmsg` with control messages (2 calls and 2 zeroed msghdr). rustix's recvmsg parses only SCM_RIGHTS and
  SCM_CREDENTIALS; socket2's gives no MSG_CTRUNC; nix's parses most into ControlMessageOwned but allocates a Vec for
  each message it does not know (IPv6 extension headers, IP_RETOPTS, ...), which the zero-allocation receive cannot
  take. libc's msghdr cannot be built by a literal on musl (private padding fields), hence the zeroed one. Cost: move
  the receive path to nix and accept an allocation for the rare unknown messages; or one shared recvmsg function
  (2 uses instead of 4).
- `ioctl` SIOCOUTQ, SIOCOUTQNSD and SIOCATMARK (tcp host ioctl_count): rustix's safe ioctls are FIONREAD and a few
  others, not these; its generic `ioctl` and `Getter::new` are unsafe; libc's sockatmark is unsafe. tcp_info's
  tcpi_notsent_bytes equals SIOCOUTQNSD for an established socket but nothing in it gives SIOCOUTQ (write_seq -
  snd_una, which only sock_diag's idiag_wqueue gives) or the urgent mark. Cost: stays, or a typed wrapper upstream.
- Runtime StopFlag `entered`/`raise` (2): the flag word is in the wasmtime executor's guest memory and the runtime
  holds its address. Needs an API change with the executor (another track's crate), see For the owner.
- translator-test (not started): wasmtime SharedMemory is `&[UnsafeCell<u8>]` with no safe read or write
  (runner.rs, tests/test_guest.rs), the untyped hooks' `MaybeUninit<ValRaw>`, the x86 intrinsics' transmutes in
  tests/reciprocals.rs (zerocopy's `simd` feature could replace those). It is the translator's harness and runs
  generated code as an executor does: likely an executor-like exception for the owner to rule on.

## Next
1. The owner's rulings on what is left (For the owner). Meanwhile, optional: one shared recvmsg function in
   linux-socket-options (6 -> 4), and `set_option` bounded by `IntoBytes + Immutable` with mirrors of the
   structures the udp, tcp, raw and icmp hosts pass (tcp_ao_add, tcp_ao_del, tcp_md5sig, ...; touches other tracks'
   raw and icmp hosts), which moves no count but makes every byte view checked.
2. translator-test: the reciprocals transmutes through zerocopy; the rest is the executor question.
3. The open finding below.

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
- zerocopy 0.8.63 (BSD-2-Clause, Apache-2.0 or MIT; Google's; 1.19 billion downloads, 351 million in the last 90 days, per crates.io on 2026-11-22; already a workspace
  dependency, `derive` only), run 3: now also of linux-socket-options and the tcp host (Linux only), for the uapi
  copies and byte views that replaced pointer casts. No new crate entered Cargo.lock.

## Measurements (ARM64 machine, Graviton4, 96 cores, Linux 7.0.0-1018-aws)
Run 3 (2026-11-22 03:03, load 2.2 falling to 1.4; log speed-compare-run3-after.log): the code at the start of run 3
(a89862c0, folder before3) against run 3's, alternated three times, two rounds each. TCP send 381-385 ns before,
382-387 after; TCP receive 396-399 before, 397-400 after; UDP send 1.501-1.515 us before, 1.495-1.512 after; UDP
receive 777-783 ns before, 755-761 after (about 3% faster in all six rounds; the receive now reads the sender's
address and the control messages through zerocopy instead of rustix's SocketAddrAny and unaligned reads).

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

## Suites (ARM64 machine, after the changes)
Run 3 (2026-11-22 03:05, load 2 to 13; log suites-run3-final.log): clippy with warnings denied on every crate of
the track and the socket crates sharing its helpers; udp conformance 78 + readiness 3 + the harness-free allocation
check; tcp conformance 140 + unit 2 + the harness-free allocation check (speed ignored); raw, icmp, route resolver,
configurable, negative-route-advisable and linux-socket-options (binaries of 2, 67, 5, 1, 1, 7, 53, 2, 1, 38, 1);
runtime 15; arm64 9. All passed. No Mac build this run (the Mac must stay usable): every change is in Linux-only
modules, and zerocopy is a Linux-only dependency of both crates.

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
resolver host and the tcp suite (run 1) and the udp host (run 3) each fail with "usage of an `unsafe` block"
(`-F unsafe-code`).

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
- What cannot be removed without your ruling, with the count that remains: linux-socket-options 6 (the raw
  setsockopt and getsockopt, two recvmsg calls and their two zeroed msghdr), tcp host 1 (the ioctl), route-resolver
  suite 2 and icmp suite 1 (raw setsockopt), runtime 2 (the StopFlag below), translator-test 16 (wasmtime's
  SharedMemory has no safe access; it runs generated code as an executor does). Why no vetted library offers each,
  and what removing it would cost, is under "What is left".
- The runtime's `StopFlag::entered(&self, flag: &AtomicU32)` is an `unsafe fn` and `raise` dereferences the stored
  address: the word is in the wasmtime executor's state memory. Removing it needs the executor to give something
  safe to call, for example `entered(&self, raise: Arc<dyn Fn() + Send + Sync>)` kept per thread by the executor
  (whose closure writes the word with the executor's own marked unsafe), one Arc clone per turn and no allocation.
  This touches crates/executor/implementation/wasmtime (another track's), so it is not done.
- Raw socket options: see "What is left". The smallest honest end state for linux-socket-options is likely two raw
  calls (set and get option bytes) under `#[expect(unsafe_code)]`, which the root manifest does not allow for a
  crate that is not an executor or a graphics crate.
