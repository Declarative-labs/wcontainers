# Status: host-tcp-sockets

## For the UDP, ICMP and raw tracks
- The shared crate exists: `crates/internet-protocol/implementation/linux-socket-options`, package
  `wcontainers-internet-protocol-linux-socket-options`, created by this track on 2026-11-21. It holds the errno
  mapping (`host_error`, `last_host_error`), the raw option calls (`set_option`, `set_int`, `set_flag`,
  `set_bytes`, `get_option`, `get_int`, `get_bytes`), the counted refusals (`Shortfalls`), the filters
  (`set_filter`), `SO_TIMESTAMPING` (`receive_timestamping_flags`, `set_timestamping`) and the three shared
  setting enums (`set_ip_socket_setting`, `set_port_setting`, `set_packet_setting`). Its body is
  `cfg(target_os = "linux")`.
- Not mapped there yet, and answered as counted refusals: multicast and anycast memberships, and a fixed IPv6
  flow label. The UDP track needs them more than TCP: add them there (tell this file when you do).
- Add to it rather than copying; change a function's signature only with a note here.

## Done
- Plan written: docs/plan-host-tcp-sockets.md.
