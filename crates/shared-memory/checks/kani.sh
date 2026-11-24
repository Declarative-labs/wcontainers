# kani.sh: every Kani harness of the crate, one stream per count of words, then one wrong change to the arithmetic.
# KANI_HOME names the rustup, cargo and kani folders that hold Kani.
set -u
here=$(pwd)
home=${KANI_HOME:-/work/ktrial}
export RUSTUP_HOME=$home/rustup CARGO_HOME=$home/cargo KANI_HOME=$home/kani PATH=$home/cargo/bin:$PATH
ulimit -v 40000000
out=/work/target/gmc-kani-out; rm -rf $out; mkdir -p $out
harnesses="counts_the_bytes_mapped loads_stores_and_swaps_only_8_byte_words_inside loads_stores_and_exchanges_only_4_byte_words_inside reads_exactly_the_bytes_asked writes_exactly_the_bytes_given parts_lie_inside_and_reach_only_their_bytes parts_read_to_a_sink_and_write_from_a_source waits_and_wakes_only_on_words_inside"
one() { # target dir, module::harness
  local s=$(date +%s)
  CARGO_TARGET_DIR=$1 timeout 2400 cargo kani -p wcontainers-shared-memory -Z stubbing --harness "foreign_implementation::proofs::$2" --exact > $out/$2.txt 2>&1
  local r=$(grep -v "aborting path" $out/$2.txt | grep -E " of [0-9]+ failed| cover properties|VERIFICATION:|^error" | tr -s ' \n' ' ')
  echo "$2: $r wall $(( $(date +%s) - s )) s, load $(cut -d' ' -f1 /proc/loadavg)"
}
stream() { # module
  for h in $harnesses; do one /work/target/gmc-kani-$1 $1::$h; done > $out/stream-$1.log
}
echo "code $(bash crates/shared-memory/checks/tree.sh)"
echo "kani $(cargo kani --version 2>&1 | tail -1), load $(cut -d' ' -f1-3 /proc/loadavg)"
s=$(date +%s)
for m in one_word two_words three_words four_words; do stream $m & done; wait
cat $out/stream-one_word.log $out/stream-two_words.log $out/stream-three_words.log $out/stream-four_words.log
echo "all: $(cat $out/stream-*.log | grep -c 'VERIFICATION:- SUCCESSFUL') successful, $(cat $out/stream-*.log | grep -c 'VERIFICATION:- FAILED') failed, covers unsatisfied in $(cat $out/stream-*.log | grep -c ' 0 of [0-9]* cover properties satisfied'), wall $(( $(date +%s) - s )) s"
echo "===== wrong change: a read one byte past the end accepted"
sed -i 's/if end > u64::try_from(length)/if end > 1 + u64::try_from(length)/' crates/shared-memory/arithmetic.rs
grep -c 'if end > 1 + u64' crates/shared-memory/arithmetic.rs
one /work/target/gmc-kani-two_words two_words::reads_exactly_the_bytes_asked
sed -i 's/if end > 1 + u64::try_from(length)/if end > u64::try_from(length)/' crates/shared-memory/arithmetic.rs
rm -rf /work/target/gmc-kani-*
echo "done"
