# Status: library swaps

The plan is docs/plan-library-swaps.md. Four swaps, L1 to L4, one commit each.

## Done

- L1, zstd image layers. image.rs reads `tar+zstd` layers (and the non-distributable form) with
  ruzstd. The layer match moved into `apply_layer`, unchanged for the other media types.
  - `Zstd` in image.rs is a reader over the blob's bytes that runs ruzstd's `FrameDecoder` frame
    after frame, passes over skippable frames, checks each frame's checksum and declared size,
    refuses a frame header with the reserved bit set and a stream with no frame at all, and streams
    straight into the tar reader. It reads the header descriptor byte itself for the reserved bit
    and for whether a size is declared, because ruzstd does not check the one and cannot tell a
    declared zero from none. After the tar's end the rest of the stream is
    read, so the last frame's checksum is checked too (a test fails without it).
  - Tests (image.rs): `a_zstd_layer_gives_the_files_of_its_gzip_twin` (two layers of every entry
    kind and whiteouts: plain, gzip and zstd give the same tree, compared whole);
    `zstd_streams_decode_as_libzstd_decodes_them` (seeded random streams of 1 to 4 frames, levels 1
    to 12, with and without checksums and sizes, skippable frames between, read in random piece
    sizes; `ZSTD_SEED`, default 0, and `ZSTD_CASES`, default 100);
    `damaged_zstd_streams_are_refused_as_libzstd_refuses_them` (a changed block byte, a changed
    checksum, a cut stream, bytes after the last frame, a skippable frame past the end, a wrong
    declared size, the reserved bit set, a declared size of zero over five bytes, an empty stream,
    a window over 128 MiB: libzstd refuses each and so do we; the hand-made frame with its size
    declared right decodes in both; and a layer whose last checksum is wrong is refused).
  - Ran on the Mac: the runtime crate's 13 tests pass; the random test ran 100 cases of seed 0 and
    2000 cases of seed 1 (again after the independent check's fixes).
  - Surface test (wasmtime executor, tests/surface.rs):
    `the_zstd_twin_of_the_oci_image_runs_as_the_image` builds the zstd twin of the `alpine:3.22`
    image Docker saves (its gzip layer decompressed and compressed again by libzstd), then lists
    every file's name, kind, mode, owner, size, time and link target and every regular file's md5
    in both, and runs the pipeline in the twin against Docker. It names the layout's files with
    image.rs's `LAYOUT_FILE`, `INDEX_FILE` and `BLOBS_DIRECTORY`, made public for it, so the
    literal check finds nothing in this track's lines.
  - Ran on the ARM machine after the independent check's fixes (job
    library-swaps/20261121-195211-1820770, load 9.5 to 17): the whole surface suite, 20 passed and 1
    ignored (as before, run by another test), the new test among them; and the runtime crate's 13
    tests.
  - Independent check of the first run: its six literal-check findings in surface.rs, the wrong
    licences of zstd-safe and zstd-sys, the reserved bit, the empty stream, the declared zero and the
    measurement of StreamingDecoder instead of our reader were all fixed in the second run.

- L1 after its second independent check: the random test now also ends streams with zero to two
  skippable frames of up to 4000 bytes (the shape of zstd:chunked layers); 100 cases of seed 0 and
  2000 of seed 1 pass on the Mac.
- L2 (a), the wasmtime cache file. `parse` and `store` in the executor's implementation.rs frame
  the file as the magic `wcxlate2` then the postcard of a `Vec<CacheModule>`, each the module's
  pieces (`Cow<[u64]>`, borrowed when writing) and wasmtime's serialisation (`&[u8]` through
  serde_bytes, borrowed from the file's bytes when reading). The hand-written counts and lengths
  and `CACHE_WORD` are gone. As before, bytes after the body are not looked at.
  - New test `a_file_of_the_earlier_framing_is_refused` (tests/cache.rs): each file the compiler
    wrote is framed again the old way under the old magic, holding the same modules; the next
    compiler refuses it with `TRANSLATION_REFUSED`, makes it again and the one after loads it.
  - Checked by hand with the old code itself (a probe test in scratch library-swaps/l2-probe, not
    in the repository): the file the old code writes is byte for byte the new file framed the old
    way (53,848 and 53,822 bytes, one module), and that old file put under the new code's name is
    refused, warned, made again and then loaded from disk. Note that here the old file's name also
    differed (the build hash covers the lock file, which changed), so it would not even be looked
    at; the magic is what refuses it when the name is the same.
