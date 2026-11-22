# Status: host-tcp-sockets

Plan: docs/plan-host-tcp-sockets.md. Milestones there: T1 (connections and listeners, the suite), T2 (state
readings, repair, authentication, multipath state, the privileged settings).

## Run 15 (2026-11-22): the owner's interfaces of a00ab267, and the check of run 14 answered
- Conformed to the owner's interfaces merged at a00ab267 (commits 61bb2b6f, 223130ff): `wait_until_ready` answers
  `ReadinessReport { ready_for, unconditional }`; `POLLERR` and `POLLHUP` are `UnconditionalEvents` and end every
  wait, asked or not; `TcpSetting::Port(PortSetting)` is `TcpSetting::Binding(BindingSetting)`; the crate rename to
  negative-route-advisable was already applied by the merge.
- Shared crate readiness.rs (this track's): `hold` takes a `ReadinessReport`; `NOTHING` (a report of no fact) is
  exported; `DATAGRAM_FACTS` is READABLE and WRITABLE only; the error and hang-up bits are their own table. The
  shared linux.rs's `set_port_setting` is `set_binding_setting`. Its own test holds an unconditional error.
- The readiness suite (crates/readiness/conformance), which every socket suite runs and nobody else had changed
  yet: its checks read the report (`ready_for` for the facts asked for, `unconditional` for the failure check);
  `ALWAYS_REPORTED` is gone, since a wait can no longer ask for an error or a hang-up; its doubles report the
  unconditional events apart. 13 of its own tests pass.
- TCP suite: the hang-up and error waits ask for nothing (`ReadyFor::empty()`), which only an unconditional event
  ends, and demand exactly HANG_UP, or ERROR with HANG_UP; the exact-facts checks compare whole reports.
- The stopped agent's uncommitted work (an answer to the independent check of run 14, whose eight broken copies
  had passed: logs ic15-*.log in scratch) is finished and committed: set_timestamp_clock now sets the unit too,
  from the value's low bit, as Linux 6.7 and later do in repair mode (`tp->tcp_usec_ts = val & 1`; run 6's
  measurement that the route fixes it was wrong), with a counted refusal (`NO_MICROSECOND_CLOCK`) and the clock put
  back on an older kernel; the check is now timestamp_clock_in_the_other_unit_counts_in_it (no refusal allowed). A
  multipath socket's urgent mark reads not reached (`SIOCATMARK` is ENOTTY on MPTCP) and its tuned values are a
  counted refusal (the clamp and quick acknowledgement are EOPNOTSUPP there); the unfilled `tcpi_last_ack_sent` and
  `tcpi_fackets` and the 6.19 fullmesh limit are counted. New check multipath_connection_answers_every_query, and
  more readings checked (windows, timers, segment counts, retransmission counts, Accurate ECN zeros, the multipath
  checksums against the captured MP_CAPABLE, the subflow identifier, the repair window's update sequences and
  largest window). Plan corrected for the clock.
- Runs: see Measurements, run 15.

## Run 14 (2026-11-22): the independent check of run 13 answered
- [blocking] LastReceivedHeader dropped every IPv6 extension header: it read `IPV6_2292PKTOPTIONS` into room for
  the packet information alone, and Linux stops at the first record that does not fit, silently. The room is now
  the packet information, three integer records and four extension headers of the largest size (RFC 8200's
  hop-by-hop, destination, routing, destination; about 8.3 KiB on the stack); a room filled to its end is counted
  (`RECORDS_FILL_THE_ROOM`), as is a guest buffer shorter than the headers (`HEADERS_CUT`). The headers are now
  walked from the control messages directly, in order, not through the shared parser's four slots. IPv4: Linux's
  `IP_PKTOPTIONS` gives TCP no options, so an IPv4 reading's header bytes are none, counted
  (`NO_RECORDED_IPV4_OPTIONS`); plan and For the owner corrected (they claimed IPv4 options). The check now sends
  an 8-byte hop-by-hop header from the IPv6 client and demands it back, with the next header filled in (`[6, 0, 0,
  0, 0, 0, 0, 0]`, as the checker's Python reference read), and empty header bytes over IPv4.
- [minor] Readings mapped but not checked, each now checked: the client's round trip time equals its smallest and
  its variance is half of it (one measurement, the SYN's: RFC 6298's first estimate); Tuned's path MTU equals
  Information's; the receive buffer is set four times the send buffer and read at least that; BBR's bandwidth is at
  most every segment sent delivered within the smallest round trip (Linux takes no delivery rate measured over less);
  the two ends' multipath tokens differ (Linux keeps tokens unique per host); the first subflow's information reads
  the bytes it received and acknowledged and a segment size; the repair check gives the client a small receive
  buffer (8 KiB) so the windows differ, and demands each end's send window be the other's receive window, the
  client's the smaller (a small window on the server's side bounds the repaired client's segment size to half of it:
  measured 524, so the small buffer is the client's).
- [minor] The incoming processor check sets a listener to processor 3 as well as 0 and None, and checks a
  connection's processor is below the machine's count.
- [minor] `MPTCP_FULL_INFO` is in Linux 6.5, not 6.7 (status and plan corrected); the newest `mptcp_info` fields are
  later still, zero and counted on an older kernel.
- [minor] MultipathSubflow allocated two vectors and made two calls each time: now one call with arrays on the
  stack for nine subflows (the first and the eight more the in-kernel path manager allows); only a subflow past
  those reads again through vectors, counted (`SUBFLOWS_PAST_THE_ROOM`).
- Broken copies: the checker's seven that passed (tuned-path-mtu-fixed, tuned-receive-buffer-is-send,
  information-rtt-is-variance, bbr-bandwidth-halves-swapped, multipath-token-zero, subflow-information-zeroed,
  repair-window-read-swapped), its last-header-bytes-dropped and incoming-processor-always-first, and
  last-header-room-of-the-packet-information (the old room) are in mutants.sh, 162 copies, every text found once.
  Each of these ten, the reworded last-header-ipv4-unread and the two subflow and processor copies beside them
  failed the suite (ARM machine 04:28 to 04:32, load 7 to 21; log mutants-run14.log in scratch).
- Runs (ARM machine, Graviton4, Linux 7.0.0-1018-aws, 04:26 to 04:34, load 7 to 24): clippy with warnings denied
  clean on the TCP host, its suite and the shared crate. Clean build (job host-tcp-sockets-clean, build folder
  deleted first; log clean-check-run14.log): 124 + 4 pass twice; `cargo tree` (normal, build, dev) of tcp-sockets,
  internet-protocol, configurable, negative-route-advice, readiness and the suite names only interface and
  conformance crates. The seven changed checks run alone 30 times and the whole suite 10 times at 12 threads: no
  failure (log repeat-run14.log). `git log debb8c65..HEAD` and `git status` of the .rs files beside the
  tcp-sockets, internet-protocol, configurable, negative-route-advice, readiness, kernel and abi interfaces: none.
- Linter (Mac, once, after the changes; log reviewable-run14.log): in this track's files only what run 13 left (7
  `conformance_not_run`, 7 `public_outside_interface`, 5 `implementation_folder`, 2 `grammar_unclear`, the
  orchestrator's name and summary answers); nothing in the new code.

## Run 13 (2026-11-22): T2, state, authentication, repair and multipath
- read_state and read_listener_state answer every query (new file linux/state.rs; what each maps to is in the
  plan): Information, CongestionControl, Queues, Tuned, TakeSavedSynchronize, Timestamp, Repair, Authentication,
  AuthenticationKey, Multipath, MultipathSubflow, LastReceivedHeader, IncomingProcessor; the listener's seven
  through the same code. set_authentication_counters and the listener's are `TCP_AO_INFO` with its counter flag.
  The settings `RepairOptions`, `RepairWindow` and `AuthenticationRepair` are honoured. The four counted reasons
  "not mapped to the host yet" are gone; new counted reasons: an older kernel's shorter `TCP_INFO` or `MPTCP_INFO`,
  a state or congestion information form the contract has no name for, and the last header's missing length.
- Measured first on the ARM machine as root in a namespace (scratch tools/probe_t2.py, Linux 7.0.0-1018-aws, load
  2 to 4): repair reads out of repair mode EPERM (window) and EINVAL (sequence); `TCP_AO_INFO` without
  authentication ENOENT; `MPTCP_INFO` 96 bytes on a multipath socket and its listener, EOPNOTSUPP on TCP and on a
  fallen-back socket; `SO_INCOMING_CPU` -1 on a fresh listener, set and read back as given; `IP_MTU` ENOTCONN on a
  listener; `TCP_CC_INFO` 0 bytes for cubic, 16 for vegas and dctcp, 20 for bbr; `TCP_TIMESTAMP` EOPNOTSUPP on
  MPTCP; a connection read in repair mode, closed, and written into a new socket (sequence numbers, options,
  window, clock) carries bytes both ways with the old peer.
- Kernel and configuration each part needs: `CONFIG_TCP_AO` (6.7+), `CONFIG_MPTCP` with `MPTCP_FULL_INFO` (6.5+; run 14 corrected this from 6.7),
  6.18+ for the Accurate ECN fields (zero and counted before), CAP_NET_ADMIN for repair mode, the `tcp_dctcp`,
  `tcp_vegas` and `tcp_bbr` modules (run-conformance.sh now loads them with `modprobe`). The machine has all.
- New checks (11): information_reads_what_the_connection_did, congestion_control_state_names_the_algorithm,
  queues_count_unread_unacknowledged_and_unsent, tuned_values_follow_the_settings,
  timestamp_reads_the_clock_in_its_unit, repaired_connection_continues_where_the_original_left_off,
  authentication_state_reads_the_keys_and_counters, authentication_repair_values_are_read_and_written (the initial
  sequence numbers against the captured SYN and SYN-ACK), multipath_state_describes_the_connection,
  last_received_header_records_where_the_last_packet_came, incoming_processor_is_read_as_received_or_set.
