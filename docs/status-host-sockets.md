# Host sockets track: status

Each fact is marked MEASURED (a command was run, both rented machines unless one is named), READ (in a file or a
crate's source) or INFERRED (not checked by running anything). Commits: main = f70acb496 (holds the owner's
c8759035f, every crate on the workspace lints), agents = origin/agents 198bb11d6. Crate versions in the lock files
(READ, both commits): rustix 1.1.6, socket2 0.6.5, nix 0.31.4 (the manifests ask for 0.31.3), libc 0.2.191.

## HS0: inventory and survey (done; independent check pending)

Jobs (MEASURED, ARM64 and x86-64, rustc 1.99.0 on both): `cargo check -p <crate> --all-targets` per crate, the same
with `--cap-lints warn` added to the target rustflags (so each unsafe use shows as a warning and the other errors
show), the same pairs with `--keep-going` at main, `cargo test -p <crate> --all-targets [--no-fail-fast] -- --list`
at agents, and one `cargo reviewable check --message-format json` per commit (linter built from its `install` source,
59ad72ce; no key on the machines, so no model question was sent). ARM64 and x86-64 gave the same compile results, the
same unsafe use counts, the same test counts and the same linter findings per crate (MEASURED: `diff` of the parsed
outputs is empty). Unsafe lines are counted with the root manifest's command (`git grep -nw unsafe ...`).

### Per crate at main (f70acb496)

"Own errors" are the errors in the crate's own files, with lints capped; "not reached" means cargo never compiled the
crate because a crate it needs fails first.

| Crate | Compiles | Own errors, by class | Unsafe lines | Unsafe uses the compiler reports |
|---|---|---|---|---|
| internet-protocol linux-socket-options | no | none besides unsafe | 40 | 40 |
| tcp-sockets host | no | E0049 x2 (a method lacks the type parameter its trait now has) | 23 | 18 |
| udp-sockets host | no | E0049 x1; its tests not reached (udp suite fails) | 11 (2 lib, 9 tests) | 2 (lib) |
| icmp-sockets host | no | E0049 x2, E0053 x2 (incompatible method type) | 0 | 0 |
| raw-sockets host | no | E0053 x2, E0049 x1 | 0 | 0 |
| route-resolver host | lib yes with lints capped; no as is (needs linux-socket-options) | none; tests not reached (icmp suite fails) | 2 (tests) | not reached |
| tcp-sockets suite | no | E0277 x41 (`Vec<u8>`, `[u8]`, `[u8; N]` are not `VolatileBytes`) | 4 | 4 |
| udp-sockets suite | no | E0277 x9 (same) | 0 | 0 |
| icmp-sockets suite | no | E0277 x42, E0599 x2, E0308 x1 (`ByteCount`, `BufferSizes` no longer `Result`) | 15 | 8 |
| raw-sockets suite | no | not reached (needs the icmp suite) | 0 | not reached |
| route-resolver suite | no | not reached (needs the icmp suite) | 4 | not reached |
| configurable suite | yes | none | 0 | 0 |
| readiness suite | yes | none | 0 | 0 |
| negative-route-advisable suite | yes | none | 0 | 0 |

All MEASURED except the unsafe lines (READ, `git grep`). Total unsafe lines at main: 99. The type errors come from the
owner's `VolatileBytes` interface change, which main's older copies predate (INFERRED from the messages).

### Per crate at agents (198bb11d6)

| Crate | Compiles (check --all-targets) | Unsafe lines | Lint table | Tests listed |
|---|---|---|---|---|
| internet-protocol linux-socket-options | yes | 2 | own `[lints.rust]`, only `unsafe_op_in_unsafe_fn = "deny"` | 1 (tests/readiness.rs) |
| tcp-sockets host | yes | 1 | own `[lints.rust]`, only `unsafe_op_in_unsafe_fn = "deny"` | 146: suite run 143, unit 2, speed 1, allocations (self-running, needs root) |
| udp-sockets host | yes | 0 | workspace | 84: suite 80, readiness 3, speed 1, allocations (self-running) |
| icmp-sockets host | yes | 0 | workspace | 75: suite run 69, counters 5, waiting 1 |
| raw-sockets host | yes | 0 | workspace | 57: suite run 54, counters 2, waiting 1 |
| route-resolver host | yes | 0 | workspace | 39: suite run 38, counters 1 |
| tcp-sockets suite | yes | 0 | workspace | (library; run by the host) |
| udp-sockets suite | yes | 0 | workspace | (library) |
| icmp-sockets suite | yes | 1 | own `[lints.rust]`, only `unsafe_op_in_unsafe_fn = "deny"` | (library) |
| raw-sockets suite | yes | 0 | workspace | (library) |
| route-resolver suite | yes | 2 | own `[lints.rust]`, only `unsafe_op_in_unsafe_fn = "deny"` | (library) |
| configurable suite | yes | 0 | workspace | 2 |
| readiness suite | yes | 0 | workspace | 13 |
| negative-route-advisable suite | yes | 0 | workspace | 7 |

MEASURED. The suite counts meet HS1's bar exactly (TCP 143, UDP 80, ICMP 69, raw 54, route resolver 38). The four
crates with their own table compile at agents only because that table replaces the workspace lints: it holds only
`unsafe_op_in_unsafe_fn = "deny"` and names no `unsafe_code` level, so `unsafe_code` stays at the compiler's default,
allowed (READ: their Cargo.toml at agents; corrected on 2026-11-25, this line said the table lowered `forbid` to
`deny`). Under main's rule, where every crate inherits `unsafe_code = "forbid"`, their 6 lines are refused.

### Linter findings by rule (`cargo reviewable check`, same on both machines)

At agents (main in brackets where it differs in total):

- linux-socket-options 322 [93]: public_outside_interface 203, function_summary_not_third_person 39,
  summary_not_one_line 18, summary_too_long 13, magic_literal 12, grammar_unclear 12, number_constant_without_link 10,
  summary_unclear 4, summary_starts_with_name 3, comment_too_long 2, summary_not_one_sentence 2, conformance_not_run 1,
  verb_not_first 1, type_summary_not_noun_phrase 1, summary_restates_declaration 1.
- tcp host 90 [39]: public_outside_interface 36, grammar_unclear 11, magic_literal 7, summary_unclear 6,
  summary_not_one_line 6, summary_too_long 5, number_constant_without_link 5, implementation_folder 5,
  function_summary_not_third_person 3, summary_restates_declaration 3, type_summary_not_noun_phrase 1,
  verb_not_first 1, summary_starts_with_name 1.
- udp host 74 [49]: function_summary_not_third_person 25, public_outside_interface 11, summary_not_one_line 9,
  constant_defined_twice 7, implementation_folder 6, summary_too_long 5, number_constant_without_link 3,
  conformance_not_run 2, grammar_unclear 2, magic_literal 1, summary_not_one_sentence 1, summary_starts_with_name 1,
  trait_not_implemented 1.
- icmp host 24 [6]: conformance_not_run 9, public_outside_interface 8, grammar_unclear 2,
  function_summary_not_third_person 2, implementation_folder 2, summary_restates_declaration 1.
- raw host 24 [6]: public_outside_interface 10, conformance_not_run 7, grammar_unclear 2,
  function_summary_not_third_person 2, implementation_folder 2, summary_restates_declaration 1.
- route-resolver host 14 [12]: verb_not_first 4, summary_unclear 3, grammar_unclear 3, public_outside_interface 3,
  conformance_not_run 1.
- tcp suite 158 [151]: verb_not_first 79, grammar_unclear 37, summary_restates_declaration 9, magic_literal 8,
  conformance_not_run 8, comment_too_long 4, summary_unclear 4, function_summary_not_third_person 3,
  summary_not_one_line 2, type_summary_not_noun_phrase 1, constant_defined_twice 1, number_constant_without_link 1,
  summary_too_long 1.
- udp suite 272 [263]: function_summary_not_third_person 88, verb_not_first 56, summary_not_one_line 30,
  number_constant_without_link 27, summary_too_long 16, magic_literal 15, grammar_unclear 14, conformance_not_run 6,
  summary_restates_declaration 5, public_outside_interface 5, summary_not_one_sentence 4, comment_too_long 2,
  summary_unclear 2, hook_name 1, summary_starts_with_name 1.
- icmp suite 238 [86]: public_outside_interface 146, verb_not_first 59, grammar_unclear 13, conformance_not_run 6,
  summary_unclear 3, comment_too_long 2, number_constant_without_link 2, summary_restates_declaration 2,
  function_summary_not_third_person 2, literal_restates_constant 1, trait_outside_interface 1, implementation_folder 1.
- raw suite 66 [52]: verb_not_first 39, public_outside_interface 10, grammar_unclear 6, conformance_not_run 6,
  summary_restates_declaration 3, summary_not_one_line 1, trait_outside_interface 1.
- route-resolver suite 55 [55]: verb_not_first 37, summary_restates_declaration 9, grammar_unclear 5,
  public_outside_interface 3, comment_too_long 1.
- configurable suite 4 [6], readiness suite 43 [43], negative-route-advisable suite 6 [9].

At main every crate that does not compile also has one `compiler_view_missing`, and the suites `conformance_missing`
(they are still named conformance.rs). Model questions left unanswered for want of a key are left out of the counts
(agents: 135 in these crates; main: 0).

### The 20 refused uses in the suites at main (added on 2026-11-25)

The owner's c8759035f put every crate on the workspace lints. MEASURED: the compiler refuses 8 uses in the ICMP suite
and 4 in the TCP suite. The route-resolver suite holds 4 more lines (READ) that the compiler never reaches, because
the route-resolver suite needs the ICMP suite, which fails first: the 8 errors shown when building the route-resolver
suite are the ICMP suite's own 8 (MEASURED: every one names `crates/icmp-sockets/conformance/conformance.rs`). So the
20 named in the task are 16 distinct uses (8 + 4 + 4) with the ICMP 8 counted twice. The ICMP suite also holds 7
more `unsafe` lines inside its exported `counting_allocator!` macro (a `GlobalAlloc` that counts), which the compiler
checks only where a test expands it (READ). Agents already removed all of these except the three socket options.

