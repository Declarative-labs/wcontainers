#!/bin/bash
# Builds the TCP conformance suite against the Linux host implementation and runs it as root in a network
# namespace of its own, whose loopback is the only network the tests see, set up as the suite asks. Arguments go to the
# test binary.
# Run from the checkout on a Linux machine with sudo: crates/tcp-sockets/implementation/host/run-conformance.sh
set -euo pipefail
# net.ipv4.tcp_fastopen's client bit (1) and listener bit (2), as ip-sysctl.rst names them.
fast_open_for_both=$((1 | 2))
# The suite's MICROSECOND_CLOCK, a loopback address whose local route gives TCP timestamps in microseconds.
microsecond_clock=127.0.0.7
cd "$(dirname "$0")/../../../.."
# An ip that knows the tcp_usec_ts route feature (iproute2 6.7 and later), from the checkout's pinned nixpkgs.
ip=$(nix build --inputs-from . --no-link --print-out-paths nixpkgs#iproute2^out)/bin/ip
binary=$(cargo test -p wcontainers-tcp-sockets-host --no-run --message-format=json 2>/dev/null |
    sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' | tail -n 1)
[ -n "$binary" ] || { echo "the test binary did not build" >&2; exit 1; }
exec sudo unshare --net sh -c "$ip"' link set lo up && '"$ip"' route add local '"$microsecond_clock"' dev lo table local features tcp_usec_ts && tc qdisc replace dev lo root fq && sysctl -qw net.ipv4.tcp_fastopen='"$fast_open_for_both"' net.ipv6.flowlabel_consistency=0 net.ipv4.tcp_fwmark_accept=1 net.ipv6.conf.lo.forwarding=1 && exec "$0" "$@"' "$binary" ${RUST_TEST_THREADS:+--test-threads=$RUST_TEST_THREADS} "$@"
