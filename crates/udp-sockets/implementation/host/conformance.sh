#!/bin/bash
# Builds the conformance tests of the host UDP sockets and runs them as root, each in its own network namespace.
#   conformance.sh <test threads> [test name filter...]
set -euo pipefail
threads=${1:?the number of test threads}
shift
cd "$(dirname "$0")/../../../.."
binary=$(cargo test -p wcontainers-udp-sockets-host --test conformance --no-run --message-format=json 2>/dev/null |
    sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' | tail -n 1)
[ -n "$binary" ] || { echo "conformance.sh: the test binary did not build" >&2; exit 1; }
sudo -n "$binary" --test-threads "$threads" "$@"