| Site at main | What it does | Safe typed call that replaces it |
|---|---|---|
| icmp suite :52, tcp suite :5679, route-resolver suite :37 | leave the test thread's network namespace (`rustix::thread::unshare_unsafe(NEWNET)`) | nix 0.31.4 `nix::sched::unshare(CloneFlags::CLONE_NEWNET)`, src/sched.rs:142 (agents uses it) |
| icmp suite :658 | direct libc call `socket(AF_PACKET, SOCK_DGRAM, ETH_P_ALL)` giving a raw descriptor | nix `nix::sys::socket::socket(AddressFamily::Packet, SockType::Datagram, flags, SockProtocol::EthAll)` returning `OwnedFd`, src/sys/socket/mod.rs:2224 (agents) |
| icmp suite :667 | `OwnedFd::from_raw_fd` on that raw descriptor | gone with the line above |
| icmp suite :669 | zeroed C structure `sockaddr_ll` for bind | none needed: agents drops the bind (next line) |
| icmp suite :674 | direct libc call `bind` of the packet socket to one link | none (nix's `LinkAddr` has no safe constructor, READ addr.rs:1846-1925). Agents instead keeps every link's packets and drops those whose `LinkAddr::ifindex()` is another link's: the same packets reach the test, not the same call. A bind would belong to the outside API **Linux packet sockets, `bind(2)` with `struct sockaddr_ll` (packet(7))** |
| icmp suite :691 | zeroed C structure `sockaddr_ll` for the sender | nix `recvfrom::<LinkAddr>` gives the address, mod.rs:2405 (agents) |
| icmp suite :694 | direct libc call `recvfrom` into raw pointers | nix `nix::sys::socket::recvfrom::<LinkAddr>(fd, &mut buffer)`, mod.rs:2405 (agents) |
| icmp suite :2281 | direct libc call `setsockopt(IPPROTO_IPV6, IPV6_ADDR_PREFERENCES)` on a probe socket | none. Outside API: **Linux `setsockopt(2)`, level IPPROTO_IPV6, ipv6(7)** (still at agents suite.rs:2364) |
| tcp suite :5650 | zeroed C structure `ifreq` | none needed: agents brings the loopback up by netlink instead (next rows) |
| tcp suite :5652 | reads the union field `ifr_ifrn.ifrn_name` of that `ifreq` | gone with the line above |
| tcp suite :5658 | `rustix::ioctl::ioctl(SIOCSIFFLAGS)` to set IFF_UP on `lo` | agents: an RTM_SETLINK message through netlink-packet-route 0.34.0 and netlink-sys 0.9.0 (MIT, outside the vetted four), index by nix `if_nametoindex` (src/net/if_.rs:18). Within the vetted crates: a rustix NETLINK_ROUTE socket (`rustix::net::socket`, AddressFamily::NETLINK) with the message bytes built by zerocopy (INFERRED, not tried) |
| route-resolver suite :138 | direct libc call `if_nametoindex` with a C string | nix `nix::net::if_::if_nametoindex`, src/net/if_.rs:18 (agents); or rustix `netdevice::name_to_index`, src/net/netdevice.rs:36 |
| route-resolver suite :1536 | direct libc call `setsockopt(IPPROTO_IP, IP_OPTIONS)` on a probe socket | none. Outside API: **Linux `setsockopt(2)`, level IPPROTO_IP, ip(7)** (agents suite.rs:1534) |
| route-resolver suite :1695 | direct libc call `setsockopt` of IP_UNICAST_IF, IPV6_UNICAST_IF or SO_BINDTOIFINDEX | SO_BINDTOIFINDEX: socket2 0.6.5 `bind_device_by_index_v4`/`_v6`, src/sys/unix.rs:2023 and 2074 (it sets SO_BINDTOIFINDEX on Linux, READ :2039-2046). IP_UNICAST_IF and IPV6_UNICAST_IF: none. Outside API: **Linux `setsockopt(2)`, IPPROTO_IP ip(7) and IPPROTO_IPV6 ipv6(7)** (agents suite.rs:1693) |

The same sites in the hosts at main (tcp host 23, udp host 11, route-resolver host 2, linux-socket-options 40 lines)
are the older form of the calls surveyed next; agents already moved them (for example udp's `TIOCOUTQ` ioctl and
`from_raw_parts`, and `recvmsg` to nix).

### Every raw option, ioctl and libc call at agents, with its safe route

At agents the raw calls are 6 (READ grep of `libc::<function>(`): `setsockopt` and `getsockopt` in
linux-socket-options linux.rs:89 and :115 (under `set_raw` and `get_bytes`), the tcp host's `ioctl` (state.rs:565),
and the three suite `setsockopt` calls above. Behind the first two sit 131 call sites of `set_option`, `set_bytes`,
`set_int`, `set_flag`, `get_bytes`, `get_option` and `get_int` in these crates (READ grep; the brief's "about 42" was a
narrower grep). They and the suite calls reach 125 distinct options and the ioctl 3 requests, listed here by option, each with every call site and
its route. A route is listed only where the crate's call reaches the same level, option and value; "none" means no
safe call in rustix 1.1.6, socket2 0.6.5, nix 0.31.4 or std does (READ: each option name searched in the three crates'
sources; std wraps only owned sockets and a few of these). Paths: lso = linux-socket-options, t/u/i/r/rr = the tcp,
udp, icmp, raw and route-resolver hosts.

Safe routes (43 options, plus 2 in part):

| Option | Sites | Route (crate, function, source file:line) |
|---|---|---|
| SO_MARK set | lso linux.rs:435; sticky Mark (send.rs:203, 227) | socket2 `set_mark`, sys/unix.rs:1765 |
| SO_MARK get | t linux.rs:1466 | socket2 `mark`, sys/unix.rs:1747 |
| SO_PRIORITY set | lso linux.rs:437; sticky Priority (send.rs:357) | socket2 `set_priority`, socket.rs:1054 |
| SO_PRIORITY get | t linux.rs:1471 | socket2 `priority`, socket.rs:1039 |
| SO_DONTROUTE set | lso linux.rs:439; sticky (send.rs:204) | nix `sockopt::DontRoute`, sockopt.rs:622 |
| SO_BROADCAST set | sticky (send.rs:207) | socket2 `set_broadcast`, socket.rs:889 |
| SO_RXQ_OVFL set | r linux.rs:375, u linux.rs:438 | nix `sockopt::RxqOvfl`, sockopt.rs:1163 |
| SO_ATTACH_FILTER, SO_DETACH_FILTER set | lso linux.rs:215, 217 (from t linux.rs:1192) | socket2 `attach_filter` sys/unix.rs:2600, `detach_filter` :2623 (feature "all", on) |
| SO_INCOMING_CPU set (-1) | lso linux.rs:313 | rustix `set_socket_incoming_cpu(u32::MAX)`, net/sockopt.rs:726 (same 32 bits, INFERRED) |
| SO_INCOMING_CPU get | t state.rs:1276 | rustix `socket_incoming_cpu`, net/sockopt.rs:714 |
| SO_SNDBUF, SO_RCVBUF get | t state.rs:872 | socket2 `send_buffer_size` socket.rs:1141, `recv_buffer_size` :1070 |
| SO_TXTIME set | sticky TransmitTime (send.rs:373) | nix `sockopt::TxTime` with `libc::sock_txtime`, sockopt.rs:1152 |
| SO_TIMESTAMPING set, no hardware clock | lso linux.rs:181; sticky (send.rs:164) | nix `sockopt::Timestamping`, sockopt.rs:904, flags by `TimestampingFlag::from_bits_retain` (4-byte int; the kernel reads the 8-byte struct only with SOF_TIMESTAMPING_BIND_PHC, INFERRED) |
| SO_BINDTOIFINDEX set | rr suite :1693 | socket2 `bind_device_by_index_v4`/`_v6`, sys/unix.rs:2023, 2074 |
| IP_TTL set (-1 default) | lso linux.rs:361 | nix `sockopt::Ipv4Ttl` (`c_int`), sockopt.rs:1210 |
| IP_TOS set | lso linux.rs:374; sticky (send.rs:200) | socket2 `set_tos_v4`, socket.rs:1648 |
| IP_MTU_DISCOVER set | lso linux.rs:381; sticky (send.rs:289) | rustix `set_ip_mtu_discover`, net/sockopt.rs:806 |
| IP_TRANSPARENT set | sticky (send.rs:213; u linux.rs:840) | socket2 `set_ip_transparent_v4`, socket.rs:1294 |
| IP_FREEBIND set | sticky (same) | socket2 `set_freebind_v4`, sys/unix.rs:2314 |
| IP_BIND_ADDRESS_NO_PORT set | lso linux.rs:816 | nix `sockopt::IpBindAddressNoPort`, sockopt.rs:573 |
| IP_PKTINFO set | i :348, r :371, u :427 (lists); t :538, :1221, :1712 | nix `sockopt::Ipv4PacketInfo`, sockopt.rs:1057 |
| IP_RECVTTL set | i, r, u lists; t :1221 | nix `sockopt::Ipv4RecvTtl`, sockopt.rs:1221 |
| IP_RECVTOS set | t :1221 (IPV4_RECORDS) | socket2 `set_recv_tos_v4`, socket.rs:1697 |
| IP_RECVERR set and get | i, r, u lists; t :1233, :536 | nix `sockopt::Ipv4RecvErr`, sockopt.rs:1182 |
| IP_RECVORIGDSTADDR set | u list | nix `sockopt::Ipv4OrigDstAddr` (IP_ORIGDSTADDR, the same number 20, READ linux-raw-sys 0.12.1 net.rs), sockopt.rs:1118 |
| IPV6_UNICAST_HOPS set (-1 default) | lso linux.rs:365 | nix `sockopt::Ipv6Ttl` (`c_int`), sockopt.rs:1230 |
| IPV6_TCLASS set | lso linux.rs:375; sticky (send.rs:223) | socket2 `set_tclass_v6`, sys/unix.rs:2684 |
| IPV6_MTU_DISCOVER set | lso linux.rs:390; sticky (send.rs:316) | rustix `set_ipv6_mtu_discover`, net/sockopt.rs:830 |
| IPV6_DONTFRAG set | sticky (send.rs:321) | nix `sockopt::Ipv6DontFrag`, sockopt.rs:1277 |
| IPV6_TRANSPARENT, IPV6_FREEBIND set | sticky | socket2 `set_ip_transparent_v6` socket.rs:1855, `set_freebind_v6` sys/unix.rs:2371 |
| IPV6_RECVPKTINFO set | i, r, u lists; t :545, :1714 | nix `sockopt::Ipv6RecvPacketInfo`, sockopt.rs:1069 |
| IPV6_RECVERR set and get | lists; t :1234, :536 | nix `sockopt::Ipv6RecvErr`, sockopt.rs:1191 |
| IPV6_RECVORIGDSTADDR set | u list | nix `sockopt::Ipv6OrigDstAddr`, sockopt.rs:1256 |
| IPV6_RECVHOPLIMIT, IPV6_RECVTCLASS set | t :1225 (IPV6_RECORDS) | socket2 `set_recv_hoplimit_v6` socket.rs:2207, `set_recv_tclass_v6` :2140 |
| TCP_KEEPIDLE set and get | t :1241, :2304, :2305 | rustix `set_tcp_keepidle` net/sockopt.rs:1499, `tcp_keepidle` :1513 |
| TCP_KEEPINTVL set | t :1243 | rustix `set_tcp_keepintvl`, net/sockopt.rs:1530 |
| TCP_KEEPCNT set | t :1245 | rustix `set_tcp_keepcnt`, net/sockopt.rs:1468 |
| TCP_USER_TIMEOUT set | t :1272 | rustix `set_tcp_user_timeout`, net/sockopt.rs:1559 |
| TCP_FASTOPEN_CONNECT set | t :1825 | nix `sockopt::TcpFastOpenConnect`, sockopt.rs:807 |
| TCP_MAXSEG get | t state.rs:872 | socket2 `tcp_mss`, sys/unix.rs:1657 |
| TCP_REPAIR set (1, 0, -1) and get | t :1341, :2325, :2326 | nix `sockopt::TcpRepair` (`u32`; -1 as `u32::MAX`, same bits, INFERRED), sockopt.rs:759 |
| TCP_CONGESTION get | t state.rs:785 | socket2 `tcp_congestion`, sys/unix.rs:2701 |
| UDP_GRO set | u :850 | nix `sockopt::UdpGroSegment`, sockopt.rs:1142 |
| TCP_QUICKACK get (in part) | t state.rs:872 | socket2 `tcp_quickack`, sys/unix.rs:1827; the set at t :2282 writes 2, which socket2's and rustix's `bool` cannot: none |

No safe route ("none"), 80 options, 3 ioctl requests and the 2 partial cases. Every one belongs to one outside API,
**the Linux socket system calls `setsockopt(2)`/`getsockopt(2)`** at the level named (socket(7) for SOL_SOCKET, ip(7)
for IPPROTO_IP, ipv6(7) for IPPROTO_IPV6, tcp(7) for IPPROTO_TCP, udp(7) for IPPROTO_UDP, udplite(7) for
IPPROTO_UDPLITE, the MPTCP uapi for SOL_MPTCP), except the last row, which is **Linux `ioctl(2)` on a socket**:

| Level | Options (call sites) |
|---|---|
| SOL_SOCKET (13) | SO_RCVMARK, SO_RCVPRIORITY (r :375, u :438); SO_ATTACH_REUSEPORT_CBPF, SO_DETACH_REUSEPORT_BPF (lso :215, :217 from :301); SO_MAX_PACING_RATE (lso :236); SO_RCVLOWAT set and get (t :1190, state :872); SO_BUF_LOCK (t :1187); SO_TXREHASH (t :1207); SO_CNX_ADVICE (t :1735); SO_WIFI_STATUS (t :2051); SO_PEEK_OFF (t :2099, :2118); SO_MEMINFO get (exchange.rs:357, u :580); SO_NO_CHECK (sticky, u :668) |
| IPPROTO_IP (12) | IP_OPTIONS (lso :397, sticky send.rs:210, rr suite :1534); IP_UNICAST_IF (sticky send.rs:202, rr :1080, rr suite :1693); IP_LOCAL_PORT_RANGE (lso :771); IP_ROUTER_ALERT (r :283, :304); IP_NODEFRAG (r :476); IP_RECVOPTS, IP_RECVFRAGSIZE (i, r, u lists); IP_MINTTL (t :1212); IP_PKTOPTIONS get (t :1400, state :1225); MCAST_JOIN_GROUP, MCAST_LEAVE_GROUP (memberships.rs:60, 65, 88, 100; also at IPPROTO_IPV6); MCAST_MSFILTER (memberships.rs:128) |
| IPPROTO_IPV6 (22) | IPV6_HOPOPTS, IPV6_RTHDRDSTOPTS, IPV6_RTHDR, IPV6_DSTOPTS (lso :399-417, sticky send.rs:231, 328-338); IPV6_FLOWINFO_SEND (lso :427, send.rs:142, t :586); IPV6_AUTOFLOWLABEL (lso :428, send.rs:143); IPV6_FLOWLABEL_MGR (send.rs:458, t :585, :1695); IPV6_ADDR_PREFERENCES (lso :455, sticky send.rs:228, icmp suite :2364); IPV6_JOIN_ANYCAST, IPV6_LEAVE_ANYCAST (memberships.rs:35, 39); IPV6_MTU set (sticky send.rs:325; rustix has only the getter); IPV6_UNICAST_IF (sticky send.rs:226, rr :1080, rr suite :1693); IPV6_ROUTER_ALERT (r :291, :305); IPV6_ROUTER_ALERT_ISOLATE (r :285); IPV6_CHECKSUM (r :839); IPV6_RECVHOPOPTS, IPV6_RECVDSTOPTS, IPV6_RECVRTHDR, IPV6_FLOWINFO, IPV6_RECVFRAGSIZE (i, r, u lists; t :1225); IPV6_MINHOPCOUNT (t :1214); IPV6_2292PKTOPTIONS get (t :1407, state :1232) |
| IPPROTO_TCP (27) | TCP_MD5SIG_EXT (t :639); TCP_AO_ADD_KEY (t :782); TCP_AO_GET_KEYS (t :808, state :1069); TCP_AO_DEL_KEY (t :854); TCP_AO_INFO set and get (t :897, state :289); TCP_AO_REPAIR set and get (state :1030, :934); TCP_WINDOW_CLAMP set and get (t :1262, state :872); TCP_SYNCNT (t :1269); TCP_DEFER_ACCEPT (t :1279); TCP_LINGER2 (t :1286); TCP_RTO_MIN_US, TCP_RTO_MAX_MS, TCP_DELACK_MAX_US (t :1294-1300); TCP_TX_DELAY (t :1302); TCP_FASTOPEN_NO_COOKIE (t :1316); TCP_INQ (t :1318); TCP_QUEUE_SEQ set and get (t :1357, state :928); TCP_SAVED_SYN get (t :1371, :1377); TCP_FASTOPEN (t :1641); TCP_FASTOPEN_KEY (t :1652); TCP_SAVE_SYN (t :1666, :2405); TCP_REPAIR_QUEUE set and get (t :1349, :2328, :2336, state :925-932); TCP_TIMESTAMP set and get (t :2379-2384, state :908); TCP_INFO get (state :329, :597); TCP_CC_INFO get (state :791); TCP_REPAIR_WINDOW set and get (state :1016, :924); TCP_REPAIR_OPTIONS (state :1000) |
| SOL_MPTCP (2) | MPTCP_INFO, MPTCP_FULL_INFO get (state :1120, :1148) |
| IPPROTO_UDP, IPPROTO_UDPLITE (4) | UDP_NO_CHECK6_RX (u :856); UDP_NO_CHECK6_TX (sticky, u :672); UDPLITE_RECV_CSCOV (u :862); UDPLITE_SEND_CSCOV (sticky, u :677) |
| partial (2) | SO_TIMESTAMPING with a bound hardware clock (the 8-byte `so_timestamping`, lso :181); TCP_QUICKACK set to 2 (t :2282) |
| ioctl (3) | SIOCOUTQ, SIOCOUTQNSD, SIOCATMARK (t state.rs:565 from :843, :850); rustix has only FIONREAD (io/ioctl.rs:92), nix's ioctl macros and libc's `sockatmark` are unsafe |

Two "safe-looking" routes are counted as none on purpose (READ): nix's `AttachReusePortCbpf` (sockopt.rs:1312) takes
`libc::sock_fprog`, whose `filter` field is a raw pointer the kernel reads and nix does not check, so a safe caller can
make the kernel read any memory (the hole the d4aee0ae5 check found); and nix's exported `sockopt_impl!` macros
(sockopt.rs:46, 100, 180) expand to `unsafe` in the calling crate, which `forbid` refuses (INFERRED).
SO_MEMINFO's 2 sites and SO_RCVMARK/SO_RCVPRIORITY are reads and receive flags a guest's datagrams depend on, so
dropping any of these is a change a guest can observe (rule 6, INFERRED).

### What 785d3fa79 brings

Its typed arms in options.rs (31 in `set_int`, 10 in `get_int`, READ) are all routes above and apply to agents as
they are; its sticky change to set marks and priorities through `ensure_int` is needed for SO_MARK and SO_PRIORITY to
reach socket2. One defect to fix when bringing it (READ, INFERRED effect): `set_int` passes SO_MARK and SO_PRIORITY
through `unsigned(value)?`, while sticky gives it `mark.0.cast_signed()`, so a mark or priority of 2^31 or more becomes
negative and is refused with EINVAL instead of set; the bit cast `cast_unsigned` keeps the value. Not in it, new here:
IP_TTL, IPV6_UNICAST_HOPS, SO_MARK and SO_PRIORITY reads, SO_INCOMING_CPU set, TCP_REPAIR set, TCP_CONGESTION,
SO_TXTIME, SO_TIMESTAMPING without a clock, SO_ATTACH_FILTER/SO_DETACH_FILTER, SO_BINDTOIFINDEX. Not brought: every
call it sends to `crates/socket-options`, its `count_queued` and multipath reads, and its suite changes (all go to that
crate); its requests.rs mirrors are only useful to an API crate for the "none" options.

### The three scripts in crates (READ at agents and main)

| Script | What it does | Replacement |
|---|---|---|
| tcp host `run-conformance.sh` | builds the TCP tests, loads `tcp_dctcp tcp_vegas tcp_bbr vrf`, and runs each binary as root in a new network namespace after `ip link set lo up`, a local route for 127.0.0.7 with `features tcp_usec_ts`, an `fq` qdisc on lo, five sysctls and a VRF device | a Rust test setup in the TCP suite: per-test-thread `nix::sched::unshare` (as `own_network_namespace` already does), netlink messages for the link, route, qdisc and VRF (as `bring_loopback_up` already does), sysctls by `std::fs::write` to /proc/sys; root comes from the job's cargo runner, and the `modprobe` line moves to the machine's job in scratch (INFERRED: netlink-packet-route 0.34 can express the route feature, qdisc and VRF) |
| tcp host `mutants.sh` | a mutation driver: copies the checkout, applies 143 text patches one at a time, runs the suite on each, prints the checks that fail | not a test: moves to scratch (SCRATCH/host-sockets/tools/) and runs from a job there |
| udp host `conformance.sh` | builds the conformance, readiness and allocations tests and runs each binary with sudo | nothing in the crate: the tests already unshare per test (tests/conformance.rs:89), so one `cargo test -p wcontainers-udp-sockets-host` as root, with the runner set by the job |

### What HS1 can and cannot reach (INFERRED from the survey)

Every one of the 6 lines left at agents has no safe route: linux-socket-options' `set_raw` and `get_bytes` stay as
long as any "none" option does, the tcp host's ioctl serves three "none" requests, and the three suite calls set
"none" options. HS1 can route the 43 options above through socket2, rustix and nix, but under `forbid`
linux-socket-options still does not compile, and with it the tcp, udp, icmp, raw and route-resolver hosts. The
runtime's RT1 needs those hosts, so RT1 holds on owner question 1 even after HS1.

## HS1 and HS2 (2026-11-25): typed routes and the API crate (done by this track's measurement; independent check pending)

Commits on `host-sockets`: 5df67976f merges the owner's main (FETCH_HEAD e295234c7; no file of these crates changed in
it), 55fd7bafc brings the fourteen folders by path from agents 198bb11d6, 3caf049bc adds the API crate, 24d3b3b6a routes
every caller. The owner ruled the shape of every outside API on 2026-11-25, so HS2 no longer waits on question 1.

