#!/bin/bash
# Verifies the statement crate in strict mode, then each plant of the given plant lists, each of which must fail.
#
# Run it from the root of an `instruction-proof` tree (Verus's library is there) holding this folder as
# crates/translator-confinement-statement, on a machine with `cargo verus` (never on the Mac):
#
#   bash crates/translator-confinement-statement/prove.sh [plants.txt plants-proofs.txt]
#
# A plant line is `[file @@] name @@ exact text @@ replacement` (`\n` a line break; the file is statement.rs when
# left out). Each plant is applied alone to a copy, verified, and the crate restored; the text must occur exactly
# once. The controls before and after must verify with 0 errors; every plant must end with status 101.
set -u
X='-Zcrate-attr=register_tool(reviewable)'
C=wcontainers-translator-confinement-statement
S=crates/translator-confinement-statement
T=${CARGO_TARGET_DIR:-target}/confinement-proof
mkdir -p "$T"
echo "load $(cut -d' ' -f1-3 /proc/loadavg)"
# Verus's library is built once without --no-cheating, which it refuses.
RUSTFLAGS= cargo verus verify -p wcontainers-processor-architecture -- "$X" > "$T/library.log" 2>&1
echo "processor-architecture status $?"
verify() {
  cargo clean -q -p $C 2>/dev/null
  start=$(date +%s)
  RUSTFLAGS= cargo verus verify -p $C -- "$X" --no-cheating > "$T/proof.log" 2>&1
  status=$?
  echo "$1: status $status in $(( $(date +%s) - start )) s: $(sed 's/\x1b\[[0-9;]*m//g' "$T/proof.log" | grep 'verification results' | tail -1)"
  sed 's/\x1b\[[0-9;]*m//g' "$T/proof.log" | grep -A14 '^error' | grep -E '^error|-->|^[0-9 ]+\|' | head -"${LINES:-30}"
}
verify control
echo "trust markers: $(grep -rnE 'assume|admit|external_body|#\[verifier|exec_allows_no_decreases|unsafe' $S --include=*.rs | wc -l)"
rm -rf "$T/original"; cp -R $S "$T/original"
for pf in "$@"; do
  grep -v '^#' "$S/$pf" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    out=$(python3 -I - "$S" "$line" <<'PY'
import sys
d, line = sys.argv[1], sys.argv[2]
parts = line.split(' @@ ')
if len(parts) == 3:
    parts = ['statement.rs'] + parts
f, name, old, new = parts
old = old.replace('\\n', '\n'); new = new.replace('\\n', '\n')
p = d + '/' + f
text = open(p).read()
n = text.count(old)
if n != 1:
    print('PLANT-NOT-APPLIED', name, n); sys.exit(0)
open(p, 'w').write(text.replace(old, new))
print(name)
PY
)
    case "$out" in PLANT-NOT-APPLIED*) echo "$out"; continue;; esac
    LINES=3 verify "plant $pf / $out"
    rm -rf $S; cp -R "$T/original" $S
  done
done
[ $# -gt 0 ] && verify control-after
exit 0