- L2 (b), translate.rs in the native executor. `store` writes through `tempfile::Builder` in the
  file's directory and `persist`s it; a file that is not put in place is removed when it drops.
  `PARTIAL` and the name made from the process number are gone. The file's mode is set to what
  `std::fs::write` gives (0o666 before the umask), since tempfile alone makes 0600 files.
  - New unit tests in translate.rs: `a_store_that_fails_leaves_no_file` (a directory holds the
    file's name: counted as `FILE_NOT_KEPT`, nothing else left; passed on the old code first) and
    `a_stored_file_is_read_back_with_the_usual_mode` (fails without the mode line: 0600 against
    0644). The existing `tests/kept.rs` and the unit tests pass.
- L2's whole-suite run, on the ARM machine (job library-swaps/20261121-203926-2476499, load 3 at
  the start, 17.9 at the end, after commit 9bb3938f): the wasmtime executor's whole suite (unit 5,
  cache 4, code 4, faults 2, fork 5, refused 77, surface 20 passed and 1 ignored as before, turns
  5, views 6), the runtime crate's 13, and the native executor's unit tests (12) and kept.rs (1),
  all passed. On the Mac the same cache, kept and unit tests pass.

## Measurements

- 2026-11-21 on the Mac (M5 Max), load average 14.6 to 38 (shared with other tracks), release
  build, 21 runs each, the `alpine:3.22` arm64 layer (8,821,760-byte tar; gzip 4,127,385 bytes;
  zstd level 3 4,115,642 bytes):
  - image load (`Image::oci`): gzip layout median 25.3 ms (25.0 to 26.1), zstd twin median 31.0 ms
    (30.8 to 33.9).
  - decode alone: flate2 16.8 ms, ruzstd's StreamingDecoder 22.9 ms, libzstd 4.8 ms (medians).
    StreamingDecoder is not what ships (one frame, no checksum check); see the next measurement for
    our reader.
- 2026-11-21 on the Mac (M5 Max), load average 40 to 42 (shared with other tracks), release build,
  41 runs each, the same layer, after the reserved-bit and size fixes:
  - image load: gzip median 30.9 ms (min 29.8), zstd twin median 41.4 ms (min 36.1).
  - decode alone: flate2 18.9 ms, StreamingDecoder 27.0 ms, our `Zstd` reader (on FrameDecoder,
    with the checks) 25.9 ms (min 25.2, max 28.1), libzstd 5.6 ms (medians). Our reader costs what
    StreamingDecoder does; ruzstd is about 4.6 times slower than libzstd here (the audit measured
    3.3 times on 64 MB). The higher load makes every figure higher than the first run's.
- The program is in the scratch folder (library-swaps/measure, with src/reader.rs a copy of the
  reader from image.rs); it is not part of the repository. It builds only with
  `RUSTC_BOOTSTRAP=1` (the kernel interface uses `#![feature(register_tool)]`; the checkout sets
  it in .cargo/config.toml): `RUSTC_BOOTSTRAP=1 nix develop <checkout> -c cargo build --release`,
  then `measure-zstd <gzip layout> <twin directory> <runs>`; the layout is what `docker save
  --platform linux/arm64 alpine:3.22` writes, unpacked.

## Dependencies added

- ruzstd 0.9.1 (MIT), with its twox-hash 2.1.5 (MIT), for the runtime.
- serde_bytes 0.11.19 (MIT OR Apache-2.0), new; postcard 1.1.3 and serde 1.0.229 (both MIT OR
  Apache-2.0), already in the workspace: for the wasmtime executor's cache file.