### The API crate: `crates/linux-socket-options` (package `wcontainers-linux-socket-options`)

Named for the one outside API, Linux's socket option and socket ioctl calls (setsockopt(2), getsockopt(2), ioctl(2) on a
socket). READ in the commit:
- `options.table`: 96 rows (method, level, option, value kind, manual page): the 80 options HS0 found with no safe
  route, split by direction and by IP level where Linux takes the option at both (for example `MCAST_JOIN_GROUP` at
  `IPPROTO_IP` and `IPPROTO_IPV6`), the 3 ioctl requests, and the two partial cases (`SO_TIMESTAMPING` with a bound
  clock, `TCP_QUICKACK` set to 2).
- `interface.rs`, the trait `LinuxSocketOptions`, one method per row, and `implementation/direct/generated.rs`, its
  implementation, are both written by `tests/generated.rs` from the table; that test fails when either file differs
  from what the table gives (MEASURED: passes on both machines), checks every method is listed once, and checks the
  suite calls every method. Typed arguments only: numbers, `bool`, byte strings the kernel reads whole, the listed uapi
  structures by reference, `&[libc::sock_filter]` for the one program option, `GroupFilter` for `MCAST_MSFILTER`.
  Reads of varying length give `Written<ROOM>` (the bytes and the length the kernel wrote) for a room the caller names
  as a constant type parameter, never a buffer it passes; `MPTCP_FULL_INFO` gives `MptcpFullInformation`, its subflow
  rooms allocated by the implementation from a count.
