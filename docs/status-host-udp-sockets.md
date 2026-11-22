# Status of the host-udp-sockets track

Plan: docs/plan-host-udp-sockets.md. Branch: agents, worktree /Users/thomaseaton/Documents/GitHub/wcontainers-agents.

## 2026-11-22: milestone C22, the owner's networking interfaces of merge 158b6c62
- Stand-ins first, each file its own commit (a53ba1fe to 166baf72): the test policy, the local system (single-kernel)
  and the test host (unicorn) take `TcpLocalEnd` by value and answer `TcpLocalEndError`, have `type BoundSocket =
  Infallible` (none binds on its own: `bind` refuses, as the policy's does after its seeded refusal), `disconnect`,
  `release`, `TcpListenerReading`, `read_buffer_sizes` (`NotSupported`: their networks keep no such size), and name the
  loopback, `InterfaceIndex(NonZeroU32::MIN)`, where they named 0. A stand-in's `disconnect` closes the connection and
  leaves `ECONNRESET` pending, what Linux leaves on a connecting or connected socket it resets; it gives no socket back.
  None of the three stand-ins implements an ICMP socket, so the ping bind is nothing of theirs.
- The three stand-in crates cannot be built: they depend on wcontainers-kernel-linux, which does not build at
  158b6c62 (64 errors from the kernel interface's new `Kernel`; the kernel track's). Their socket files were instead
  type-checked with clippy, each in a crate of its own whose root stubs what the file takes from its crate
  (pyro-parked/wcontainers-scratch/host-udp-sockets/standin-typecheck.sh, run as the arm-run command): the policy's
  tcp.rs and udp.rs, the local system's tcp.rs and udp.rs and the test host's tcp.rs compile with no finding; a
  `BoundSocket` of `u8` there fails, so the check does compile them. The test host's udp.rs (it takes the local
  system's crate) only by reading. Build all three once the kernel builds:
  `cargo clippy --all-targets -p wcontainers-runtime-single-kernel -p wcontainers-executor-unicorn -p wcontainers-system-seeded-test-policy`.
- `Runtime` with `type Config` and `new`: nothing in the tree implements `Runtime` (single-kernel has no such impl),
  so there was nothing to fold.
- UDP host (a62b3f64): `read_buffer_sizes` is socket2's `send_buffer_size` and `recv_buffer_size` (`SO_SNDBUF`,
  `SO_RCVBUF`); portable.rs refuses it with the counted reason `BUFFER_SIZES` (other systems keep no doubled size). A
  received packet's interface comes from the shared `Ancillary::packet_fields`, which now refuses a packet without
  packet information (`EPROTO`) instead of naming 0.
- The shared crate linux-socket-options was conformed by another track (04600f02: `packet_fields` answering a
  `Result`, `Ancillary::interface`, `selected_interface` for the `InterfaceSelector`, `Slice1` sources,
  `REPORT_INTERFACE_NOT_GIVEN` gone); this track left those files alone. With that commit (ARM, load 8 to 11): clippy
  finds nothing in this track's files, the suite 78 + 5 pass three runs in a row.
- Suite: 78 checks (was 76). New: `a_datagram_names_the_interface_holding_its_destination` (`IP_PKTINFO` and
  `IPV6_PKTINFO`: a datagram to 127.0.0.1 and to ::1 names one interface, to 192.0.2.3 the multicast route's, to
  203.0.113.1 and to 2001:db8:1::1 a third, as the round-2 probe measured: to 10.9.0.1 on d1 named d1);
  `buffer_sizes_read_back_as_set_and_doubled` (`SO_SNDBUF`, `SO_RCVBUF`: 16384 and 24576 read 32768 and 49152; 1 GiB
  reads an even size below twice that). Added to old checks: a transmit report names the loopback beside its packet
  and no interface without one (probe: TSONLY reports carry no `IP_PKTINFO`); a rejected transmit time names none;
  an include filter of two sources (another, then the sender) takes the datagram. Gone: the check that an arrival
  interface is not 0 (the type cannot be). Per-packet interfaces in the suite are `InterfaceSelector::Requested`;
  the suite checks no `Bound` selector (its meaning is with the owner, through the internet sockets track).
- Run on the ARM machine (job udp-conform, load 7 to 12): clippy of the host and the suite with their tests, no
  finding in this track's files; the suite 78 + 5 pass; the macOS build (build-std, aarch64-apple-darwin) clean.
- Check (pyro-parked/wcontainers-scratch/host-udp-sockets/c22-check.sh, log c22-check.log beside it; ARM, load 14 to
  19): from a deleted build folder, clippy finds nothing in this track's files, the suite 78 + 5 pass, then 10 runs
  in a row with none failing. Five broken copies, each caught: read_buffer_sizes swapped (buffer_sizes_read_back_as_set_and_doubled);
  every arrival named the loopback (a_datagram_names_the_interface_holding_its_destination, a_bound_interface_is_taken,
  ipv6_memberships_take_their_datagrams); a report with its packet naming no interface (transmit_reports_carry_the_key_of_their_send);
  an include filter sending its first source only (multicast_membership_takes_the_groups_datagrams); a rejected
  transmit time naming the control message's index (rejected_transmit_times_are_reported_with_their_keys).
  `git diff main` of every interface crate's top-level files: nothing of the networking or runtime interfaces differs.


## 2026-11-22: conformed to the interfaces of a00ab267
- `wait_until_ready` answers `ReadinessReport`. The host holds kept error queue entries as
  `ReadinessReport { ready_for: empty, unconditional: ERROR }` through the shared `Watched::hold`. The settings use
  `UdpSocketSetting::Binding(BindingSetting::..)` and the shared `set_binding_setting`. Changed in linux.rs, portable.rs
  and tests/readiness.rs (commit 10cf2478).
- Suite: `finished` and `finished_soon` give the report. Checks that wanted ERROR in the facts now want it in
  `unconditional`. The exact-fact checks compare whole reports: a socket with nothing queued is WRITABLE with no events;
  with a datagram, READABLE | WRITABLE; with an ICMP error, WRITABLE and ERROR. The waits that only an error can end
  already asked for nothing. No check was removed; a wait for URGENT_DATA or PEER_FINISHED_SENDING still waits on.
