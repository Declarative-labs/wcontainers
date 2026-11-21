#!/bin/bash
# Builds the tests of the host UDP sockets and runs them as root, each conformance test in its own network namespace.
#   conformance.sh <test threads> [test name filter...]
set -euo pipefail
threads=${1:?the number of test threads}
shift
cd "$(dirname "$0")/../../../.."
# Built first with cargo's own messages, so that a build that fails says why.
cargo test -p wcontainers-udp-sockets-host --test conformance --test readiness --no-run
binaries=$(for test in conformance readiness; do
    cargo test -p wcontainers-udp-sockets-host --test "$test" --no-run --message-format=json 2>/dev/null |
        sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' | tail -n 1
done)
[ "$(echo "$binaries" | grep -c .)" -eq 2 ] || { echo "conformance.sh: the test binaries did not build" >&2; exit 1; }
failed=0
for binary in $binaries; do
    sudo -n "$binary" --test-threads "$threads" "$@" || failed=1
done
exit "$failed"
