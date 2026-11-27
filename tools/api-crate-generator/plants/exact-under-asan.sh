#!/bin/bash
# The exactly sized C tests (the Direct3D 11 direct crate's tests/exact) under AddressSanitizer, the C instrumented
# with zig's clang: the command a plant of the length rules must fail.
set -uo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
cd "$root"
host=$(rustc -vV | sed -n "s/^host: //p")
env RUSTC_BOOTSTRAP=1 STAND_INS_ASAN=1 "CC_$(echo "$host" | tr - _)=$root/tools/api-crate-generator/harness/stand-ins/zigcc" \
  ASAN_OPTIONS=detect_leaks=0:abort_on_error=1:halt_on_error=1:allocator_may_return_null=1 \
  RUSTFLAGS="-Zcrate-attr=feature(register_tool) -Zcrate-attr=register_tool(reviewable) -Zsanitizer=address -C force-frame-pointers=yes -C link-arg=-latomic" \
  CARGO_TARGET_DIR=${EXACT_TARGET:-$root/target/exact-asan} \
  cargo test -q --release --target "$host" -p wcontainers-direct3d-11-direct --features c-caller --test exact -- --test-threads=1
