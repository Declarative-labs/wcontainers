# miri.sh: the host suite (anonymous memory), the heap suite and the attacks under Miri. Run in the synced tree.
set -u
here=$(pwd)
load() { echo "load $(cut -d' ' -f1-3 /proc/loadavg)"; }
miri() { ( export RUSTUP_HOME=/work/eup/rustup CARGO_HOME=/work/eup/cargo PATH=/work/eup/cargo/bin:$PATH CARGO_TARGET_DIR=/work/target/rprim-miri; "$@" ); }
filter() { grep -E "^test |FAILED|Undefined Behavior|panicked at|test result: |error(\[|:)|unsupported|Running" | grep -v "^test .* ok$" | grep -v " 0 passed; 0 failed" | head -${1:-20}; }
echo "===== $(miri cargo +nightly miri --version 2>&1) $(load)"
s=$(date +%s)
miri cargo +nightly miri test -p wcontainers-shared-memory --features host --test host --test heap --test attacks 2>&1 | filter 30
echo "wall $(( $(date +%s) - s )) s"
echo "-- tree borrows"
s=$(date +%s)
miri env MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test -p wcontainers-shared-memory --features host --test host --test heap --test attacks 2>&1 | filter 30
echo "wall $(( $(date +%s) - s )) s"
echo "-- 16 seeds of every threaded test"
s=$(date +%s)
for t in host heap attacks; do
  miri env MIRIFLAGS=-Zmiri-many-seeds=0..16 cargo +nightly miri test -p wcontainers-shared-memory --features host --test $t -- reads_whole_words keeps_the_rest two_views a_part_on_another a_view_outlives > /work/target/rprim-seeds.txt 2>&1
  echo "$t: exit $?, threaded tests $(grep -oE '^test [a-z_:]+' /work/target/rprim-seeds.txt | sort -u | wc -l), reports of undefined behavior $(grep -c 'Undefined Behavior' /work/target/rprim-seeds.txt), failures $(grep -c 'FAILED' /work/target/rprim-seeds.txt)"
done
echo "wall $(( $(date +%s) - s )) s"
echo "-- the futex test, ignored above: why"
miri cargo +nightly miri test -p wcontainers-shared-memory --features host --test host -- --ignored waits_until_woken 2>&1 | grep -E "Undefined Behavior|test result: [^0]*[1-9]|unsupported" | head -3
mutate() { # name, file, old, new
  local name=$1 file=$2 old=$3 new=$4 dir=/work/target/rprim-mmut-$1
  rm -rf "$dir"; mkdir -p "$dir"; (cd "$here" && tar --exclude=./target -cf - .) | tar -C "$dir" -xf -
  python3 - "$dir/$file" "$old" "$new" <<'PY'
import sys
p, old, new = sys.argv[1:]
s = open(p).read()
assert s.count(old) == 1, "the line to change is not there once"
open(p, "w").write(s.replace(old, new))
PY
  echo "===== wrong change $name, host suite under Miri: $(cd $dir && diff <(cd $here && cat $file) $file | grep '^>')"
  ( cd "$dir"; miri cargo +nightly miri test -p wcontainers-shared-memory --features host --test host -- anonymous 2>&1 | grep -E "Undefined Behavior|test result: |panicked at" | head -4 )
  rm -rf "$dir"
}
mutate slice-one-word-long crates/shared-memory/interface.rs 'core::slice::from_raw_parts(base, count)' 'core::slice::from_raw_parts(base, count + 1)'
mutate length-rounded-to-a-page crates/shared-memory/foreign_implementation/host/implementation.rs '        self.map.len()
    }' '        self.map.len().next_multiple_of(4096)
    }'
rm -rf /work/target/rprim-miri
echo "done $(load)"
