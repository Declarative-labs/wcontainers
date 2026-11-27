#!/bin/bash
# Checks the generator's C layout of every record of a set against MinGW's headers, as zig compiles them for 64-bit
# Windows: one compile-time assertion per size and per named field offset. A record the headers do not declare is
# listed and left out of a second compile.
#   check-layout.sh <set> <work folder>      run from the repository's root, inside its `nix develop`
set -uo pipefail
set_name=$1
work=$2
generator=$PWD/tools/api-crate-generator
winmd=$("$generator/fetch-metadata.sh" "$work/metadata") || exit 1
(cd "$generator" && CARGO_TARGET_DIR=$work/generator cargo build -q --release) || exit 1
binary=$work/generator/release/api-crate-generator
compile() { ZIG_GLOBAL_CACHE_DIR=$work/zig-cache ZIG_LOCAL_CACHE_DIR=$work/zig-cache zig cc -target x86_64-windows-gnu -c -o "$work/layout.o" -ferror-limit=0 "$1" 2>&1; }
"$binary" layout-check "$winmd" "$set_name" "$work/layout-first.c"
missing=$(compile "$work/layout-first.c" | sed -n "s/.*unknown type name '\([A-Za-z0-9_]*\)'.*/\1/p; s/.*use of undeclared identifier '\([A-Za-z0-9_]*\)'.*/\1/p" | sort -u)
echo "records the headers do not declare: $(echo "$missing" | grep -c .)"
echo "$missing" | paste -sd' '
# shellcheck disable=SC2086
"$binary" layout-check "$winmd" "$set_name" "$work/layout.c" $missing
compile "$work/layout.c" > "$work/layout-errors.txt"; echo "compile status $?"
echo "assertions that fail: $(grep -c 'static assertion failed' "$work/layout-errors.txt")"
grep -o '"\(SIZE\|OFFSET\) [^"]*"' "$work/layout-errors.txt" | head -40
grep -E 'error:' "$work/layout-errors.txt" | grep -v 'static assertion failed' | sort | uniq -c | sort -rn | head -20
# The check must see a wrong layout: one planted assertion, which must be the only failure.
{ cat "$work/layout.c"; echo '_Static_assert(sizeof(D3D11_BUFFER_DESC) == 23, "SIZE planted");'; } > "$work/layout-planted.c"
echo "planted: $(compile "$work/layout-planted.c" | grep -c 'static assertion failed') failure (must be 1)"
