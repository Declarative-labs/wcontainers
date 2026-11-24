# native.sh: the crate's suites, the builds for other targets, the copy's listing, the compile refusals and the wrong
# changes, natively. Run in the synced tree.
set -u
here=$(pwd)
arch=$(uname -m)
load() { echo "load $(cut -d' ' -f1-3 /proc/loadavg)"; }
filter() { grep -E "^test |FAILED|panicked at|test result: |error(\[|:)|signal|SIGBUS|SIGSEGV|Running" | grep -v "^test .* ok$" | grep -v " 0 passed; 0 failed" | head -${1:-30}; }
echo "code $(bash crates/shared-memory/checks/tree.sh)"
echo "===== $arch native suites $(load)"
cargo test -p wcontainers-shared-memory -p wcontainers-shared-memory-conformance --features host 2>&1 | filter 60
echo "-- the same suites optimized $(load)"
cargo test --release -p wcontainers-shared-memory --features host --test host --test guard --test process 2>&1 | grep -E "test result: |FAILED|panicked|signal" | head -10
echo "-- what the guard and process tests saw"
cargo test -q -p wcontainers-shared-memory --features host --test guard --test process -- --nocapture 2>&1 | grep -E "copied|bytes read|test result"
echo "-- clippy"
cargo clippy -q -p wcontainers-shared-memory -p wcontainers-shared-memory-conformance --features host --all-targets --no-deps -- -D warnings 2>&1 | tail -20
echo "-- the other architecture's host build, the floor's target and a target with no C library build"
case $arch in x86_64) other=aarch64 ;; *) other=x86_64 ;; esac
for target in $other-unknown-linux-musl aarch64-unknown-none wasm32-unknown-unknown; do
  features=$([ "$target" = "$other-unknown-linux-musl" ] && echo "--features host")
  cargo build -q -p wcontainers-shared-memory $features --target $target 2>&1 | tail -5; echo "$target: status ${PIPESTATUS[0]}"
done

echo "===== the listings of HostMapping's read, write, swap and compare-and-exchange and the region's copy, optimized"
rm -rf target/gmc-asm
CARGO_TARGET_DIR=target/gmc-asm cargo rustc -q --release -p wcontainers-shared-memory --features host --lib -- --emit asm -C codegen-units=1 2>&1 | tail -3
s=$(ls target/gmc-asm/release/deps/wcontainers_shared_memory-*.s | head -1)
for pattern in 'HostMapping.*SharedMemory.*4read' 'HostMapping.*SharedMemory.*5write' 'region4copy' 'HostMapping.*SharedMemory.*4swap' 'HostMapping.*SharedMemory.*16compare_exchange'; do
  symbol=$(grep -E "^_?_ZN.*${pattern}.*:\$|^_R.*${pattern}.*:\$" "$s" | head -1 | tr -d ':')
  echo "--- symbol: ${symbol:-none matching $pattern}"
  [ -n "$symbol" ] && awk -v f="$symbol:" '$0==f{p=1} p{print} p&&/\.cfi_endproc/{exit}' "$s" | grep -vE '^\s*\.(cfi|p2align|loc|file)' | head -60
done
echo "--- every line of the crate's listing that names memcpy"
grep -n memcpy "$s" | head -20
rm -rf target/gmc-asm

[ "${REFUSALS:-1}" = 1 ] && {
echo "===== compile refusals"
for case in outlives part_of_part field seal_named seal_missing byte_word heap redirect; do
  echo "-- $case"
  RUSTFLAGS="--cfg refused=\"$case\"" CARGO_TARGET_DIR=target/gmc-refused cargo test -q --no-run -p wcontainers-shared-memory --features host --test refused 2>&1 | grep -E "^error" | grep -v "could not compile" | head -3
done
rm -rf target/gmc-refused; }

[ "${MUTANTS:-1}" = 1 ] || exit 0
mutate() { # name, file, tests, then old and new texts in pairs
  local name=$1 file=$2 tests=$3 dir=/work/target/gmc-mut-$1
  shift 3
  rm -rf "$dir"; mkdir -p "$dir"; (cd "$here" && tar --exclude=./target -cf - .) | tar -C "$dir" -xf -
  python3 - "$dir/$file" "$@" <<'PY'
import sys
p, *pairs = sys.argv[1:]
s = open(p).read()
for old, new in zip(pairs[::2], pairs[1::2]):
    assert s.count(old) == 1, "the text to change is not there once: " + old
    s = s.replace(old, new)
open(p, "w").write(s)
PY
  echo "===== wrong change $name: $(cd $dir && diff <(cd $here && cat $file) $file | grep '^>' | tr -s ' ' | head -3)"
  ( cd "$dir"; CARGO_TARGET_DIR=/work/target/gmc-mutn cargo test -q -p wcontainers-shared-memory --features host $tests 2>&1 | filter 8 )
  rm -rf "$dir"
}
R=crates/shared-memory/foreign_implementation/region
H=crates/shared-memory/foreign_implementation/host/implementation.rs
A=crates/shared-memory/arithmetic.rs
mutate copy-reads-one-byte-past-the-end $R/implementation.rs "--test guard" 'memcpy(to.cast(), from.cast(), length);' 'memcpy(to.cast(), from.cast(), length + 1);'
mutate copy-starts-one-byte-early $R/implementation.rs "--test guard" 'memcpy(to.cast(), from.cast(), length);' 'memcpy(to.wrapping_sub(1).cast(), from.wrapping_sub(1).cast(), length);'
mutate swap-is-a-load-and-a-store $R/implementation.rs "--test host" 'Ok(unsafe { AtomicU64::from_ptr(word) }.swap(value, Ordering::AcqRel))' 'Ok(unsafe { let old = word.read_volatile(); word.write_volatile(value); old })'
mutate compare-exchange-is-a-load-and-a-store $R/implementation.rs "--test host" 'let exchanged = unsafe { AtomicU32::from_ptr(word) }.compare_exchange(
            current,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        );' 'let exchanged: Result<u32, u32> = unsafe {
            let old = word.read_volatile();
            if old == current { word.write_volatile(new); Ok(old) } else { Err(old) }
        };'
