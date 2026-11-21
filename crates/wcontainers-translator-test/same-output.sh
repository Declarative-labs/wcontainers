#!/usr/bin/env bash
# Whether the translator's output is byte for byte the same as at an earlier commit: the benchmark set (with
# the thread module) and the real binaries, at full and bounded look-ahead, for speed and for size. Each side
# writes every module's length and BLAKE3 hash (tests/output.rs); the lists are then compared with cmp. The
# earlier commit is built from its own copy into a target directory of its own (copies made by git archive
# carry the commit's times, so cargo could take another commit's build for it), and its lists are kept, so it
# runs once per commit and set.
#
# From the repository root:
#   WC_OUTPUT_ROOT=<directory> nix develop -c crates/wcontainers-translator-test/same-output.sh <commit> [sets]
# Sets, comma separated (default all): bench busybox musl rg git python java go node bun (bench and musl for each
# of the four probe answers, the rest for this machine's). Node and Bun take
# many GB and minutes each; run them alone. The real binaries come from this checkout's target/binaries/<guest>.
# Both sides run this tree's tests/output.rs, so they write the same list.
set -u
root=$(git rev-parse --show-toplevel)
cd "$root" || exit 1
dir=${WC_OUTPUT_ROOT:?set WC_OUTPUT_ROOT to a directory for the copies and lists}
base=$(git rev-parse "${1:?name the earlier commit}") || exit 1
sets=${2:-bench,busybox,musl,rg,git,python,java,go,node,bun}
export RAYON_NUM_THREADS=${RAYON_NUM_THREADS:-4}
test_command=(cargo test --release -p wcontainers-translator-test --test output -- --ignored --nocapture)

tree=$dir/trees/${base:0:12}
if [ ! -f "$tree/Cargo.toml" ]; then
  rm -rf "$tree" && mkdir -p "$tree/target" && git archive "$base" | tar -x -C "$tree" || exit 1
  # A commit from before the binaries were kept per guest reads ARM64's files at the top.
  if grep -q 'join(G::NAME)' "$tree/crates/wcontainers-translator-test/src/binaries.rs"; then
    ln -s "$root/target/binaries" "$tree/target/binaries"
  else
    ln -s "$root/target/binaries/arm64" "$tree/target/binaries"
  fi
fi
output=$tree/crates/wcontainers-translator-test/tests/output.rs
# This tree's tests/output.rs needs a commit with the real binaries kept per guest (3e4420d on); before
# `PROBE_ANSWERS` it is given ARM64's four answers. Earlier commits are compared through lists kept from before.
if grep -q 'PROBE_ANSWERS' "$tree/crates/wcontainers-translator-test/src/guest/mod.rs" 2>/dev/null; then
  cp crates/wcontainers-translator-test/tests/output.rs "$output"
else
  sed -e 's/G::PROBE_ANSWERS/[0i32, 1, 2, 3]/' crates/wcontainers-translator-test/tests/output.rs > "$output"
fi
# A commit from before the translator took a link address is given none: ARM64 never builds an absolute one.
if ! grep -q 'link_address' "$tree/crates/wcontainers-translator/src/code.rs"; then
  sed -e '/\.link_address,$/d' "$output" > "$output.adapted" && mv "$output.adapted" "$output"
fi
before=$dir/lists/${base:0:12}
now=$dir/lists/now
mkdir -p "$before" "$now"

status=0
for set in ${sets//,/ }; do
  if [ ! -s "$before/$set.txt" ]; then
    (cd "$tree" && CARGO_TARGET_DIR=$dir/target/${base:0:12} WC_OUTPUT_DIR=$before WC_OUTPUT_SETS=$set \
      "${test_command[@]}" > "$dir/before-$set.log" 2>&1) || { echo "$set: the earlier commit failed, see $dir/before-$set.log"; status=1; continue; }
  fi
  rm -f "$now/$set.txt"
  WC_OUTPUT_DIR=$now WC_OUTPUT_SETS=$set "${test_command[@]}" > "$dir/now-$set.log" 2>&1 \
    || { echo "$set: this tree failed, see $dir/now-$set.log"; status=1; continue; }
  if cmp -s "$before/$set.txt" "$now/$set.txt"; then
    echo "$set: the same, $(wc -l < "$now/$set.txt" | tr -d ' ') modules"
  else
    echo "$set: DIFFERENT"
    diff "$before/$set.txt" "$now/$set.txt" | head -20
    status=1
  fi
done
exit $status
