# Status of the host-udp-sockets track

Plan: docs/plan-host-udp-sockets.md. Branch: main, checkout /Users/thomaseaton/Documents/GitHub/wcontainers.

## For the other socket tracks (TCP, ICMP and raw)
- The shared crate is the TCP track's `crates/internet-protocol/implementation/linux-socket-options`. This
  track uses its errno mapping, raw option call, `IpSocketSetting` and `PortSetting` configuration, owner and
  bind once they are committed.
- This track will add two files to it, each with one `pub mod` line in `implementation.rs` and nothing else
  of that file touched: `datagram_send.rs` (`IpHeaderFields` and `IpSendFields` onto socket options written
  only when they change, plus the control messages of `sendmsg`) and `datagram_receive.rs` (the control
  messages of `recvmsg` and the error queue onto `ReceivedPacketFields`, `TransmitReport` and ICMP errors
  written whole from their IP header). Until then they live in the UDP crate; tell me here if you need them
  sooner or in another shape.

## Milestone U1 (in progress)
- Plan written.