- `implementation/direct` (`DirectLinuxSocketOptions`): its own lint table `unsafe_code = "deny"`; **4 marked lines**,
  each `#[expect(unsafe_code, reason = "..")]` on the one statement (implementation.rs:118 `setsockopt` of plain values
  and of the program header, :229 `getsockopt` into a byte room, :338 `getsockopt` of `MPTCP_FULL_INFO` whose header
  names the answer's two runs, :365 `ioctl` of the three requests that write one int). A private `Plain` marker trait,
  implemented only by the generated list of uapi structures and by numbers, limits what the set reaches the kernel as.
  Hostile guards in safe code before the calls: `TCP_AO_GET_KEYS` asking for more than one answer is refused with
  EINVAL (the kernel writes one answer per key asked, past the length it is given), a program longer than 65535
  instructions is refused with EINVAL, a returned length is never more than the room.
- `conformance/suite.rs`: 13 checks, each in a network namespace of its own with its loopback up, every method called;
  hostile cases: rooms shorter than the value (8 bytes of `TCP_INFO` equal the first 8 of the whole; 1 byte of
  `SO_MEMINFO`), a room longer than the value (the rest stays zero), `TCP_SAVED_SYN` into one byte (EINVAL, headers kept
  and read whole after), an empty program, one of 4097 and one of 65536 instructions (EINVAL), 41 bytes of IPv4 options
  and an IPv6 header claiming more than given (EINVAL), a key query of 2 and of 2^32-1 answers (EINVAL), options on
  sockets of the wrong kind (UDP refusing `SO_TXREHASH` with EOPNOTSUPP, `IP_ROUTER_ALERT` with EINVAL, a TCP option
  on UDP with ENOPROTOOPT, the ioctls on a listener and a Unix socket), MPTCP full information into rooms of 0, 1 and 1
  byte. A closed descriptor cannot be given: `BorrowedFd` is open by construction. The kernel answers the suite
  records were measured on Linux 7.0.0-1018-aws (four expectations corrected to what it answers: `SO_TXREHASH` is TCP's,
  router alerts refuse `IPPROTO_RAW` and UDP sockets, `TCP_TIMESTAMP` is written only in repair mode, an empty
  `TCP_AO_INFO` write is taken).

### Callers (READ in 24d3b3b6a)

- linux-socket-options: new `options.rs` keeps `set_flag`, `set_int` and `get_int` by level and name, each arm a
  typed call of socket2, rustix, nix or the API crate, any other option ENOPROTOOPT; `set_option`, `set_bytes`,
  `get_bytes`, `get_option` and `set_raw` are gone, their 50-odd call sites now name the typed call. `sticky.rs` writes
  each slot through its typed call. `StorageBytes` is the API crate's `SocketStorage`.
- 785d3fa79's typed arms are taken with its SO_MARK/SO_PRIORITY defect fixed (marks and priorities go to socket2 as
  the same 32 bits). Found while routing: socket2's `attach_filter` casts the instruction count to `u16`, so 65537
  instructions would attach a one-instruction program; the shared crate refuses more than 65535 first, as before.
- The three suite `setsockopt`s and the tcp host's `ioctl` call the API crate (or socket2's `bind_device_by_index_v4/6`
  for `SO_BINDTOIFINDEX`); `FIONREAD` goes to rustix `ioctl_fionread`. The four `[lints.rust]` tables are now
  `[lints] workspace = true`; the root manifest names none of these crates.
- Scripts: `run-conformance.sh`, `mutants.sh` and `conformance.sh` are out of the crates, kept in
  SCRATCH/host-sockets/tools/. The TCP suite's namespace setup (route feature, fq qdisc, sysctls, VRF device) now lives
  in the job's cargo runner SCRATCH/host-sockets/hs1/tcp-namespace.sh; a Rust setup inside the TCP suite is still owed
  (INFERRED: netlink-packet-route can express it), so the TCP suite is not yet "one cargo command".

### HS1/HS2 bar, measured on both machines (rustc 1.99.0, Linux 7.0.0-1018-aws)

| | ARM64 | x86-64 |
|---|---|---|
| `cargo check --all-targets`, 17 packages | exit 0 | exit 0 |
| clippy `--all-targets -D warnings` | exit 0 | exit 0 |
| `cargo fmt --check` | exit 0 | exit 0 |
| TCP suite (as root, runner namespace) | 143/143 | 143/143 in 1 of 3 full runs, 142 in the other two; before the change 143 in 1 of 3, 142 and 139 in the others |
| UDP 80, ICMP 69, raw 54, route resolver 38 | all pass | all pass |
| API crate suite 13, generated test 3 | pass | pass |
| unit and other tests of the hosts (readiness, counters, waiting, allocations) | pass | pass |

x86-64's TCP misses are timing checks that fail at the commit before the change too (MEASURED: at 55fd7bafc,
`congestion_control_state_names_the_algorithm`, `information_reads_what_the_connection_did`,
`timestamp_reads_the_clock_in_its_unit` and `record_end_starts_a_new_segment` failed in one run or another; after,
`record_end_starts_a_new_segment` failed in two; each passes five times alone on both commits). No test removed or
ignored.

Same kernel calls before and after (MEASURED, ARM64): every test binary of the five suites run under `strace -f -e
trace=setsockopt,getsockopt,ioctl`, at 55fd7bafc and at the new code, one test thread: 35885 and 36002 calls, 520
distinct each (a set's arguments but the descriptor, a read's level and name, an ioctl's request, each with its
outcome). The diff holds only values that differ run to run (ports chosen, sequence numbers, timestamps, filter
addresses) and one change of form: `SO_TIMESTAMPING` without a bound clock is now 4 bytes of flags (nix
`sockopt::Timestamping`) instead of the 8-byte `struct so_timestamping` with `bind_phc` 0; Linux reads a value of any
other length as the flags with no clock bound (INFERRED from net/core/sock.c, read, not copied), and every outcome is
the same.

Speed (MEASURED, median of 4000 batches of 32, three runs each, same machine and load):

| | before (55fd7bafc) | after (24d3b3b6a) |
|---|---|---|
| ARM64 TCP send / receive | 390-392 / 381-383 ns | 389-390 / 381-383 ns |
| ARM64 UDP send / receive | 1.442-1.465 us / 790-806 ns | 1.453-1.461 us / 800-805 ns |
| x86-64 TCP send / receive | 341-357 / 365-383 ns | 307-358 / 337-369 ns |
| x86-64 UDP send / receive | 1.652-1.663 us / 879-881 ns | 1.444-1.646 us / 762-826 ns |

Linter (`cargo reviewable check`, ARM64, no model key so model questions unanswered and left out; x86-64 not run):
NOT at 0. Findings per folder: tcp host 89, udp host 73, icmp host 24, raw host 24, route-resolver host 14,
linux-socket-options 329, tcp suite 159, udp suite 272, icmp suite 240, raw suite 67, route-resolver suite 59,
configurable 4, readiness 43, negative-route-advisable 6 (all within a few of HS0's counts at agents); the new API
crate: direct 30 (public_outside_interface 17, summary_starts_with_name 9, trait_outside_interface 1 for the private
`Plain` marker, 3 more), suite 54, interface.rs 9 (interface_item 3 for the types beside the trait, comment budget 1,
5 more), tests 4. New and worth a ruling: depends_on_implementation and names_implementation in the icmp, raw and
route-resolver suites, which now depend on the API crate's direct implementation for their probes; the smallest fix is
for those suites to take a `LinuxSocketOptions` value from their caller.

Still owed for HS1/HS2: the linter findings above to 0 or each listed with its smallest example; the TCP suite's
namespace setup in Rust; x86-64 lint run; the independent check.

## HS1/HS2 repair (2026-11-26): what the kernel writes, as a class

The independent check of 1ea5c793e BLOCKED (READ: orchestrator log, check probe in SCRATCH/host-sockets-check2/ck/probe.rs):
`set_ipv6_flowlabel_mgr(&self, socket, value: &in6_flowlabel_req)` is safe, but for a get-and-create request with label
0 Linux writes the label it allocates back into the value, behind the shared reference, while the marked reason on the
`setsockopt` call said every value is only read. Repaired for every row, not for that method. Commits: 6fe895c92 merges
the owner's main (ac696ea19; it changed only `crates/executor/interface.rs` and `crates/kernel/interface.rs`, no file of
this track), 8ac0c13ce the class repair, d4179e4d1 its lock lines, aa2855180 a probe gap the planted faults found,
84b79700e the TCP namespace in the suite, 73a3e6da2 the linter repairs of the API crate.

### The kernel column and the signatures made from it (READ)

- `options.table` has a sixth column, `kernel`: `reads` (a set whose value the kernel only reads), `writes-back` (a set
  whose value it may write into), `within` (a get that writes at most the length it is told), `past` (a get that may
  write past it), `through` (a get that also writes at the addresses its value holds). The generator refuses a set
  marked as a get or the other way, and a `writes-back` row whose value is a number or flag.
- Values: 94 rows `reads` or `within`; `set_ipv6_flowlabel_mgr` `writes-back`; `get_tcp_ao_get_keys` `past` (one answer
  per key asked, which `get_keys` limits to one); `get_mptcp_full_info` `through`.
