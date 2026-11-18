#!/usr/bin/env bash
# The long runs to the plan's pass bars (section 9), in chunks, resumable. At 100 percent: layer 1 at 10,000
# cases a form (100 a form in Chrome), layer 2 at one million programs (1,000 on the real CPU, 200 in Chrome),
# layer 3 at 100,000 Csmith programs compared with Linux (1,000 in Chrome; programs that do not end on Linux in
# time are made but not compared, so it runs chunks until that many were compared). Each chunk is one test run;
# when it ends, one line is appended to the ledger. A chunk the ledger holds as passed on an accepted commit is
# skipped, so a stopped run loses at most the chunk it was in; a chunk that failed or was stopped runs again. A
# failing chunk is recorded with its replay command and the run goes on. The run is of one commit: it refuses
# a tree with uncommitted changes, and builds and runs a copy of the commit in WC_LONG_DIR, so work in the
# checkout while it runs does not reach it.
#
# From the repository root, detached (it runs for many hours):
#   WC_LONG_DIR=<directory> nohup nix develop -c crates/wcontainers-translator-test/long-runs.sh > <log> 2>&1 &
# Knobs:
#   WC_LONG_DIR      where the ledger (ledger.txt) and each chunk's log go; required.
#   WC_LONG_PERCENT  how much of each bar to run, 1 to 100 (default 100).
#   WC_LONG_LAYERS   which layers, comma separated (default 1,2,3).
#   WC_LONG_SEED     the base seed (default 1): layer 1's chunk k runs seed SEED*1000+k; layers 2 and 3 run
#                    seed SEED from the chunk's first case.
#   WC_LONG_ACCEPT   the oldest commit whose ledger lines count (default HEAD): chunks that passed on it or on
#                    any commit after it up to HEAD are skipped.
#   WC_LONG_JOBS     cases or forms at once inside a chunk (default 6, the most this machine allows).
#   WC_LONG_LIMIT    minutes a chunk may take before it is stopped and recorded as stopped (default 90).
#
# Ledger lines are tab separated key=value fields: layer, chunk (k/of; layer 3 k alone), knobs (the chunk's
# environment), cases (for layer 3 the programs compared), sample (cases also on the real CPU or in Chrome;
# for layer 3 also the programs made and those that did not end on Linux in time), mismatches, seconds, load (at start and end), commit,
# result (pass, fail or stopped) and, for a failing chunk, failing (what failed) and replay (a command).
set -u
root=$(git rev-parse --show-toplevel)
cd "$root" || exit 1
dir=${WC_LONG_DIR:?set WC_LONG_DIR to the directory for the ledger and logs}
percent=${WC_LONG_PERCENT:-100}
layers=${WC_LONG_LAYERS:-1,2,3}
base=${WC_LONG_SEED:-1}
jobs=${WC_LONG_JOBS:-6}
limit=${WC_LONG_LIMIT:-90}
accept=$(git rev-parse "${WC_LONG_ACCEPT:-HEAD}") || exit 1
mkdir -p "$dir/logs"
ledger=$dir/ledger.txt
touch "$ledger"

if [ -n "$(git status --porcelain -- crates Cargo.lock Cargo.toml flake.nix flake.lock rust-toolchain.toml)" ]; then
  echo "the tree has uncommitted changes; commit them first, as a run is of one commit" >&2
  exit 1
fi
commit=$(git rev-parse HEAD)
accepted=$( (echo "$accept"; git rev-list "$accept..HEAD") | sort)
# The commit's own copy, built into one target directory shared by every copy.
tree=$dir/trees/${commit:0:12}
if [ ! -f "$tree/Cargo.toml" ]; then
  rm -rf "$tree" && mkdir -p "$tree" && git archive HEAD | tar -x -C "$tree" || exit 1
fi
cd "$tree" || exit 1
export CARGO_TARGET_DIR=$dir/target
# zig's cache for the run alone: zig processes that rebuild the same entry of a shared cache at once can
# deadlock on its manifest locks, and other jobs on the machine use the shared one (~/.cache/zig).
export ZIG_GLOBAL_CACHE_DIR=$dir/zig-cache
# A chunk's cases run on a pool of its jobs' threads, which the translator and wasmtime share; anything run
# outside that pool keeps to the same number.
export RAYON_NUM_THREADS=$jobs
test_command="cargo test --release -p wcontainers-translator-test"

loadavg() { sysctl -n vm.loadavg 2>/dev/null | tr -d '{}' | xargs; }
# The share of a bar at this percent, at least one.
part() { local n=$(( $1 * percent / 100 )); echo $(( n > 0 ? n : 1 )); }
ceil() { echo $(( ($1 + $2 - 1) / $2 )); }

# The cases of this chunk's line in the ledger that passed on an accepted commit; nothing if there is none.
held() {
  local layer=$1 knobs=$2
  awk -F'\t' -v layer="layer=$layer" -v knobs="knobs=$knobs" \
    '$1 == layer && $3 == knobs && $10 == "result=pass" { sub(/^commit=/, "", $9); sub(/^cases=/, "", $4); print $9, $4 }' "$ledger" |
    while read -r at n; do grep -qxF "$at" <<< "$accepted" && echo "$n"; done | head -1
}