- tempfile 3.27.0 (MIT OR Apache-2.0), already in the workspace, for the native executor.
- zstd 0.13.5 (BSD-3-Clause), zstd-safe 7.4.1 (BSD-3-Clause), zstd-sys 2.1.3+zstd.1.5.7
  (BSD-3-Clause AND BSD-2-Clause), as their Cargo.toml files say: dev-dependencies only, in the runtime and the
  wasmtime executor, as the reference for the tests. flate2 and sha2 also became dev-dependencies
  of the wasmtime executor for the twin.

## Departures from the plan and the audit

- The audit said the swap was "one match arm wrapping StreamingDecoder, about 5 lines". The code
  says otherwise: `StreamingDecoder` reads one frame only, refuses a skippable frame and never
  compares the checksum it reads, and the tar reader stops at the tar's end before the last
  checksum. A layer made of several frames (zstd:chunked layers are) would lose everything after
  the first. So the reader is about 80 lines of our own on ruzstd's `FrameDecoder`.
- ruzstd leaves three checks of RFC 8878 to its caller (the reserved bit, a declared size of zero,
  an empty stream), so the reader makes them from the header descriptor byte it holds before the
  decoder reads it.
- The surface test is in the wasmtime executor's tests/surface.rs, outside this track's files,
  because only it runs an image; one test and one helper were added there, nothing else changed.
  The three layout names in image.rs became public for it.

- L2's earlier-framing test frames a file the old way itself, so it does not depend on an old
  build; the probe above showed that framing is exactly what the old code wrote.
- The native executor's kept files keep their old mode (0644 under the usual umask), while the
  wasmtime executor's cache files were already 0600 (tempfile's default). Kept as it was; see "For
  the owner".

## Findings of the independent check of L1, run 2

- Fixed: the random test never ended a stream with a skippable frame (now it does, see above).
- Not fixable here, recorded: on damaged zstd input ruzstd's verdicts are not always libzstd's.
  The check's probe (scratch library-swaps/check-l1/probe, flips.rs) found, over 8,736 single-bit
  flips of checksummed frames, 8 where libzstd says "Data corruption detected" and we accept with
  exactly the original data; without checksums 378 flips libzstd accepts with wrong output where
  we refuse, and 8 we accept with the right output where libzstd refuses. These are in ruzstd's
  literal and table decoding, not in our framing, and we never gave wrong output where libzstd
  refused. Matching them would mean changing ruzstd's entropy decoding. The blob's sha256 is
  checked before decoding, so damage in transit never reaches the decoder.
- Not fixable now, recorded: L1's Cargo.lock lines went into the code commits 8d9b76b9 and
  c43a2109 rather than a commit of their own (they held only this track's lines). History is not
  rewritten; from L2 on the lock lines have their own commits.

## For the owner

- The native executor's kept translations are made 0644 (under the usual umask) and the wasmtime
  executor's cache files 0600. Both are as they were before this track; one of them could follow
  the other if you want them alike.

## Next

- L2 is done (the task's L2 holds what the plan calls L2 and L3).
- L4: the ARM64 encoder's field placement on aarchmrs-instructions.

## How to run

- `nix develop -c cargo test -p wcontainers-runtime-single-kernel` (on the Mac, with
  `CARGO_TARGET_DIR=target/library-swaps`); more random cases: `ZSTD_SEED=1 ZSTD_CASES=2000`.
- `cargo test -p wcontainers-executor-wasmtime --test cache` (the cache file) and `cargo test -p
  wcontainers-executor-hardware-virtualization --lib --test kept` (the native kept files) run on
  the Mac.
- Lint only this track's packages with `cargo clippy --no-deps -p <package> --all-targets -- -D
  warnings`: without `--no-deps` clippy stops on the kernel interface's own warnings.
- The surface test needs Docker with `alpine:3.22` for linux/arm64: `cargo test -p
  wcontainers-executor-wasmtime --test surface -- zstd_twin` (run on the ARM machine through
  arm-run).
- Shared Cargo.lock: other tracks leave uncommitted lines in it. Only this track's lines were
  committed, through a separate index file (`GIT_INDEX_FILE` with `read-tree HEAD`, `update-index
  --cacheinfo`, `commit-tree`, `update-ref` with the old head), then the same blobs were put in the
  real index for those paths.
