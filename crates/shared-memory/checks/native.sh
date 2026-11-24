# native.sh: the crate's suites, the compile refusals and the wrong changes, natively. Run in the synced tree.
set -u
here=$(pwd)
load() { echo "load $(cut -d' ' -f1-3 /proc/loadavg)"; }
filter() { grep -E "^test |FAILED|panicked at|test result: |error(\[|:)|signal|SIGBUS|Running" | grep -v "^test .* ok$" | grep -v " 0 passed; 0 failed" | head -${1:-30}; }
echo "===== $(uname -m) native suites $(load)"
cargo test -p wcontainers-shared-memory -p wcontainers-shared-memory-conformance --features host 2>&1 | filter 40
echo "-- host test names"
cargo test -q -p wcontainers-shared-memory --features host --test host -- --list 2>&1 | grep -c ": test$"
echo "-- clippy"
cargo clippy -q -p wcontainers-shared-memory -p wcontainers-shared-memory-conformance --features host --all-targets --no-deps -- -D warnings 2>&1 | tail -5
echo "-- the inside's target, no host feature"
cargo build -q -p wcontainers-shared-memory --target aarch64-unknown-none 2>&1 | tail -5; echo "status $?"
[ "${REFUSALS:-1}" = 1 ] && {
echo "===== compile refusals"
for case in outlives part_of_part field replace seal_named seal_missing heap redirect; do
  echo "-- $case"
  RUSTFLAGS="--cfg refused=\"$case\"" CARGO_TARGET_DIR=/work/target/rprim-refused cargo test -q --no-run -p wcontainers-shared-memory --features host --test refused 2>&1 | grep -E "^error" | grep -v "could not compile" | head -3
done
echo "-- heap, with --cfg miri set on a normal build (a convention, not a guarantee: this builds)"
RUSTFLAGS='--cfg refused="heap" --cfg miri' CARGO_TARGET_DIR=/work/target/rprim-refused cargo test -q --no-run -p wcontainers-shared-memory --features host --test refused 2>&1 | grep -E "^error" | grep -v "could not compile" | head -3; echo "status ${PIPESTATUS[0]}"
rm -rf /work/target/rprim-refused; }
echo "-- the race test alone, its tries"
cargo test -q -p wcontainers-shared-memory --features host --test host -- --nocapture a_shrink_then_seal 2>&1 | grep -E "tries|test result"

[ "${MUTANTS:-1}" = 1 ] || exit 0
mutate() { # name, file, then old and new texts in pairs
  local name=$1 file=$2 dir=/work/target/rprim-mut-$1
  shift 2
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
  echo "===== wrong change $name: $(cd $dir && diff <(cd $here && cat $file) $file | grep '^>')"
  ( cd "$dir"; CARGO_TARGET_DIR=/work/target/rprim-mutn cargo test -q -p wcontainers-shared-memory --features host 2>&1 | filter 12 )
  rm -rf "$dir"
}
H=crates/shared-memory/foreign_implementation/host/implementation.rs
mutate length-rounded-to-a-page "$H" '        self.map.len()
    }' '        self.map.len().next_multiple_of(4096)
    }'
mutate grow-seal-accepted "$H" 'seals.contains(SealFlags::SHRINK)' 'seals.intersects(SealFlags::SHRINK | SealFlags::GROW)'
mutate any-seal-accepted "$H" 'seals.contains(SealFlags::SHRINK)' 'seals.contains(SealFlags::SHRINK) || true'
mutate length-unchecked "$H" '            if offset
                .checked_add(length as u64)
                .is_none_or(|end| end > size)' '            if false && offset
                .checked_add(length as u64)
                .is_none_or(|end| end > size)'
mutate offset-off-by-one crates/shared-memory/arithmetic.rs 'if at.checked_add(length as u64)? > count as u64 * WORD as u64' 'if at.checked_add(length as u64)? > count as u64 * WORD as u64 + 1'
mutate part-past-parent crates/shared-memory/arithmetic.rs 'if start > end || end > count as u64 * WORD as u64' 'if start > end || end > count as u64 * WORD as u64 + 1'
mutate size-read-before-the-seals "$H" "        file: BorrowedFd<'fd>,
    }" "        file: BorrowedFd<'fd>,
        size: u64,
    }" '            if !fcntl_get_seals(file)' '            let size = u64::try_from(fstat(file).map_err(io::Error::from)?.st_size)
                .unwrap_or(u64::MIN);
            if !fcntl_get_seals(file)' 'Ok(Self { file })' 'Ok(Self { file, size })' '            let size = u64::try_from(fstat(self.file).map_err(io::Error::from)?.st_size)
                .unwrap_or(u64::MIN);' '            let size = self.size;'
mutate huge-pages-allowed "$H" 'if fstatfs(file).map_err(io::Error::from)?.f_type as u32 == libc::HUGETLBFS_MAGIC as u32' 'if false && fstatfs(file).map_err(io::Error::from)?.f_type as u32 == libc::HUGETLBFS_MAGIC as u32'
mutate base-one-word-late "$H" 'NonNull::new(self.map.as_mut_ptr().cast())' 'NonNull::new(self.map.as_mut_ptr().wrapping_add(8).cast())'
mutate base-at-the-page-start "$H" 'NonNull::new(self.map.as_mut_ptr().cast())' 'NonNull::new(self.map.as_mut_ptr().wrapping_sub(self.map.as_ptr() as usize % 4096).cast())'
rm -rf /work/target/rprim-mutn
echo "done $(load)"
