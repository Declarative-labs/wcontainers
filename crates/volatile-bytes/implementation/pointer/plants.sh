#!/usr/bin/env bash
# The plants of the unsafe copies in implementation.rs: each is one wrong form of the code that feeds a copy, and the
# test that must fail on it; a crash counts, as the empty buffer's plant copies to a dangling pointer. Run from the
# workspace root, on Linux (one test reads a memory file). Each plant is applied alone, the test is run, and the file is
# put back; the script fails unless the test passes unplanted and fails on every plant. A shift of every address by the
# same byte is the same through every view, so only a test that reads the memory file itself catches it.
set -u
file=crates/volatile-bytes/implementation/pointer/implementation.rs
pointer="wcontainers-volatile-bytes-pointer pointer"
mapping="wcontainers-volatile-bytes-shared-mapping mapping"
# name @@ the code as it is @@ the plant @@ the package, the test target and the test that catches it
plants=(
  "end one byte late@@let left = length.checked_sub(at)?;@@let left = length.checked_add(1)?.checked_sub(at)?;@@$pointer reads_and_writes_nothing_at_the_length"
  "end one byte early@@let left = length.checked_sub(at)?;@@let left = length.checked_sub(1)?.checked_sub(at)?;@@$pointer reads_and_writes_the_last_byte"
  "start one byte late@@Some((address.checked_add(at)?, buffer.min(left)))@@Some((address.checked_add(at)?.checked_add(1)?, buffer.min(left)))@@$mapping file::writes_at_an_offset_reach_the_files_bytes_at_that_offset"
  "start one byte early@@Some((address.checked_add(at)?, buffer.min(left)))@@Some((address.checked_add(at.saturating_sub(1))?, buffer.min(left)))@@$pointer reads_and_writes_at_an_offset_inside"
  "count one past the buffer@@Some((address.checked_add(at)?, buffer.min(left)))@@Some((address.checked_add(at)?, buffer.checked_add(1)?.min(left)))@@$pointer reads_and_writes_at_an_offset_inside"
  "count one short of the buffer@@Some((address.checked_add(at)?, buffer.min(left)))@@Some((address.checked_add(at)?, buffer.saturating_sub(1).min(left)))@@$pointer reads_and_writes_at_an_offset_inside"
  "offset dropped from the start@@Some((address.checked_add(at)?, buffer.min(left)))@@Some((address, buffer.min(left)))@@$pointer reads_and_writes_at_an_offset_inside"
  "offset dropped from the count@@let left = length.checked_sub(at)?;@@let left = length;@@$pointer reads_and_writes_nothing_past_the_end"
  "count from the buffer alone@@Some((address.checked_add(at)?, buffer.min(left)))@@Some((address.checked_add(at)?, buffer))@@$pointer reads_and_writes_what_is_left_of_a_longer_buffer"
  "count from the view alone@@Some((address.checked_add(at)?, buffer.min(left)))@@Some((address.checked_add(at)?, left))@@$pointer reads_and_writes_nothing_with_an_empty_buffer"
  $'offset and count wrapping@@let left = length.checked_sub(at)?;\n    Some((address.checked_add(at)?, buffer.min(left)))@@let left = length.wrapping_sub(at);\n    Some((address.wrapping_add(at), buffer.min(left)))@@'"$pointer reads_and_writes_nothing_at_the_largest_offset"
  "read's source and destination swapped@@copy(into.as_mut_ptr(), from, count);@@copy(from, into.as_ptr(), count);@@$pointer reads_and_writes_at_an_offset_inside"
  "write's source and destination swapped@@copy(to, bytes.as_ptr(), count);@@copy(bytes.as_ptr().cast_mut(), to, count);@@$pointer writes_at_an_offset_leave_every_other_byte"
  "read not from the start@@    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {@@    fn read(&self, into: &mut [u8]) -> usize { self.read_at(1, into) }
    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {@@$pointer reads_and_writes_as_from_the_start"
)

original=$(cat "$file"; printf x)
original=${original%x}
restore() { printf '%s' "$original" > "$file"; }
trap restore EXIT

failed=0
for plant in "${plants[@]}"; do
  rest=$plant
  name=${rest%%@@*}; rest=${rest#*@@}
  code=${rest%%@@*}; rest=${rest#*@@}
  planted=${rest%%@@*}; read -r package target test <<< "${rest#*@@}"
  run() { cargo test -q -p "$package" --test "$target" -- --exact "$test" > /dev/null 2>&1; }
  after=${original#*"$code"}
  if [[ $after == "$original" || $after == *"$code"* ]]; then
    echo "BROKEN   $name: the code is not in $file exactly once"; failed=1; continue
  fi
  restore
  if ! run; then
    echo "BROKEN   $name: $test fails unplanted"; failed=1; continue
  fi
  printf '%s' "${original/"$code"/"$planted"}" > "$file"
  if ! cargo test -q --no-run -p "$package" --test "$target" > /dev/null 2>&1; then
    echo "BROKEN   $name: the plant does not build"; failed=1
  elif run; then
    echo "MISSED   $name: $test passes"; failed=1
  else
    echo "CAUGHT   $name: $target::$test"
  fi
  restore
done
exit $failed
