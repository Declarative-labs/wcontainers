#!/usr/bin/env bash
# Plants each wrong implementation of a plant list in turn and checks that Verus refuses it.
#
# Run from the workspace root, inside its Nix shell (which has `cargo verus`):
#   crates/byte-sink/foreign_implementation/refuse_plants.sh crates/byte-sink/foreign_implementation/plants.tsv
#
# A plant list has one plant per line, its fields separated by tabs: the package, the file, the exact
# text replaced (found exactly once), the text put in its place, and what is wrong. Lines starting
# with `#` are comments. The file is put back after each plant. The script exits 0 only when the
# untouched code verifies and every plant is refused.
set -u -o pipefail

list=${1:?usage: refuse_plants.sh <plant list>}
verify() {
    cargo verus focus --package "$1" -- --no-cheating -Aduplicate_features 2>&1
}
summary() {
    grep -E '^verification results::' | tail -1
}

status=0
packages=$(grep -v '^#' "$list" | cut -f1 | sort -u)
for package in $packages; do
    if ! output=$(verify "$package"); then
        echo "UNTOUCHED FAILS $package: $(summary <<<"$output")"
        echo "$output" | tail -30
        exit 1
    fi
    echo "untouched $package verifies: $(summary <<<"$output")"
done

number=0
refused=0
while IFS=$'\t' read -r package file old new wrong; do
    [[ -z $package || $package == \#* ]] && continue
    number=$((number + 1))
    original=$(cat "$file"; printf x)
    original=${original%x}
    occurrences=$(grep -cF -- "$old" "$file")
    if [[ $occurrences != 1 ]]; then
        echo "plant $number: the text replaced occurs $occurrences times in $file: $old"
        status=1
        continue
    fi
    printf '%s' "${original/"$old"/"$new"}" >"$file"
    output=$(verify "$package")
    verified=$?
    printf '%s' "$original" >"$file"
    reason=$(grep -m1 -E '^error' <<<"$output")
    if [[ $verified != 0 ]]; then
        refused=$((refused + 1))
        echo "plant $number REFUSED ($wrong): $reason; $(summary <<<"$output")"
    else
        echo "plant $number ACCEPTED ($wrong): $(summary <<<"$output")"
        status=1
    fi
done <"$list"
echo "$refused of $number plants refused"
exit $status
