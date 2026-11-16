#!/bin/sh
# The benchmark set against elfconv (plan section 10): each workload of programs/bench.c built as an ordinary
# static glibc program with clang 16 inside elfconv's published image and converted by it to WASI WebAssembly,
# then run in wasmtime (compiled ahead, best of WC_BENCH_RUNS) and natively in Docker (best of the same), each
# printed as times its own native build; then ours, the same workloads in wasmtime at full look-ahead with every
# check. Run from the repository root: `nix develop -c crates/wcontainers-translator-test/elfconv.sh`.
# Needs Docker and the image ghcr.io/yomaytk/elfconv:arm64 (6.1 GB, pulled once). Converted programs are kept
# in target/elfconv and not converted again.
set -eu
image=ghcr.io/yomaytk/elfconv:arm64
runs=${WC_BENCH_RUNS:-3}
root=$(pwd)
out=$root/target/elfconv
mkdir -p "$out"
cp "$root/crates/wcontainers-translator-test/programs/bench.c" "$out/bench.c"
load() { sysctl -n vm.loadavg 2>/dev/null || cut -d' ' -f1-3 /proc/loadavg; }

# Build and convert inside the image; bench.c's own _start is renamed away so glibc's starts it.
docker run --rm --name wc-translator-elfconv-convert -v "$out":/work --entrypoint /bin/bash "$image" -c '
export LD_LIBRARY_PATH=${LD_LIBRARY_PATH:-}
source /root/.bash_profile
cd /root/elfconv/bin
for k in 1 2 3 4 5 6 7 8; do
  name=bench-$k
  [ -s /work/$name.elf.wasm ] && continue
  clang-16 --target=aarch64-linux-gnu -O2 -static -DSCALE=1 -DWORKLOAD=$k -D_start=bench_own_start \
    -o /work/$name.elf /work/bench.c
  TARGET=aarch64-wasi32 ./exe.sh /work/$name.elf > /work/$name.log 2>&1 || echo "workload $k: elfconv failed to convert"
  cp $name.elf.wasm /work/ 2>/dev/null || echo "workload $k: elfconv made no WebAssembly"
done'

# Native times of the same glibc executables.
docker run --rm --name wc-translator-elfconv-native -v "$out":/work --entrypoint /bin/bash "$image" -c "
for k in 1 2 3 4 5 6 7 8; do
  t=\$(for r in \$(seq $runs); do { /usr/bin/time -f %e /work/bench-\$k.elf > /dev/null; } 2>&1; done | sort -n | head -1)
  echo \$k \$t
done" > "$out/native.txt"

for k in 1 2 3 4 5 6 7 8; do
  native=$(awk -v k="$k" '$1 == k { print $2 }' "$out/native.txt")
  if [ ! -s "$out/bench-$k.elf.wasm" ]; then
    echo "workload $k: elfconv made no WebAssembly; native $native s"
    continue
  fi
  wasmtime compile -o "$out/bench-$k.cwasm" "$out/bench-$k.elf.wasm"
  best=$(for r in $(seq "$runs"); do
    /usr/bin/time -p wasmtime run --allow-precompiled "$out/bench-$k.cwasm" 2>&1 >/dev/null | awk '/^real/ { print $2 }'
  done | sort -n | head -1)
  output=$(wasmtime run --allow-precompiled "$out/bench-$k.cwasm" 2>&1 | head -c 200 | tr '\n' ' ')
  echo "workload $k: elfconv in wasmtime $best s, native $native s, $(echo "$best $native" | awk '{ printf "%.2f", $1 / $2 }') times native; output: $output(load $(load))"
done

# Ours on the same workloads (built without a C library, so only the ratios to native compare).
WC_BENCH_RUNS=$runs WC_BENCH_VARIANTS=1 WC_BENCH_CHROME=0 cargo test --release -p wcontainers-translator-test \
  --test benchmarks measure_benchmarks -- --ignored --nocapture 2>&1 | grep -E "native|times native"