- Run on the ARM machine (job udp-conform): clippy with warnings denied clean on the host and the suite; the suite ten
  times in a row, each 76 + 5 pass. The Mac's `nice -n 19 cargo check -p wcontainers-udp-sockets-host` (portable.rs)
  is clean, with no warning.
- Leftovers of the internet sockets track, checked: the resolver's IPv6 cap (65575) and its check
  a_path_mtu_is_capped_at_the_largest_packet_of_its_version were already in (66c1bc0b), and the advice suite already
  takes `NotSupported` and wants the same answer twice (324d9a19). Run three times on the ARM machine: resolver 37 + 1,
  advice suite 7. Nothing changed there.
- The system suite has `call_stream_transport_methods`, a placeholder like the FUSE and DNS ones. It names the four
  methods rather than calls them, since the trait gives no endpoint. The FUSE and DNS placeholders now drop their
  futures with `drop(..)`: clippy's `let_underscore_future` refused `let _ =` (commit e48be540).

## 2026-11-21: waiting is the Readiness trait
- Repair after the independent check: two long comments of the suite made one sentence; the plan names
  wait_until_ready. `NEVER_ON_UDP` stays beside ICMP's `NEVER_ON_DATAGRAMS`: this suite does not depend on ICMP's.
  Run on the ARM machine (job readiness-repair): 76 + 5 pass.
- `wait_for_events` and `PollEvents` are gone. `HostUdpSockets` implements `wcontainers_readiness::Readiness<HostUdpSocket>`
  (`wait_until_ready`, on the six facts of `ReadyFor`). A wait now gives every fact the socket has, not only those asked
  for. A UDP socket never reports URGENT_DATA or PEER_FINISHED_SENDING; a wait for only those waits for an error.
- Shared `readiness` (all four socket implementations): `Watcher::watch(fd, possible: ReadyFor)` takes the facts a
  kind of socket can have (`DATAGRAM_FACTS` for UDP, ICMP and raw; `ReadyFor::all()` for TCP);
  `Watched::wait_until_ready(ReadyFor)` (was `wait_for_events`); `Watched::hold(ReadyFor)`; `Watched::events_now`,
  `NO_EVENTS`, `poll_flags` and `poll_events` are gone; the thread name is `WATCHER_THREAD`.
- The edge-triggered registration stays: UDP no longer needs it (a wait asks only for facts the poller registers),
  but TCP's PEER_FINISHED_SENDING has no registration of its own and is registered as readable.
- Suite: it runs the readiness suite (`crates/readiness/conformance`) as module `readiness_contract`, on
  `waiting_for_a_datagram` and `waiting_for_data_until_an_icmp_error`. Its own copies are deleted
  (a_wait_for_data_finishes_when_a_datagram_arrives, a_wait_for_writing_finishes_at_once, a_dropped_wait_is_withdrawn,
  an_icmp_error_ends_a_wait_for_data, a_wait_for_no_event_ends_on_an_error). Deleted as only of the twelve bits:
  a_wait_for_one_event_gives_that_event_alone (normal and band bits) and the helper `only_asked_for`. many_waits_each_wake_once is now waits_on_many_sockets_each_wake_once
  (one wait on each of `MANY_WAITS` sockets). `WAITED_SOCKETS` and `WAITS_ON_ONE` are gone: raw and ICMP, take
  `MANY_WAITS` from the readiness suite. New check a_send_after_a_disconnect_carries_the_former_peers_address.
- Waiting checks now: the readiness suite's 6 plus 10 of this suite's own, and the implementation's 5 readiness
  tests (the old "seventeen" was wrong: there were 16). Run on the ARM machine: 76 + 5 pass, ten runs in a row.
- Next agent: the send without an address that the task asked a check of cannot be made through `send_to`, which always
  names a destination; the new check catches a disconnect that keeps the peer instead, since the implementation leaves out
  the address only when sending to its peer. `ROUTE_MTU` is now `linux_raw_sys::if_ether::ETH_DATA_LEN` (the
  literal check took 1500 for ICMP's `NARROW_LINK_MTU`); the literal check reports nothing in this track's files.
  Checked again later the same day: 76 + 5 pass, five runs before that change and five after; readiness suite 13, shared crate 1.
- For the owner: the trait's comment does not say that ERROR and HANG_UP end a wait whether or not they were asked for,
  which the implementations do and the readiness suite checks.

## For the other socket tracks (TCP, ICMP and raw)
- New, run 7: the shared `readiness` registers a waited socket edge-triggered (it was one-shot, level-triggered)
  and deletes the registration once no call waits unwoken. Reason: polling can register only readable,
  writable and priority, so a wait for POLLRDBAND, POLLMSG or POLLRDHUP is registered as readable; one-shot
  level-triggered, a socket with data to read but none of those fired again at every arming, and the watcher
  ran without stop (measured by `a_wait_for_an_event_the_socket_lacks_keeps_the_watcher_still`, which fails
  on the old code). Edge-triggered, it is looked at once per change of the socket. A wait that has to wait now
  costs one `epoll_ctl` more (add, then delete when woken, instead of one re-arm). Your public calls did not
  change; TCP's 95, raw's 46 + 2, ICMP's 38 + 1 and the resolver's 31 tests pass with it on the ARM machine.
- New, run 7: `ErrorEntry::read` reads on the stack and allocates only the entry's bytes (it zeroed a new
  64 KiB vector for every read, also of an empty queue). `ErrorQueue::keep_host_entries` reads the host's
  queue into the kept entries. ICMP and raw: your receives still make an error queue `recvmsg` before every
  normal one. UDP now reads the error queue only when a normal receive fails with an error other than
  would-block: Linux sets the socket error with every ICMP error it queues (`__udp4_lib_err`,
  `__udp6_lib_err`) and a receive answers that error before anything else; a send that fails may have taken
  it, so a failed send moves the host's entries into the kept ones. Measured for UDP: a receive that would
  block 332 ns (it was 1.146 µs; a plain `recv` 203 ns). Whether ping and raw sockets set the socket error the
  same way is yours to check.
