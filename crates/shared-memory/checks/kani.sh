# kani.sh: every Kani harness of the crate, one stream per count of words, then the futex harness on a wrong change.
set -u
here=$(pwd)
export RUSTUP_HOME=/work/ktrial/rustup CARGO_HOME=/work/ktrial/cargo KANI_HOME=/work/ktrial/kani PATH=/work/ktrial/cargo/bin:$PATH
ulimit -v 40000000
out=/work/target/rprim-kani-out; rm -rf $out; mkdir -p $out
one() { # target dir, module::harness
  local s=$(date +%s)
  CARGO_TARGET_DIR=$1 timeout 1200 cargo kani -p wcontainers-shared-memory -Z stubbing --harness "foreign_implementation::proofs::$2" --exact > $out/$2.txt 2>&1
  local r=$(grep -v "aborting path" $out/$2.txt | grep -E " of [0-9]+ failed| cover properties|VERIFICATION:" | tr -s ' \n' ' ')
  echo "$2: $r wall $(( $(date +%s) - s )) s, load $(cut -d' ' -f1 /proc/loadavg)"
}
stream() { # module
  for h in counts_the_words_mapped loads_stores_and_swaps_only_words_inside reads_exactly_the_bytes_asked writes_exactly_the_bytes_given parts_lie_inside_and_reach_only_their_bytes parts_read_to_a_sink_and_write_from_a_source waits_and_wakes_on_the_low_half_of_words_inside; do
    one /work/target/rprim-kani-$1 $1::$h
  done > $out/stream-$1.log
}
echo "kani $(cargo kani --version 2>&1 | tail -1), load $(cut -d' ' -f1-3 /proc/loadavg)"
s=$(date +%s)
for m in one_word two_words three_words four_words; do stream $m & done; wait
cat $out/stream-one_word.log $out/stream-two_words.log $out/stream-three_words.log $out/stream-four_words.log
echo "all 28: $(cat $out/stream-*.log | grep -c 'VERIFICATION:- SUCCESSFUL') successful, $(cat $out/stream-*.log | grep -c 'VERIFICATION:- FAILED') failed, covers unsatisfied in $(cat $out/stream-*.log | grep -c ' 0 of [0-9]* cover properties satisfied'), wall $(( $(date +%s) - s )) s"
echo "===== wrong change: the futex on the high half"
sed -i 's/cfg!(target_endian = "big")/cfg!(target_endian = "little")/' crates/shared-memory/interface.rs
grep -c 'target_endian = "little"' crates/shared-memory/interface.rs
one /work/target/rprim-kani-four_words four_words::waits_and_wakes_on_the_low_half_of_words_inside
sed -i 's/cfg!(target_endian = "little")/cfg!(target_endian = "big")/' crates/shared-memory/interface.rs
rm -rf /work/target/rprim-kani-*_word*
echo "done"
