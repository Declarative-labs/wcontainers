# Status: no unsafe code (track no-unsafe)

## Where it stands (2026-11-23, run 1 of milestone U1)
Milestone U1: no unsafe code in the host socket crates, the runtime, their suites and the old ARM64 crate. Not done.

Free of unsafe and inheriting the workspace's lints (their lines are gone from the root Cargo.toml's list):
- crates/wcontainers-arm64: the register page's word is read through its fields.
- crates/route-resolver/implementation/host: its counter test enters its namespace and finds its link through nix.
- crates/tcp-sockets/conformance: unshare through nix; the check's own loopback is brought up through netlink
  (netlink-packet-route, netlink-sys) instead of the SIOCSIFFLAGS ioctl.

Fewer uses, still on the list (counts are the `unsafe` words left in the crate's .rs files):
- linux-socket-options 40 -> 30. Gone: the readiness watcher's four (polling's `add` is unsafe; mio's register is
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
- tcp host 23 and translator-test 16: not touched yet.

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
- `recvmsg` with control messages and walking them (linux-socket-options receive_message, ControlMessages,
  datagram/receive.rs; tcp host linux/state.rs). rustix's recvmsg parses only SCM_RIGHTS and SCM_CREDENTIALS;
  socket2's gives no MSG_CTRUNC; nix's parses most into ControlMessageOwned but allocates a Vec for each message it
  does not know (IPv6 extension headers, IP_RETOPTS, ...), which the zero-allocation receive cannot take. Cost: move
  the receive path to nix and accept an allocation for the rare unknown messages, or read the raw control bytes
  some other safe way (none found).
- uapi structures built and passed as bytes (`zeroed`, `as_bytes`, `storage`, the in6_addr union read, the group
  filter, the send control buffer: addresses.rs, memberships.rs, send.rs, exchange.rs; tcp host's zeroed tcp_ao_*,
  tcp_md5sig, in6_flowlabel_req). linux-raw-sys's and libc's structures derive no zerocopy traits. Cost: our own
  `#[repr(C)]` zerocopy mirrors of each structure, each checked against the uapi's size and offsets at compile time.
- `ioctl` SIOCOUTQ/TIOCOUTQ (udp host count_unsent_bytes) and the tcp host's ioctls: rustix wraps FIONREAD only.
- udp host receive's tail: the kernel's write into a MaybeUninit tail is read as bytes. Cost: an initialised tail,
  kept per socket so that a receive does not zero 64 KiB.
- Counting allocators in tests (udp host tests/readiness.rs, tcp host linux.rs tests): `GlobalAlloc` is an unsafe
  trait. stats_alloc counts the whole process, which the icmp and raw waiting tests (one test per binary) take; the
  udp receive check and the tcp unit tests count per thread, which stats_alloc cannot (allocation-counter can).
- Runtime StopFlag `entered`/`raise` (2): the flag word is in the wasmtime executor's guest memory and the runtime
  holds its address. Needs an API change with the executor (another track's crate), see For the owner.
- translator-test (not started): wasmtime SharedMemory is `&[UnsafeCell<u8>]` with no safe read or write
  (runner.rs, tests/test_guest.rs), the untyped hooks' `MaybeUninit<ValRaw>`, the x86 intrinsics' transmutes in
  tests/reciprocals.rs (zerocopy's `simd` feature could replace those). It is the translator's harness and runs
  generated code as an executor does.

## Next
1. tcp host and udp host: the counting allocators (allocation-counter for per-thread counts), the zeroed uapi
   structures that zerocopy mirrors can replace, the udp receive tail.
2. Decide with the owner the receive path (nix with an allocation for unknown control messages, or keep).
3. translator-test.

## Dependencies added (all MIT or MIT/Apache; each contains unsafe code inside)
- mio 1.2.5 (MIT; 1.3 billion downloads, tokio's event loop), default features off, `os-ext`: the readiness
  watcher of linux-socket-options and the runtime's network. Replaces polling 3.11.0 (Apache/MIT, 300 million),
  whose `add` is unsafe; polling, concurrent-queue and hermit-abi left Cargo.lock.
- nix 0.31 (MIT; 1 billion; already a workspace dependency): setfsuid, if_nameindex, if_nametoindex, unshare,
  setns, the packet socket and recvfrom, and mkfifo on macOS, in more crates than before.
- stats_alloc 0.1.10 (MIT; 22 million): the allocator of the raw and icmp hosts' waiting tests (dev-dependency only).
- netlink-packet-core 0.9.0, netlink-packet-route 0.34.0, netlink-sys 0.9.0 (MIT; 47 million each; already used by
  the route resolver host): the tcp suite's loopback.

## Measurements (ARM64 machine, Graviton4, 96 cores, Linux 7.0.0-1018-aws)
Speed of one send and one receive over loopback, `tests/speed.rs` of each host (median of 4000 batches of 32,
64-byte payloads, release build, root), before (HEAD copied before the change) and after, run alternately at load
5 to 10: TCP send 386-388 ns both, receive 408-411 ns both; UDP send 1.546-1.569 us before, 1.529-1.565 us after;
UDP receive 781-785 ns before, 776-785 ns after. No difference measured. An earlier single before run at load 23
gave UDP send 1.45 us; the alternating runs are the comparison.

## Suites (ARM64 machine, after the changes, 2026-11-23)
udp conformance 78 + readiness 5; tcp 140 + 4; the raw, icmp and route resolver hosts, the configurable and
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