- New, run 6: the shared `readiness` asks the host about a socket only while a call waits on it. `watch` no
  longer registers; a wait that has to wait arms a one-shot, level-triggered registration for the events its
  socket's waiting calls ask for, the thread arms it again only for calls still waiting, and the last call gone
  deletes one still armed. Measured: no watcher wake over 1200 datagrams nobody waited for (it was about one per
  datagram). Your public calls did not change; TCP's 90, raw's 45, ICMP's 37 and the resolver's 31 tests pass
  with it on the ARM machine. The watcher thread is now named `socket waits` (`readiness::WATCHER`; Linux keeps
  15 bytes of a thread's name, so the old one was cut).
- New, run 6: the shared crate builds on every unix system. `implementation.rs` is now only its root:
  `host.rs` (every unix: `sock`, `io_host_error`, `host_error`, `last_host_error`, `Shortfalls`; off Linux the
  host's error numbers become Linux's through a table) and `readiness.rs` are built everywhere, `linux.rs` (what
  was `implementation.rs`), `datagram` and `icmp_error` on Linux only. Every path you use is unchanged.
- New, run 6: `set_port_setting` makes the nonlocal addresses, bound interface and incoming processor settings
  through socket2 (`set_ip_transparent_*`, `set_freebind_*`, `bind_device_by_index_*`, `set_cpu_affinity`);
  only an incoming processor of none stays raw (`SO_INCOMING_CPU` -1, which socket2 cannot set).
- ICMP track: the literal check reports `WAITED_SOCKETS` and `WAITS_ON_ONE` in both our suites as one
  redefinition; yours is the copy of the UDP suite's, whose two are public since run 7: please import them from
  `wcontainers-udp-sockets-conformance` and delete yours (this track keeps its suite free of a dependency on
  another suite under work).
- Git, run 5 (this track's mistakes, all left as they are, since history is not rewritten): f0463350, a lock
  file commit, committed the whole staging area and took the TCP track's staged folder rename (four pure renames,
  no content); 93340ae9 took the route resolver track's folders back from `host` to `linux-netlink` and another
  track's `translate.rs` edit; 5296c9c4 then renamed the resolver's folders back and changed
  `crates/executor/implementation/hardware-virtualization/translate.rs` (76 lines). The end state is consistent:
  the resolver's folder is `host`, and `translate.rs` has a later commit by its own track (88d924b3). Every
  commit since names its paths.
- New, run 5: `readiness` in the shared crate, the host's readiness of sockets on `polling` 3.11.0 (Apache-2.0
  OR MIT), chosen over mio because it registers a socket of any kind on every system (mio cannot register an
  arbitrary socket on Windows). `Readiness::new()` makes one epoll set and one thread that waits on it;
  `Readiness::watch(OwnedFd)` gives a `Watched` that owns the descriptor and deletes its registration before it
  closes; `Watched::wait_for_events(PollEvents)` is the waiting call (nothing allocated per wait once a socket's
  waiter list has grown; dropping it withdraws the wait); `Watched::hold` adds events the implementation holds
  itself (UDP: entries it took off the error queue); `poll_flags`, `poll_events`, `NO_EVENTS`. TCP, ICMP and raw:
  use it for your `wait_for_events`; the only place a wake is fired is `readiness::wake`.
- New, run 5, socket2 0.6.5 (MIT OR Apache-2.0, feature `all`) in the shared crate: `sock(&fd)` (a `SockRef`),
  `io_host_error`, `bind_to`. `open_socket`, `bind`, `bind_address`, `bind_chosen_by_exit_node`, the buffer,
  sharing, multicast loopback and all-groups settings, `datagram::addresses::local_address`,
  `datagram::exchange::send_message` and the integer slots of `datagram::sticky` socket2 covers (TOS, traffic
  class, mark, priority, broadcast, freebind, transparency) now go through socket2; no signature changed.
  `recvmsg` stays libc: socket2's `RecvFlags` gives no `MSG_CTRUNC`, which `receive` counts.
- Careful: socket2 and the standard library keep a `SocketAddrV6`'s flow information as the socket address
  holds it (network order); rustix swaps it. `datagram::send` now gives `label.to_be()`. Any socket address with
  a flow label you hand to socket2 needs the same.
- ICMP and raw crates did not build at the time (no `wait_for_events` yet), so this track could not run their
  suites against the socket2 change; TCP's crate built.
- This track added the datagram half to the shared crate `crates/internet-protocol/implementation/linux-socket-options`,
  as `pub mod datagram`:
  - `datagram::send`: `SendState` (per socket) writes a route's selectors (`apply_route`), a send's header
    (`apply_header`) and its `IpSendFields` (`apply_send_fields`) as socket options, each written only when it
    differs from what the socket last had (`datagram::sticky`), and as `sendmsg` control messages (`Control`).
    It keeps `SO_TIMESTAMPING` itself (`set_receive_timestamps`).
  - `datagram::receive`: `receive` (one `recvmsg`), `receive_with_tail` (the same, with what does not fit the
    buffer going on into a second one), `Ancillary::parse`, `packet_fields`, `transmit_report`, and `Headers`,
    `ipv4_header`, `push_ipv6_headers`, `udp_checksum` to write a packet whole from its IP header.
    `udp_checksum` takes the payload in parts and returns the checksum as etherparse's header fields hold it.
  - `datagram::memberships`: `set_multicast_group` (join, then the whole source filter with `MCAST_MSFILTER`;
    leave; a join on no interface is made on the interface Linux would choose) and `AnycastMemberships`.
  - `datagram::addresses`: socket address conversions.
- `implementation.rs`: `bind_address`, `host_port_range`, and `bind_chosen_by_exit_node`, which binds at once to
  a port of a `PortChoice` from any range (ports outside the host's `net.ipv4.ip_local_port_range` are tried
  one at a time, counted, `PORTS_OUTSIDE_THE_HOSTS_RANGE`).
- Run 4 added to the ICMP track's `datagram::exchange` (the UDP crate now uses `send_message` and `ErrorQueue`,
  its own copies are gone): `ErrorQueue::expect_transmit_time_errors` notes a send that asked for transmit time
  errors; `ErrorQueue::received` is `error_entry` with a transmit time rejection given its send's key and
  reason (`TransmitTimeRejected`), and the packet if the send asked; `ErrorQueue::settle` reads the error queue
  into the kept entries once the host holds none of the socket's sends (`SO_MEMINFO`), and forgets the noted
  sends with no rejection there; `holds_entries`. ICMP and raw tracks: switching from `error_entry` to
  `ErrorQueue::received` gives your sockets the same.