# Run one chunk: layer, chunk number, chunks (empty when not known ahead), knobs, test filter; parse its
# summary with the layer's parser. Sets ran to the chunk's cases (0 if not known).
run_chunk() {
  local layer=$1 k=$2 of=$3 knobs=$4 filter=$5
  local name=$k${of:+/$of}
  ran=0
  local passed
  passed=$(held "$layer" "$knobs")
  if [ -n "$passed" ]; then
    echo "layer $layer chunk $name: passed in the ledger, skipped"
    ran=$passed
    return
  fi
  local log="$dir/logs/layer$layer-$k${of:+-of-$of}-${commit:0:12}.log"
  local command="$knobs $test_command --test layer$layer $filter -- --nocapture"
  local load_before started status
  load_before=$(loadavg)
  started=$(date +%s)
  echo "layer $layer chunk $name: started $(date '+%F %T'), load $load_before: $command"
  env $knobs timeout "$((limit * 60))" $test_command --test "layer$layer" $filter -- --nocapture 2>&1 |
    grep -v "wcontainers warning" > "$log"
  status=${PIPESTATUS[0]}
  local seconds=$(( $(date +%s) - started ))
  local load_after
  load_after=$(loadavg)
  local cases="?" sample="" mismatches="?" failing="" replay=""
  "parse_layer$layer" "$log"
  local result=pass
  if [ "$status" -eq 124 ]; then
    result=stopped
    # The stopped run's reference containers; nothing else of this half runs beside the driver.
    docker ps -aq --filter name=wc-translator- | xargs docker rm -f > /dev/null 2>&1
  elif [ "$status" -ne 0 ] || [ "$mismatches" != 0 ]; then
    result=fail
  fi
  local line
  line=$(printf 'layer=%s\tchunk=%s\tknobs=%s\tcases=%s\tsample=%s\tmismatches=%s\tseconds=%s\tload=%s / %s\tcommit=%s\tresult=%s' \
    "$layer" "$name" "$knobs" "$cases" "$sample" "$mismatches" "$seconds" "$load_before" "$load_after" "$commit" "$result")
  if [ "$result" != pass ]; then
    [ -n "$replay" ] || replay="$command"
    line=$(printf '%s\tfailing=%s\treplay=%s\tlog=%s' "$line" "$failing" "$replay" "$log")
  fi
  echo "$line" >> "$ledger"
  echo "layer $layer chunk $name: $result, $cases cases, $mismatches mismatches, $seconds s"
  [[ "$cases" =~ ^[0-9]+$ ]] && ran=$cases
}

# Each parser sets cases, sample, mismatches, failing and replay from a chunk's log.
parse_layer1() {
  local main chrome forms="" in_chrome="" chrome_failing=0
  main=$(grep -E '^layer 1: [0-9]+ forms,' "$1" | tail -1)
  chrome=$(grep -E '^layer 1 in Chrome:' "$1" | tail -1)
  if [ -n "$main" ]; then
    cases=$(echo "$main" | awk '{ print $3 * $5 }')
    forms=$(echo "$main" | sed -E 's/.*failing: \[(.*)\]$/\1/; s/"//g; s/ //g')
    mismatches=$(echo "$main" | sed -E 's/.* ([0-9]+) failing.*/\1/')
  fi
  if [ -n "$chrome" ]; then
    in_chrome=$(echo "$chrome" | sed -E 's/.*:\ ([0-9]+) cases.*/\1/')
    chrome_failing=$(echo "$chrome" | sed -E 's/.* ([0-9]+) failing.*/\1/')
    sample="$in_chrome in Chrome"
    [ "$mismatches" = "?" ] || mismatches=$(( mismatches + chrome_failing ))
  fi
  failing=$forms
  if [ -n "$forms" ]; then
    replay="$knobs WC_LAYER1_FORMS=$forms $test_command --test layer1 every_covered_form_matches_the_cpu -- --nocapture"
  fi
  if [ "$chrome_failing" != 0 ]; then
    failing="$failing in Chrome: $(echo "$chrome" | sed -E 's/.*failing: \[(.*)\]$/\1/; s/"//g; s/ //g')"
    [ -n "$replay" ] || replay="$knobs $test_command --test layer1 sample_matches_the_cpu_in_chrome -- --nocapture"
  fi
}

