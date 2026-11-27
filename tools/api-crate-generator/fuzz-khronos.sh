#!/bin/bash
# Fuzzes the OpenGL ES and EGL glue under AddressSanitizer with a hostile caller (the suite's `fuzz`): over the
# system's EGL (Mesa), and over the generated stand-in library (crates/opengl-es/implementation/direct/fake/fake_egl.c), which reads and writes
# the most each command may and lies wherever the registry leaves the answer to the library.
#   fuzz-khronos.sh <work folder> <runs>      run from the repository's root, inside `nix develop`
# <runs> lists "mode:seed:calls" (mode mesa or liar), run side by side, each one process.
set -uo pipefail
work=$1; shift
runs=$*
mkdir -p "$work/fake" "$work/runs"
cc -O1 -g -std=c11 -shared -fPIC -o "$work/fake/libEGL.so.1" crates/opengl-es/implementation/direct/fake/fake_egl.c -ldl; echo "fake build status $?"
# The repository's own flags, AddressSanitizer, and the executable's symbols exported so that the stand-in finds
# AddressSanitizer's region check.
export RUSTFLAGS="-Zcrate-attr=feature(register_tool) -Zcrate-attr=register_tool(reviewable) -Zsanitizer=address -C force-frame-pointers=yes -C link-arg=-rdynamic"
export RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR=$work/asan
cargo test -q -j "${ARM_JOBS:-12}" -p wcontainers-opengl-es-direct --release --target x86_64-unknown-linux-gnu --no-run 2>&1 | tail -3; echo "asan build status ${PIPESTATUS[0]}"
binary=$(ls -t "$work"/asan/x86_64-unknown-linux-gnu/release/deps/conformance-* | grep -v '\.d$' | head -1)
echo "binary $binary, $(nm "$binary" | grep -c __asan_report) __asan_report symbols"
read -r mesa glvnd < <(nix build --inputs-from . nixpkgs#mesa nixpkgs#libglvnd --no-link --print-out-paths | paste -sd' ')
export ASAN_OPTIONS=detect_leaks=0:halt_on_error=1:abort_on_error=1
for run in $runs; do
    IFS=: read -r mode seed calls <<< "$run"
    log=$work/runs/$mode-$seed-$calls.log
    (
        if [ "$mode" = mesa ]; then
            export LP_NUM_THREADS=2 EGL_PLATFORM=surfaceless WCONTAINERS_EGL_FOLDER=$glvnd/lib __EGL_VENDOR_LIBRARY_FILENAMES=$mesa/share/glvnd/egl_vendor.d/50_mesa.json
        else
            export WCONTAINERS_EGL_FOLDER=$work/fake FAKE_LIES=1 FAKE_SEED=$seed FAKE_SAY=1
        fi
        start=$(date +%s)
        WCONTAINERS_FUZZ_SEED=$seed WCONTAINERS_FUZZ_CALLS=$calls timeout 14400 "$binary" --exact direct::fuzz_from_the_environment --nocapture --test-threads=1 > "$log" 2>&1
        echo "status $? seconds $(( $(date +%s) - start ))" >> "$log"
    ) &
done
wait
for run in $runs; do
    IFS=: read -r mode seed calls <<< "$run"
    log=$work/runs/$mode-$seed-$calls.log
    echo "RUN $run: $(grep -oE 'FUZZ seed.*|^FAKE.*|ERROR: AddressSanitizer.*|SUMMARY.*|panicked.*|^status.*' "$log" | tr '\n' ' ' | cut -c1-400)"
done