## Milestone U1, run 7 (2026-11-21): the second independent check's findings fixed
- Every poll bit is now waited for in the suite: `a_wait_gives_the_events_the_socket_has_and_no_others`
  (every bit asked; exactly POLLOUT, POLLWRNORM, POLLWRBAND, then also POLLIN, POLLRDNORM with a datagram, then
  POLLERR with an ICMP error; IPv4 and IPv6), `a_wait_for_one_event_gives_that_event_alone` (POLLWRNORM and
  POLLWRBAND at once, POLLRDNORM when a datagram comes, each alone), `a_wait_for_an_event_udp_never_has_waits_on`
  (POLLPRI, POLLHUP, POLLRDBAND, POLLMSG, POLLRDHUP: unwoken for 200 ms with data queued). A wait that reports
  every bit asked (the check's M21) fails two of them.
- A receive that would block makes one system call and allocates nothing (it made a second, on the error
  queue, after zeroing a new 64 KiB vector): see "For the other socket tracks". Own test
  `a_receive_that_would_block_allocates_nothing` (allocations of its own thread over 1200 receives: 0).
- The readiness is edge-triggered (above), found while writing the never-had checks.
- New suite checks: `a_wait_for_data_or_writing_finishes_when_room_is_made` (catches a registration not
  widened for an overlapping interest, M11), `a_header_of_another_protocol_is_refused` (UDP and UDP-Lite each
  sent as the other: refused, nothing arrives; M14), `a_checksum_of_the_other_protocol_is_refused` (partial
  coverage on UDP, omitted on UDP-Lite; M15).
- `a_full_send_buffer_counts_its_unsent_bytes` now holds for either accounting: at least the payload held (all
  but the first two datagrams) and less than twice the size given and one datagram more. Linux's own count (at
  least twice the size given) is checked by the implementation's own `a_full_send_buffer_counts_the_memory_linux_counts`.
- `an_icmp_error_waits_for_a_receive_after_later_sends`: an ICMP error, then a send (which Linux fails with that
  error), then a receive gives the ICMP packet (IPv4 and IPv6); it catches a failed send that keeps no entries.
- Own test `a_woken_wait_costs_the_watcher_nothing_until_it_is_polled` (0 watcher wakes over 1200 datagrams
  after a wait was woken and before it is polled).
- `conformance.sh` builds with cargo's messages first, so a failed build says why.
- The five broken copies of the findings (M21, M11, M14, M15, and the readiness of before this run) are each
  caught by the checks named above (scratch `run7/mutants.sh`, log `run7/mutants.log`); the independent check
  below covers them too.

## Independent check (run 7, 2026-11-21, ARM64 machine, load 2.3 to 7.8, 243 s)
- Script: scratch `independent-check.sh` (run 6's kept as `independent-check-run6.sh`); log
  `independent-check-run7.log`. Clean build (fresh target folder, 13 s): 75 + 4 pass (75 + 5 with the test added after it, last run at 22:23 UTC, load 1.3). 30 runs in a row: none
  failing (55 s).
- 55 broken copies: 49 caught by a named check, among them this run's (every bit asked reported, overlap not
  widened, any protocol, any checksum, failed send keeping nothing, kept entries not first, level-triggered).
  One more was caught only after the check ran: the registration kept after every call was woken passed, and
  `a_woken_wait_costs_the_watcher_nothing_until_it_is_polled` (added for it) fails on it (scratch `run7/kept.sh`).
  Five pass, as expected: the would-block receive reading the error queue and an error entry read on the heap
  (costs only, measured above: neither changes an answer), the wake `hold` gives (see "Not checked"), negative
  route advice doing nothing and IPv4 checksum omission ignored (below).
- cargo tree (normal, build and dev edges) of the udp-sockets, internet-protocol, configurable,
  negative-route-advice and kernel interface crates and the UDP suite: no implementation crate. No reviewed
  file changed: `git diff ae9c53ca HEAD` of the udp-sockets, internet-protocol, configurable and
  negative-route-advice interfaces is empty, and this run's commits touch only the two implementation crates,
  the suite and this track's docs.

## Milestone U1, run 6 (2026-11-21): the independent check's findings fixed
- The crate builds on every unix system: `implementation.rs` is its root, `linux.rs` the Linux body (unchanged
  but for the items below), `portable.rs` the body elsewhere (opening, binding, connecting, waiting and the
  settings socket2 covers; a counted refusal for the rest, listed below). Checked for macOS with clippy on the
  ARM machine (`-Zbuild-std`, target aarch64-apple-darwin): clean. Not run there: no Mac tests, by the rule that
  the Mac only edits. The suite builds only on Linux now (it checks Linux on a Linux network; it already did not
  build elsewhere, for the atomic clock).
- Readiness: see "For the other socket tracks"; the implementation's own `tests/readiness.rs` checks it
  (`a_socket_nobody_waits_on_costs_the_watcher_nothing`, `a_wait_allocates_nothing_once_warm`).
- socket2 now also: `set_recv_tos_v4`, `set_recv_hoplimit_v6`, `set_recv_tclass_v6` at open, `cpu_affinity` for
  the incoming processor, and the shared port settings above.
- A send with an IPv6 header on an IPv4 socket answers EAFNOSUPPORT, as Linux's sendto does (it answered
  ENOPROTOOPT from the IPv6 option written first).