parse_layer2() {
  local summary
  summary=$(grep -E '^layer 2: [0-9]+ cases run' "$1" | tail -1)
  [ -n "$summary" ] || return
  cases=$(echo "$summary" | awk '{ print $3 }')
  sample=$(echo "$summary" | sed -E 's/.*at once, ([0-9]+) on the CPU, ([0-9]+) in Chrome;.*/\1 on the CPU, \2 in Chrome/')
  mismatches=$(echo "$summary" | sed -E 's/.*; ([0-9]+) failing.*/\1/')
  failing=$(echo "$summary" | sed -E 's/.*failing: \[(.*)\]$/\1/; s/ //g')
  if [ -n "$failing" ]; then
    local seed=${knobs#*WC_LAYER2_SEED=}
    seed=${seed%% *}
    replay="WC_LAYER2_SEED=$seed WC_LAYER2_FROM=${failing%%,*} WC_LAYER2_CASES=1 WC_LAYER2_NATIVE=1 WC_LAYER2_CHROME=1 $test_command --test layer2 -- --nocapture"
  fi
}

parse_layer3() {
  local summary
  summary=$(grep -E '^layer 3: [0-9]+ Csmith programs' "$1" | tail -1)
  [ -n "$summary" ] || return
  cases=$(echo "$summary" | sed -E 's/.*: ([0-9]+) compared.*/\1/')
  sample=$(echo "$summary" | sed -E 's/^layer 3: ([0-9]+) Csmith programs.*\(([0-9]+) also in Chrome\), ([0-9]+) did not end.*/\2 in Chrome, \1 made, \3 did not end on Linux in time/')
  mismatches=$(echo "$summary" | sed -E 's/.* ([0-9]+) failing.*/\1/')
  failing=$(grep -oE '^csmith [0-9]+ \(case [0-9]+\)' "$1" | sed -E 's/.*case ([0-9]+)\)/\1/' | sort -n | paste -sd, -)
  if [ -n "$failing" ]; then
    local seed=${knobs#*WC_LAYER3_SEED=}
    seed=${seed%% *}
    replay="WC_LAYER3_SEED=$seed WC_LAYER3_FROM=${failing%%,*} WC_LAYER3_CASES=1 WC_LAYER3_CHROME=1 $test_command --test layer3 csmith_programs_match_linux -- --nocapture"
  fi
}

echo "long runs at $percent percent of the bars, layers $layers, base seed $base, commit $commit, $jobs at once; ledger $ledger"
$test_command --no-run 2>&1 | tail -1
[ "${PIPESTATUS[0]}" -eq 0 ] || exit 1
all_started=$(date +%s)

# Layer 1: chunks of up to 1,000 cases a form, each its own seed.
if [[ ",$layers," == *,1,* ]]; then
  total=$(part 10000)
  chunks=$(ceil "$total" 1000)
  chrome=$(ceil "$(part 100)" "$chunks")
  for ((k = 0; k < chunks; k++)); do
    cases=$(( k < chunks - 1 ? 1000 : total - 1000 * (chunks - 1) ))
    knobs="WC_LAYER1_SEED=$((base * 1000 + k)) WC_LAYER1_CASES=$cases WC_LAYER1_CHROME=$chrome WC_LAYER1_JOBS=$jobs"
    run_chunk 1 $((k + 1)) "$chunks" "$knobs" ""
  done
fi

# Layer 2: chunks of up to 25,000 programs from one seed, by case offset.
if [[ ",$layers," == *,2,* ]]; then
  total=$(part 1000000)
  chunks=$(ceil "$total" 25000)
  natives=$(ceil "$(part 1000)" "$chunks")
  chrome=$(ceil "$(part 200)" "$chunks")
  for ((k = 0; k < chunks; k++)); do
    cases=$(( k < chunks - 1 ? 25000 : total - 25000 * (chunks - 1) ))
    knobs="WC_LAYER2_SEED=$base WC_LAYER2_FROM=$((k * 25000)) WC_LAYER2_CASES=$cases WC_LAYER2_NATIVE=$natives WC_LAYER2_CHROME=$chrome WC_LAYER2_JOBS=$jobs"
    run_chunk 2 $((k + 1)) "$chunks" "$knobs" ""
  done
fi

# Layer 3: chunks of 500 Csmith programs from one seed, by case offset, until the programs compared (in chunks
# that passed, or ran in this run) reach the bar. At most twice the chunks the bar would take if every program
# were compared, so a reference that keeps failing cannot hold the run for ever.
if [[ ",$layers," == *,3,* ]]; then
  total=$(part 100000)
  chunks=$(ceil "$total" 500)
  chrome=$(ceil "$(part 1000)" "$chunks")
  compared=0
  for ((k = 0; compared < total && k < 2 * chunks; k++)); do
    knobs="WC_LAYER3_SEED=$base WC_LAYER3_FROM=$((k * 500)) WC_LAYER3_CASES=500 WC_LAYER3_CHROME=$chrome WC_LAYER3_JOBS=$jobs"
    run_chunk 3 $((k + 1)) "" "$knobs" csmith_programs_match_linux
    compared=$(( compared + ran ))
  done
  echo "layer 3: $compared programs compared in $k chunks"
fi

echo "long runs ended after $(( $(date +%s) - all_started )) s"
awk -F'\t' '{ split($1, l, "="); split($10, r, "="); split($4, c, "="); k = l[2] " " r[2]; n[k]++; s[k] += c[2] }
  END { for (k in n) print "layer " k ": " n[k] " chunks, " s[k] " cases in the ledger" }' "$ledger" | sort
