# verus.sh: Verus on the proof's copy, the agreement test of the copy and the code, and wrong changes to the copy.
set -u
cd crates/shared-memory/proof
v() { ( export RUSTUP_HOME=/work/vtrial/rustup CARGO_HOME=/work/vtrial/cargo PATH=/work/vtrial/cargo/bin:/work/vtrial/verus-x86-linux:$PATH RUSTFLAGS=; "$@" ); }
echo "===== $(v verus --version 2>&1 | grep -i version | head -1) load $(cut -d' ' -f1-3 /proc/loadavg)"
s=$(date +%s.%N)
v verus --crate-type=lib arithmetic.rs 2>&1 | tail -25
echo "wall $(echo "$(date +%s.%N) - $s" | bc) s"
echo "===== the agreement test"
s=$(date +%s)
CARGO_TARGET_DIR=/work/target/rprim-proof cargo test -- --nocapture 2>&1 | grep -E "test |test result: [^0]*[1-9]|error|warning|compared" | head -20
echo "wall $(( $(date +%s) - s )) s"
python3 - <<'PY'
s = open("arithmetic.rs").read()
for name, old, new in [
    ("offset-off-by-one", "if at.checked_add(length as u64)? > count as u64 * WORD as u64 {", "if at.checked_add(length as u64)? > count as u64 * WORD as u64 + 1 {"),
    ("part-past-parent", "if start > end || end > count as u64 * WORD as u64 {", "if start > end || end > count as u64 * WORD as u64 + 1 {"),
    ("head-one-too-many", "head: if room < length { room } else { length },", "head: if room < length { room + 1 } else { length },"),
    ("overflow", "at.checked_add(length as u64)?", "Some(at + length as u64)?"),
]:
    assert s.count(old) == 1, name
    open(f"/tmp/rprim-{name}.rs", "w").write(s.replace(old, new))
PY
for name in offset-off-by-one part-past-parent head-one-too-many overflow; do
  echo "===== wrong change to the copy: $name"
  v verus --crate-type=lib /tmp/rprim-$name.rs 2>&1 | grep -E "^error|verification results" | head -4
  rm -f /tmp/rprim-$name.rs
done
for name in head-one-too-many-in-the-code bound-off-by-one-in-the-code; do
  cp ../arithmetic.rs /tmp/rprim-real.rs
  case $name in
    head-*) sed -i 's/head: if room < length { room } else { length },/head: if room < length { room + 1 } else { length },/' ../arithmetic.rs ;;
    bound-*) sed -i 's/if start > end || end > count as u64 \* WORD as u64 {/if start > end || end > count as u64 * WORD as u64 + 1 {/' ../arithmetic.rs ;;
  esac
  echo "===== wrong change to the code, agreement test: $name ($(diff /tmp/rprim-real.rs ../arithmetic.rs | grep -c '^>') line changed)"
  CARGO_TARGET_DIR=/work/target/rprim-proof cargo test 2>&1 | grep -E "panicked|test result" | head -3
  cp /tmp/rprim-real.rs ../arithmetic.rs
done
rm -rf /work/target/rprim-proof
echo done
