#!/usr/bin/env bash
# Whether layer 1 and layer 2 draw the same cases for each seed as at an earlier commit. Each side runs its own
# tests/cases.rs (which writes the cases as text hashes, in the same form on both), then the lists are
# compared with cmp. The earlier commit is built from its own copy and its lists are kept. Layer 1's cases in
# no form and its hand-made cases are compared too when the earlier commit writes them.
#
# From the repository root:
#   WC_OUTPUT_ROOT=<directory> nix develop -c crates/wcontainers-translator-test/same-cases.sh <commit> <seed>...
# WC_CASES_LAYER1 and WC_CASES_LAYER2 (as tests/cases.rs reads them) pass through to both sides.
set -u
root=$(git rev-parse --show-toplevel)
cd "$root" || exit 1
dir=${WC_OUTPUT_ROOT:?set WC_OUTPUT_ROOT to a directory for the copies and lists}
base=$(git rev-parse "${1:?name the earlier commit}") || exit 1
shift
[ $# -gt 0 ] || { echo "name at least one seed"; exit 1; }
test_command=(cargo test --release -p wcontainers-translator-test --test cases -- --ignored --nocapture)
counts=${WC_CASES_LAYER1:-64}-${WC_CASES_LAYER2:-64}

tree=$dir/trees/${base:0:12}
if [ ! -f "$tree/Cargo.toml" ]; then
  rm -rf "$tree" && mkdir -p "$tree/target" && git archive "$base" | tar -x -C "$tree" || exit 1
fi
before=$dir/cases/${base:0:12}/$counts
now=$dir/cases/now/$counts
mkdir -p "$before" "$now"

status=0
for seed in "$@"; do
  if [ ! -s "$before/layer2-$seed.txt" ]; then
    (cd "$tree" && CARGO_TARGET_DIR=$dir/target WC_CASES_DIR=$before WC_CASES_SEED=$seed \
      "${test_command[@]}" > "$dir/cases-before-$seed.log" 2>&1) \
      || { echo "seed $seed: the earlier commit failed, see $dir/cases-before-$seed.log"; status=1; continue; }
  fi
  rm -f "$now/layer1-$seed.txt" "$now/layer2-$seed.txt" "$now/no-form-$seed.txt"
  WC_CASES_DIR=$now WC_CASES_SEED=$seed "${test_command[@]}" > "$dir/cases-now-$seed.log" 2>&1 \
    || { echo "seed $seed: this tree failed, see $dir/cases-now-$seed.log"; status=1; continue; }
  lists="layer1-$seed layer2-$seed"
  [ -f "$before/no-form-$seed.txt" ] && lists="$lists no-form-$seed hand"
  for list in $lists; do
    if cmp -s "$before/$list.txt" "$now/$list.txt"; then
      echo "$list (seed $seed): the same, $(wc -l < "$now/$list.txt" | tr -d ' ') lines"
    else
      echo "$list (seed $seed): DIFFERENT"
      diff "$before/$list.txt" "$now/$list.txt" | head -10
      status=1
    fi
  done
done
exit $status
