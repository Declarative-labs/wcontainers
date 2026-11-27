#!/bin/bash
# Builds the generator, writes the OpenGL ES and EGL crates twice and compares the two, puts them in the repository's
# crates/, then checks, lints and tests them over the system's EGL (Mesa, llvmpipe, no window system), by default and
# under AddressSanitizer.
#   check-khronos.sh <work folder> [steps]     run from the repository's root, inside its `nix develop`
set -uo pipefail
work=$1; shift
steps=${*:-generate check clippy test asan}
repository=$PWD
generator=$repository/tools/api-crate-generator
has() { case " $steps " in *" $1 "*) return 0 ;; esac; return 1; }
step() { echo; echo "=== $*"; }
jobs=${ARM_JOBS:-12}
crates=(-p wcontainers-egl -p wcontainers-egl-direct -p wcontainers-opengl-es -p wcontainers-opengl-es-direct -p wcontainers-opengl-es-conformance -p wcontainers-opengl-es-webgl)

step "versions and load"
rustc --version; cut -d" " -f1-3 /proc/loadavg

if has generate; then
    step "the registries, pinned"
    mapfile -t inputs < <("$generator/fetch-registries.sh" "$work/registries") || { echo "registries: failed"; exit 1; }
    printf '%s\n' "${inputs[@]}"
    step "the generator: build with every warning, and its tests"
    (cd "$generator" && CARGO_TARGET_DIR=$work/generator cargo build -j "$jobs" --release 2>&1 | grep -E '^(warning|error)|-->' ; echo "build status ${PIPESTATUS[0]}")
    (cd "$generator" && CARGO_TARGET_DIR=$work/generator cargo test -j "$jobs" --release --bins 2>&1 | grep -E '^test |test result|panicked'; echo "test status ${PIPESTATUS[0]}")
    binary=$work/generator/release/api-crate-generator
    step "generate twice"
    rm -rf "$work/first" "$work/second"
    "$binary" khronos "${inputs[@]}" "$work/first" > "$work/report.txt"; echo "first status $?"
    "$binary" khronos "${inputs[@]}" "$work/second" > /dev/null; echo "second status $?"
    diff -r "$work/first" "$work/second" > /dev/null && echo "IDENTICAL: the two runs wrote the same files" || echo "DIFFERENT: the two runs differ"
    cat "$work/report.txt"
    # The browser implementation (crates/opengl-es/implementation/webgl) is written by hand: kept as it is.
    for crate in egl opengl-es; do mkdir -p "crates/$crate"; rsync -a --delete --exclude /implementation/webgl "$work/first/crates/$crate/" "crates/$crate/"; done
    find crates/egl crates/opengl-es -name '*.rs' | sort | xargs wc -l | tail -1
fi
if has check; then
    step "check, Linux and wasm32"
    cargo check -j "$jobs" "${crates[@]}" --all-targets --message-format short 2>&1 | grep -E ': (warning|error)|^error' | sort | uniq | sort -t: -k4 | head -120; echo "check status ${PIPESTATUS[0]}"
    cargo check -j "$jobs" -p wcontainers-egl -p wcontainers-opengl-es -p wcontainers-egl-direct -p wcontainers-opengl-es-direct -p wcontainers-opengl-es-webgl --target wasm32-unknown-unknown 2>&1 | grep -E '^(warning|error)|-->' | head -20; echo "wasm32 check status ${PIPESTATUS[0]}"
fi
if has clippy; then
    step "clippy -D warnings, all targets"
    cargo clippy -j "$jobs" "${crates[@]}" --all-targets --message-format short -- -D warnings 2>&1 | grep -E ': (warning|error)|^error' | sort | uniq | head -120; echo "clippy status ${PIPESTATUS[0]}"
    cargo clippy -j "$jobs" -p wcontainers-opengl-es-webgl --target wasm32-unknown-unknown --message-format short -- -D warnings 2>&1 | grep -E ': (warning|error)|^error' | sort | uniq | head -60; echo "wasm32 clippy status ${PIPESTATUS[0]}"
fi
# Mesa's EGL from the repository's own nixpkgs (libglvnd's libEGL with Mesa's vendor library, llvmpipe), with no
# window system.
read -r mesa glvnd < <(nix build --inputs-from . nixpkgs#mesa nixpkgs#libglvnd --no-link --print-out-paths | paste -sd' ')
echo "EGL: $glvnd/lib/libEGL.so.1 with $mesa/share/glvnd/egl_vendor.d/50_mesa.json"
export EGL_PLATFORM=surfaceless WCONTAINERS_EGL_FOLDER=${WCONTAINERS_EGL_FOLDER:-$glvnd/lib}
export __EGL_VENDOR_LIBRARY_FILENAMES=$mesa/share/glvnd/egl_vendor.d/50_mesa.json
if has test; then
    step "the suite on the direct crates over the system's EGL"
    timeout 900 cargo test -j "$jobs" -p wcontainers-opengl-es-direct --release -- --test-threads=4 2>&1 | grep -E '^test |test result|panicked|error|left|right|assert' | head -60; echo "test status ${PIPESTATUS[0]}"
fi
if has asan; then
    step "the suite under AddressSanitizer"
    RUSTFLAGS="-Zcrate-attr=feature(register_tool) -Zcrate-attr=register_tool(reviewable) -Zsanitizer=address -C force-frame-pointers=yes" \
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR=$work/asan \
        timeout 1800 cargo test -j "$jobs" -p wcontainers-opengl-es-direct --release --target "$(rustc -vV | sed -n 's/^host: //p')" -- --test-threads=4 2>&1 |
        grep -E '^test |test result|panicked|ERROR: AddressSanitizer|SUMMARY|left|right' | head -60; echo "asan status ${PIPESTATUS[0]}"
fi
if has unsafe; then
    step "lines naming unsafe per crate (the root manifest's way: comments included)"
    for crate in crates/egl crates/egl/implementation/direct crates/opengl-es crates/opengl-es/implementation/direct crates/opengl-es/implementation/webgl crates/opengl-es/conformance crates/gpu-webgl crates/nir; do
        [ -d "$crate" ] || continue
        echo "$(find "$crate" -name '*.rs' -not -path "$crate/implementation/*" -exec grep -c unsafe /dev/null {} + 2>/dev/null | awk -F: '{s+=$NF} END {print s+0}') $crate"
    done
    echo "marks: $(grep -rc 'expect(unsafe_code' crates/egl crates/opengl-es | awk -F: '{s+=$NF} END {print s+0}')"
fi
if has lint && [ -n "${LINTER:-}" ]; then
    step "the linter on the two crates (no rule loosened)"
    "$LINTER" reviewable check > "$work/linter.txt" 2>&1; echo "linter status $?"
    awk '/^error\[reviewable::/ { match($0, /reviewable::[a-z_]+/); rule = substr($0, RSTART + 12, RLENGTH - 12); next }
         /^ *--> crates\/(egl|opengl-es)\// { count[rule]++ } END { for (r in count) print count[r], r }' "$work/linter.txt" | sort -rn
fi