mutate range-one-byte-long $A "--test host" 'if end > u64::try_from(length)' 'if end > 1 + u64::try_from(length)'
mutate misaligned-word-accepted $A "--test host" 'if !index.is_multiple_of(size) {' 'if false && !index.is_multiple_of(size) {'
mutate part-past-its-memory crates/shared-memory/interface.rs "--test host" 'offsets.end > self.len()' 'offsets.end > self.len() + 1'
mutate length-rounded-to-a-page "$H" "--test host" 'Region::new(base, map.len())' 'Region::new(base, map.len().next_multiple_of(4096))'
mutate grow-seal-accepted "$H" "--test host" 'seals.contains(SealFlags::SHRINK)' 'seals.intersects(SealFlags::SHRINK | SealFlags::GROW)'
mutate length-unchecked "$H" "--test host" '            if offset
                .checked_add(length as u64)
                .is_none_or(|end| end > size)' '            if false && offset
                .checked_add(length as u64)
                .is_none_or(|end| end > size)'
mutate size-read-before-the-seals "$H" "--test host" "        file: BorrowedFd<'fd>,
    }" "        file: BorrowedFd<'fd>,
        size: u64,
    }" '            if !fcntl_get_seals(file)' '            let size = u64::try_from(fstat(file).map_err(io::Error::from)?.st_size)
                .unwrap_or(u64::MIN);
            if !fcntl_get_seals(file)' 'Ok(Self { file })' 'Ok(Self { file, size })' '            let size = u64::try_from(fstat(self.file).map_err(io::Error::from)?.st_size)
                .unwrap_or(u64::MIN);' '            let size = self.size;'
mutate huge-pages-allowed "$H" "--test host" 'if fstatfs(file).map_err(io::Error::from)?.f_type as u32 == libc::HUGETLBFS_MAGIC as u32' 'if false && fstatfs(file).map_err(io::Error::from)?.f_type as u32 == libc::HUGETLBFS_MAGIC as u32'
mutate base-one-word-late "$H" "--test host" 'let base = map.as_mut_ptr();' 'let base = map.as_mut_ptr().wrapping_add(8);'
mutate base-at-the-page-start "$H" "--test host" 'let base = map.as_mut_ptr();' 'let base = map.as_mut_ptr().wrapping_sub(map.as_ptr() as usize % 4096);'
P=crates/shared-memory/foreign_implementation/host/process.rs
mutate parent-death-signal-unset $P "--test process -- the_other_process_ends_when" '    set_parent_process_death_signal(Some(Signal::KILL)).expect("the parent-death signal is set");' ''

# A planted fault fails the process test; then counts the other processes left running, and ends them.
orphans() { pgrep -fc -- '--exact the_other_process' || true; }
planted() { # name, then old and new texts in pairs beside the planted fault
  local name=$1 dir=/work/target/gmc-plant-$1 log=/work/target/gmc-plant-$1.log
  shift
  rm -rf "$dir"; mkdir -p "$dir"; (cd "$here" && tar --exclude=./target -cf - .) | tar -C "$dir" -xf -
  python3 - "$dir/$R/implementation.rs" 'memcpy(to.cast(), from.cast(), length);' 'memcpy(to.cast(), from.cast(), length + 1);' <<'PY'
import sys
p, old, new = sys.argv[1:]
s = open(p).read()
assert s.count(old) == 1, "the text to change is not there once: " + old
open(p, "w").write(s.replace(old, new))
PY
  [ $# = 0 ] || python3 - "$dir/$P" "$@" <<'PY'
import sys
p, *pairs = sys.argv[1:]
s = open(p).read()
for old, new in zip(pairs[::2], pairs[1::2]):
    assert s.count(old) == 1, "the text to change is not there once: " + old
    s = s.replace(old, new)
open(p, "w").write(s)
PY
  echo "===== planted fault, a copy one byte long, in the process test: $name; other processes before: $(orphans)"
  ( cd "$dir"; CARGO_TARGET_DIR=/work/target/gmc-plant timeout 900 cargo test -q -p wcontainers-shared-memory --features host --test process > "$log" 2>&1; echo "cargo status $?" )
  grep -E "^test .*FAILED|panicked at|test result: |signal" "$log" | head -8
  sleep 3
  echo "other processes still running 3 s after the test ended: $(orphans)"
  pkill -KILL -f -- '--exact the_other_process' || true
  rm -rf "$dir" "$log"
}
planted "without the parent-death signal" '    set_parent_process_death_signal(Some(Signal::KILL)).expect("the parent-death signal is set");' ''
planted "with the parent-death signal"
rm -rf /work/target/gmc-mutn /work/target/gmc-plant
echo "done $(load)"
