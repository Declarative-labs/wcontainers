#!/bin/sh
# Makes the kernel harness's `runtime` feature read LOOK_AHEAD (full, none or bounded; bounded when unset, the
# WebAssembly executor's own default), as the feature's comment in the stand-in's Cargo.toml says it does: today
# driver.rs's make_back always takes the executor's default. Run it in a throwaway copy, never in a checkout you commit
# from, from the repository's root; running it twice changes nothing.
# Usage: sh tools/runtime-modes/la-switch.sh, then for example LOOK_AHEAD=full cargo test -p wcontainers-executor-unicorn --features runtime ...
set -eu
manifest=crates/executor/implementation/unicorn/Cargo.toml
driver=crates/executor/implementation/unicorn/driver.rs
if grep -q 'LOOK_AHEAD' "$driver"; then
    echo "look-ahead switch: already in place"
    exit 0
fi
grep -q '^wcontainers-arm64 = { path = "../../../arm64", optional = true }$' "$manifest"
grep -q '^runtime = \["harness", "dep:wcontainers-executor-wasmtime", "dep:wcontainers-arm64"\]$' "$manifest"
[ "$(grep -c '^        \.\.defaults$' "$driver")" = 1 ]
sed -i.orig \
    -e '/^wcontainers-arm64 = { path = "..\/..\/..\/arm64", optional = true }$/a\
wcontainers-translator = { path = "../../../translator", optional = true }' \
    -e 's/^runtime = \["harness", "dep:wcontainers-executor-wasmtime", "dep:wcontainers-arm64"\]$/runtime = ["harness", "dep:wcontainers-executor-wasmtime", "dep:wcontainers-arm64", "dep:wcontainers-translator"]/' \
    "$manifest"
sed -i.orig -e 's/^        \.\.defaults$/        look_ahead: match std::env::var("LOOK_AHEAD").as_deref() {\
            Ok("full") => wcontainers_translator::LookAhead::Full,\
            Ok("none") => wcontainers_translator::LookAhead::None,\
            Ok("bounded") | Err(_) => defaults.look_ahead,\
            Ok(other) => panic!("LOOK_AHEAD is full, none or bounded, not {other}"),\
        },\
        ..defaults/' "$driver"
rm -f "$manifest.orig" "$driver.orig"
echo "look-ahead switch: $(grep -c 'dep:wcontainers-translator' "$manifest") $(grep -c 'LOOK_AHEAD' "$driver")"