- The independent check's findings of run 12, answered: the suite tally (below, now 108 checks and 7 builders);
  the reviewed-files claim (run 12's bullet now names abi apart); the set-twice checks now compare what the
  readings show (tuned values, authentication state without the verified count, last received header) after
  connect and after the settings are set, besides the outcomes; authentication_counters_are_set_only_where_
  authentication_is and listener_state_reads_a_listener demand the real answers (a listener's counters set and
  read back, no NotSupported way out); the listener clock check takes the listener into repair mode first and
  demands the clock set if that is taken, refused if not (Linux refuses both, EPERM). The linter's structural
  findings stay open under For the owner.
- Suite now: 108 checks that each take one value plus 7 functions that build values (configured_connection,
  configured_listener, advised_connection and the four waiting_* bundles); `tests!` runs 124 tests, the
  implementation's own 4 beside them.
- Runs (ARM machine, Graviton4, Linux 7.0.0-1018-aws, 03:50 to 04:05, load 0.5 to 5.7): 124 + 4 pass, seven full
  runs at `RUST_TEST_THREADS=12`, each 7.3 s. Clean build (job host-tcp-sockets, build folder deleted first, log
  clean-check-run13.log in scratch): clippy with warnings denied clean on the TCP host and suite, the shared crate
  and the UDP, ICMP and raw hosts (9.5 s); 124 + 4 pass twice; `cargo tree` (normal, build, dev) of tcp-sockets,
  internet-protocol, configurable, negative-route-advice, readiness and the suite names only interface and
  conformance crates (abi, configurable, executor, internet-protocol, kernel, negative-route-advice, readiness,
  tcp-sockets and their suites). `git log 5ec01130..HEAD` of the .rs files beside the tcp-sockets,
  internet-protocol, configurable, negative-route-advice, readiness and kernel interfaces: none; abi's changed only
  in other tracks' commits (listed under run 12).
- Broken copies: 22 new ones in mutants.sh (one or more per new query, method and setting: the state number, the
  bytes acknowledged, congestion information and name, the queues, the urgent mark, the clamp, the clock's unit,
  the repair sequences, options, window and authentication values and their byte order, both counter methods, the
  authentication requirement and key, the multipath bytes, the subflow's addresses, the last header, the listener's
  state, the incoming processor); the three old ones for the refusals that are gone were replaced. Each of the 22
  failed the suite (ARM machine 03:57 to 04:02, load 2.2 to 2.5; log mutants-t2.log in scratch). 152 copies; every
  text found once (tools/check_mutants.sh, which now knows linux/state.rs).
- Linter (Mac, once in the middle and once at the end; logs reviewable-t2.log and reviewable-t2-end.log in scratch):
  the new code's findings were fixed (summaries that were only a link, an alignment and a reserved field written
  with numbers, a constant without a link, `NO_PROCESSOR` defined twice, now exported by the shared crate, and
  "bbr" defined in both crates, which the implementation no longer names: BBR's information is the only form of its
  length); the two zero-length alignments it then reported were fixed after it and not run again. Left in this
  track's files: the structural ones under For the owner (7 `conformance_not_run`, 7 `public_outside_interface`, 5
  `implementation_folder`), 2 `grammar_unclear` (model confidence) and the orchestrator's name and summary answers.
  `conformance_untested_method` reports nothing on the TCP interface.

## Run 12 (2026-11-22): the independent check of run 11 answered
- record_end_starts_a_new_segment was flaky (the checker: 123 of 150 lone runs failed): it read the capture as soon
  as the data was read, before the capture thread had recorded the segments. It now waits (up to DEADLINE) until the
  captured data segments hold every byte sent, then asserts the same two lengths. ARM machine, clean build, load 3 to
  9: 0 of 150 lone runs failed, 0 of 20 full runs at 12 threads.
- listener_backlog_bounds_the_accept_queue now gives the backlog of 0 three ways: to `listen` (the argument, no
  setting), as a listen setting, and set later. New broken copy listen-backlog-argument-ignored fails it (and the
  two ICMP checks of a synchronizing connection).
- timestamp_clock_set_in_repair_mode_stamps_segments takes any refusal outside repair mode (the contract names no
  error), for the connection and the listener. New broken copy clock-set-outside-repair fails it.
- Accept gives the SYN's frame length: every TCP listener now saves its SYNs from the link header (`TCP_SAVE_SYN`
  2), so `link_layer_length` is the link header found before the IP header plus the IP length (counted
  `NO_SYNCHRONIZE_FRAME` when no IP header is found). `largest_fragment` stays None, now under its own counted reason
  `NO_SYNCHRONIZE_FRAGMENT_SIZE` (For the owner). connect_and_accept_name_both_ends checks that a frame length, when
  given, holds the whole packet; new broken copy synchronize-frame-is-the-link-header fails it.
- The T1 section below is rewritten to match the suite as it is (113 tests, the readiness checks in the method
  map; it said 99 checks and five builders, which was wrong: 97 checks and 7 builders, as the next check counted).
- Linter at the start (Mac, load 9 to 21; log reviewable-run12-start.log in scratch): in this track's crates only
  what run 11 left (7 `conformance_not_run`, 7 `public_outside_interface`, 5 `implementation_folder`, 2
  `grammar_unclear`) and the orchestrator's `name_unanswered` and `summary_unanswered` (no key on this Mac).
- Linter at the end (Mac; log reviewable-run12-end.log): the same, plus three `magic_literal` for the backlog check's
  case names written as strings, fixed after it by an enum (`BacklogGiven`); not run again.
- Checks (ARM machine, Graviton4, Linux 7.0.0-1018-aws, 02:43 to 02:55, load 0.1 to 9.4): clippy on both crates, no
  warning in this track's files (two in crates/kernel/interface.rs, not this track's), also on the Mac; 113 + 4
  pass. Clean build (job host-tcp-sockets-clean, build folder emptied): 113 + 4 pass; `cargo tree` (normal, build,
  dev) of tcp-sockets, internet-protocol, configurable, negative-route-advice, readiness and the suite names only
  interface and conformance crates (abi, configurable, executor, internet-protocol, kernel, negative-route-advice,
  readiness, tcp-sockets and their suites). No reviewed file of tcp-sockets, internet-protocol, configurable,
  negative-route-advice, readiness or kernel changed since 5ec01130; abi's did, in other tracks' commits (357bd107,
  56e334a8, 73b9e266, 9854b9b5, ca411e59).
- Broken copies (mutants.sh, ARM machine, 02:53 to 03:20, load 1 to 4; log mutants-run12.log in scratch): all 133
  texts found once, and every copy fails the suite.

## Run 11 (2026-11-22): the linter's literal findings
- Linter at the start (Mac, 2 min 37 s, load 133; log reviewable-run11-start.log in scratch): in this track's two
  crates 113 + 48 `magic_literal` and 86 + 11 `number_constant_without_link`, as run 10 left them.
- Done: every free 0, 1, 2 and -1 in linux.rs and conformance.rs that was a value is the type's `MIN`,
  `u32::from(true)` for a C bit field, or a named constant (`UNSET`, `FIN_WAIT2_RESET`, `NO_PEEK_OFFSET`,
  `NO_REPORT_FLAGS`, `ANY_PORT`, `NO_BYTES`, `RETRY_INTERVAL`, `ONE_WAKE` and the like); `windows(2)` and
  `step_by(2)` became a zip and an even-index filter, and the no-delay check counts its sends instead of writing 1
  and 2.
- Done: every named number has an https link: values Linux or a standard fixes link to the man page, kernel
  document, kernel source or RFC; the suite's own choices link to the new plan section "The suite's own choices"
  (https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices),
  which lists each value with its reason (DECISIONS.md 11 of the linter). The plan file reaches main only when the
  orchestrator merges agents.
- Done in the shared crate linux-socket-options (host.rs, linux.rs, icmp_error.rs, readiness.rs, implementation.rs,
  tests/readiness.rs, the files this track made): its literal findings and summary findings, the same way; its own
  choices (`REASON_SLOTS`, `CONTROL_ROOM`) link to the plan section "The shared crate's own choices".
  `receive_message` gives `recvmsg` an array of one vector and its length in place of the number 1.
- mutants.sh: nine texts it breaks had changed; all 130 are found once again, and the nine copies plus
  peek-offset-ignored each fail named checks (ARM machine, load under 1).
- Checks after the changes (ARM machine, load 0.4 to 8): clippy with warnings denied on the shared crate, the TCP,
  UDP, ICMP and raw implementations and the TCP suite: clean; also on the Mac. TCP 113 + 4, UDP 76 + 5, the ICMP and
  raw binaries 58 and 47, the shared crate's own test 1: all pass.
