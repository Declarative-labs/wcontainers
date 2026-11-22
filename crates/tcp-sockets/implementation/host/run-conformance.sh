#!/bin/bash
# Builds the TCP conformance suite against the Linux host implementation and runs it as root in a network
# namespace of its own, whose loopback is the only network the tests see, set up as the suite asks. Arguments go to the
# test binaries.
# Run from the checkout on a Linux machine with sudo: crates/tcp-sockets/implementation/host/run-conformance.sh
set -euo pipefail
# net.ipv4.tcp_fastopen's client bit (1) and listener bit (2), as ip-sysctl.rst names them.
fast_open_for_both=$((1 | 2))
# The suite's MICROSECOND_CLOCK, a loopback address whose local route gives TCP timestamps in microseconds.
microsecond_clock=127.0.0.7
cd "$(dirname "$0")/../../../.."
# An ip that knows the tcp_usec_ts route feature (iproute2 6.7 and later), from the checkout's pinned nixpkgs.
ip=$(nix build --inputs-from . --no-link --print-out-paths nixpkgs#iproute2^out)/bin/ip
# Exit status when the test binaries do not build, apart from a suite that fails (1).
did_not_build=2
# The build's diagnostics go to standard error as cargo renders them; its JSON gives the binaries' paths.
if ! built=$(cargo test -p wcontainers-tcp-sockets-host --no-run --message-format=json-render-diagnostics); then
    echo "the test binaries did not build" >&2; exit $did_not_build
fi
binaries=$(sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' <<<"$built")
[ -n "$binaries" ] || { echo "the test binaries did not build" >&2; exit $did_not_build; }
setup="$ip"' link set lo up && '"$ip"' route add local '"$microsecond_clock"' dev lo table local features tcp_usec_ts && tc qdisc replace dev lo root fq && sysctl -qw net.ipv4.tcp_fastopen='"$fast_open_for_both"' net.ipv6.flowlabel_consistency=0 net.ipv4.tcp_fwmark_accept=1 net.ipv6.conf.lo.forwarding=1'
# The congestion controls the suite reads besides the built-in ones, loaded for the whole host (namespaces share them).
sudo modprobe -a tcp_dctcp tcp_vegas tcp_bbr
status=0
# The suite's binary and the implementation's own tests, each in a namespace of its own.
for binary in $binaries; do
    sudo unshare --net sh -c "$setup"' && exec "$0" "$@"' "$binary" ${RUST_TEST_THREADS:+--test-threads=$RUST_TEST_THREADS} "$@" || status=1
done
exit $status
