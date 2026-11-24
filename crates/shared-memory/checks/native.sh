# native.sh: the crate's suites, the compile refusals and the wrong changes, natively. Run in the synced tree.
set -u
here=$(pwd)
load() { echo "load $(cut -d' ' -f1-3 /proc/loadavg)"; }
filter() { grep -E "^test |FAILED|panicked at|test result: |error(\[|:)|signal|SIGBUS|Running" | grep -v "^test .* ok$" | grep -v " 0 passed; 0 failed" | head -${1:-30}; }
echo "===== $(uname -m) native suites $(load)"
cargo test -p wcontainers-shared-memory -p wcontainers-shared-memory-conformance --features host 2>&1 | filter 40
echo "-- host test names"
cargo test -q -p wcontainers-shared-memory --features host --test host -- --list 2>&1 | grep -c ": test$"
echo "-- clippy"
cargo clippy -q -p wcontainers-shared-memory -p wcontainers-shared-memory-conformance --features host --all-targets --no-deps -- -D warnings 2>&1 | tail -5
echo "-- the inside's target, no host feature"
cargo build -q -p wcontainers-shared-memory --target aarch64-unknown-none 2>&1 | tail -5; echo "status $?"
[ "${REFUSALS:-1}" = 1 ] && {
echo "===== compile refusals"
for case in outlives part_of_part field replace seal_named seal_missing heap; do
  echo "-- $case"
  RUSTFLAGS="--cfg refused=\"$case\"" CARGO_TARGET_DIR=/work/target/rprim-refused cargo test -q --no-run -p wcontainers-shared-memory --features host --test refused 2>&1 | grep -E "^error" | grep -v "could not compile" | head -3
done
rm -rf /work/target/rprim-refused; }
[ "${MUTANTS:-1}" = 1 ] || exit 0
mutate() { # name, file, old, new
  local name=$1 file=$2 old=$3 new=$4 dir=/work/target/rprim-mut-$1
  rm -rf "$dir"; mkdir -p "$dir"; (cd "$here" && tar --exclude=./target -cf - .) | tar -C "$dir" -xf -
  python3 - "$dir/$file" "$old" "$new" <<'PY'
import sys
p, old, new = sys.argv[1:]
s = open(p).read()
assert s.count(old) == 1, "the line to change is not there once"
open(p, "w").write(s.replace(old, new))
PY
  echo "===== wrong change $name: $(cd $dir && diff <(cd $here && cat $file) $file | grep '^>')"
  ( cd "$dir"; CARGO_TARGET_DIR=/work/target/rprim-mutn cargo test -q -p wcontainers-shared-memory --features host 2>&1 | filter 12 )
  rm -rf "$dir"
}
H=crates/shared-memory/foreign_implementation/host/implementation.rs
mutate length-rounded-to-a-page "$H" '        self.map.len()
    }' '        self.map.len().next_multiple_of(4096)
    }'
mutate grow-seal-accepted "$H" 'seals.contains(SealFlags::SHRINK)' 'seals.intersects(SealFlags::SHRINK | SealFlags::GROW)'
mutate any-seal-accepted "$H" 'seals.contains(SealFlags::SHRINK)' 'seals.contains(SealFlags::SHRINK) || true'
mutate length-unchecked "$H" '        if offset
            .checked_add(length as u64)
            .is_none_or(|end| end > size)' '        if false && offset
            .checked_add(length as u64)
            .is_none_or(|end| end > size)'
mutate offset-off-by-one crates/shared-memory/arithmetic.rs 'if at.checked_add(length as u64)? > count as u64 * WORD as u64' 'if at.checked_add(length as u64)? > count as u64 * WORD as u64 + 1'
mutate part-past-parent crates/shared-memory/arithmetic.rs 'if start > end || end > count as u64 * WORD as u64' 'if start > end || end > count as u64 * WORD as u64 + 1'
rm -rf /work/target/rprim-mutn
echo "done $(load)"
