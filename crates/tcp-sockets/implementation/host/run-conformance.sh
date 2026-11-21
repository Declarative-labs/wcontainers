#!/bin/bash
# Builds the TCP conformance suite against the Linux host implementation and runs it as root in a network
# namespace of its own, whose loopback is the only network the tests see, set up as the suite asks. Arguments go to the
# test binary.
# Run from the checkout on a Linux machine with sudo: crates/tcp-sockets/implementation/host/run-conformance.sh
set -euo pipefail
# net.ipv4.tcp_fastopen's client bit (1) and listener bit (2), as ip-sysctl.rst names them.
fast_open_for_both=$((1 | 2))
cd "$(dirname "$0")/../../../.."
binary=$(cargo test -p wcontainers-tcp-sockets-host --no-run --message-format=json 2>/dev/null |
    sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' | tail -n 1)
[ -n "$binary" ] || { echo "the test binary did not build" >&2; exit 1; }
exec sudo unshare --net sh -c 'ip link set lo up && tc qdisc replace dev lo root fq && sysctl -qw net.ipv4.tcp_fastopen='"$fast_open_for_both"' net.ipv6.flowlabel_consistency=0 net.ipv4.tcp_fwmark_accept=1 && exec "$0" "$@"' "$binary" ${RUST_TEST_THREADS:+--test-threads=$RUST_TEST_THREADS} "$@"