- The generator writes a `writes-back` value as `&mut T` and routes it to `set_value_written`, which hands the kernel a
  pointer from that exclusive borrow; every other set takes its value by shared reference or by copy (numbers and flags
  are copied into a local before the call). Today the one changed signature is
  `fn set_ipv6_flowlabel_mgr(&self, socket: BorrowedFd<'_>, value: &mut uapi::in6_flowlabel_req)`; its three callers
  (shared crate send.rs `lease_flow_label`, tcp host `hold_flow_label` and the reflection setting) now pass `&mut`.
- The four marked reasons are rewritten: the `setsockopt` one says each value is owned or borrowed for the call and
  comes from an exclusive borrow wherever the table says the kernel writes into it; the `getsockopt` one says the kernel
  writes only inside the buffer, at most the length told for every `within` row and that length per key for the one
  `past` row; the `MPTCP_FULL_INFO` and `ioctl` ones name their column. No reason says "only reads" any more (READ:
  grep). The private `Plain` marker's comment says the same.
- The suite now also checks the write-back through the safe method: a get-and-create request with label 0 succeeds and
  the request holds a label other than 0 afterwards.

### The probe: every row on the running kernel (MEASURED, both machines)

`implementation/direct/probe.rs`, a unit test of the direct crate run as root in a namespace of its own. It walks
`ROWS`, which the generator writes from the table into `generated.rs`, so a row added to the table is a row the probe
must call: it fails on a row without a case, and the generator test `the_probe_calls_every_method` fails too (it needs
no root). Each call puts the value between 1024 canary bytes on each side (patterns 0xa5 and 0x5a, so a byte written
equal to one shows through the other); a get's room is filled with canaries, and once with zeros; `MPTCP_FULL_INFO`'s
header and each run of rooms get a canary entry on each side; the reuse-port program's header and its instructions are
each guarded. After each call every byte is compared. A row fails when any call wrote before its value, when no call
succeeded, or when what it did contradicts its column (a `reads` set changed its value; a `writes-back` set was never
seen writing into it; a `within` get wrote past the length told or answered a longer one; a `past` get was never seen
writing past; a `through` get wrote outside its header and rooms or never into them; any row but `through` wrote at an
address its value holds). The calls go through the same private functions the methods use, with the same level,
option and length.

Result on Linux 7.0.0-1018-aws, ARM64 and x86-64 (same counts, `probe exit 0 rows 96`): every set left its value as
given except `IPV6_FLOWLABEL_MGR`, which wrote into it in 2 of its 8 calls (the get-and-create ones); no call wrote
before or past its value; `TCP_AO_GET_KEYS` with two keys asked wrote the second answer past the room in 2 of 6 calls;
`MPTCP_FULL_INFO` wrote into its rooms in 4 of 8 calls and nowhere else; every get answered a length no longer than it
was told (shorter in 3 to 9 of 12 calls for the variable ones). Per-row lines (`PROBE <method> <column> calls ok inside
past before shorter longer through errors`) are in SCRATCH/host-sockets/hs3/arm-tcp*.log and the bar logs.

Planted faults (MEASURED, ARM64, SCRATCH/host-sockets/hs3/plant.sh; each regenerated, then the probe run):

| Planted in the table | Caught by |
|---|---|
| `IPV6_FLOWLABEL_MGR` marked `reads` | generator check, and the probe: "wrote into a value its column says it only reads" |
| `TCP_AO_GET_KEYS` marked `within` | generator check, and the probe: "wrote past the length it was told" |
| `MPTCP_FULL_INFO` marked `within` | generator check; the probe missed it in the first round, fixed in aa2855180, then caught |
| `IPV6_JOIN_ANYCAST` marked `writes-back` | generator check, and the build: the suite's `&` no longer fits the `&mut` signature |
| `SO_RCVMARK` marked `writes-back` | the generator refuses the row (a flag cannot be written back) |
| a new row `get_tcp_maxseg` with no case | `the_probe_calls_every_method`, `the_suite_calls_every_method`, and the probe: "rows the probe does not call" |

### HS1's leftovers

- TCP suite namespace (84b79700e): `prepared_network()` in the suite makes each check's namespace with nix's
  `unshare` and netlink messages (loopback up, the local route for 127.0.0.7 with `RTAX_FEATURE_TCP_USEC_TS`, the `fq`
  queueing discipline on loopback, the VRF device with table 10 brought up) and writes the four namespace settings
  under /proc/sys; every test of `tests!` (and of the nested configurable, negative-route and readiness macros) calls
  it first. The suite's /proc tables are read from `/proc/thread-self/net/` (three checks failed with `/proc/net`,
  which is the first thread's namespace). The job runner is now `sudo` alone; `tcp-namespace.sh` is no longer used.
  The modules `tcp_dctcp tcp_vegas tcp_bbr vrf` are still loaded by the job (`modprobe`): loading a module is the
  host's, not a namespace's. MEASURED: 143/143 on ARM64 in 3 runs and x86-64 in 4 runs (the x86-64 timing misses of
  1ea5c793e did not recur). Still owed elsewhere: the ICMP suite runs `ip` and `tc` from iproute2 through `Command`
  (READ suite.rs:296), so the job must put iproute2 on PATH; not changed in this run.
- Linter, rules that need no model (`cargo reviewable check` at the linter's main da55b9d7, ARM64, no key; model rules
  `grammar_unclear`, `summary_unclear`, `*_unanswered`, `verb_not_first`, `function_summary_not_third_person`,
  `type_summary_not_noun_phrase`, `summary_restates_declaration`, `hook_without_participle` left out). MEASURED in the
  track's folders, before (84b79700e) -> after (a run at the API repair, one fix later not rerun):

