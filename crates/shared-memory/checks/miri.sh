# miri.sh: the host suite (anonymous memory), the heap suite and the attacks under Miri, which runs its own memcpy in
# place of the C library's; then two wrong changes Miri must report. Run in the synced tree. MIRI_HOME names the rustup
# and cargo folders that hold a nightly with Miri.
set -u
here=$(pwd)
home=${MIRI_HOME:-/work/eup}
load() { echo "load $(cut -d' ' -f1-3 /proc/loadavg)"; }
miri() { ( export RUSTUP_HOME=$home/rustup CARGO_HOME=$home/cargo PATH=$home/cargo/bin:$PATH CARGO_TARGET_DIR=/work/target/gmc-miri; "$@" ); }
filter() { grep -E "^test |FAILED|Undefined Behavior|panicked at|test result: |error(\[|:)|unsupported|Running|ignored" | grep -v "^test .* ok$" | grep -v " 0 passed; 0 failed" | head -${1:-20}; }
echo "code $(bash crates/shared-memory/checks/tree.sh)"
echo "===== $(miri cargo +nightly miri --version 2>&1) $(load)"
for borrows in "" -Zmiri-tree-borrows; do
  echo "-- ${borrows:-stacked borrows}"
  s=$(date +%s)
  miri env MIRIFLAGS="$borrows" cargo +nightly miri test -p wcontainers-shared-memory --features host --test host --test heap --test attacks 2>&1 | filter 40
  echo "wall $(( $(date +%s) - s )) s"
done
mutate() { # name, file, old, new
  local name=$1 file=$2 old=$3 new=$4 dir=/work/target/gmc-mmut-$1
  rm -rf "$dir"; mkdir -p "$dir"; (cd "$here" && tar --exclude=./target -cf - .) | tar -C "$dir" -xf -
  python3 - "$dir/$file" "$old" "$new" <<'PY'
import sys
p, old, new = sys.argv[1:]
s = open(p).read()
assert s.count(old) == 1, "the line to change is not there once"
open(p, "w").write(s.replace(old, new))
PY
  echo "===== wrong change $name, heap suite and attacks under Miri: $(cd $dir && diff <(cd $here && cat $file) $file | grep '^>' | tr -s ' ')"
  ( cd "$dir"; miri cargo +nightly miri test -p wcontainers-shared-memory --features host --test heap --test attacks 2>&1 | grep -E "Undefined Behavior|test result: |panicked at" | head -4 )
  rm -rf "$dir"
}
mutate heap-one-byte-long crates/shared-memory/foreign_implementation/heap/implementation.rs 'Region::new(base, words.len() * size_of::<u64>())' 'Region::new(base, words.len() * size_of::<u64>() + 1)'
mutate range-one-byte-long crates/shared-memory/arithmetic.rs 'if end > u64::try_from(length)' 'if end > 1 + u64::try_from(length)'
rm -rf /work/target/gmc-miri
echo "done $(load)"
