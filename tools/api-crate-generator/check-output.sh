#!/bin/bash
# Builds the generator, writes a set's crates twice and compares the two, then builds the crates written in a
# workspace of their own (beside copies of the repository crates they name) under the repository's lints.
#   check-output.sh <set> <work folder>      run from the repository's root, inside its `nix develop`
set -uo pipefail
set_name=$1
work=$2
repository=$PWD
generator=$repository/tools/api-crate-generator
step() { echo; echo "=== $*"; }

step "versions"
rustc --version; cargo --version; cut -d" " -f1-3 /proc/loadavg

step "the generator: build, every warning shown, and its own tests"
winmd=$("$generator/fetch-metadata.sh" "$work/metadata") || { echo "metadata: status $?"; exit 1; }
(cd "$generator" && CARGO_TARGET_DIR=$work/generator cargo build --release 2>&1 | grep -E '^(warning|error)|-->' ; echo "build status ${PIPESTATUS[0]}")
(cd "$generator" && CARGO_TARGET_DIR=$work/generator cargo test --release 2>&1 | grep -E '^test |test result|panicked'; echo "test status ${PIPESTATUS[0]}")
binary=$work/generator/release/api-crate-generator

step "generate twice"
rm -rf "$work/first" "$work/second"
"$binary" generate "$winmd" "$set_name" "$work/first" > "$work/report.txt"; echo "first status $?"
"$binary" generate "$winmd" "$set_name" "$work/second" > /dev/null; echo "second status $?"
diff -r "$work/first" "$work/second" > /dev/null && echo "IDENTICAL: the two runs wrote the same files" || echo "DIFFERENT: the two runs differ"
find "$work/first" -name '*.rs' | sort | xargs wc -l | tail -1
sed -n '/^CRATE/p;/^ROWS/,/^UNMAPPED/p' "$work/report.txt"
echo "unmapped lines: $(sed -n '/^UNMAPPED/,$p' "$work/report.txt" | tail -n +2 | wc -l)"

step "build the crates written, in a workspace of their own"
build=$work/build
rm -rf "$build"; mkdir -p "$build/crates"
cp -r "$work/first/crates/." "$build/crates/"
for crate in volatile-bytes byte-sink byte-source; do cp -r "$repository/crates/$crate" "$build/crates/"; done
# The owner's pointer crate is what the direct crates copy bytes with; his other implementation is not needed.
rm -rf "$build/crates/volatile-bytes/implementation/shared-mapping"
mkdir -p "$build/.cargo"; cp "$repository/.cargo/config.toml" "$build/.cargo/"
cp "$repository/rust-toolchain.toml" "$build/"
{
    echo '[workspace]'
    echo 'resolver = "3"'
    echo 'members = ["crates/*", "crates/*/conformance", "crates/*/implementation/*", "crates/*/implementation/*/harness"]'
    echo
    sed -n '/^\[workspace.package\]/,/^$/p' "$repository/Cargo.toml"
    echo '[workspace.metadata.reviewable]'
    echo 'prefix = "wcontainers"'
    echo '# The two rule sets that ask a hosted model: no key is on this machine, and nothing of this goes out.'
    echo 'off = ["grammar", "comment-grammar"]'
    echo
    sed -n '/^\[workspace.lints.rust\]/,/^$/p;/^\[workspace.lints.clippy\]/,/^$/p' "$repository/Cargo.toml"
} > "$build/Cargo.toml"
cat "$build/Cargo.toml"
(cd "$build" && CARGO_TARGET_DIR=$work/target cargo check -j "${ARM_JOBS:-12}" --workspace 2>&1 | grep -E '^(warning|error)' | sort | uniq -c | sort -rn | head -40; echo "check status ${PIPESTATUS[0]}")
(cd "$build" && CARGO_TARGET_DIR=$work/target cargo check -j "${ARM_JOBS:-12}" --workspace --message-format short 2>&1 | grep -E ': (warning|error)' | head -60)
(cd "$build" && CARGO_TARGET_DIR=$work/target cargo clippy -j "${ARM_JOBS:-12}" --workspace -- -D warnings 2>&1 | grep -E '^(warning|error)' | sort | uniq -c | sort -rn | head -30; echo "clippy status ${PIPESTATUS[0]}")

step "the direct crates' own tests, each under a time limit"
directs=$(cd "$build/crates" && for folder in */implementation/direct; do sed -n 's/^name = "\(.*\)"/-p \1/p' "$folder/Cargo.toml" | head -1; done)
(cd "$build" && CARGO_TARGET_DIR=$work/target timeout 600 cargo test -j "${ARM_JOBS:-12}" --release $directs 2>&1 | grep -E '^test |test result|panicked|error'; echo "test status ${PIPESTATUS[0]}")

step "unsafe lines per crate written (the root manifest's way: lines naming unsafe, comments included)"
(cd "$work/first/crates" && for crate in $(find . -name Cargo.toml -exec dirname {} \; | sort); do
    echo "$(find "$crate" -name '*.rs' -not -path "$crate/implementation/*" -exec grep -c unsafe /dev/null {} + | awk -F: '{s+=$NF} END {print s+0}') $crate"
done)

if [ -n "${LINTER:-}" ]; then
    step "the linter on the crates written (no rule loosened; the two model rule sets off: no key here)"
    (cd "$build" && "$LINTER" reviewable check > "$work/linter.txt" 2>&1; echo "linter status $?")
    tail -3 "$work/linter.txt"
    echo "findings in the crates written, by rule (the copied repository crates left out):"
    generated=$(cd "$work/first/crates" && ls | paste -sd'|')
    awk -v crates="^ *--> crates/($generated)/" '
        /^error\[reviewable::/ { match($0, /reviewable::[a-z_]+/); rule = substr($0, RSTART + 12, RLENGTH - 12); next }
        rule != "" && /^ *-->/ { if ($0 ~ crates) count[rule]++; rule = "" }
        END { for (r in count) { printf "%7d %s\n", count[r], r; total += count[r] } printf "%7d TOTAL\n", total }' "$work/linter.txt" | sort -rn
fi
