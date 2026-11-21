#!/bin/bash
# Breaks the Linux host implementation in copies of the checkout, one way each, and runs the conformance suite
# on every copy: each must fail, and the checks that fail are printed. The copies live under the build folder.
# Run from the checkout on a Linux machine with sudo: crates/tcp-sockets/implementation/linux-host/mutants.sh
set -uo pipefail
checkout=$(cd "$(dirname "$0")/../../../.." && pwd)
implementation=crates/tcp-sockets/implementation/linux-host/implementation.rs
icmp_error=crates/internet-protocol/implementation/linux-socket-options/icmp_error.rs
mutants=${CARGO_TARGET_DIR:-$checkout/target}/mutants
# Each mutant: a name, the file it breaks, the text it replaces and what it puts there.
names=(more-to-come-ignored abort-closes-in-order peek-offset-ignored push-does-nothing icmp-errors-refused
    quoted-addresses-swapped report-packet-dropped statistics-dropped)
files=("$implementation" "$implementation" "$implementation" "$implementation" "$implementation" "$icmp_error"
    "$implementation" "$implementation")
from=(
    'flags |= libc::MSG_MORE;'
    'if manner == TcpClose::Abort {'
    'offset = count(from.0)?;'
    'let no_delay = sockopt::tcp_nodelay(socket).map_err(host_error)?;'
    'libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 => {'
    'let inner = ipv4_header(us, destination,'
    'report.packet_length = Some(ByteCount(message.length));'
    '.then(|| transmit_statistics(data))'
)
to=(
    ';'
    'if manner == TcpClose::Abort && false {'
    'offset = 0; let _ = from;'
    'return Ok(()); #[allow(unreachable_code)] let no_delay = sockopt::tcp_nodelay(socket).map_err(host_error)?;'
    'libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 if false => {'
    'let inner = ipv4_header(destination, us,'
    'report.packet_length = None;'
    '.then(|| { let _ = data; None::<TcpTransmitStatistics> })'
)
status=0
for index in "${!names[@]}"; do
    copy=$mutants/${names[$index]}
    rm -rf "$copy" && mkdir -p "$copy"
    rsync -a --exclude /target --exclude /.git "$checkout/" "$copy/"
    # The copies share a build folder, so every file any copy breaks is rebuilt in each.
    touch "${files[@]/#/$copy/}"
    file=${files[$index]}
    grep -qF "${from[$index]}" "$copy/$file" || { echo "${names[$index]}: the text to break is gone"; status=1; continue; }
    sed -i "s|$(printf '%s' "${from[$index]}" | sed 's/[][\.*^$|]/\\&/g')|$(printf '%s' "${to[$index]}" | sed 's/[|&\\]/\\&/g')|" "$copy/$file"
    echo "== ${names[$index]}"
    output=$(CARGO_TARGET_DIR=$mutants/target "$copy/crates/tcp-sockets/implementation/linux-host/run-conformance.sh" 2>&1)
    grep -E '^test .* FAILED$|^test result' <<<"$output"
    grep -q '^test result: FAILED' <<<"$output" || { echo "${names[$index]}: the suite did not fail"; status=1; }
done
exit $status
