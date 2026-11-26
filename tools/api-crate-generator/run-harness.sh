#!/bin/bash
# Runs the spike's harness, ported, on a slice's crates as check-output.sh built them.
#   run-harness.sh <build folder> <target folder> [tests] [attacks] [asan] [fuzz] [measure]
# Each attack test runs alone under a time limit, in the default build, the small-table build and under
# AddressSanitizer; FUZZ_RUNS lists "mode:seed:calls" runs, each one process under AddressSanitizer.
set -u
build=$1 target=$2; shift 2
steps=${*:-tests attacks asan}
here=$(cd "$(dirname "$0")" && pwd)
cd "$build"
step() { echo; echo "=== $*"; }
has() { case " $steps " in *" $1 "*) return 0 ;; esac; return 1; }
package=(-p wcontainers-direct3d-11-direct-harness)
filter='^ATTACK|^CHECK2|^FUZZ|^test |test result|panicked|error|SUMMARY|ERROR: AddressSanitizer|WRITE of|READ of|SEGV|signal|Abort|abort|heap-buffer|allocation-size|assertion|left:|right:'
each_attack() {
    for test in attack_c attack_table check2_c game; do
        for name in $(CARGO_TARGET_DIR=$target cargo test --release "$@" --test $test -- --list 2>/dev/null | sed -n 's/: test$//p'); do
            CARGO_TARGET_DIR=$target timeout 300 cargo test --release "$@" --test $test -- --exact "$name" --test-threads=1 --nocapture 2>&1 |
                grep -E "$filter" | grep -vE '^test result|running' | head -8
            echo "  $test $name: status ${PIPESTATUS[0]}"
        done
    done
}
if has tests || has attacks; then
    for feature in "" small-table; do
        step "every test of the harness, each alone under a time limit; ${feature:-default}"
        each_attack "${package[@]}" ${feature:+--features $feature}
    done
fi
asan_env() {
    export RUSTC_BOOTSTRAP=1 STAND_INS_ASAN=1 CC_x86_64_unknown_linux_gnu=$here/harness/stand-ins/zigcc
    export ASAN_OPTIONS=detect_leaks=0:abort_on_error=1:halt_on_error=1:allocator_may_return_null=1
    export RUSTFLAGS="-Zsanitizer=address -C force-frame-pointers=yes -C link-arg=-latomic"
}
if has asan; then
    asan_env
    asan=("${package[@]}" --target x86_64-unknown-linux-gnu)
    target_plain=$target; target=$target/asan
    step "asan build"
    CARGO_TARGET_DIR=$target cargo test --release "${asan[@]}" --tests --no-run 2>&1 | grep -E '^(error|warning)|-->|Finished' | head -20
    step "asan: is the C instrumented? (asan symbols in the stand-ins' objects)"
    nm "$(find $target -name 'libstand_ins.a' | head -1)" 2>/dev/null | grep -c '__asan_report'
    step "asan: every test of the harness"
    each_attack "${asan[@]}"
    target=$target_plain
    unset RUSTC_BOOTSTRAP STAND_INS_ASAN CC_x86_64_unknown_linux_gnu RUSTFLAGS
fi
if has fuzz; then
    asan_env
    asan=("${package[@]}" --target x86_64-unknown-linux-gnu)
    CARGO_TARGET_DIR=$target/asan cargo test --release "${asan[@]}" --test fuzz --no-run 2>&1 | tail -1
    binary=$(CARGO_TARGET_DIR=$target/asan cargo test --release "${asan[@]}" --test fuzz --no-run --message-format=json 2>/dev/null | grep -o '"executable":"[^"]*fuzz-[^"]*"' | cut -d'"' -f4 | tail -1)
    for run in ${FUZZ_RUNS:-liar-correct:1:100000}; do
        IFS=: read -r mode seed calls <<<"$run"
        step "fuzz under AddressSanitizer: $mode, seed $seed, $calls calls"
        log=$target/fuzz-$mode-$seed.log
        FUZZ_MODE=$mode FUZZ_SEED=$seed FUZZ_CALLS=$calls FUZZ_THREADS=${FUZZ_THREADS:-8} timeout 3600 "$binary" --ignored --exact fuzz --nocapture >$log 2>&1
        status=$?
        grep -E '^FUZZ' $log
        [ $status = 0 ] || { echo "--- the run's last lines"; grep -v 'the liar panics\|^note: run with' $log | tail -40; }
        echo "status $status"
    done
fi