- Suite: 7 new checks (newest waker, a wait whose event went, two waits on one socket, a wait for no event,
  unsent bytes of a full send buffer, a wait for writing until room is made, the other version's header), and
  the incoming processor check now sets two processors and reads a connected socket's from where its sender ran.
  The implementation's own test `a_report_comes_before_a_datagram_that_arrived_after_it` checks the errors-first
  order. 67 + 2 tests pass on the ARM machine.

## Milestone U1: done again after the async ruling (run 5, 2026-11-21)
- Run 5 did the ruling's three items: the crate is named `host`; `wait_for_events` waits on the host's
  readiness (shared `readiness`, on polling); the basic calls and the settings socket2 covers go through
  socket2. Suite: 6 new checks of waiting. All 59 tests pass on the ARM64 machine; independent check below.
- Crate `crates/udp-sockets/implementation/host` (package `wcontainers-udp-sockets-host`, type
  `HostUdpSockets`, socket `HostUdpSocket`), body `cfg(target_os = "linux")`. Refusals, host cuts and
  slow paths counted per reason (`counters()`), warned once on standard error.
- Suite `crates/udp-sockets/conformance/conformance.rs`: 58 checks against the traits alone. `set` sets every
  setting twice and asserts both answers are equal; the Configurable and NegativeRouteAdvice suites run on a
  socket too. Every received UDP and UDP-Lite checksum, IPv4 header checksum and ICMP checksum is verified.
  The network the suite needs beyond loopback is listed at the top of the file; the implementation's test makes it.
- The implementation's own test `a_buffer_the_host_cuts_is_counted` (in `tests/conformance.rs`).
- Last run: 59 of 59 pass on the ARM64 machine, and 30 runs in a row.
- Waiting checks, polled by hand with a waker that counts its wakes (no executor): a_wait_for_data_finishes_when_a_datagram_arrives
  (pending and not woken for 200 ms, woken once when a datagram comes, only events asked for), a_wait_for_writing_finishes_at_once,
  a_dropped_wait_is_withdrawn (the waker given back, not woken, a new wait sees the datagram), many_waits_each_wake_once
  (35 waits on 32 sockets, each woken once, and not again by a second datagram before it is polled),
  an_icmp_error_ends_a_wait_for_data (error condition, IPv4 and IPv6), a_wait_sees_an_error_a_send_read_first.

## How to run
- On the ARM64 machine (root):
  `arm-run /Users/thomaseaton/Documents/GitHub/wcontainers-agents --name udp-conform -- 'crates/udp-sockets/implementation/host/conformance.sh 12'`
  (the argument is the test threads; further arguments filter tests by name). It builds as the user and runs the
  two test binaries, `conformance` and `readiness`, with `sudo -n`.
- The build for macOS, checked on the ARM machine: `RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR=/work/target/host-udp-sockets-darwin
  cargo clippy -Zbuild-std=std,core,alloc,panic_abort --target aarch64-apple-darwin -p wcontainers-internet-protocol-linux-socket-options
  -p wcontainers-udp-sockets-host -p wcontainers-udp-sockets-conformance --tests`.
- The test makes 80 network namespaces (`NETWORKS`) at the first test's start, while every other
  test waits before opening a socket (a child process holds its parent's descriptors until it starts its
  program, which kept closed sockets alive), with `ip -batch` and `tc` (test tools, never dependencies); each
  test then enters one of its own.
- Lint: `cargo clippy -p wcontainers-internet-protocol-linux-socket-options -p wcontainers-udp-sockets-host -p wcontainers-udp-sockets-conformance --tests`
  on the ARM64 machine: clean for these crates (`-D warnings` stops at two warnings of the reviewed
  `crates/kernel/interface.rs`). Every build and test runs there; the Mac only edits, formats (rustfmt on the
  files alone) and runs the literal check (it needs git): nothing of this track's is reported.
- Independent check: pyro-parked/wcontainers-scratch/host-udp-sockets/independent-check.sh, passed as the arm-run
  command (`-- "$(cat .../independent-check.sh)"`): a clean build and run, 30 runs in a row, 38 broken copies of
  the implementation and of the shared crate's send, exchange and readiness files, and cargo tree of the interface
  crates. Log of run 5: independent-check-run5.log beside it.
- No random tests: every check is a fixed scenario.

## Measurements
- Run 7, 2026-11-21, ARM64 machine, load 2.7: a receive that would block 332 ns (three runs, 331 to 332), a
  plain `recv` that would block 203 ns, a receive of a datagram 1.036 to 1.041 µs (the check's `verify_alloc.rs`,
  10 000 each, as root). The 74 + 4 tests take 1.10 to 1.12 s and 0.52 s at 12 threads (load 2 to 4).
- Run 6, 2026-11-21, ARM64 machine, load 6 to 20: the 67 + 2 tests take 0.90 to 0.92 s and 0.21 to 0.22 s at 12
  threads. The watcher thread's wakes (voluntary context switches) over 1200 datagrams sent and received with
  no wait: 0 (the independent check measured 9473 of 10 000 before the change); allocations over 1200 waits
  that had to wait, once warm: 0.
- Run 6, ARM64 machine, load 22: Linux's `SIOCOUTQ` of a UDP socket with paced datagrams of 1000 bytes held is
  2304 bytes per datagram held (Python, fq on loopback): 0, 2304, 4608, 6912, 9216 after 1 to 5 sends.
- Run 5, 2026-11-21, ARM64 machine, load 9.8 to 12.6: the 59 tests take 0.89 to 0.90 s at 12 threads; a clean
  build and run 12 s; 30 runs in a row 29 s. No timing of a wait itself was taken.
- 2026-11-21, ARM64 machine (96 cores): the 53 tests take 0.88 s at 12 threads, load 1.1 (making the 64
  networks is part of it). Run 3: 48 tests in 0.86 to 0.87 s at load 3.4 to 10.5; 30 runs in 28 s.
- Pacing on the machine's loopback with fq, 5 datagrams of 1000 bytes at 50 000 bytes per second: arrivals at
  0.1, 20.9, 41.7, 62.6 and 83.4 ms (Python, run 2); the check asks for at least 80 ms, and less than 80 ms
  once the rate is unset.
- IPv6 path MTU modes over a local route of MTU 1500 (Python, run 3): Dont and Want fragment (largest fragment
  1496), Do answers EMSGSIZE, Probe, Interface and Omit send 3000 bytes whole.
- etf on a dummy interface, clock TAI (Python, run 4, load 3.6 to 8.7): with a delta of 0 every future time is
  missed (ECANCELED, code 2, after its time) and the send succeeds; a past time, another clock (realtime,
  monotonic) or a socket in deadline mode is invalid at once (EINVAL, code 1) and the send answers ENOBUFS;
  with a delta of 100 000 ns nothing is reported. The entry gives the time as data high and information low.
- Source preferences (Python, run 4): with `use_tempaddr` 2 and 2001:db8:1::1/64 mngtmpaddr, connect to
  2001:db8:1::99 takes the temporary address by default and with temporary, the public one with public; home
  and care-of change nothing, even with an address marked home.

## Independent check (run 6, 2026-11-21, ARM64 machine, load 15 to 19, 184 s)
- Script: scratch `independent-check.sh` (run 5's kept as `independent-check-run5.sh`); log
  `independent-check-run6.log`. Clean build (fresh target folder, 13 s): 67 + 2 pass. 30 runs in a row: none
  failing (38 s).
- 46 broken copies: 44 caught by a named check, among them the findings' four (incoming processor always the
  first: the_incoming_processor_is_read_back; unsent bytes doubled: a_full_send_buffer_counts_its_unsent_bytes;
  first waker kept: a_wait_polled_again_wakes_its_newest_waker and a_wait_whose_event_went_waits_for_the_next;
  errors-first off: a_report_comes_before_a_datagram_that_arrived_after_it), and the new ones (registration kept
  edge-triggered: a_socket_nobody_waits_on_costs_the_watcher_nothing; never armed: 8 checks and both readiness
  tests; not armed again for calls still waiting: a_wait_for_writing_finishes_when_room_is_made; the other
  family sent: a_header_of_another_version_is_refused). The two expected to pass still pass (below).
- cargo tree (normal, build and dev edges) of the udp-sockets, internet-protocol, configurable,
  negative-route-advice and kernel interface crates and the UDP suite: no implementation crate. No reviewed
  file changed: this run's commits touch only the two implementation crates, the suite and this track's docs,
  and `git diff ae9c53ca HEAD` of the udp-sockets, internet-protocol, configurable and negative-route-advice
  interfaces is empty.

## Independent check (run 5, 2026-11-21, ARM64 machine, load 9.8 to 12.6, 120 s)
- Clean build (fresh target folder, 12 s): 59 of 59 pass. 30 runs in a row at 12 threads: none failing (29 s).
- 38 broken copies. 36 are caught by a named check: run 4's 30, still caught, and the waits': a wait that
  finishes at once (4 checks), a dropped wait left registered (a_dropped_wait_is_withdrawn), a wait never woken
  (3), a wait that ignores the errors held (a_wait_sees_an_error_a_send_read_first), and a waker cloned instead of
  taken, so woken again by every datagram (caught by many_waits_each_wake_once only after the check sent a
  second datagram before polling; rerun separately after that change). The other two pass, as expected:
  negative route advice doing nothing and IPv4 checksum omission ignored, whose effects cannot be seen (below).
- cargo tree (normal, build and dev edges) of wcontainers-udp-sockets, -internet-protocol, -configurable,
  -negative-route-advice, -kernel and -udp-sockets-conformance: no implementation crate.
- No reviewed file changed: none of this track's commits touches an `interface.rs` or a data file beside one, and
  `git diff ae9c53ca HEAD` of the udp-sockets, internet-protocol, kernel, configurable and negative-route-advice
  interface crates is empty.

## Which check fails when a method breaks
- open: every check (owner: an_owner_is_routed_as_its_user). bind: bind_returns_the_bound_address, the sharing
  checks, both exit node checks, a_dual_stack_socket_takes_ipv4. connect: a_connected_socket_takes_only_its_peers_datagrams,
  and connect_takes_its_route_selectors for its selectors. disconnect: disconnect_keeps_the_binding. send_to:
  every check that sends; each header and send field by its own check; a header of the other protocol or a
  checksum of the other protocol sent anyway by the two refusal checks; a failed send that loses the error
  queue's entries by an_icmp_error_waits_for_a_receive_after_later_sends. receive: every check that receives;
  a_short_buffer_keeps_the_whole_length (cut in the UDP header and in the payload), receive_with_nothing_queued_would_block,
  a_closed_port_answers_with_an_icmp_error, transmit_reports_carry_the_key_of_their_send,
  rejected_transmit_times_are_reported_with_their_keys. count_unsent_bytes: pacing_holds_datagrams_in_the_send_buffer,
  sending_settings_are_taken, and a_full_send_buffer_counts_its_unsent_bytes for its value (a count doubled is
  caught; one halved only by the implementation's own a_full_send_buffer_counts_the_memory_linux_counts). read_incoming_processor: the_incoming_processor_is_read_back (two processors set, and a
  connected socket's from its sender's processor). close:
  close_frees_the_port. configure: each setting's check, and the two Configurable contract checks.
  wait_until_ready (2026-11-21): sixteen waiting checks (the readiness suite's six and this suite's ten) and the
  implementation's five readiness tests; a wait giving facts it lacks is caught by
  a_wait_gives_the_facts_the_socket_has_and_no_others, one not widened for an overlapping interest by
  a_wait_for_data_or_writing_finishes_when_room_is_made, a watcher kept running by the implementation's own
  a_wait_for_an_event_the_socket_lacks_keeps_the_watcher_still; a wait that keeps its first waker is caught by
  a_wait_polled_again_wakes_its_newest_waker, a registration not armed again for calls still waiting by
  a_wait_for_writing_finishes_when_room_is_made, a registration left on by the readiness test. The order of a
  send's reports before later datagrams: the implementation's own a_report_comes_before_a_datagram_that_arrived_after_it.
- give_negative_route_advice: negative_route_advice_keeps_a_socket_sending and negative_route_advice_contract_holds
  catch only a refusal. One that does nothing and answers Ok passes the suite: its effect cannot be seen
  through the traits (see below).

## Honoured, and how each is checked
- open (both versions, both protocols, the owner by routing), bind (kernel port, a taken port, the exit node's
  choice around a wide reserved range and from a range below the host's own, IPv6 only and dual stack),
  connect (with its selectors), disconnect (explicit port kept, a kernel-chosen port given up, as Linux does),
  send_to, receive, count_unsent_bytes, read_incoming_processor, negative route advice, close, and
  wait_for_events (POLLIN, POLLRDNORM, POLLOUT, POLLWRNORM, POLLWRBAND, POLLERR each seen, alone and with
  every bit asked; POLLPRI, POLLHUP, POLLRDBAND, POLLMSG and POLLRDHUP waited for and not given, as Linux gives
  them to no UDP socket: no shutdown call, no `SO_SELECT_ERR_QUEUE` setting, no priority band; POLLNVAL cannot
  come for an open socket; the error condition also for entries the implementation took off the error queue).
- Header fields checked by what arrives or what is refused: TTL, hop limit, DSCP, ECN, flow label, IPv4 options,
  IPv6 hop-by-hop, destination and routing destination options, segment routing header, IPv4 and IPv6 source
  address, mark, interface; link scope (`ENETUNREACH` to an address behind a gateway, universe sends); every
  path MTU discovery mode over a route of MTU 1500 (Want and Dont fragment, Do is too long with that MTU, Probe,
  Interface and Omit send whole), IPv4 and IPv6; local fragmentation forbidden makes a Want datagram over the
  route's MTU too long (MTU 1500), allowed fragments it; fragment MTU with Do.
- Route selectors through connect, by the local address it returns: IPv6 source preferences (public gives the
  public address, temporary the temporary one, the system default the host's choice, temporary), and an IPv4
  interface given (that interface's address, not the route's).
- Send fields: priority (received), transmit time (held at least 200 ms by fq on the monotonic clock),
  transmit time errors (etf on the atomic clock: a past time, the realtime clock and deadline mode are
  reported invalid, a missed time missed with its packet, each with its send's key; a send not asking gets no
  report), transmit reports scheduled and software with key and packet or not.
- Datagram fields: checksum computed (valid and not zero on arrival), omitted over IPv6, UDP-Lite partial
  coverage against the receiver's minimum; segment size.
- Received fields: interface, specific destination, largest fragment, mark, priority, software timestamps, no
  hardware timestamp on loopback, drops since previous, segment size; a packet cut by a short buffer keeps the
  whole length and the bytes of the whole packet, checksum included.
- Settings checked by their effect: address and port sharing, nonlocal addresses, bound interface (sends over
  it; a socket bound to another interface takes no loopback datagram until unbound), incoming processor, IPv4
  multicast (join, loopback off and on, source filters Include and Exclude with sources, leave), IPv6 multicast
  (join on the chosen interface and on one given, leave, loopback off and on), all-groups on and off for both
  versions, IPv6 anycast join and leave, the port group's filter (choosing and detached), receive buffer, send
  buffer, maximum pacing rate (set and unset), timestamping, receive coalescing, zero checksum receipt, minimum
  checksum coverage (set, and none: any coverage arrives).
- ICMP errors on the error queue as whole ICMP packets with valid checksums (port unreachable, IPv4 and IPv6).

## Not checked
- The wake `Watched::hold` gives when a send moves an error off the host into the kept entries while a call
  waits: it matters only when the send wins a race with the watcher thread, which sees the host's error too, so
  no check through the traits can make it happen (the check's M19, dropping that wake, passes).

## Not honoured on other unix systems (portable.rs), each a counted refusal
- Run 7, the check's finding that no datagram can be exchanged there: left as it is. A send's header asks for
  IPv4 path MTU discovery Want (Linux's default, DF set) and fields socket2 has no call for there, and a receive
  owes the whole IP header, which needs control messages (`IP_RECVTTL`, `IP_RECVDSTADDR`, `IP_RECVIF`) that
  socket2 does not parse; parsing them by hand for a system no machine of this project runs tests on would be
  untested code. The refusals stay counted under their reasons.
- UDP-Lite (no such sockets), an owner other than the process's user (only Linux has a thread's filesystem
  user), the exit node's port choice (Linux's `IP_LOCAL_PORT_RANGE`), route selectors beyond destination and
  ports, disconnect (socket2 has no connect to `AF_UNSPEC`), every send (the IPv4 path MTU discovery of Linux's
  default, Want, and most header and send fields have no socket2 call there), every receive (the IP header needs
  the control messages socket2 does not parse), the unsent bytes, the incoming processor, negative route advice,
  and every setting but buffers, address and port sharing and multicast loopback. Settings at the one value
  every socket there has (no pacing, nonlocal refused, no bound interface, group filter or processor, separate
  receives, zero checksums dropped, no timestamps) are taken; a minimum checksum coverage answers ENOPROTOOPT,
  as Linux does on a UDP socket. No Windows build: its sockets are not descriptors and its readiness check would
  need WSAPoll.

## Not honoured or not checkable, with the Linux reason
- Mobile IPv6 source preferences (home, care-of): written (`IPV6_PREFER_SRC_HOME`, `_COA`), but Linux applies
  them only in source address rule 4, which it builds only with mobile IPv6 built in; the machine's kernel has
  `CONFIG_IPV6_MIP6=m`, and an address marked home is chosen the same with either (measured).
- IPv4 checksum omission (`SO_NO_CHECK`): written, but not checkable through a host receiver: Linux gives a UDP
  socket no received checksum, so a receiver writes the computed one. IPv6 omission is checked, because Linux
  drops a zero checksum unless the receiver accepts it.
- Wireless status reports: Linux reports them (`SO_WIFI_STATUS`) with no key of the send; a send asking for one
  answers `NotSupported`, counted.
- Acknowledgement reports: Linux makes them only for TCP. Completion and hardware transmit reports, and
  `link_layer_length` (Linux gives the frame length only in `SCM_TIMESTAMPING_PKTINFO`, which comes only with a
  hardware receive timestamp), need a driver with a hardware clock; loopback has none, so none is checked.
- Neighbour confirmation (`MSG_CONFIRM`): taken, but its only effect is the neighbour entry's confirmed time,
  which no socket can read; loopback and the dummy interfaces have no neighbour entries.
- Negative route advice has no system call: a connected socket is connected again to its peer, which drops its
  cached route and takes a new transmit hash. An unconnected socket answers Ok and does nothing, which is all
  there is to do: Linux caches a route only for a connected UDP socket (`udp_sendmsg` checks `sk_dst_cache`
  only when connected), so there is no route to drop. Its effect cannot be seen through the traits: Linux uses that hash
  for the automatic IPv6 flow label, which the contract replaces by the label each send gives (measured: label 0
  before and after), and UDP's multipath hash is of addresses and ports.
- Transmit time rejections: a rejection Linux makes at once also fails the send with ENOBUFS, as Linux does for
  a socket with `IP_RECVERR`; the report follows on the error queue. A rejection of a time no noted send gave
  (a send that did not ask, on a socket whose last send did) is dropped, counted and warned.
- Header fields Linux never gives a UDP socket: IPv4 identification and flags (written as zero and none), the
  received UDP checksum (recomputed), the sender's UDP-Lite coverage (written as zero, the whole datagram).
- In an ICMP error: the quoted IPv4 TTL and UDP checksum are zero, and the quoted IP total length and UDP length
  are those of the part quoted (Linux gives only the quoted payload, after the headers it strips).
- An error queue entry longer than the buffer reports only the part read: Linux's error queue gives no length.
- A transmit report without its packet has interface 0: Linux gives the interface only with the packet.
- A send whose source port is not the socket's, and a connect whose source is not the bound address: Linux sends
  from the bound port and address only; `NotSupported`, counted.
- Send and receive buffer sizes above the host's `wmem_max`/`rmem_max` are cut by the host; taken, counted and
  warned.
- Routing headers: Linux takes only segment routing (type 4) and, with mobile IPv6 built in, type 2.

## Dependencies and licences
- socket2 0.6.5 (MIT OR Apache-2.0, feature `all`) for the calls and settings it covers; polling 3.11.0
  (Apache-2.0 OR MIT) for the host's readiness, in the shared crate; both were already in the workspace.
- rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; the suite uses its clocks, with its
  `linux_4_11` feature for the atomic clock), linux-raw-sys 0.12.1 (same), libc 0.2.191 (MIT OR Apache-2.0),
  etherparse 0.21.0 (MIT OR Apache-2.0); all already in the workspace. The suite also uses
  wcontainers-configurable-conformance and wcontainers-negative-route-advice-conformance. iproute2 (GPL) is a
  test tool the implementation's test runs, never a dependency.

## Departures from the plan
- Run 7: the readiness is edge-triggered, not one-shot level-triggered (the plan says so now): a one-shot
  level-triggered registration cannot wait for an event the poller only approximates without running at once.
- Run 7: the suite's unsent byte check takes either accounting, and Linux's own count moved to the
  implementation's test, until the owner rules (below).
- Run 6: the brief asks for socket2 everywhere with what Linux alone has behind `cfg(target_os = "linux")`;
  run 5 built nothing off Linux. Now the crate and the shared crate build on every unix system, with two bodies
  (Linux's whole one, and the socket2 one elsewhere), since nearly every call of the Linux body is a Linux
  option or control message. Windows is not built (above).
- Run 6: the errors-first order is checked by the implementation's own test, not the suite: the contract does
  not say in which order a report and a datagram come.
- Run 6: the suite is `cfg(target_os = "linux")`.
- Run 5: the crate is `host` (the ruling's name); the plan says so now. The readiness lives in the shared crate,
  so that TCP, ICMP and raw share one; it runs one thread per implementation value, since nothing else waits on
  the host's events. `recvmsg` stays on libc (socket2 gives no `MSG_CTRUNC`), and `connect` to `AF_UNSPEC`
  (disconnect) on rustix (socket2 has none).
- The shared crate's calls are used instead of this track's own (opening as the
  owner, binding, ICMP errors, sending, the error queue), and the datagram modules moved there.
- The plan's test namespace had loopback only. It now also has two dummy interfaces: multicast0 (IPv4 and IPv6
  multicast, 192.0.2.3/24, a gateway route) and scheduled0 (203.0.113.1/24, 2001:db8:1::1/64 with a temporary
  address, an etf qdisc of delta 0), routes of MTU 1500 to 127.0.0.2 and 2001:db8::2, a user rule, an fq qdisc on
  loopback and a segment-routing sysctl, all listed at the top of the suite. Reasons: Linux takes no IPv6
  multicast over loopback and its loopback interface brings back every multicast datagram; path MTU modes
  differ only where a route's MTU is below the interface's; transmit time errors need a qdisc that keeps a
  schedule; source preferences need a public and a temporary address.
- A receive reads the error queue first only while a send's reports or entries may wait, and otherwise only
  after a normal receive fails: run speed, one system call per datagram. While reports may wait, it also reads
  `SO_MEMINFO` to learn when none can still come.
- A receive takes what does not fit its buffer into a 64 KiB tail on the stack (uninitialised, a second
  `iovec`), so that a cut datagram's checksum is the whole datagram's.

## For the owner
- `TcpConnections::disconnect` answers no error, so an implementation that cannot read the pending error must
  choose one. Example: the in-process stand-ins (single-kernel, unicorn) close the connection and report
  `ECONNRESET`, which is wrong for a connection whose connect already failed (Linux would leave its own error).
- The interface has no constructor: proposed `trait StandaloneUdpSockets: UdpSockets { fn new() -> Result<Self, HostError>; }`
  (it can fail now: making the epoll set or its thread); meanwhile `HostUdpSockets::new()`.
- `wait_for_events` works as written. One consequence: nothing in the interfaces lets the embedder wait on the
  host's events, so a host implementation needs a thread of its own that waits in `epoll_wait` and wakes from
  there. Example: a kernel with every guest thread waiting on sockets sleeps in its scheduler while this
  thread sleeps in `epoll_wait`, and each datagram costs a wake across threads. A system-level call that
  waits on the host's readiness for a while would let one thread do both; not needed for correctness.
- `UdpSocketSetting::MinimumChecksumCoverage(None)` is not defined. Example: Linux's `UDPLITE_RECV_CSCOV` of 0
  means full coverage required (a datagram of coverage 8 is then dropped), while a socket never set takes any
  coverage. This implementation reads None as no minimum (it writes 8, the UDP header, which takes every valid
  coverage), so a guest's "full coverage required" cannot be said through the contract.
- `UDPLITE_SEND_CSCOV`, `UDPLITE_RECV_CSCOV`, `SO_EE_ORIGIN_TXTIME` and the two `SO_EE_CODE_TXTIME_*` are in no
  established crate. The kernel implementation keeps private copies in
  `crates/kernel/implementation/linux/linux_extra.rs`; the shared crate has its own. One should import the other.
- `TransmitReport.interface` cannot be filled for a report without its packet: an `Option<InterfaceIndex>` would
  say so; this implementation writes 0.
- There is no way through the interfaces to read the counters of refusals; `counters()` is inherent.
- `count_unsent_bytes` says "the bytes sent ... that have not yet left its send buffer", but Linux's `SIOCOUTQ`
  on a UDP socket gives the memory its datagrams take. Example (measured): one paced datagram of 1000 bytes held
  counts 2304. This implementation gives Linux's value, which is what a Linux program reads; the suite takes
  either (at least the payload held, less than the most the buffer holds).
- Off Linux the host's error numbers become Linux's through a table in the shared crate; the runtime's
  `crates/runtime/implementation/single-kernel/errno.rs` has the same kind of table, private. One crate of it
  would serve both.
- The shared crate's name says linux; it now also holds what every unix build shares (readiness, socket2
  handles, errors, refusals).
- `PollEvents` has no empty value or set operations, so every user writes its own all-false value (the suite
  and the shared crate each have `NO_EVENTS`).