- Linter at the end (Mac, load 22; log reviewable-run11-end.log): this track's crates have no `magic_literal` and no
  `number_constant_without_link` left; after it the new `summary_without_full_stop` findings (a summary ending in its
  link) were fixed by moving each link to a paragraph of its own, and one `magic_literal` (a comparison with zero of
  a closure argument named `status`, which the rule reads as a status) by renaming it. Left in this track's crates:
  the structural ones run 10 recorded (7 `conformance_not_run`, 7 `public_outside_interface`, 5
  `implementation_folder`, all under For the owner) and 2 `grammar_unclear` (the model's low confidence).
- Left in the shared crate: its datagram half (datagram.rs and datagram/*, the UDP track's files: about 60 summary
  findings and 7 constants without a link), and 20 `public_outside_interface` plus a `conformance_not_run` on its
  Cargo.toml, both because a helper crate that implements no interface has no place in the layout (For the owner).
  `constant_defined_twice` in crates/udp-sockets/implementation/host/portable.rs (three reason texts the same as
  this track's portable.rs) is reported on the UDP copy.

## Run 10 (2026-11-22): the owner's linter
- Linter run once at the start (Mac, 2 min 44 s, 96 073 findings in the workspace; log reviewable-start.log in
  scratch): about 430 in this track's two crates, 150 in the shared crate linux-socket-options, 3 in tcp-sockets'
  interface.rs (not ours).
- Done: the implementation builds on every unix system, since the linter reads the Mac's build and found no
  `TcpSockets` there (`trait_not_implemented`). implementation.rs holds the three exported types, their Linux-only
  fields under `cfg`; linux.rs is the old Linux code unchanged but for imports; portable.rs (new, macOS and other
  unix) does connect, listen, accept, plain send and receive, urgent data, peeks at offset 0, shutdown, close and
  abort, buffers, keep-alive on or off, no-delay, urgent inline, address and port sharing and the waits on socket2,
  rustix and the shared watcher, and answers every other call and setting with a counted refusal (named reasons in
  portable.rs). It is built and linted on the Mac, not run: no check runs there (the suite needs Linux).
- Done: the implementation runs the suite from its own `conformance.rs` (a `[[test]]`, one `tests!` invocation, as
  the layout asks); its own four tests stay in linux.rs. run-conformance.sh runs every test binary of the package,
  each in a namespace of its own. mutants.sh and scratch tools/check_mutants.sh now break linux.rs (all 130 texts
  found once).
- Done: the suite builds on every unix system (the packet socket is a `cfg`'d helper that fails the check off
  Linux; BPF and IPv4 option codes from linux-raw-sys in place of libc). `tests!` is one arm: a `#[test]` per suite
  function in file order, then the required suites as the layout writes them: `configurable_tcp_connections`,
  `configurable_tcp_listeners` and `negative_route_advice` run the whole suites of those interfaces on
  `configured_connection`, `configured_listener` and `advised_connection` (the three `*_contract_holds` functions are
  gone). The readiness suite runs as before under `readiness_tcp_connections`, `readiness_tcp_connections_hanging_up`
  and `readiness_tcp_listeners`.
- Done: every doc summary in this track's files is one line, one sentence, at most 150 characters, and a function's
  starts with a verb in the third person (suite functions "Checks that ..."); what a summary lost went into a second
  paragraph.
- Not done, still findings: `magic_literal` (about 160: mostly 0 and 1 given to a name, which want `u16::MIN` and
  the like or a named constant) and `number_constant_without_link` (about 100: every numeric constant needs an https
  link, also the suite's own choices, DECISIONS.md 11). The constructor `new()` and `counters()` are an impl block of
  the exported type (`public_outside_interface`) until the interface has a `StandaloneTcpSockets` (For the owner).
  The four readiness bundle functions (`waiting_for_*`) are reported as suite functions that return a value, and the
  readiness modules are not the layout's one line: the readiness suite takes a `Waiting` bundle with a closure that
  makes the handle ready, which a `(T, T::Connection)` tuple cannot carry (For the owner). Findings in the shared
  crate linux-socket-options (made by this track, edited by three) are not touched yet.
- Linter run once before finishing (01:53, log reviewable-end.log in scratch), this track's two crates: 113 + 48
  `magic_literal`, 86 + 11 `number_constant_without_link`, 7 `conformance_not_run` (the readiness bundles and modules,
  above), 2 `grammar_unclear` (`end`, `unread`: the model's confidence was low; nothing to change in the names), the
  constructor and counters (above), and two the layout and the brief disagree on, both under For the owner: the
  fields of `HostTcpConnection` and `HostTcpListener` must be `pub` (a type that fills an associated type is plain
  data), and the impls in linux.rs of `Configurable`, `NegativeRouteAdvice` and `Readiness` are reported as
  `implementation_folder` (the linter reads the Mac's build, where linux.rs is only tokens; portable.rs's same impls
  pass). After that run, fixed: the crate summary on one line, the abort reason defined once (implementation.rs),
  portable.rs's own impl block made free functions; 113 + 4 pass again on the ARM machine (01:54, load 1.7).
  `conformance_untested_method` reports nothing on the TCP interface.
- Next: the literals (each 0 or 1 given to a name as the type's `MIN` or a named constant; each numeric constant
  derived from a named one or linked to a page that writes the choice down), then the shared crate's findings.
- Independent check: see Measurements, run 10.

## 2026-11-21: waiting is the Readiness trait
- `wait_for_events`, `wait_for_listener_events` and `PollEvents` are gone. `HostTcpSockets` implements
  `wcontainers_readiness::Readiness<HostTcpConnection>` and `Readiness<HostTcpListener>` (`wait_until_ready` on the six
  facts of `ReadyFor`), through the shared `Watcher`, each socket watched with `ReadyFor::all()`. A wait gives every
  fact that holds, not only those asked for.
- Suite (module names as of run 10 in brackets): it re-exports the readiness suite and runs it as `connection_readiness_contract` (`readiness_tcp_connections`)
  (`waiting_for_data`, and `waiting_for_urgent_data_until_a_report` for the failure check: a queued transmit report,
  an error with no hang-up), `connection_hang_up_readiness_contract` (`readiness_tcp_connections_hanging_up`; the failure check alone on
  `waiting_for_urgent_data_until_both_ways_are_shut`, a hang-up with no error) and
  `listener_readiness_contract` (`readiness_tcp_listeners`; `waiting_for_a_connection`). The listener runs five of the six checks, not the failure
  check: no call of the TCP traits makes a listener get an error or a hang-up.
- Deleted as copies of the readiness suite: wait_for_events_finishes_when_data_arrives,
  wait_for_events_answers_at_once_when_ready, dropping_a_wait_cancels_it, wait_ends_with_the_errors_always_reported,
  listener_wait_finishes_when_a_connection_comes, dropping_a_listener_wait_cancels_it. Deleted as of the twelve bits:
  the normal-writable interest and the "no data_to_read given" assertions. many_waits_each_wake_once is now
  waits_on_many_connections_each_wake_once (one wait on each of 16 connections; two on one is the readiness suite's).
- A real check for each fact: READABLE (the readiness suite, and a listener with a connection to accept), WRITABLE
  (wait_for_room_finishes_when_the_peer_reads), URGENT_DATA, PEER_FINISHED_SENDING (with no hang-up), HANG_UP (new:
  wait_for_a_hang_up_finishes_when_both_ways_are_shut, both ends shut down writing, no error) and ERROR (new:
  wait_for_an_error_finishes_when_the_peer_resets). mutants.sh names the new texts of the shared crate.
- Repair after the independent check: new a_wait_gives_the_facts_the_connection_has_and_no_others (exact facts when
  fresh, with data, urgent data behind data, the peer finished, both ways shut, a transmit report queued) and
  a_wait_gives_the_facts_the_listener_has_and_no_others. Run on the ARM machine (job readiness-repair): 116 passed,
  three runs; the checker's seven TCP mutants and a new one (ALWAYS_REPORTED of ERROR alone) each fail the suite.
  Clippy: no warning in this track's files (two in crates/kernel/interface.rs, not this track's). Literal check: nothing.
- For the owner: the trait's comment does not say that ERROR and HANG_UP end a wait whether or not they were asked for,
  which the implementation does and the readiness suite checks.

## For the UDP, ICMP and raw tracks
- New, run 15 (61bb2b6f): the shared readiness answers `ReadinessReport`; `Watched::hold` takes a
  `ReadinessReport` (the ICMP and raw hosts already call it so); `readiness::NOTHING` is a report of no fact;
  `DATAGRAM_FACTS` is READABLE | WRITABLE (errors and hang-ups are always reported, no longer facts to watch). The
  readiness suite's `ALWAYS_REPORTED` is gone and its checks read `ready_for` and `unconditional`. The shared
  `set_port_setting` is now `set_binding_setting` (UDP track: your call at udp-sockets host linux.rs:784).
- New, run 6: `receive_message` (TCP's only) no longer allocates: `ReceivedMessage` has `length`, `flags` and
  `control` (the bytes), and `messages()` walks them in place as level, type and data (`ControlMessages`, which
  takes any control bytes the kernel wrote). UDP track: `Ancillary::parse` could take its messages from
  `ControlMessages::new` so the crate has one CMSG walk. TCP now sends its transmit report requests through
  socket2's `sendmsg` with the UDP track's `datagram::send::Control`.
- TCP uses the shared `readiness` for both waits; since 12b98f7f (UDP track) it arms an edge-triggered registration
  only while a call waits, and `open_socket` makes TCP's sockets with socket2 (9ad7b201).
- The shared crate is `crates/internet-protocol/implementation/linux-socket-options` (package
  `wcontainers-internet-protocol-linux-socket-options`), made by this track. Its body is
  `cfg(target_os = "linux")`. It holds: `host_error`, `last_host_error`; the raw option calls `set_option`,
  `set_int`, `set_flag`, `set_bytes`, `get_option`, `get_int`, `get_bytes`; `Shortfalls` (counted refusals,
  warned once, readable as `CounterReading`s); `set_filter`; `receive_timestamping_flags`, `set_timestamping`;
  `set_ip_socket_setting`, `set_port_setting`, `set_packet_setting`; `ControlBuffer`, `receive_message`,
  `timestamps`, `take_error_queue_message`, `transmit_event` and the `TIMESTAMP_STAGE_*` and
  `TIMESTAMPING_TIMES` constants; and (since 2026-11-21 evening) `family_of`, `socket_address`, `open_socket`
  (non-blocking, owner by the thread's filesystem user, EPERM when the host may not act as that user),
  `allowed_ranges`, `set_port_range`, `in_port_ranges`, `bind` (with `choose_now` for listeners and datagram
  sockets) and `peer_for` (an IPv4 peer of an IPv6 socket, mapped). `receive_message` now also gives the raw
  control bytes (`control_bytes`), for `datagram::receive::Ancillary::parse`; `take_error_queue_message`,
  `ErrorQueueMessage` and `transmit_event` are gone (TCP reads its error queue through `Ancillary` now).
- New, 2026-11-21 evening: `icmp_error::write_icmp_error`, the ICMP error of an error queue entry written whole
  from its IP header, generalised from the UDP track's private `icmp_error`: it takes `quoted`, the part of the
  quoted transport header Linux took off (the 8-byte UDP header for UDP, nothing for TCP, whose payload starts
  at the TCP header), and builds the ICMP header and checksum with etherparse. UDP track: please switch to it
  and drop your copy (`icmp_error` and `rest_of_header` in crates/udp-sockets/implementation/host).
- UDP track: `datagram/receive.rs` does not build for `aarch64-unknown-linux-musl` (the Mac's Linux lint
  target): `msg_controllen` and `cmsg_len` are `u32` there, `usize` on gnu. This track now lints on the ARM
  machine (gnu).
- UDP track: your `datagram/receive.rs` defines its own `TIMESTAMPING_TIMES`; the shared one is now `pub`, so
  import it (the literal check reports both as a redefinition until one goes). Your `pub mod datagram;` line in
  `implementation.rs` was left out of this track's commits, since its files were not committed yet: commit it
  with them.
- New, run 8: `Shortfalls` counts without a lock (a fixed array of 64 slots, each a reason taken once and an atomic
  count; reasons past 64 are counted under one named overflow reason); `readings()` is still sorted by reason.
  TCP now takes multicast groups through `datagram::memberships::set_multicast_group` and anycast through
  `AnycastMemberships` (Linux takes `MCAST_JOIN_GROUP` and `MCAST_MSFILTER` on a TCP socket, measured; only the
  old `IP_ADD_MEMBERSHIP` is EPROTO there). `set_ip_socket_setting` and `set_packet_setting` still refuse
  memberships and a fixed flow label for whoever calls them without handling those first.

## Done in run 9 (2026-11-22): the independent check of run 8 answered
- Memberships by their effect: multicast_groups_are_joined_and_left reads `/proc/net/igmp` and `/proc/net/igmp6`
  (groups 224.0.0.123 and ff02::123, which no other check joins): none before, one user after a join and after a
  second, none after a leave, a second leave and the close. New anycast_addresses_are_joined_and_left does the same
  with `/proc/net/anycast6` (fd00::123): the namespace now has `net.ipv6.conf.lo.forwarding=1`, since Linux lets a
  host (not a router) join only anycast addresses its interface's prefixes hold.
- The owner: new connect_and_listen_make_sockets_of_their_owner connects as uid 1001 and listens as uid 1002 over
  IPv4 and IPv6 and finds each in `/proc/net/tcp` or `tcp6` (by its ports) with that uid.
- The TCP Authentication Option's settings are honoured (they need no privilege, as the checker measured):
  `AddAuthenticationKey` is `TCP_AO_ADD_KEY` (peer prefix, an IPv4 one of an IPv6 socket mapped, interface, both
  identifiers, algorithm, MAC length, options excluded or not, the key's use as current or requested next); Linux
  answers EEXIST for a key whose identifiers a held key has, so when `TCP_AO_GET_KEYS` shows the held key is the one
  asked for, that is a success (its use applied through `TCP_AO_INFO`). `RemoveAuthenticationKey` is
  `TCP_AO_DEL_KEY` (with the keys to use next and removal when unused); an absent key's removal is a success, the
  keys to use next still applied. `AuthenticationOptions` and `InitialAuthenticationCounters` are `TCP_AO_INFO`,
  read first, since Linux writes the requirement and the ICMP choice at every call. `AuthenticationRepair` and
  `set_authentication_counters` stay T2 (counted refusals).
  New authentication_keys_admit_only_connections_signed_with_them (11 cases on the wire, option kind 29: its key
  identifiers and its length, 4 + the MAC length 16 the check gives, unlike Linux's default 12): matching keys
  admit, every segment both ways signed; a key added twice; the key asked to be current signs the SYN; a key
  removed with another made current; another secret, no key, and options covered on one end only are refused; a
  listener's key removed twice admits plain clients; a listener requiring authentication refuses plain clients
  until it no longer requires it. New authentication_options_choose_the_keys_in_use: options set twice on an
  established connection switch the key its data is signed with. The listener settings list gains the four
  settings (for a peer other than the clients), all required to succeed; the refusable list a key and counters on
  established connections (Linux: EISCONN both times).
- A listener's `UnreadByteReports` reaches the connections it accepts (accept copies the flag, as Linux copies
  `TCP_INQ`); checked by new listener_unread_byte_reports_reach_the_connections_it_accepts.
- keep_alive_probes_end_an_unanswered_connection runs with 1 and with 2 probes: as many probes as asked, an
  interval apart, then the reset an interval after the last.
- accept_names_where_the_synchronize_arrived also accepts multipath connections (their arrival interface comes from
  the host's addresses), to 127.0.0.2, which loopback holds only by its prefix: the exact-address-only copy now
  fails. Linux gives an accepted multipath socket mark 0 (measured), so the mark is checked for TCP only.
- Shared `Shortfalls`: a reason counted in the overflow slot is warned of at every count, since that slot cannot
  tell its reasons apart.
- Note: commit d60b9609 ("refactored everything to fit the new reviewable rust format", not this track's) took
  part of this run's uncommitted edits of implementation.rs, conformance.rs and the shared host.rs with it; the
  rest is in 198547b0.

## Done in run 8 (2026-11-21): the independent check of run 7 answered
- The two flaky checks. listener_backlog_bounds_the_accept_queue: two of 60 full runs still failed after a first
  change (accepting at once), both with two connections accepted: a SYN that came while the queue was momentarily
  empty, or two handshakes finishing side by side, got in. The check now connects one client, waits (through the
  listener's wait) until it is queued, then connects the others (a full queue drops their SYNs), settles, accepts
  one, and watches for 30 ms with a last try after the period however late the thread ran.
  pacing_rate_spreads_what_is_sent: 1 MiB at 1 MiB/s with a bar of 400 ms (measured, ARM machine, load 2: paced
  704 ms, lifted 0.65 ms; the old 512 KiB took 0.2 s against a bar of 100 ms).
- A fixed IPv6 flow label is honoured at connect: the socket takes the label from the flow label manager
  (`IPV6_FLOWLABEL_MGR`, shared with this process's sockets), `IPV6_FLOWINFO_SEND` on, and connects to an address
  whose `sin6_flowinfo` carries it (std keeps that field as given, so it is put in network order). Measured on the
  ARM machine: the manager needs CAP_NET_ADMIN (uid 1000 in a root network namespace: EPERM; root: taken), so
  Linux's EPERM is what an unprivileged host answers. Linux keeps a connection's label for good, so the same label
  again is a success and another, or automatic or none after it, or any fixed label on a listener or accepted
  connection, is the counted refusal `FLOW_LABEL_FIXED_AT_CONNECT`. Checked by fixed_flow_label_marks_every_segment
  (every segment the client sends, the SYN first, carries it; the same label again changes nothing; another is
  refused with the label kept, or carried).
- Multicast and anycast memberships go to Linux (see above); multicast_groups_are_joined_and_left joins and leaves
  an IPv4 and an IPv6 group on the loopback interface twice each, all taken, and the connection still carries bytes.
- MD5 signature keys (`Md5Key`) are honoured through `TCP_MD5SIG_EXT` (prefix and interface; an IPv4 prefix of an
  IPv6 socket mapped; removing an absent key is a success, Linux's ENOENT otherwise). Checked by
  md5_keys_admit_only_connections_signed_with_them (a keyed listener takes clients signed with its key, a key set
  again replacing the last, and every segment both ways carries kind 19; it answers no SYN with another key, no key,
  or a key removed; removed twice, the listener takes plain clients). The TCP Authentication Option stays in T2:
  see Departures.
- The window probe's check now chooses the receive repair queue first and, after the probe, sends (which in repair
  mode fills the queue chosen) and peeks the bytes back: refused if the queue was lost.
- no_delay_sends_small_segments_while_others_are_unacknowledged has a fourth case: a push past cork and Nagle first,
  then Nagle must still hold the second small segment.
- `Shortfalls` counts without a lock (see above), so accepts on many threads no longer meet at one mutex.
- `NotSentLowWater(None)` is written as `u32::MAX` (no bar); 0 would mean the host's sysctl, which is `u32::MAX` in
  the suite's namespace too, so no check tells them apart: under For the owner.
- `RecordsLastReceivedHeader` kept by accepted connections stays unobservable before T2's read_state.

## Done in run 7 (2026-11-21): the independent check of run 6 answered
- `RecordsLastReceivedHeader` is honoured: Requested turns on `IP_PKTINFO`, `IP_RECVTTL`, `IP_RECVTOS` (whose records
  `IP_PKTOPTIONS` gives a TCP socket) and, on IPv6 sockets, `IPV6_RECVPKTINFO`, `IPV6_RECVHOPLIMIT`,
  `IPV6_RECVTCLASS`, `IPV6_RECVHOPOPTS`, `IPV6_RECVDSTOPTS`, `IPV6_RECVRTHDR` (whose records `IPV6_2292PKTOPTIONS`
  gives from the last packet that came in order); NotRequested turns them off. Measured as uid 1000 on the ARM
  machine: each is taken without privilege. A listener with it keeps it for the connections it accepts (accept
  no longer turns off their packet information then). The suite takes both values at connect and on established
  IPv4 and IPv6 connections, and on IPv4 and IPv6 listeners (listener_settings_set_twice_equal_set_once demands
  success). Its effect shows only through `TcpQuery::LastReceivedHeader`, which is read_state (T2).
- `SaveSynchronize(FromLinkHeader)` is honoured: the listener's `TCP_SAVE_SYN` is 2 (measured as uid 1000: taken
  without privilege; on loopback the saved headers start with a 14-byte Ethernet header). Accept finds where the
  IP header starts (the first place an IP header from the peer to the local address holds a TCP header between
  their ports that ends the headers, by etherparse's lax slices; counted when none is found) and gives the IP
  header from there whichever way the SYN was saved; `TakeSavedSynchronize` then gives the link header too.
  saved_synchronize_is_taken_once covers link, link then network, link then off; it also checks that accept's
  header is the SYN's IPv4 header from 127.0.0.1 to 127.0.0.1.
- Reading a saved SYN again into 64 KiB when it is longer than the first room is a counted slow path
  (`LONG_SAVED_SYNCHRONIZE`); the first room now holds an Ethernet header too.
- Peeks at an offset no longer disturb other receives: every receive holds the connection's `receiving` lock, since
  `SO_PEEK_OFF` is the socket's for the call (an uncontended lock beside a system call; Linux takes the socket's
  own lock for each receive anyway). Checked by the implementation's own peeks_from_two_threads_each_start_where_asked
  (two threads, 20 000 peeks each, one at offset 5 and one at 0, every one exact); it fails without the lock.
- New wait checks: wait_for_urgent_data_finishes_when_it_arrives (exceptional_condition, `POLLPRI`) and
  wait_for_the_peers_shutdown_finishes_when_it_comes (peer_shut_down_writing, `POLLRDHUP`, with no hang-up); each
  fails when the shared readiness does not register its bit (the checker's broken copy and one for `RDHUP`).
- An IPv4 SYN's arrival interface is now the one Linux keeps for the connection, read from `IP_PKTOPTIONS` (the
  listener has `IP_PKTINFO` on; measured: an accepted socket reports interface 1 for 127.0.0.1, and a dual-stack
  listener's IPv4-mapped connection too): Linux records there the interface of the segment that completed the
  handshake (`tcp_v4_syn_recv_sock`: `mc_index = inet_iif(skb)`), the SYN's own only for fast open, and that is
  counted (`ARRIVAL_INTERFACE_OF_THE_HANDSHAKE`). The host's addresses (`getifaddrs`, cached until a netlink notice
  of an address change) now serve only multipath connections, which have no packet options. No check on one host
  tells the two apart: a SYN to an address a dummy interface holds is reported on that interface by both
  (measured, interface 2).
- Broken copies: 106 (mutants.sh); the new ones and the three mended ones are under Measurements.

## Done in run 6 (2026-11-21): the independent check of run 5 answered
- Multipath is checked on the wire (multipath_is_negotiated_when_both_ends_ask, which replaces
  multipath_connections_carry_bytes): the capture marks segments with the MPTCP option (kind 30, which etherparse
  reports as an unknown option); a multipath client and listener both offer it in the SYN and SYN-ACK, a plain
  client to a multipath listener neither, a multipath client to a plain listener only in the SYN; bytes cross each.
- shutdown_of_both_directions_ends_both now also reads the client's own end of stream (0 at once, as Linux gives).
- The reporting send goes through socket2's `sendmsg` (`MsgHdr::with_buffers`, `with_control`), its control message
  written by the shared `datagram::send::Control`. The receive with control messages stays libc's `recvmsg` in the
  shared `receive_message`: socket2 0.6.5's `RecvFlags` offers only `is_truncated`, `is_end_of_record`,
  `is_out_of_band`, `is_confirm` and `is_dontroute` (src/lib.rs:387, src/sys/unix.rs:623-659), no `MSG_CTRUNC`, and
  both the error queue and now the timestamped receive count a cut (`CUT_CONTROL`). The control messages are walked
  in place (no `Vec`); checked by a_receive_with_control_messages_allocates_nothing (100 timestamped receives with
  unread-byte reports, none allocating).
- set_timestamp_clock: Linux cannot change a connection's clock unit, which the route gives (`tcp_usec_ts`;
  measured: a connection over a route with the feature advances 50 056 in 50 ms, one without 52, and writing
  `TCP_TIMESTAMP` with either low bit changes neither). The implementation reads the unit from `TCP_INFO`'s
  `TCPI_OPT_USEC_TS` and refuses the other with a counted reason. The suite's namespace adds a local route for
  127.0.0.7 with the feature (iproute2 7.1 from the checkout's pinned nixpkgs; the machine's 6.1 lacks it):
  timestamp_clock_set_in_repair_mode_stamps_segments checks each unit on its own route (at least 50 ms or 50 000 µs
  past the reading after 50 ms, at most the time it ran), and
  timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused that the other unit counts in it or is refused.
- saved_synchronize_is_taken_once covers multipath listeners: a save setting there must either be refused (Linux:
  ENOPROTOOPT, errno 92, for on and off) or leave the SYN to be taken.
- Discarding error queue entries reads each as the take does but counts no cut packet (`next_error_queue_entry`
  gives whether one was cut; only the take counts it); checked by discarding_cuts_no_packet.
- New checks for the methods that had none: authentication_counters_are_set_only_where_authentication_is (a
  connection without authentication may not take counters: Linux says EISCONN; a listener's, if set, read back,
  as Linux does, else NotSupported) and listener_state_reads_a_listener (Information reads Listen and
  CongestionControl the one given, else NotSupported). Both T2 methods still answer the counted refusal.
- Broken copies: 99 now (mutants.sh); the new ones and their results are under Measurements.

## Done in run 5 (2026-11-21): T1 complete
- The suite finds the loopback interface by its `IFF_LOOPBACK` flag (nix's `getifaddrs`) instead of naming it
  "lo": the literal check finds nothing in this track's files now.
- Socket creation is socket2's (the shared `open_socket`, moved by the UDP track in 9ad7b201). The one rustix
  `socket_with` left in the implementation opens the netlink socket that hears of address changes, not a TCP socket.
- Independent check, see Measurements.

## Done in run 4 (2026-11-21, after the async ruling and the reorganization)
- Renamed to `host`: folder `crates/tcp-sockets/implementation/host`, package `wcontainers-tcp-sockets-host`, types
  `HostTcpSockets`, `HostTcpConnection`, `HostTcpListener` (the UDP track's lock commit f0463350 took the staged
  folder rename; the contents moved in 4f2a4794).
- `wait_for_events` and `wait_for_listener_events` on the shared `readiness` (polling 3.11.0 over epoll; polling
  rather than mio because it registers a socket of any kind on every system, raw ones on Windows included, which
  the raw and ICMP tracks need too). A connection or listener holds its socket as a `Watched`, whose drop
  deletes the registration before the descriptor closes. `HostTcpSockets::new()` now answers a `Result`: it
  starts the readiness thread.
- Suite: eight wait checks (finishes when data arrives and not before; answers at once when ready; dropping a wait
  cancels it; 16 connections with two waits each, half fed, each fed wait woken exactly once and the others
  never; room to send comes back when the peer reads; a reset ends a wait for an event that never comes, with
  the always-reported error or hang-up; a listener's wait finishes when a connection comes; dropping a listener's
  wait cancels it). Plus, in the implementation's own tests, a counting allocator: 100 waits that each have to
  wait allocate nothing on the waiting thread once the connection has waited once
  (a_wait_allocates_nothing_once_warm). And the stopped agent's check reports_name_their_sends_among_plain_ones.
- socket2 0.6.5 (`SockRef` on the socket's descriptor) for connect, fast-open `sendto`, listen, accept
  (`accept4`, which also gives the peer), plain send and receive, shutdown, the abort's linger, the local address,
  and the settings it covers whole: keep-alive on or off, no-delay, cork, urgent inline, quick acknowledgement,
  segment size, thin linear timeouts, congestion control, not-sent low water; and the reads and writes of
  no-delay, cork, quick acknowledgement and address sharing inside the action methods. Kept on rustix and libc,
  because socket2 lacks them or does not compose: the keep-alive idle, interval and probes (socket2's
  `set_tcp_keepalive` also turns keep-alive on, so setting the idle time alone would change another setting), the
  user timeout (socket2 cuts a time that is not whole milliseconds where the contract's answer is EINVAL), sends
  and receives with control messages (socket2 passes raw control bytes only; the shared crate parses them), and
  every Linux-only option. Creating the socket stays in the shared `open_socket` (see above).

## T1 as it stands (rewritten in run 12, 2026-11-22)
- `crates/tcp-sockets/implementation/host` (package `wcontainers-tcp-sockets-host`, type `HostTcpSockets`,
  connection `HostTcpConnection`, listener `HostTcpListener`, all fields private; public beyond the traits: `new()`
  and `counters()`). On Linux (linux.rs) real non-blocking TCP and MPTCP sockets: connect (settings, bindings,
  exit-node port choice, fast open with data by `MSG_FASTOPEN`, until first send by `TCP_FASTOPEN_CONNECT`), listen
  (backlogs among its settings applied after `listen`), accept (SYN header from `TCP_SAVED_SYN`, saved from the link
  header so the frame length is known), send and receive with every request, shut_down, close, stop_listening, the
  error queue (transmit reports with packet, statistics or wireless status; ICMP errors written whole), every plain
  setting of settings.rs, the action methods, and `Readiness` for connections and listeners; since run 13 (T2) every
  state query, the authentication counters, repair options and windows and authentication repair. On other unix systems
  (portable.rs) the basic calls only, everything else a counted refusal; built and linted on the Mac, never run.
- Suite `crates/tcp-sockets/conformance`: 108 checks (97 before T2) that each take one value, against the traits
  alone, plus 7 functions that build values for the required suites (Configurable twice, NegativeRouteAdvice, and
  four Readiness bundles); `tests!` runs them as 124 tests, and the implementation's own four (allocation checks,
  discarding_cuts_no_packet, the two-thread peeks) run beside it. The suite builds on every unix system and runs on
  Linux only: a capture thread reads the host's packet socket (`AF_PACKET`, every loopback packet seen twice, counted
  once) filtered to one listener's port, and the checks read headers, sequence numbers, windows, TCP timestamps,
  times and the host's tables in /proc. The namespace it needs (root, `fq` on loopback, `net.ipv4.tcp_fastopen=3`,
  `net.ipv6.flowlabel_consistency=0`, a microsecond-clock route, loopback forwarding) is written at the top of the
  suite and made by run-conformance.sh.
- Which check fails when a method or setting breaks (broken copies in mutants.sh; results under Measurements):
  connect, listen, accept: every check; listen's backlog argument: listener_backlog_bounds_the_accept_queue and the
  two ICMP checks for a synchronizing connection; their owner: connect_and_listen_make_sockets_of_their_owner; their
  protocol: multipath_is_negotiated_when_both_ends_ask; accept's frame length: connect_and_accept_name_both_ends.
  send and receive: bytes_arrive_in_order_both_ways and the request checks (urgent_*, peek_*, more_to_come_*,
  record_end_starts_a_new_segment, unread_byte_reports_*, receive_software_timestamps_*, the transmit report
  checks). shut_down: the three shutdown checks (ReadAndWrite as Write fails shutdown_of_both_directions_ends_both).
  close: orderly_close_*, abort_resets_the_peer. stop_listening: stopped_listener_refuses_connections.
  take_error_queue_entry and discard_error_queue_entries: the report, ICMP and discarding checks. push_queued_data:
  more_to_come_holds_data_until_pushed, push_ignoring_neither_*. send_delayed_acknowledgement:
  delayed_acknowledgement_is_sent_on_demand. rearm_keep_alive_timer: rearming_the_keep_alive_timer_sends_a_due_probe.
  send_window_probe: window_probe_is_answered_and_changes_no_setting. set_timestamp_clock:
  timestamp_clock_set_in_repair_mode_stamps_segments (any refusal outside repair mode, the unit counted in it) and
  timestamp_clock_in_the_other_unit_counts_in_it, timestamp_reads_the_clock_in_its_unit,
  repaired_connection_continues_where_the_original_left_off (the listener's: refused unless the listener takes
  repair mode, see For the owner). set_authentication_counters and the listener's:
  authentication_counters_are_set_only_where_authentication_is, authentication_state_reads_the_keys_and_counters.
  read_listener_state: listener_state_reads_a_listener (Information, CongestionControl), tuned_values_follow_the_settings
  (Tuned), timestamp_reads_the_clock_in_its_unit (Timestamp), the authentication checks (Authentication,
  AuthenticationKey), incoming_processor_is_read_as_received_or_set (IncomingProcessor). read_state, by query:
  Information information_reads_what_the_connection_did and the repair check; CongestionControl
  congestion_control_state_names_the_algorithm; Queues queues_count_unread_unacknowledged_and_unsent; Tuned
  tuned_values_follow_the_settings and the set-twice checks; TakeSavedSynchronize saved_synchronize_is_taken_once;
  Timestamp timestamp_reads_the_clock_in_its_unit; Repair the repair check and
  authentication_repair_values_are_read_and_written; Authentication and AuthenticationKey
  authentication_state_reads_the_keys_and_counters and the set-twice checks; Multipath and MultipathSubflow
  multipath_state_describes_the_connection; every query on a multipath connection and listener
  multipath_connection_answers_every_query; LastReceivedHeader last_received_header_records_where_the_last_packet_came
  and the set-twice checks; IncomingProcessor incoming_processor_is_read_as_received_or_set. Repair settings:
  QueueSequence, RepairOptions, RepairWindow the repair check; AuthenticationRepair the authentication repair check. give_negative_route_advice: negative_route_advice_picks_a_new_flow and
  the module negative_route_advice. wait_until_ready on a connection: the modules readiness_tcp_connections and
  readiness_tcp_connections_hanging_up, waits_on_many_connections_each_wake_once,
  wait_for_room_finishes_when_the_peer_reads, wait_for_urgent_data_finishes_when_it_arrives,
  wait_for_the_peers_shutdown_finishes_when_it_comes, wait_for_a_hang_up_finishes_when_both_ways_are_shut,
  wait_for_an_error_finishes_when_the_peer_resets (these two ask for nothing, so only an unconditional event ends
  them), a_wait_gives_the_facts_the_connection_has_and_no_others (whole reports); on a
  listener: the module readiness_tcp_listeners and a_wait_gives_the_facts_the_listener_has_and_no_others. configure
  (both traits): the modules configurable_tcp_connections and configurable_tcp_listeners, and each setting's check:
  send and receive buffers, buffer locks, low waters (send_settings_*, receive_settings_*); window clamp
  (window_clamp_bounds_the_synchronize_window); pacing rate, transmit delay (their checks); hop limit, traffic class,
  path MTU discovery, flow label (header_settings_mark_each_segment); filter, minimum hop limit (arrival_settings_*);
  keep-alive and its idle, interval and probes (keep_alive_probes_end_an_unanswered_connection); no-delay, quick
  acknowledgement, delayed acknowledgement maximum, retransmission minimum and maximum, thin linear timeouts, user
  timeout, SYN retries, defer accept, FIN-WAIT-2, congestion control, address and port sharing, nonlocal addresses,
  transmit rehash, repair, memberships, MD5 and authentication keys (their checks); cork, urgent inline, unread byte
  reports, maximum segment size, ICMP error reports; listener backlog (given to listen, as a setting or later), fast
  open queue and keys, flow label reflection (their checks).
- Settings whose effect only the set-twice checks see (now by the readings too, run 13), since loopback or the
  traits show nothing more: multicast loopback and all-groups (no effect on TCP traffic), packet priority (not on
  the wire), unicast interface, route scope, source preferences, bound interface (one interface), IPv4 options and
  IPv6 extension headers on established connections, the IPv6 variants of traffic class and path MTU discovery, and
  the authentication options' ICMP choice (in the authentication state the set-twice checks compare). Since run 13
  `RecordsLastReceivedHeader` is checked by last_received_header_records_where_the_last_packet_came,
  `InitialAuthenticationCounters` by authentication_state_reads_the_keys_and_counters and the incoming processor by
  incoming_processor_is_read_as_received_or_set. The mark is checked by
  accept_names_where_the_synchronize_arrived; `SaveSynchronize` with all three values through `TakeSavedSynchronize`
  (saved_synchronize_is_taken_once).

## Measurements
- Run 15, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-22 05:15 to 05:16, load 7 to 8: clippy with
  warnings denied (`--no-deps --all-targets`) clean on the TCP host, its suite, the shared crate and the readiness
  suite; readiness suite 13, shared crate 1, TCP 125 + 4 pass.
- Run 15 clean check (job host-tcp-sockets-clean, build folder deleted first, 05:18, load 19 to 21; log
  clean-check-run15.log in scratch): the same clippy clean (10.6 s); readiness 13, shared 1, TCP 125 + 4 pass twice;
  `cargo tree` (normal, build, dev) of tcp-sockets, internet-protocol, configurable, negative-route-advisable,
  readiness, the TCP suite and the readiness suite names only interface and conformance crates (abi, configurable,
  executor, internet-protocol, kernel, negative-route-advisable, readiness, tcp-sockets and their suites).
  `git log a00ab267..HEAD` and `git status` of every interface.rs and the .rs files beside the tcp-sockets,
  internet-protocol, readiness, negative-route-advisable, configurable, kernel and abi interfaces: none.
- Run 15 broken copies (mutants.sh, 05:17 to 05:24, load 13 to 14; log mutants-readiness-report.log in scratch): 27
  copies, each failed the suite. New: unconditional-events-end-no-wait, hang-up-unreported, error-unreported (each
  fails the readiness modules' failure checks, the two waits that ask for nothing and the exact-facts check), the two
  waits finishing at once (rewritten for the report type), clock-microseconds-refused-on-a-millisecond-route and
  clock-milliseconds-refused-on-a-microsecond-route (timestamp_clock_in_the_other_unit_counts_in_it),
  multipath-queues-refused and multipath-tuned-passes-the-errno (multipath_connection_answers_every_query). The
  checker's ten of run 14 that had passed, ic-* (send buffer, timeouts, windows, multipath last times, subflow
  identifier, segments in, authentication ICMP choice, repair options, largest window, checksums), now each fail
  the check named under run 15 above. The three asked for by the milestone's check: more-to-come-ignored
  (more_to_come_holds_data_until_pushed, push_ignoring_neither_sends_data_held_for_more), abort-closes-in-order
  (abort_resets_the_peer, wait_for_an_error_finishes_when_the_peer_resets), peek-offset-ignored
  (peek_at_an_offset_skips_bytes, peeks_from_two_threads_each_start_where_asked). 179 copies in mutants.sh, every
  text found once (tools/check_mutants.sh).
- Run 15: portable.rs (macOS and other unix) changed only by the renames and the report type; it was not built,
  since builds may no longer run on the Mac and the ARM machine has no standard library for a non-Linux target.
- Run 10, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-22 01:48, load 4.8 to 5.1: after the split, clippy
  with warnings denied clean (tcp host, suite, shared crate); suite 113 passed, the implementation's own 4 passed.
- Run 10 independent check, job host-tcp-sockets-clean (01:52 to 01:53, load 1.2 to 2.3; log clean-check-run10.log
  in scratch): build folder deleted first, clippy with warnings denied clean (9.3 s), 113 + 4 passed (7.26 s);
  `cargo tree` (normal, build, dev) of tcp-sockets, internet-protocol, configurable, negative-route-advice,
  readiness, kernel, abi and the suite names only abi, configurable, executor, internet-protocol, kernel,
  negative-route-advice, readiness, tcp-sockets and the four conformance crates. Broken copies: more-to-come-ignored
  (more_to_come_holds_data_until_pushed, push_ignoring_neither_sends_data_held_for_more), abort-closes-in-order
  (abort_resets_the_peer, wait_for_an_error_finishes_when_the_peer_resets), peek-offset-ignored
  (peek_at_an_offset_skips_bytes and the implementation's peeks_from_two_threads_each_start_where_asked) each failed.
  `git diff ae9c53ca HEAD` of the .rs files beside the tcp-sockets, internet-protocol, configurable,
  negative-route-advice, readiness and kernel interfaces shows only the owner's commits b15e307a and 9ce79c65; this
  run's commits touch only this track's folders, its docs and one line of Cargo.lock.
- On the Mac (macOS, 2026-11-22): both crates, the conformance target included, build and lint clean; nothing runs
  there.
- Run 9, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-22 00:04 to 00:12, load 1.2 to 2.6: clippy with
  warnings denied clean (tcp host, suite, shared crate); 106 passed, 0 failed, six runs, each 7.2 to 7.3 s (the
  keep-alive check now waits about 4 s for its second count). The first run failed only in
  accept_names_where_the_synchronize_arrived on a multipath accept's mark (0, see For the owner), now TCP only.
- Run 9 broken copies (mutants.sh, 00:06 to 00:11, load 1.6 to 2.6; logs mutants-run9.log and run9-b.log in
  scratch): multicast-noop (multicast_groups_are_joined_and_left), anycast-noop (anycast_addresses_are_joined_and_left),
  owner-ignored (connect_and_listen_make_sockets_of_their_owner), listener-unread-reports-kept (its new check),
  keep-alive-probes-always-one (keep_alive_probes_end_an_unanswered_connection), accept-interface-exact-address-only
  (accept_names_where_the_synchronize_arrived), and every authentication copy (mac length, coverage, key use, a held
  key's use, a held key refused, removal's current key, an absent key's removal refused, the options' current key,
  the requirement, and each of the three settings doing nothing) failed the suite. Three passed the first time
  (key use, a held key's use, removal's current key: the key Linux chooses without a current one was the same);
  the check now adds the keys in both orders and removes with three keys held, and all three fail it. 130 copies.
- Run 9 independent check from a clean clone of main at 209469ac (job host-tcp-sockets-clean, 00:12 to 00:13, load
  1.6 to 5; log clean-check-run9.log in scratch): build folder emptied, clippy with warnings denied clean (9 s), 106
  passed in 7.24 s; `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel, abi
  and the suite names only abi, configurable, executor, internet-protocol, kernel, negative-route-advice,
  tcp-sockets and the two conformance crates; `git diff ae9c53ca HEAD` of the .rs files beside the tcp-sockets,
  internet-protocol, kernel, configurable and negative-route-advice interfaces is empty (abi's and system's changed
  only in other tracks' commits 564626c0, fd65d351, 4ee5def7, 5da46611). The literal check found one redefinition
  (`HEXADECIMAL`), renamed in 592ee1b9; again from a clean clone at 592ee1b9 (00:14, load 4.6): 106 passed, no
  literal finding in this track's files.
- Run 8, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 23:17 to 23:21, load 2.8 to 5.3: clippy with
  warnings denied clean (tcp host, suite, shared crate); 60 full runs in a row at `RUST_TEST_THREADS=12`, each 101
  passed, 0 failed, 3.44 to 3.51 s (log repeat-run8b.log in scratch). Before the backlog check's second change, 60
  runs (23:13 to 23:17, load 1.6 to 3.7) failed 2 times, both in it with two connections accepted (repeat-run8.log).
  After the literal fix (23:24, load 1.8): three runs, each 101 passed; the literal check finds nothing in this
  track's files (it found `PrefixLength(32)` and a flow label equal to the route-resolver suite's `LABEL`, both
  changed).
- Run 8 broken copies (mutants.sh, 23:21 to 23:22, load 4 to 5), each failed in the check named:
  window-probe-repair-queue-lost (window_probe_is_answered_and_changes_no_setting), push-leaves-no-delay-on
  (no_delay_sends_small_segments_while_others_are_unacknowledged), md5-key-ignored and md5-absent-removal-refused
  (md5_keys_admit_only_connections_signed_with_them), fixed-flow-label-not-held and fixed-flow-label-forgotten
  (fixed_flow_label_marks_every_segment), multicast-refused (multicast_groups_are_joined_and_left). 113 copies now.
- Run 8 independent check from a clean clone of main at 18d3ef97 (job host-tcp-sockets-clean, 23:22 to 23:23, load
  2.6 to 4.7; log clean-check-run8.log in scratch): build folder emptied, clippy with warnings denied clean (10 s),
  101 passed in 3.47 s; `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel,
  abi and the suite names only interface and conformance crates (abi, configurable, executor, internet-protocol,
  kernel, negative-route-advice, tcp-sockets and the two conformance crates); `git diff ae9c53ca HEAD` of the .rs
  files beside the tcp-sockets, internet-protocol, kernel, configurable and negative-route-advice interfaces is
  empty (abi's and system's changed only in other tracks' commits 564626c0, fd65d351, 4ee5def7). Again from a clean
  clone at e040df6a (23:25, load 2.3 to 4.5): clippy clean (9.2 s), 101 passed in 3.49 s.
- Probe of Linux's answers on a TCP socket (scratch tools/probe_memberships.py, ARM machine 23:05, load 2): as root
  and as uid 1000, `MCAST_JOIN_GROUP` and `MCAST_LEAVE_GROUP` on the loopback interface taken for IPv4 and IPv6, on
  no interface ENODEV; anycast join EADDRNOTAVAIL as root, EPERM as uid 1000; the flow label manager taken as root
  (an exclusive label a second time EPERM, a process-shared one taken again), EPERM as uid 1000; on an IPv4 socket
  ENOPROTOOPT.
- Run 7, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 22:33, load 1.9 to 2.7: clippy with warnings
  denied clean (tcp host, suite); 98 passed, 0 failed, three runs, each 3.4 s (94 suite checks and four of the
  implementation's own).
- Run 7 independent check from a clean clone of main at e7bf5cbd (job host-tcp-sockets-clean, 22:34 to 22:35, load
  1.8 to 4.5; log clean-check-run7.log in scratch): build folder emptied, clippy with warnings denied clean (9.5 s),
  98 passed in 3.46 s; `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel,
  abi and the suite shows no implementation crate; `git diff ae9c53ca e7bf5cbd` of the .rs files beside the
  tcp-sockets, internet-protocol, kernel, configurable and negative-route-advice interfaces is empty (abi's and
  system's changed only in other tracks' commits 564626c0, fd65d351, 4ee5def7). Literal check: no finding in this
  track's files.
- Run 7 broken copies (mutants.sh, 22:28 to 22:30, load 2 to 5; log mutants-run-7.log in scratch), each failed:
  multipath-save-syn-refusal-skipped, link-header-kept-as-network, link-header-not-saved,
  link-header-given-by-accept, saved-synchronize-not-kept, saved-synchronize-off-ignored,
  saved-synchronize-taken-twice (saved_synchronize_is_taken_once); records-refused (both connection setting checks
  and the listener's); receive-unlocked (peeks_from_two_threads_each_start_where_asked);
  urgent-wait-unregistered (wait_for_urgent_data_finishes_when_it_arrives); peer-shutdown-unregistered
  (wait_for_the_peers_shutdown_finishes_when_it_comes); quoted-addresses-swapped
  (icmp_error_reaches_a_synchronizing_connection_over_ipv4; its text follows another track's uncommitted edit of
  icmp_error.rs). Not caught, as said above: `RecordsLastReceivedHeader` doing nothing (no read before T2) and the
  IPv4 arrival interface taken from the host's addresses instead.
- Run 6, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 21:59 to 22:04, load 4 to 16: clippy with
  warnings denied clean (tcp host, suite, shared crate); 95 passed, 0 failed, five runs, each 3.4 s (92 suite checks
  and three of the implementation's own). Literal check (22:03): no finding in this track's files.
- Run 6 broken copies (mutants.sh, 22:00 to 22:02, load 5 to 9; log mutants-run-6.log in scratch), each failed:
  clock-always-refused (both clock checks, the window probe check), clock-unit-ignored
  (timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused), clock-microseconds-as-milliseconds
  (timestamp_clock_set_in_repair_mode_stamps_segments), multipath-is-tcp (multipath_is_negotiated_when_both_ends_ask,
  saved_synchronize_is_taken_once), tcp-is-multipath (every test: a multipath listener refuses `TCP_SAVE_SYN`),
  shutdown-both-is-write (shutdown_of_both_directions_ends_both), multipath-save-syn-refusal-skipped
  (saved_synchronize_is_taken_once), discard-counts-cuts (discarding_cuts_no_packet), authentication-counters-fake
  and listener-authentication-counters-fake (authentication_counters_are_set_only_where_authentication_is),
  listener-state-fake (listener_state_reads_a_listener), report-packet-dropped and statistics-dropped (their report
  checks, after the error queue change).
- Run 6 independent check from a clean clone of main at ff302f08 (job host-tcp-sockets-clean, 22:01 to 22:02, load 5
  to 6; log clean-check-run6.log in scratch): build folder emptied, clippy clean (9.3 s), 95 passed in 3.44 s;
  `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel, abi and the suite
  shows no implementation crate; `git diff ae9c53ca HEAD` of every .rs file beside the tcp-sockets,
  internet-protocol, kernel, configurable, negative-route-advice and abi interfaces is empty; system's interface.rs
  differs by two lines from other tracks' commits.
- Run 5, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 21:28, load 14 to 15: clippy with warnings
  denied clean, 90 passed in 3.43 s. Literal check (21:29): no finding in this track's files.
- Run 5 independent check from a clean clone of main at 02f9c69c (job host-tcp-sockets-clean, 21:30 to 21:32, load
  11 to 31; log clean-check-run5.log in scratch): build folder emptied, clippy clean (9.6 s), 90 passed in 3.42 s;
  `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel, abi and the suite
  shows no implementation crate; four broken copies each failed: abort-closes-in-order (abort_resets_the_peer,
  wait_ends_with_the_errors_always_reported), push-does-nothing (more_to_come_holds_data_until_pushed,
  push_ignoring_neither_keeps_the_cork), wait-never-finishes (seven wait checks and the allocation check),
  readiness-wakes-nothing (four wait checks). `git diff ae9c53ca HEAD` of every interface.rs and data file this
  track uses (tcp-sockets, internet-protocol, kernel, configurable, negative-route-advice) is empty; abi's and
  system's interface.rs changed only in other tracks' commits 564626c0, fd65d351, 4ee5def7.
- Run 4, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 21:20, load 1.7 to 3.2: clippy with warnings
  denied clean; 90 passed, 0 failed, three runs in a row, each 3.4 s (89 suite checks and the allocation check).
- Run 4 broken copies (mutants.sh, now 90 copies), same machine, 20:49 to 21:19, load 2 to 21: every copy failed the
  suite (logs mutants-run-5.log and mutants-run-5b.log in scratch). New copies and what fails: the connection's
  wait finishing at once (six wait checks and the allocation check), never finishing (seven), the listener's
  wait finishing at once (both listener wait checks), never finishing (listener_wait_finishes_when_a_connection_comes),
  the shared readiness never withdrawing a dropped wait (both dropping checks and the allocation check), the
  readiness firing no wake (four, after the data and listener checks were made to demand a wake: they had passed
  on a poll alone), and the stopped agent's check of plain sends between reported ones. The first run stalled on
  never-finishing because the allocation check parked without a deadline; it now has one.
- ARM machine (Graviton4, 96 cores, Linux 7.0.0-1018-aws), 2026-11-21 19:34, load 3.4 to 5.8: 72 passed, 0
  failed, five runs in a row, each 3.3 s (the keep-alive, SYN retry and retransmission checks wait about 3 s).
- Broken copies (mutants.sh), same machine: 63 copies, 2026-11-21 19:50 to 19:57 (load 4 to 9.5, 6 min 53 s),
  each failed the suite; the 64th (exit-node port by the kernel only, as before the binding change) at 19:58 failed
  exit_node_chooses_a_port_outside_the_reserved. Each copy breaks one method, request or setting (every setting
  of the list above doing nothing, one at a time); the checks each one failed are in the scratch log
  mutants-run-3.log and mutants-run-4.log. Two copies of the first run passed and were mended: the timestamping
  copy broke code connections never reach (it now breaks the arm they use), and the retransmission maximum was
  missed because Linux can send a tail loss probe first (the check now looks at the longest gap over 3.2 s).
- Independent check from a clean clone of main at 239247f8 (job host-tcp-sockets-clean, 2026-11-21 19:49, load
  10 to 11): main's own Cargo.lock now parses; build directory emptied, clippy with warnings denied clean (9.3 s),
  72 passed; `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel, abi and
  the suite shows no implementation crate; `git diff 4308b9f7 HEAD` of every interface.rs and data file this track
  uses is empty (only other tracks' conformance suites of configurable and negative-route-advice changed).
- After the binding change, 72 passed three times (19:52, load 9.3 to 9.6), clippy clean on Linux and the Mac.
- Calibration with Python on the same kernel (scratch tools/probe*.py), load 6 to 12: a connection whose peer
  reads nothing takes 2 720 256 bytes; with a 64 KiB send buffer 256 000, a locked send buffer 175 589, a 16 KiB
  not-sent low water 160 768. The peer then holds 113 152 unread; with a 1 MiB receive low water 2 069 998.
  `TCP_TX_DELAY` of 50 ms delays nothing without `fq` and 50.2 ms with it. 512 KiB at a pacing rate of 1 MB/s took
  0.20 s without `fq` and 0.23 s with it.
- Earlier runs: 43 and then 48 checks passed on the same machine (2026-11-21 18:27 and 18:50).

## How to run
- Every build and test runs on the ARM machine; the Mac only edits, formats (`nix develop -c rustfmt --edition
  2024 <file>`) and commits.
- Suite on the ARM machine (needs sudo; makes its own network namespace):
  `cloud/arm-run <checkout> --name host-tcp-sockets -- crates/tcp-sockets/implementation/host/run-conformance.sh`
  (`RUST_TEST_THREADS=12` before it keeps to the machine's share).
- Broken copies (they break linux.rs): the same with `crates/tcp-sockets/implementation/host/mutants.sh [names]` (about 90 s a
  copy, 179 copies; names pick some). Whether every copy's text is still there once: scratch
  tools/check_mutants.sh <checkout> (on the Mac, no build).
- Lint for Linux on the ARM machine: `cloud/arm-run <checkout> --name host-tcp-sockets -- 'cargo clippy -j
  $ARM_JOBS --no-deps --tests -p wcontainers-tcp-sockets-host -p wcontainers-tcp-sockets-conformance --
  -D warnings'`, and the same on the Mac without arm-run (with `CARGO_TARGET_DIR=$PWD/target/host-tcp-sockets
  nix develop -c`; there both crates build empty).
- Files copied from the Mac can carry times older than the ARM machine's last build, so cargo there may skip
  them: `touch` the changed files in the command when a result looks stale.
- The literal check (run 4, on the ARM machine in a copy given its own `git init`, since arm-run copies no .git)
  found eight findings in this track's files that earlier runs had not seen; seven are fixed (named constants,
  a different test flow label, alignment by `cmsghdr`'s type, the fast open sysctl as its two named bits); the
  eighth, the name "lo", went in run 5. Command: `rm -rf target/lit && mkdir -p target/lit && tar
  --exclude=./target -cf - . | tar -xf - -C target/lit && cd target/lit && git init -q && git add -A && cargo run
  -q -p wcontainers-literals -- check`, then look for tcp-sockets in its output.
- Cargo.lock in this shared checkout: commit this track's lines through a private index (`GIT_INDEX_FILE` set to
  a scratch file, `git read-tree HEAD`, `git apply --cached` of a patch of those lines, `git add` of own files,
  `git commit`), then `git add` own files in the real index; other tracks' staged and unstaged changes stay.
- The shared crate linux-socket-options was being reorganized by another track during run 5 (its
  implementation.rs staged as linux.rs): if this crate stops building on it, wait and try again.

## Next
- T1 and T2 are done but for the owner's rulings below. The linter's structural findings in this track's files wait
  on them; the linter was not run in run 15 (the orchestrator stopped its use until it is incremental), so run it
  once when told it may be used again.

## Departures from the plan
- Run 13: Tuned's path MTU is `tcpi_pmtu` for connections and listeners alike (`IP_MTU` answers ENOTCONN on a
  listener); the congestion information's form is chosen by name for DCTCP and by length for BBR and Vegas, since
  `TCP_CC_INFO` does not say which it wrote.
- Run 11: the suite's own numbers (timeouts, sizes, ports, keys) link to a section of the plan that writes each
  choice down, the only link DECISIONS.md 11 leaves for a number no standard fixes.
- Run 10: the implementation's body is no longer Linux only (plan corrected): the owner's linter reads the Mac's
  build, so portable.rs gives the types and traits there.
- Run 4: the plan's "the contract waits for nothing" is gone with the async ruling: the two waits are the shared
  readiness's (plan corrected), and the basic calls moved to socket2 (plan corrected).
- The plan said `SO_KEEPALIVE` rearms the keep-alive timer; Linux re-times it only on `TCP_KEEPIDLE` (and on
  keep-alive turned on from off), so the idle time is written again. Plan corrected.
- The binding and socket opening helpers moved into the shared crate for the UDP track.
- Run 1 moved the ICMP part of the error queue and three kinds of transmit report to T2; run 2 did them in T1.
- The plan's router namespace for ICMP errors became raw sockets on loopback: the check captures the SYN and
  sends the error itself, which needs no namespace setup beyond the suite's own.
- wireless_status_request_still_sends only checks that asking works and that loopback brings no status: no
  wireless link exists on the test machine.
- Cargo.lock: this track commits only its own lines of it (run 3: nix and memoffset, and its two packages'
  entries), applied to the index from a patch of those hunks; other tracks' uncommitted lines stay.
- Run 3: the plan said `SCM_TS_OPT_ID` carries a send's report key; Linux refuses it on TCP, so the key is kept by
  the kernel's byte offset (plan corrected). The plan's suite gained the wire checks above and the refused settings
  in the set-twice lists (plan corrected).
- Run 3: connections no longer choose their exit-node port at connect (which let one port serve several peers);
  they bind it at once like listeners, one path that keeps to ranges outside the host's own.
- Run 3: the suite's body is Linux only (it was empty of Linux-only calls before): the wire checks need the host's
  packet socket.

- Run 8 and 9: the plan put all authentication in T2, while the milestone asks for every setting of settings.rs
  that needs no privilege. MD5 keys (run 8) and the TCP Authentication Option's keys, options and initial counters
  (run 9) moved to T1, plan corrected; the authentication repair values (repair mode needs CAP_NET_ADMIN) and the
  method `set_authentication_counters` stay T2.

## Dependencies added
- Run 13: linux-raw-sys gains its `ioctl` feature (`FIONREAD`, `TIOCOUTQ`, `SIOCATMARK`); same licence. Neither
  libc nor linux-raw-sys carries `struct mptcp_info`, `mptcp_full_info`, `mptcp_subflow_info`, the `tcp_cc_info`
  forms or the Accurate ECN fields of `tcp_info`, so linux/state.rs defines them `repr(C)` from Linux's uapi headers
  (each linked), and Linux's TCP state numbers (include/net/tcp_states.h, not uapi).
- Run 10: the suite takes linux-raw-sys 0.12.1 (features net, if_tun: Linux's classic BPF and IPv4 option codes on
  every system) in place of libc: Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT. The implementation's socket2
  and rustix (now with `process`, for the user the process runs as) are on every unix system.
- Run 5: nix 0.31.4 in the suite too (Linux only, `net` feature: the loopback interface by its flag): MIT.
- Run 4: socket2 0.6.5 with feature `all` (the basic calls): MIT OR Apache-2.0. polling 3.11.0 comes through the
  shared crate (the UDP track added it): Apache-2.0 OR MIT.
- rustix 1.1.6 (also in the suite, for its raw and packet sockets): Apache-2.0 WITH LLVM-exception OR Apache-2.0
  OR MIT. netlink-packet-core 0.9.0 (the statistics' attributes): MIT. linux-raw-sys 0.12.1: the same. libc
  0.2.191 (also in the suite since run 3, for BPF codes): MIT OR Apache-2.0. etherparse 0.21.0: MIT OR
  Apache-2.0. Run 3: nix 0.31.4 with only its `net` feature (interface addresses): MIT; it brings memoffset
  0.9.1: MIT.

## For the owner
- The shared helper crate crates/internet-protocol/implementation/linux-socket-options implements no interface,
  so the layout reports everything it exports (`public_outside_interface`, 20: `Shortfalls`, `Watcher`, `host_error`,
  ...) and wants a conformance target it cannot have. The brief asked for it (one crate the socket tracks share in
  place of four private copies). Example: `Shortfalls::refuse`, used by all four socket implementations. It needs a
  place in the layout (an interface of its own, or a kind of crate the linter allows to export).
- The brief says an implementation's fields are private; the linter's `public_outside_interface` says a type that
  fills an associated type is plain data with every field `pub`. Example: `HostTcpConnection` holds its socket's
  descriptor and registration (`socket: Watched`); made public, any caller could take the descriptor out. Kept
  private.
- The linter reads the Mac's build, so a Linux-only file is read as tokens: its impls of the required traits
  (`impl Readiness<HostTcpConnection> for HostTcpSockets` in linux.rs) are reported as `implementation_folder`,
  while the same impls in portable.rs pass. Example: linux.rs:2039.
- The readiness suite cannot be run as the layout writes a required suite (`tests!($crate::f($value))` with
  `f` giving `(T, T::Connection)`): its checks need a way to make the handle ready, so it takes a `Waiting` bundle
  with a closure. Example: `a_wait_finishes_when_the_handle_becomes_ready` must send data to the connection while a
  wait is pending, which nothing in `(sockets, connection)` can do. The TCP suite runs it in its bundle form until
  the readiness suite changes.
- `TcpSetting::NotSentLowWater(None)` does not say whether None is no bar or the host's default: Linux reads 0 as
  its sysctl `net.ipv4.tcp_notsent_lowat` and `u32::MAX` as no bar. Example: with the sysctl at 16384, None as 0
  bars a connection at 16 KiB unsent, as `u32::MAX` not at all. The implementation writes `u32::MAX`.
- A fixed IPv6 flow label (`FlowLabelChoice::Label`) can be asked of a listener or an established connection, but
  Linux fixes a TCP socket's label only as it connects (`np->flow_label` from the connect address). Example: a
  connection made without a label and then given a fixed label is answered NotSupported.
- Building one now starts a host thread and an epoll set, so it can fail: the suggested trait is
  `pub trait StandaloneTcpSockets: TcpSockets + Sized { fn new() -> Result<Self, HostError>; }` (the crate has
  `HostTcpSockets::new() -> Result<Self, HostError>`).
- `owner: UserId` is undefined when the exit node runs as one user: a guest's root (`UserId(0)`) connecting
  through a host process running as uid 1000 can only be answered EPERM (the host cannot make a socket owned
  by another user without CAP_SETUID), so on an unprivileged host every guest connect fails. The suite
  therefore runs as root.
- `TcpAccepted::synchronize` asks for fields Linux never reports for a SYN: for IPv4 the arrival interface (Linux
  keeps that of the segment that completed the handshake, given in its place and counted), and the SYN's own mark,
  priority and timestamps (it gives the accepted socket's mark and priority and no timestamps). Its
  `largest_fragment` is always None (counted): Linux keeps no fragment size for a SYN, and `ReceivedPacketFields`
  does not say whether None means "not reassembled" or "not known". Example: a SYN that came in two fragments is
  reported exactly like one that did not.
- Linux gives an accepted multipath socket mark 0 whatever its SYN's mark (`tcp_fwmark_accept` marks only the
  subflow), so `TcpAccepted::synchronize.mark` of a multipath accept is the socket's 0 (counted with the other
  fields of the socket). Example: a multipath client with mark 7 to 127.0.0.2 is accepted with mark 0.
- A multipath accept has no SYN to give: Linux has no `TCP_SAVED_SYN` (nor packet options) on MPTCP sockets, so
  `synchronize_header` is empty and `synchronize.length` is 0, counted (`NO_SAVED_SYNCHRONIZE`), with the interface
  taken from the host's addresses (counted). Example: a multipath client to a multipath listener on 127.0.0.1 is
  accepted with length 0. The contract has no way to say the SYN is unknown (an `Option` would).
- `TransmitReport::interface` is required, but Linux names an interface in a report only when the socket has
  `IP_PKTINFO` on: otherwise the implementation gives index 0, Linux's own "none".
- `TcpTransmitStatistics::received_hop_limit` is required, but Linux gives `TCP_NLA_TTL` only in a report an
  acknowledgement brought (example: a software send report has none): the implementation gives 0 then.
- A wireless status report names no send on Linux (`ee_data` is 0): the implementation gives key 0, so the
  request's `key` cannot be honoured for it.
- Linux gives a TCP socket's transmit reports the packet or statistics, never both, and the choice is the
  socket's: example, a send asking for statistics after one asked for the packet is answered NotSupported.
- `TransmitReportKey` cannot be passed to Linux for TCP (`SCM_TS_OPT_ID` is refused on TCP sockets): the
  implementation keeps each send's key by the byte offset the kernel reports instead.
- `RecordsLastReceivedHeader(Report)` does not say which header is recorded for a TCP socket: IPv4's
  `IP_PKTOPTIONS` gives control messages (packet information, TTL, TOS), not the header bytes that
  `TcpReading::LastReceivedHeader` carries.
- Linux 7.0 does not keep a `TCP_WINDOW_CLAMP` set before connect: measured with Python, a clamp of 8192 bounds the
  SYN's and the handshake's windows (8000 with a segment size of 1000), then the windows grow (15 808, 23 712, ...)
  and the clamp reads 104 960. Set after the handshake it never shrinks the window already offered. So
  `WindowClamp` is honoured as Linux honours it, and the suite checks the SYN's window only.
- The Vegas form of `CongestionControlInformation` names Linux's fields the other way round from what they hold:
  `tcpv_rtt` (`round_trip_time`) is Vegas's base RTT, the lowest ever seen, and `tcpv_minrtt`
  (`minimum_round_trip_time`) the lowest of the current round, 0x7fffffff µs before its first sample. Measured after
  a 1 MiB transfer on loopback: round_trip_time 15 µs, minimum_round_trip_time 2147.483647 s. Given as Linux gives
  them.
- `MultipathInformation::fell_back_to_tcp` is never true on Linux 7.0: `MPTCP_INFO` on a connection that fell back
  answers EOPNOTSUPP (only the diag interface reports the flag). Example: a multipath client to a plain listener.
- `TcpReading::LastReceivedHeader` asks for the header and fields of the last packet, but Linux keeps for a TCP
  socket only the records (`IP_PKTOPTIONS`, `IPV6_2292PKTOPTIONS`): no length and no fixed header, and for IPv4 no
  options (`IP_PKTOPTIONS` writes only `IP_PKTINFO`, `IP_TTL` and `IP_TOS`). The implementation gives length 0 and
  the IPv6 extension headers recorded as the header bytes, both counted, and none for IPv4, counted. Example: a plain
  IPv4 segment reads as header length 0 with interface 1 and destination 127.0.0.1; an IPv6 segment with an 8-byte
  hop-by-hop header reads its 8 bytes, `[6, 0, 0, 0, 0, 0, 0, 0]` (6 the next header its sender filled in).
- `TcpQuery::CongestionControl` with a buffer shorter than the name: Linux cuts the name (`TCP_CONGESTION` copies
  what fits), the implementation does the same, and the contract does not say. Example: "reno" into 3 bytes reads
  "ren".
- `TcpInformation::bytes_acknowledged` counts the SYN on Linux: 1 048 577 after 1 MiB acknowledged.
- `set_listener_timestamp_clock` asks for a listener "in repair mode", but Linux never lets a listener into repair
  mode (`tcp_can_repair_sock` refuses `TCP_LISTEN`): the call is always EPERM on Linux. The suite asks the listener
  into repair mode first and demands the clock set if it is taken, refused if not. Neither clock call names its error outside repair mode, so the suite takes any refusal
  (Linux: EPERM; example: an implementation answering NotSupported passes too).
- `FlowLabelReflection` on Linux reflects the SYN's flow label in the SYN-ACK only (measured: the accepted
  connection's data then carries a label of its own); the contract does not say which segments reflect it.

