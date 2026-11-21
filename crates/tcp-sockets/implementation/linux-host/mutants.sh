#!/bin/bash
# Breaks the Linux host implementation in copies of the checkout, one way each, and runs the conformance suite
# on every copy: each must fail, and the checks that fail are printed. The copies live under the build folder.
# Run from the checkout on a Linux machine with sudo: crates/tcp-sockets/implementation/linux-host/mutants.sh
set -uo pipefail
checkout=$(cd "$(dirname "$0")/../../../.." && pwd)
file=crates/tcp-sockets/implementation/linux-host/implementation.rs
mutants=${CARGO_TARGET_DIR:-$checkout/target}/mutants
# Each mutant: a name, the text it replaces and what it puts there.
names=(more-to-come-ignored abort-closes-in-order peek-offset-ignored push-does-nothing)
from=(
    'flags |= libc::MSG_MORE;'
    'if manner == TcpClose::Abort {'
    'offset = count(from.0)?;'
    'let no_delay = sockopt::tcp_nodelay(socket).map_err(host_error)?;'
)
to=(
    ';'
    'if manner == TcpClose::Abort \&\& false {'
    'offset = 0; let _ = from;'
    'return Ok(()); #[allow(unreachable_code)] let no_delay = sockopt::tcp_nodelay(socket).map_err(host_error)?;'
)
status=0
for index in "${!names[@]}"; do
    copy=$mutants/${names[$index]}
    rm -rf "$copy" && mkdir -p "$copy"
    rsync -a --exclude /target --exclude /.git "$checkout/" "$copy/"
    grep -qF "${from[$index]}" "$copy/$file" || { echo "${names[$index]}: the text to break is gone"; status=1; continue; }
    sed -i "s|$(printf '%s' "${from[$index]}" | sed 's/[][\.*^$|]/\\&/g')|${to[$index]}|" "$copy/$file"
    echo "== ${names[$index]}"
    output=$(CARGO_TARGET_DIR=$mutants/target "$copy/crates/tcp-sockets/implementation/linux-host/run-conformance.sh" 2>&1)
    grep -E '^test .* FAILED$|^test result' <<<"$output"
    grep -q '^test result: FAILED' <<<"$output" || { echo "${names[$index]}: the suite did not fail"; status=1; }
done
exit $status