| Rule | Before | After |
|---|---|---|
| public_outside_interface | 459 | 442 |
| summary_not_one_line | 76 | 70 |
| conformance_not_run | 70 | 56 |
| number_constant_without_link | 69 | 63 (62 INFERRED with `TWO_ANSWERS`' link) |
| magic_literal | 59 | 52 |
| summary_too_long | 46 | 42 |
| summary_starts_with_name | 23 | 10 |
| constant_defined_twice | 19 | 18 |
| implementation_folder | 16 | 16 |
| comment_too_long | 11 | 11 |
| summary_not_one_sentence | 7 | 7 |
| depends_on_implementation, names_implementation, trait_outside_interface | 4, 3, 3 | 4, 3, 3 |
| interface_item | 3 | 2 |
| stray_source | 1 | 0 |
| literal_restates_constant, trait_not_implemented, comment_budget_exceeded, hook_name | 1 each | 1 each |
| total | 847 (corrected 2026-11-26: the independent check measured 847 at the run's start; 873 was wrong, and the rows above it come from that wrong list) | 803 (the check: 802) |

  No exclusion added. What is left, by cause, each with its smallest example:
  - Structural, a ruling needed (not changed): `public_outside_interface` 200 in the shared crate
    `internet-protocol/implementation/linux-socket-options`, whose public functions the four hosts call (for example
    `pub fn storage_bytes` in datagram/addresses.rs), and 146 in the ICMP suite, whose helpers the raw and route
    suites import; `implementation_folder` 16, the hosts implementing `Configurable`, `Readiness` and
    `NegativeRouteAdvisable` of other interfaces (for example tcp host portable.rs:741), which the owner's interfaces
    ask of them; `depends_on_implementation` 4 and `names_implementation` 3, the ICMP and route suites setting
    `IPV6_ADDR_PREFERENCES`, `IP_OPTIONS` and `IP_UNICAST_IF` on their own probe sockets through
    `DirectLinuxSocketOptions`, the only safe way (the rule has no case for an outside API's direct crate);
    `trait_outside_interface` for the direct crate's private `Plain` marker and the two suites' `LentBuffers`.
  - Rule against rule (not changed): the probe names the suite (`conformance_not_run`) to reuse its `own_network` and
    `storage_of`, which therefore are public (`public_outside_interface` 2); copying them instead makes
    `constant_defined_twice`. The probe's `LARGE_ROOM`, `SEQUENCE`, `SEGMENT_SIZE` and `ONE_ADDRESS_V4` equal constants
    of crates it may not name.
  - Linter fault (READ, smallest example): `interface_item` at interface.rs:71 says the compiler's view of
    `fn get_so_meminfo<const ROOM: usize>(&self, socket: BorrowedFd<'_>) -> Result<Written<ROOM>, HostError>` differs
    from the text; it prints the const parameter as `<ROOM>()`. And `interface_item` at :29 counts the zerocopy derives
    on `SocketStorage` as macro-written impl blocks. `magic_literal` 1 is `#[repr(C, align(8))]`, where Rust takes
    only a literal.
  - `comment_budget_exceeded` in the generated interface.rs: 4506 characters for 253 lines (3842 allowed), down from
    8626; one sentence per method of 96 is still over.
  - Mechanical, not reached in this run: summary length and lines (112), named numbers without a link (62), magic
    literals (51), long comments (11) in the hosts and the TCP, UDP, ICMP and readiness suites.

### The bar at the repair, both machines (MEASURED: SCRATCH/host-sockets/hs3/{arm,x86}-bar3.log)

| | ARM64 | x86-64 |
|---|---|---|
| `cargo check --all-targets`, 17 packages | exit 0 | exit 0 |
| clippy `--all-targets -D warnings` | exit 0 | exit 0 |
| `cargo fmt --check` | exit 0 | exit 0 |
| TCP 143, UDP 80, ICMP 69, raw 54, route resolver 38 | all pass | all pass |
| API suite 13, generator tests 4, probe 1 (96 rows) | pass | pass |
| hosts' other tests (unit 2, readiness 3+1, counters 5+2+1, waiting 1+1) | pass | pass |
| ignored | only the two speed tests, run apart | same |

Speed, both commits run the same way now (as root in the host's namespace, median of 4000 batches of 32, three runs):

| | 1ea5c793e | 73a3e6da2 |
|---|---|---|
| ARM64 TCP send / receive | 395-400 / 400-407 ns | 398-401 / 402-406 ns |
| ARM64 UDP send / receive | 1.471-1.516 us / 808-812 ns | 1.493-1.498 us / 807-813 ns |
| x86-64 TCP send / receive | 309-335 / 363-394 ns | 317-321 / 353-382 ns |
| x86-64 UDP send / receive | 1.431-1.464 us / 760-778 ns | 1.445-1.521 us / 765-804 ns |

(Earlier figures in this file were measured inside the runner's namespace with `fq` on loopback, so they differ from
these by the environment, not the code.) One rule slip to report: the x86-64 jobs before 07:15 ran with x86-run's
default of 96 build jobs instead of 12; every later one passed `--jobs 12`.

### For the runtime track

Safe to merge: 73a3e6da2 (code), or the status commit after it, which changes only this file. Everything the runtime's socket hosts need
builds and passes there on both machines; the only interface change for a caller is `set_ipv6_flowlabel_mgr` taking
`&mut`.

## Repair of the guard for future rows (2026-11-26, after the third check BLOCKED)

The third check (SCRATCH/host-sockets-check3/) planted `get_tcp_ao_keys_room room past` and nothing refused it. Commits:
52a7b3778 (generator, probe, canaries), b599e5b85 (linter repairs by hand). The owner's main (ac696ea19) was already
merged (MEASURED: `git merge-base --is-ancestor FETCH_HEAD HEAD`). No interface file of the owner changed; the API
crate's options.table and interface.rs did not change either (MEASURED: regenerated interface.rs identical).

### (1) The generator refuses every pairing it cannot make safe (READ: tests/generated.rs `refusal`)

- Form and column: sets take `reads` only, except a `struct:` value, which may be `writes-back` (exclusive borrow);
  gets of a number, `room` or `ioctl` take `within` only; `keys` takes `past` only; `full_info` takes `through`
  only. Bytes and runs lost `writes-back` (no option writes them back and the probe has no exclusive form for them).
- Form and option, both ways: `program` only for `SO_ATTACH_FILTER`/`SO_ATTACH_REUSEPORT_CBPF`, `group_filter` only
  for `MCAST_MSFILTER`, `keys` only for `TCP_AO_GET_KEYS`, `full_info` only for `MPTCP_FULL_INFO`, `ioctl` only for
  `SIOCOUTQ`, `SIOCOUTQNSD`, `SIOCATMARK` (each writes one int); and none of those options in any other form.
- Level: an `ioctl` row has level `-`, and only it. `struct:`/`slice:` name only the 11 uapi structures known to hold
  no address the kernel follows; `bytes` only for the 6 options that read an opaque run.
- Tests (MEASURED, both machines, `--test generated`: 10 passed): the full matrix of 2 directions x 13 forms x 5
  columns (130 pairs: 17 taken, 113 refused, each checked), the check's planted row, levels, bound options, unknown
  structures and byte options, and every table row taken.

### (2) The probe calls the generated trait methods (READ: implementation/direct/probe.rs)

Every case is written `row!(method, builder(...))`, which expands to a call of `OPTIONS.method(...)` through the
trait, so a changed signature fails to build. Lent memory sits between canaries: structures in a `#[repr(C)]` fence,
compared after the call by reading their bytes with nix's `process_vm_readv` on the probe's own process (linux-raw-sys
structures give no byte view; read only); byte strings and runs likewise. Memory the method owns carries the
implementation's canaries (4); a canary stop is caught (`catch_unwind`) and fails the row, and a socket that cannot be
made fails its row without stopping the others. `past` is now measured as: every key query asking more answers than
the room holds is refused (4 of 4 refused, none taken).

### (3) The four `within` rows the kernel was never seen writing (MEASURED, both machines, probe `written` count)

| Row | Case that makes the kernel write | Written |
|---|---|---|
| get_ip_pktoptions | connected TCP pair with `IP_PKTINFO` on (nix), the kernel writes an `IP_PKTINFO` record | 1 of 4 rooms (the large one; the others are shorter than one record) |
| get_ipv6_2292pktoptions | connected TCP pair with `IPV6_RECVPKTINFO` on (nix) | 1 of 4 |
| get_tcp_cc_info | connected pair running Vegas (socket2 `set_tcp_congestion`), which fills `tcpvegas_info` | 3 of 4 (all rooms but the empty one) |
| get_tcp_ao_info | socket with two TCP-AO keys; the old probe filled the room with canaries, which the kernel reads as nonzero reserved fields (EINVAL), and the zero room showed zeros over zeros | 4 of 5 (the empty room is EINVAL) |

### (4) The canary that catches a getsockopt told more than its room (READ: implementation.rs)

Every room, number copy and header the implementation hands the kernel is followed by 64 canary bytes (0xa5, 0xa6, ...,
none zero), checked after each call; a changed one stops the program (`assert!`), as a stack protector does. Covered:
`get_value`, `get_written`, `get_keys` (a whole second answer's worth plus the guard), `ioctl_int`, the
`MPTCP_FULL_INFO` header and one extra room per run, `set_number` (also checks the value unchanged), `set_program`'s
header and `set_group_filter`'s bytes. Speed with them (MEASURED, same job, alternating, 3 rounds): ARM64 TCP 399-400 /
406-409 ns before vs 399-400 / 406-407 after, UDP 1.482-1.503 us / 815-819 ns vs 1.486-1.491 us / 813-814 ns; x86-64
noisy (other jobs): TCP send 361-367 vs 371-395 ns, UDP 1.698-2.051 us vs 1.686-2.045 us.

Planted faults (MEASURED, ARM64, SCRATCH/host-sockets/hs4/plant.sh and api2.sh, arm-plant.log, arm-api2.log):

| Planted | Caught by |
|---|---|
| IPV6_FLOWLABEL_MGR marked `reads` | probe: "wrote into a value its column says it only reads" (inside 2 of 8) |
| the check's row `get_tcp_ao_keys_room room past` | generator: "a room value cannot be past" |
| the same row marked `within` | generator: "the keys form carries only TCP_AO_GET_KEYS" |
| SIOCATMARK `past`; TCP_INFO `through`; SO_RCVMARK `writes-back` | generator |
| `struct:libc::sock_fprog`; a new ioctl `SIOCGSTAMP`; the program option as `bytes` | generator |
| the generator gives the opposite reference | the build (28 errors in the suite) |
| a probe case removed | `the_probe_calls_every_method` and the probe |
| getsockopt told 8 bytes more than the room (caught by accident before) | the canaries: get_so_meminfo, get_ip_pktoptions, get_ipv6_2292pktoptions and more stop with "canary changed" |
| full information told one entry more than its rooms | the canary of the extra room |
| get_keys letting two answers through | the canaries ("wrote past the 304 bytes it was told"), and the API suite |

### The bar at b599e5b85, both machines (MEASURED: hs4/{arm,x86}-bar2.log)

check --all-targets, clippy `-D warnings`, fmt --check of the 17 packages: exit 0. Suites: TCP 143, UDP 80, ICMP 69, raw
54, route resolver 38, API suite 13, generator tests 10, probe 1 (96 rows), the hosts' other tests: all pass; only the
two speed tests ignored. Speed in the bar: ARM64 TCP 401-402 / 403-404 ns, UDP 1.473-1.488 us / 803-805 ns; x86-64 TCP
340-363 / 384-422 ns, UDP 1.661-1.725 us / 873-899 ns.

### Linter, rules that need no model (MEASURED, ARM64, linter da55b9d7, hs4/arm-lint-before.log and -final.log)

| Rule | 52a7b3778 (before the hand repairs) | b599e5b85 |
|---|---|---|
| public_outside_interface | 442 | 453 (+11: the suite's constants the probe now imports) |
| summary_not_one_line | 74 | 5 |
| number_constant_without_link | 64 | 0 |
| conformance_not_run | 56 | 56 |
| magic_literal | 53 | 34 |
| summary_too_long | 45 | 2 |
| constant_defined_twice | 27 | 18 |
| implementation_folder | 16 | 16 |
| comment_too_long | 11 | 10 |
| summary_starts_with_name | 10 | 0 |
| summary_not_one_sentence | 7 | 0 |
| depends_on_implementation, names_implementation, trait_outside_interface | 4, 3, 3 | 4, 3, 3 |
| interface_item | 2 | 2 |
| trait_not_implemented, literal_restates_constant, hook_name, comment_budget_exceeded | 1 each | 1 each |
| total | 821 | 610 |

No exclusion, no rule loosened. Left of the mechanical kinds: the module comments of the TCP and UDP suites (1240 and
1786 characters, a list of the network each check needs), 6 more long comments, the summaries of 5 module docs, and
string literals given to `ask_routing` in the TCP suite.

### The two linter faults (SCRATCH/host-sockets/linter-faults/)

`const-generic-method-view/` and `zerocopy-derive-impls/` hold the smallest crates of each shape and a NOTE. MEASURED:
neither reproduces at da55b9d7 yet (the linter gives only model-unanswered findings and a magic literal there), while
crates/linux-socket-options/interface.rs:71 and :29 still give them. Not worked around.

### Questions for the orchestrator (each tried with the structure the rule asks first)

1. `public_outside_interface` in `crates/internet-protocol/implementation/linux-socket-options` (the shared crate the
   four hosts call, for example `pub fn storage_bytes(address: SocketAddr) -> StorageBytes` in datagram/addresses.rs:62).
   It is an implementation shared by hosts of four interfaces, not a suite helper, so the conformance-folder structure
   does not apply; the rule allows a trait only for the brief's three reasons. Which structure should it take?
2. `public_outside_interface` for the ICMP suite's helpers the raw and route suites import (for example
   `pub fn ip(commands: &str)`, `pub const NETWORK: &str`). The interface whose conformance folder they belong in is
   `crates/internet-protocol`, which has no conformance crate; making one is a new crate (a stop for the orchestrator).
   Tried in small: the probe now imports 11 constants from `crates/linux-socket-options/conformance` instead of
   repeating them, and that conformance crate's `pub const`s are each counted `public_outside_interface` (+11, -9
   `constant_defined_twice`). Should shared helpers in a conformance crate be `pub`, and may the orchestrator create
   `crates/internet-protocol/conformance`?
3. `implementation_folder` 16: the hosts implement other interfaces' traits, for example
   `impl<'a> Configurable<HostTcpConnection, TcpSetting<'a>> for HostTcpSockets` (tcp host portable.rs:741) and
   `impl NegativeRouteAdvisable<HostTcpConnection> for HostTcpSockets` (:768). The layout `cargo reviewable new
   foreign-implementation` makes lives inside the other interface's crate and adds a `mod` to its interface.rs (READ:
   syntax/foreign.rs, examples/interface_item), an owner's file, and is for a type from outside the workspace. Not done.
   Where should these impls go?
4. `depends_on_implementation` 4 / `names_implementation` 3: the ICMP and route suites set `IPV6_ADDR_PREFERENCES`,
   `IP_OPTIONS`, `IP_UNICAST_IF` on their own probe sockets through
   `wcontainers_linux_socket_options_direct::DirectLinuxSocketOptions` (icmp suite.rs:2368, route suite.rs:1533, :1661).
   The structure the rule asks (the suite takes a `LinuxSocketOptions` value from its caller) changes those suites'
   `tests!` macros and every host's conformance_run; not done this run. Go ahead?
5. `trait_outside_interface` 3: the direct crate's private `trait Plain {}` (the marker that limits what a set hands the
   kernel) and `pub trait LentBuffers: IcmpSockets` / `: RawSockets` in the two suites. Keep, or how?
6. `conformance_not_run` 56 and the probe: the probe is a unit test of the direct crate and uses the API suite's
   `own_network`, `storage_of` and now its constants; is a suite crate the right home for what both use?

### Not done this run

- The ICMP suite's `ip` and `tc` through `Command` (READ: about 195 call sites over the ICMP, raw and route suites, with
  links, addresses, routes, neighbours, rules by mark, user, protocol and port, prio, fq and etf queueing, and JSON reads
  of classes, neighbours and temporary addresses) is still iproute2; the netlink rewrite is next.

## Round 4 (2026-11-26): the guard keyed by what the kernel sees, the probe repaired, question 4, netlink groups 1-4

The fourth check (SCRATCH/host-sockets-check4/, READ: its logs and planted rows) did not hold: the generator matched a bound
option by its bare name, so `SO_GET_FILTER` (26, the number of `SO_ATTACH_FILTER`) as `slice:tcp_repair_opt` and
`TCP_CC_INFO` named at `SOL_SOCKET` were taken and generated as safe methods, every guard green, and the marked reason on
`set_bytes_at` was false. Commits: 8ba636218 (guard, reasons, hostile rows, probe, huge count), 566f15438 (question 4),
ffb5fa31d (linter repairs of the new code), 58d6079df, a2bd36ec2, 580addb83, 6ee323c7d (netlink groups 1-4); pushed. The owner's main
(FETCH_HEAD ac696ea19) was already merged (MEASURED: `git merge-base --is-ancestor FETCH_HEAD HEAD`); no interface file of
the owner changed.

### (1) The kernel table, keyed by (level number, option number) (READ: implementation/direct/tests/generated/kernel.rs)

- `LEVELS`: the 7 levels a row may name, each with the names of its own number space (91 names); `REQUESTS`: the 4 ioctl
  requests. Every number comes from the constant of that same path (`name!(uapi::SO_GET_FILTER)` is the path and
  `uapi::SO_GET_FILTER`), from linux-raw-sys, libc, or `tests/generated/local.rs`, which the direct crate compiles too
  (`#[path]`), so a table name means one number in both; none is typed in the table.
- `ENTRIES`: 91 entries keyed by the pair of numbers, each saying for a set and for a get what the kernel does with the
  value (`ReadsNumber`, `ReadsBytes`, `ReadsStructure(T)`, `ReadsRun(T)`, `ReadsGroupFilter`, `WritesBack(T)`,
  `FollowsProgram`, `Follows`, `WritesWithin`, `WritesPerKey`, `WritesPast`, `WritesThroughFullInfo`, `WritesInt`,
  `WritesStructure(T)`, or `Unknown`), with the manual page and kernel handler it was read from. Complete for every pair
  the 96 rows use; a direction no row uses is `Unknown`. Known-dangerous pairs are entries too: `(SOL_SOCKET, 26)` set
  `FollowsProgram`, get `WritesPast` (sk_get_filter counts the length in instructions); `(SOL_TCP, TCP_ZEROCOPY_RECEIVE)`
  get `Follows`; `SIOCGSTAMP_OLD` writes a 16-byte `__kernel_old_timeval`.
- The generator resolves a row's level text in `LEVELS`, its option text only among that level's names, and then the
  entry by the two numbers. A row is refused when a name does not resolve, when there is no entry, when the direction is
  `Unknown`, `Follows`, `WritesPast` or `WritesStructure`, when its kernel column is not the one the entry implies, or
  when its form does not fit what the kernel does (a `struct:T` only where the kernel reads exactly `T`, a `program` only
  where it follows a program, a `room` only where it writes within the length told, ...). The method must be an
  identifier. Bare-name matching (`BOUND_FORMS`, `BYTE_OPTIONS`) is gone. Two names with one pair are one entry
  (`SO_GET_FILTER` and `SO_ATTACH_FILTER`, `SO_TIMESTAMPING` and `SO_TIMESTAMPING_OLD`: the test checks they resolve to
  the same entry).

### (2) A property per marked reason, each with a planted row (READ: tests/generated.rs `REASONS`)

| Marked reason | Property over the generated code and the entry | Planted rows that break it (each also refused) |
|---|---|---|
| `set_bytes_at` | the set is routed to the private function matching what the kernel does (`set_value` only for `ReadsStructure(T)`/`ReadsRun(T)`/`ReadsBytes` of the row's own type, `set_value_written` with `&mut` only for `WritesBack(T)`, `set_program` only for `FollowsProgram`, ...) | `IPV6_FLOWLABEL_MGR` marked `reads`; `SO_GET_FILTER` as `slice:tcp_repair_opt`; `TCP_REPAIR_OPTIONS` as `struct:group_req` |
| `get_within` | `get_value`/`get_written` only where the kernel `WritesWithin`, `get_keys` only where `WritesPerKey` | a room for `TCP_AO_GET_KEYS`, for `SO_GET_FILTER`, for `TCP_ZEROCOPY_RECEIVE` |
| `full_info_into` | `get_full_info` exactly where the kernel `WritesThroughFullInfo` | `full_info` on `MPTCP_INFO`; `MPTCP_FULL_INFO` as a room |
| `ioctl_within` | `ioctl_int` exactly for a request whose kernel `WritesInt`, with no level | `SIOCGSTAMP_OLD` as `ioctl` |
| every reason's "the descriptor is borrowed" | the method is an identifier, so the generator writes its signature (`socket: BorrowedFd<'_>`) | a method text that writes a method of its own taking `fd: i32` |

`set_bytes_at` now takes the lent value itself (`Lent::Shared(&T)` or `Lent::Exclusive(&mut T)`) and computes the address
and length from it inside, so "the kernel is told only memory the caller lent" holds by construction, not by each caller.
The four marked reasons were rewritten to name the pair-keyed table and the property test each rests on.
MEASURED (both machines, `--test generated`): 12 passed, among them `every_row_of_the_table_meets_every_marked_reason` and
`a_planted_row_breaking_each_marked_reason_is_seen_by_its_property_and_refused`.

### (3) Thirty hostile rows (MEASURED, both machines: `every_hostile_row_is_refused`, the reasons printed in hs5/arm-api3.log)

Taken: 0 of 30. By kind: SO_GET_FILTER/SO_ATTACH_FILTER's number 26 under four forms (slice, room, c_int count, bytes); a
name of another level's space (TCP_CC_INFO at SOL_SOCKET, MCAST_JOIN_GROUP at SOL_TCP where 42 is TCP_AO_REPAIR,
SO_ATTACH_FILTER at SOL_TCP, SIOCOUTQ as a SOL_TCP option); the right pair given another structure (TCP_AO_REPAIR as
`group_req`, TCP_REPAIR_OPTIONS as one value); a structure holding an address (`libc::sock_fprog`, `libc::iovec`,
TCP_ZEROCOPY_RECEIVE, MPTCP_FULL_INFO as a room); a count the kernel trusts (TCP_AO_GET_KEYS as a room `past` and
`within`, `full_info` on MPTCP_INFO, `keys` on TCP_SAVED_SYN); write-back lent shared or on a number or bytes; an option
that changes how a later one reads (TCP_ULP, and the SOL_TLS level it opens); an aliasing ioctl (SIOCGSTAMP_OLD, SIOCOUTQ
given a level, SO_RCVMARK as a request); a literal number, a `libc::` path, code in the method name, a room marked `past`.

With the kernel table itself made wrong in a planted copy (hs5/plant.sh K3, K4), `every_hostile_row_is_refused` and the
planted-reason test fail too (MEASURED), so a wrong entry is seen by more than the probe.

### (4) The probe: why it was weak, and the repair (READ: check4 logs; implementation/direct/probe.rs)

Why (READ, check4 arm-plant2.log): canaries see only what the kernel writes. P9 (a set whose kernel follows an address in
the value) passed with 0 faults because the kernel only read through it; P11 (a `within` room for SO_GET_FILTER on a socket
with no filter) passed because the kernel had nothing to write, while the same row on a filtered socket overflowed (P12
canary stop, P13 SIGSEGV): the verdict depended on the case writer choosing a socket state where the kernel acts.

Repair:
- Every value a caller lends (structures, exclusive structures, runs, byte strings, key queries) is also lent twice with
  every word set to an address no process maps (1 and 16, below `vm.mmap_min_addr`, 32768 on ARM64, MEASURED); a kernel that
  follows an address answers EFAULT, and any EFAULT fails the row. The words are written into a box of the probe's own
  through `process_vm_writev` on its own process (nix, safe), as structures give no byte view.
- A `within` room row must be seen given less room than the kernel had to write (a room above zero and below the longest
  answer, answered whole or refused); `get_ip_pktoptions` and `get_ipv6_2292pktoptions` gained a 20-byte room, longer than a
  control message header and shorter than the message, which the kernel fills and cuts.
- Result at the tip (MEASURED both machines): probe exit 0, 96 rows, every room row `pressed true`, no EFAULT.

Planted (MEASURED, ARM64, hs5/plant.sh, arm-plant.log), the generator bypassed or the kernel table made wrong so that the
probe alone must catch it:

| Planted | Caught by the probe |
|---|---|
| K1: check's P9 row (generator bypassed), the check's real program pointer as its value | "was answered EFAULT: the kernel followed an address the value holds" (poisoned 2, followed 2) |
| K2: check's P11 row (bypassed), socket without a filter | "was never given less room than the kernel had to write" |
| K3: kernel table says SO_ATTACH_FILTER's set reads a run, P9 row | EFAULT, as K1 (and the generator tests fail) |
| K4: kernel table says SO_GET_FILTER's get is within, P11 row | "never given less room" (and the generator tests fail) |
| K5: K4 on a 4-instruction filtered socket | the implementation's canary: "wrote past the 8 bytes it was told for option 26" |
| K6 (control): K3 with the poisoned calls removed | not caught (probe exit 0): the poisoned calls are what catch K1/K3 |

Still not seen by the probe: a kernel reading past the length it is told (no safe way to place a value against unmapped
memory); that length is now the lent value's own size by construction in `set_bytes_at`.

### (5) `get_full_info` with a huge count (READ: implementation.rs; MEASURED: API suite, both machines)

The count must fit the kernel's 32 bits (else EINVAL) and the rooms are reserved with `try_reserve_exact` (ENOMEM instead
of an abort). The API suite now asks `usize::MAX` subflows (EINVAL) and `u32::MAX` subflows of mebibyte rooms (ENOMEM):
13 passed on both machines.

### Question 4 (ruled GO): the suites take a `LinuxSocketOptions` from their caller

The ICMP suite's `host_source`, `sources_by_preference` and `source_preferences_are_taken`, the raw suite's
`ipv6_source_preferences_choose_the_source` and the route suite's three checks that set options take an `O:
LinuxSocketOptions`; the three `tests!` macros take `($value, $options)`; the suites depend on the interface crate only;
the three hosts pass `wcontainers_linux_socket_options_direct::DirectLinuxSocketOptions` (dev-dependency). MEASURED: the
suites' `depends_on_implementation` 4 and `names_implementation` 3 are gone, but the linter now gives `conformance_not_run`
6 in the hosts' conformance_run.rs ("the file must be exactly one invocation ... `tests!(<value>);`", "it names another
implementation, `wcontainers_linux_socket_options_direct`"). Rule against rule, smallest example:
`wcontainers_raw_sockets_conformance::tests!(HostRawSockets::new().expect(".."), DirectLinuxSocketOptions);` Written here
for the orchestrator; nothing loosened.

### The bar at 566f15438, both machines (MEASURED: hs5/{arm,x86}-bar.log; `--jobs 12` on both)

check --all-targets, clippy `-D warnings`, fmt --check of the 17 packages: exit 0. Suites: TCP 143, UDP 80, ICMP 69, raw 54,
route resolver 38, API suite 13, generator tests 12, probe 1 (96 rows), hosts' other tests: all pass; only the two speed
tests ignored. Speed: ARM64 TCP 400-401 / 409-410 ns, UDP 1.524-1.545 us / 808-810 ns; x86-64 TCP 310-313 / 340-353 ns, UDP
1.439-1.709 us / 759-794 ns (b599e5b85's bar: ARM64 401-402 / 403-404 ns, 1.473-1.488 us / 803-805 ns).

### The bar at f7227d94e (code of 6ee323c7d), both machines (MEASURED: hs5/{arm,x86}-bar-tip.log; `--jobs 12`)

check --all-targets, clippy `-D warnings`, fmt --check of the 17 packages: exit 0. TCP 143, UDP 80, ICMP 69, raw 54, route
resolver 38, API suite 13, generator tests 12, probe 1 (96 rows), the hosts' other tests: all pass; only the two speed tests
ignored. Speed: ARM64 TCP 401 / 405-406 ns, UDP 1.515-1.525 us / 799-801 ns; x86-64 TCP 339-356 / 382-419 ns, UDP
1.551-1.644 us / 820-871 ns.

### Netlink instead of `ip` and `tc` (groups 1-4 done, each measured on both machines before the next)

`crates/icmp-sockets/conformance/netlink.rs`: `batch` (which every `ip` call of the three suites goes through) now sends
each line of a kind done there as the one request iproute2 sends for it, through netlink-packet-route 0.34.0 and
netlink-sys 0.9.0 (MIT, already in the lock for the TCP suite), and checks the acknowledgement; other lines still go to
iproute2, in order. Defaults follow iproute2's (address scope, route protocol `boot`, main table, link scope without a
gateway, RTA_TABLE past 255, IFA_FLAGS for flags past a byte, permanent neighbours, rule action and table). Group 1: `link
add` (dummy, veth), `link set` (up, mtu, arp on, multicast on, addrgenmode none), `address add|replace` (broadcast, nodad,
mngtmpaddr). Group 2: `route add` (via, via inet6, dev, table, metric, mtu [lock], hoplimit, prohibit, blackhole,
unreachable, encap seg6). Group 3: `rule add` (fwmark, uidrange, ipproto, dport, sport, from, oif, dscp, flowlabel,
lookup, prohibit) and `neigh add`. Group 4, the reads: `settle_addresses`' poll of the local table, a neighbour's
confirmed age (ticks divided by `rustix::param::clock_ticks_per_second`, as `ip -s neigh` divides), a link's temporary
address and its link-layer address by netlink dumps; a link's multicast groups from `/proc/thread-self/net/igmp` and
`igmp6`, which is where `ip maddr show` reads them (`joined_groups`, `link_address`, now used by the raw suite too). The
words of `ip` are named constants (`word::*`).

Measured with a logging `ip`/`tc` in front of the real ones (hs5/measure.sh; every line that still reaches iproute2):

| | lines to iproute2, ARM64 | x86-64 | ICMP / raw / route suites, both machines | wall, ARM64 (rounds 2-3) |
|---|---|---|---|---|
| before (566f15438) | 1458 | 1458 | 69 / 54 / 38 pass | 1.04-1.07 / 2.93 / 0.13 s |
| group 1 (58d6079df) | 619 | 619 | pass | 1.03-1.07 / 2.94-2.97 / 0.12-0.13 s |
| group 2 (a2bd36ec2) | 281 | 282 | pass | 1.01-1.05 / 2.94-2.97 / 0.13-0.14 s |
| group 3 (580addb83) | 226 | 226 | pass | 1.04 / 2.95 / 0.13 s |
| group 4 (6ee323c7d) | 45, none of them `ip` | 45 | pass | 1.00-1.03 / 2.95 / 0.13-0.16 s |

Left (45, all `tc`, MEASURED): 22 `qdisc add` (prio with its default priority map, fq, etf with `CLOCK_TAI`, clsact), 8
`qdisc del`, 10 `-s -j class show` (packets of class 1:3), 4 `-s -j filter show` (the flower action's packets), 1 `filter
add ... flower dst_mac ... action pass`. Group 5 is next and waits on nothing: netlink-packet-route 0.34 has flower
filters but no prio or etf options and no gact action (READ: src/tc), so those are written as the uapi structures
`tc_prio_qopt`, `tc_etf_qopt` and `tc_gact` inside raw attributes, and the class and action statistics are read from
TCA_STATS2. Not yet done: a check that each converted line
leaves the same kernel state as iproute2's (a dump comparison); the suites passing on both machines is the evidence so far.

### Linter (MEASURED at 566f15438, ARM64, linter da55b9d7, no model key; hs5/arm-lint.log)

Rules that need no model, the track's folders: 632 (b599e5b85: 610). New: `conformance_not_run` +6 in the hosts and -7
`depends_on_implementation`/`names_implementation` in the suites (question 4, above); `constant_defined_twice` for the
local numbers, which the kernel crate also defines and does not export (as before); the rest in the new code was repaired
in ffb5fa31d (summaries, a link, two literals, the `#[path]` the linter could not follow). At 6ee323c7d (MEASURED, hs5/
arm-lint3.log): 642. Left in this round's code, not repaired yet: netlink.rs 6 `magic_literal`, 3 summary length, 1
`constant_defined_twice`; the ICMP suite +3 `magic_literal`, +2 `constant_defined_twice`, +2 `literal_restates_constant`,
+1 `number_constant_without_link`, +1 `comment_too_long`, +1 `public_outside_interface` (`joined_groups`/`link_address`
for the raw suite, -1 for `SHOW_GROUPS`); `local.rs` `comment_budget_exceeded` 1 (810 characters for 10 lines of code,
each number needing its link).

## Owner questions

1. Ruled 2026-11-25 (every outside API: one crate named for it, a safe trait, the unsafe in `implementation/direct`):
   done above as `crates/linux-socket-options`.
2. Which socket options may a hostile guest make the host kernel act on at all? Open; the list is the table and the
   safe routes above.
3. The agents-made `SocketOptions` primitive (d4aee0ae5): read for ideas only (its typed routes in 785d3fa79 taken,
   checked against the kernel by the strace comparison); nothing of `crates/socket-options` is used.
4. nix as a vetted crate (its `recvmsg` treats the whole address buffer as written): open; nix is used as before, now
   also for 18 socket options.

## SECURITY

- 2026-11-26 round 4 (8c01a1037 -> 6ee323c7d): unsafe lines unchanged, the direct crate 4 -> 4 (the same four calls;
  `set_bytes_at` now computes address and length from the lent borrow inside; all four reasons rewritten to name the
  pair-keyed kernel table and their property tests), every other crate of the track 0 -> 0; MEASURED by `git grep -nw
  unsafe` without comments and attributes. No lint table changed, no exclusion, no linter text changed. New dependencies:
  none; new uses of crates already in the lock: the ICMP suite takes netlink-packet-core 0.9.0, netlink-packet-route
  0.34.0, netlink-sys 0.9.0 (MIT), linux-raw-sys's `netlink` feature and rustix's `param` feature; the raw suite takes the interface crate; the
  three hosts take the direct crate as a dev-dependency. The probe also writes its own memory with nix's
  `process_vm_writev` (feature `uio`, already on). Hostile-guest suite: not built and not run (the thread-test design
  review is owed to the owner first); the generator's refusals and properties, the thirty hostile rows, the probe and
  the canaries are the hostile-caller checks of the API crate. Options a guest can reach: unchanged, the 96 methods.
- 2026-11-26 guard repair (e36328391 -> b599e5b85): unsafe lines unchanged, the direct crate 4 -> 4 (the same four
  calls, two reasons rewritten to name the canaries), every other crate of the track 0 -> 0; MEASURED by `git grep -nw
  unsafe` without comments and attributes. No lint table changed, no exclusion. New dependencies: none; the direct
  crate's dev-dependency nix takes two more features, `process` and `uio`, for the probe's `process_vm_readv` read of
  its own memory (read only). Hostile-guest suite: not built and not run (the thread-test design review is owed to the
  owner first); the generator's refusals, the probe and the canaries are the hostile-caller checks of the API crate.
- 2026-11-26 repair: unsafe lines unchanged, every crate of the track 0 -> 0 except the direct crate 4 -> 4 (the same
  four calls; `get_into`, `full_info_into` and `ioctl_within` now hold three of them); MEASURED by `git grep -nw unsafe`
  without comments and attributes at 1ea5c793e and at the tip. New dependencies: none; the direct crate's tests now use
  socket2 0.6.5 and nix 0.31.4 (already in the lock), the TCP suite takes linux-raw-sys's `netlink` feature.
  Hostile-guest suite: not built and not run (the thread-test design review is owed to the owner first); the probe and
  the API suite are the hostile-caller checks of the API crate.
- Unsafe lines (root manifest's count, `git grep` at HEAD), before HS1 (55fd7bafc, agents' copies) and after
  (24d3b3b6a): linux-socket-options 2 -> 0, tcp host 1 -> 0, icmp suite 1 -> 0, route-resolver suite 2 -> 0, every
  other crate of the track 0 -> 0; new `crates/linux-socket-options/implementation/direct` 0 -> 4, its interface,
  suite and test 0. Against main's copies (99 lines) the track is now 4. MEASURED.
- Marked exceptions: the 4 lines of the direct crate, each with its own `#[expect(unsafe_code, reason = "..")]` on the
  one statement, in a crate whose own table denies `unsafe_code`; every other crate of the track inherits
  `forbid`. No linter exclusion added; no lint table lowers `forbid`.
- New dependencies: none from outside the workspace. The API crate uses linux-raw-sys 0.12.1, libc 0.2.191, zerocopy
  0.8 (all already in the lock), its suite socket2 0.6.5, nix 0.31.4 and netlink-packet-core 0.9.0, netlink-packet-route
  0.34.0, netlink-sys 0.9.0 (MIT, already in the TCP suite). New uses: the shared crate takes nix's `socket` and `time`
  features; the route-resolver suite takes socket2.
- Options a guest can make the host kernel act on: the 96 methods of `options.table` and the safe routes listed under
  HS0 (question 2 open).
- Hostile-guest suite: not run and not built (the thread-test design review is owed to the owner first). The API
  crate's suite is the hostile-caller check of that crate only.
