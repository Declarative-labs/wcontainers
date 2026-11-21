# Library swaps

Four places where our own code does a job an established library already does, from the library
audit of 2026-11-21. Each swap keeps behaviour exactly: same inputs, same outputs, same errors,
proven by the tests that cover the code (one is added where none does). The library replaces our
code in the same commit. A library that turns out not to fit is dropped with one concrete example
of why, and our code stays. Each new dependency's licence goes in the status file.

## L1. zstd image layers (runtime, single-kernel image.rs)

- Today image.rs refuses `application/vnd.oci.image.layer.v1.tar+zstd` (and the non-distributable
  form). Read them with ruzstd (MIT, pure Rust, builds for WebAssembly).
- A layer is a zstd stream, which may hold several frames and skippable frames, and a frame may
  carry a checksum. ruzstd's `StreamingDecoder` decodes one frame and does not check the checksum,
  so a small reader over the blob's bytes runs ruzstd's `FrameDecoder` frame after frame, passes
  over skippable frames and checks each checksum, as libzstd does.
- Streaming: the blob is read once (as for gzip, to check its digest) and decoded straight into the
  tar reader; the decompressed layer is never held whole.
- Tests: in image.rs, a gzip layout and its zstd twin from the same tar give the same file tree;
  seeded random streams (a case-count knob) of several frames, skippable frames and checksums,
  made by libzstd (the reference, through the `zstd` crate as a dev-dependency only), decode to
  what libzstd decodes; a damaged frame and a wrong checksum are refused. In the wasmtime
  executor's surface test, the zstd twin of the `alpine:3.22` image runs as the gzip image.
- Measure the decode speed against libzstd on the Mac and record it.

## L2. The wasmtime cache file framing (executor, wasmtime)

- The hand-written framing of a cache file (about 50 lines) becomes serde plus postcard, with a
  borrowed `&[u8]` for the module bytes so that loading copies nothing it did not copy before.
- The magic changes so that an old cache file is refused, as any unreadable file is today.

## L3. Write-then-rename in hardware-virtualization's translate.rs

- The write to a partial file and rename (about 16 lines) becomes `tempfile`'s
  `NamedTempFile::persist` in the same directory. Same failure behaviour: a failed store leaves no
  file behind and never a half-written one under the final name.

## L4. ARM64 encoder field placement (wcontainers-arm64 encode.rs)

- `word()`, `field_of()` and `width()` (a HashMap and a string search over Arm's forms table for every
  field of every word) go. The milestone compares aarchmrs-instructions (BSD-3-Clause, generated from the
  same Arm release, no fit checks) on three forms with a table of our own made from the forms data we
  already ship, and takes one; either way our `DoesNotFit` check wraps every field.
- What stays ours: forms.rs and a64-forms.txt, logical immediates, move sequences, branch scaling,
  the lookup from a word to its form, replace, retarget and moves_of_x0.
- Proven by the encoder's existing tests and the translator's tests that use it, and a new seeded test
  that holds every builder to the old by-name placement over values that fit and that do not.

## Order

L1, L2, L3, L4, one commit per swap. Each file is shared with another track: a file another agent
has uncommitted changes in waits, and the next item goes first.
