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

- L2 after its independent check: `store` writes the magic and then the body with postcard's
  `to_io` straight into a buffered writer over the temporary file, so the whole file is no longer
  built in memory (peak: the modules' serialisations, about the file). postcard's own writer drops
  the host's error, so a small writer (`KeptError`) keeps it and `store` still reports the error
  number (`TRANSLATION_NOT_KEPT` carries it). postcard's `use-std` feature replaces `alloc` for
  `to_io`; it adds no crate to the lock file. New unit tests in implementation.rs:
  `a_cache_file_is_the_magic_then_the_postcard_of_its_modules` (same bytes as before) and
  `a_cache_write_that_fails_gives_the_host_error_number` (ENOSPC kept, whether the magic or the
  body meets it).
- L3 of the task (the plan's L4), the ARM64 encoder's field placement, in encode.rs (run 2 also
  changed one type at each of its callers; see Departures).
  - Tried aarchmrs-instructions 0.5.0-2026-09 with aarchmrs-types 0.1.2 (both BSD-3-Clause) on
    MOVZ_64_movewide, LDR_64_ldst_pos and TBNZ_only_testbranch, each call behind our fit check, in
    scratch (library-swaps/l3/bench, not in the repository): same words and same refusals as ours
    on 2,000,000 drawn inputs per form. Not taken; see "Departures".
  - Chosen: a table of our own from the forms data we ship. Each builder's form and fields are a
    `Shape` (fixed bits and the fields' places, in the order the builder gives values), found in
    `forms::all()` once, on first use (`LazyLock`), as the old HashMap was built once. Building a
    word then reads the shape and places each value after our fit check (`Field::place`, the old
    check unchanged). The HashMap, `table()`, `field_of()`, `width()` and `word()` are gone.
  - Run 2 (commit 48dd9ad7), after the independent check: the forms the encoder writes are no longer
    strings. `encode::form::LDR` and the others are `encode::Form`, a form's place in `form::ALL`
    (made by a small macro from the names in a private `name` module, so each name is written once);
    `Form::name()` gives Arm's name. The builders that are given their form (`load_store`,
    `branch_register`, `exception`, `move_wide`, `logical_register`, `load_pair`, `load_add`,
    `compare_swap_of`) index their shapes by it: no string is compared when a word is built. They
    accept exactly the forms the old code accepted. `word()` is gone as the plan said; its one caller
    (the native executor's tests/runner `monitor_call`, an `msr tpidr_el1, x0`) uses the new
    `write_system_register([o0, op1, CRn, CRm, op2], rt)`, on the MSR shape with the same fit check.
    `retarget` takes the branch offset field from a table of every form of Arm's table, made once,
    instead of comparing field names. `replace` still finds its field by Arm's name (see Departures).
  - New test in encode.rs, `every_builder_places_its_fields_as_the_table_read_by_name_does`: every
    builder (and the system register write, `retarget`, the reaches and the float status moves)
    against the old placement by name over `forms::all()`, on drawn values that fit and that do not
    (offsets out of reach, misaligned, immediates too wide, registers past 31), results and refusals
    compared. Branch offsets are drawn within twice each form's own reach, so about half fit for
    cbz, cbnz, tbnz, b, bl and adr. `WCONTAINERS_ENCODE_SEED` (default 0x6a09e667f3bcc908) and
    `WCONTAINERS_ENCODE_CASES` (default 10,000). Ran 10,000 cases of the default seed and 200,000 of
    seed 1.
  - Mutants (scratch library-swaps/l3/mutants.py over a copy of the crate in l3/mut, 12 of them):
    the new test catches 11 at the default 10,000 cases (tbnz b5 and b40 swapped, a fit check that
    masks, tlbi or msr CRn and CRm swapped, retarget reading another form's field or keeping old
    bits, load and atomic fields swapped, a kind's shape of the next form, a reach one bit short).
    The twelfth, movz's and movk's names swapped in `name`, it cannot see (its reference reads the
    same names); tests/encode.rs catches it. tests/encode.rs alone misses four (the masking fit
    check, tlbi and msr CRn/CRm, retarget's field), so the new test must not be weakened.
  - Ran on the Mac (load 10 to 17), after commit 48dd9ad7: wcontainers-arm64's whole suite (unit 1,
    encode 2, exact 4, native_routes 1), all passing; the native executor's unit tests (12), kept
    (1), and its tests that build words with the encoder: carry, native (1215 forms, 0 failing),
    exits (4 programs, 0 failed, both modes; they run `monitor_call`), floor (21 cases, 0 failed),
    interrupts (0 failed), probe (0 forms of 16 bytes or fewer differ, as before), registers,
    host_branches (0 differing), all exit 0. The literal check finds nothing in the changed lines.
    Not run: the native executor's speed, busybox and kernel tests do not build at the moment
    (tests/container/mod.rs, another track's, names `wcontainers_abi::BackendRequestId`, and
    speed.rs does not match the kernel's `Start` any more); nothing of the encoder is involved.
- The track's whole suites after both commits (4c88f1b1), on the ARM machine (job
  library-swaps/20261121-215605-4139430, load 4.1 at the start, 27.9 at the end): wcontainers-arm64
  (1, 2, 4, 1), the wasmtime executor (unit 7, cache 4, code 4, faults 2, fork 5, refused 77,
  surface 20 passed and 1 ignored as before, turns 5, views 6) and the runtime crate's 13, all
  passed.

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

- 2026-11-21 on the Mac (M5 Max), load average 6 to 12 (shared with other tracks), release build,
  2,000,000 drawn words per form, the last of three rounds, two runs (library-swaps/l3/bench), run 1:
  - before (a copy of the old `word()`): movz 42.5 to 60.2 ns, ldr (by name) 51.1 to 69.9 ns, tbnz
    73.1 to 107.5 ns per word.
  - aarchmrs with our fit check: movz 1.0 to 1.5 ns, ldr 1.2 to 2.1 ns, tbnz 1.2 to 2.0 ns.
  - aarchmrs-instructions builds clean in 1.2 s (release) and 1.3 s (debug) at load 14.
- 2026-11-21 on the Mac (M5 Max), load average 13.5 to 14.5, the same bench, five runs, after commit
  48dd9ad7: before 42.6 to 43.2 (movz), 50.8 to 51.5 (ldr), 73.4 to 75.1 (tbnz) ns; aarchmrs 1.02
  to 1.07, 1.17 to 1.38, 1.19 to 1.40 ns; the encoder now 1.78 to 1.80, 2.56 to 2.57 (ldr through
  `load_store(form::LDR, ...)`, 8.1 to 9.8 ns in run 1 while it compared names), 2.77 to 2.80 ns.
  Ours takes about 1.7 to 2.3 times aarchmrs's time per word (the shape behind a `LazyLock` and
  the fit check per field); the old code took 17 to 42 times ours.
- Measured and not kept (see "Departures"): a build script that writes Arm's table as constants
  made every shape a compile-time constant: movz 1.4 ns, ldr 8.1 to 8.4 ns, tbnz 1.6 ns (run 1,
  load 6 to 12, ldr still by name); the crate's release rebuild went from 1.40 s to 1.72 s.

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

- L3 (the encoder): the plan said aarchmrs-instructions. Not taken, though its words are the same
  and it is faster (about half our time per word, both under 3 ns, against 43 to 75 ns before),
  because it cannot replace our table: it has no lookup from a word to a form, and `replace`,
  `retarget`, `moves_of_x0` and the builders that are given their form need ours. Taking it would
  place fields from two sources. Its field widths would also have to be written beside each call
  for our fit check (`BitValue::new_u32` checks only in debug builds), where our shapes read them
  from the table. (Run 1 said "the same speed"; the independent check and run 2 measured about twice
  aarchmrs's time.)
- L3: the table is built once at first use, not at compile time. A compile-time table needs a
  build script whose output the encoder includes with `include!(concat!(env!("OUT_DIR"),
  "/encodings.rs"))`; that path is a new finding of the literal check, which may not grow, and it
  measured no faster.
- L3 run 2: making the forms typed changed callers outside this track's files, one type each, so
  that no name is compared at run time (the milestone's bar): carry.rs's `sized` returns
  `encode::Form`s, the native executor's routines.rs's `each` takes one, tests/encode.rs compares
  `name.name()` and `form::ERET.name()` with the table's names (two lines; what they check is
  unchanged), and tests/runner/mod.rs's `monitor_call` uses `write_system_register` in place of
  `word()`. Nothing else in those files changed.
- L3: `replace(word, field, value)` still finds its field by Arm's name among the fields of the
  word's form, as the plan keeps it ours: it edits a copied word of any of Arm's forms, and its
  callers give Arm's field names, some read from the table itself (tests/common's
  `every_register`), so a typed field would mean typing every field name of Arm's table.
- L2 (b): the `tempfile` line in the native executor's Cargo.toml (commit 88d924b3) is outside
  translate.rs; the swap needs it and the line is this track's only. Not listed here before; the
  independent check found it.

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

## Findings of the independent check of L3, run 1

- By-name builders compared names at each call: fixed, the forms are typed (`encode::Form`).
- `word()` searched field names: gone, as the plan said; `write_system_register` replaces its one
  use. `retarget` no longer compares names; `replace` still does (Departures).
- The new test almost never drew a tbnz offset that fits: offsets are now drawn per form within
  twice its reach; the tbnz field swap is caught at the default 10,000 cases.
- "Same speed as aarchmrs": corrected to about twice its time, measured again above.
- tests/encode.rs alone misses a masking fit check and the CRn/CRm swaps: recorded under Done; the
  new test catches them and must stay.

## For the owner

- L2's independent check noted that a cache file holding only the magic and a module count of zero
  is loaded as a translation (as before L2). Left as it is: it is what `store` writes for a
  translation with no modules, so refusing it would change behaviour; the directory is written only
  by compilers.

- The native executor's kept translations are made 0644 (under the usual umask) and the wasmtime
  executor's cache files 0600. Both are as they were before this track; one of them could follow
  the other if you want them alike.

## Next

- L2 is done (the task's L2 holds what the plan calls L2 and L3); its check's three points are
  answered above.
- L3 (the plan's L4) is done, including run 2's answers to its independent check. Nothing of the
  plan is left.

## How to run

- `nix develop -c cargo test -p wcontainers-runtime-single-kernel` (on the Mac, with
  `CARGO_TARGET_DIR=target/library-swaps`); more random cases: `ZSTD_SEED=1 ZSTD_CASES=2000`.
- `cargo test -p wcontainers-executor-wasmtime --test cache` (the cache file) and `cargo test -p
  wcontainers-executor-hardware-virtualization --lib --test kept` (the native kept files) run on
  the Mac.
- `cargo test -p wcontainers-arm64` (the encoder; `WCONTAINERS_ENCODE_SEED`, `WCONTAINERS_ENCODE_CASES`
  for more cases) and `cargo test -p wcontainers-executor-hardware-virtualization --lib --test carry
  --test native` (the native executor's words; Mac only) run on the Mac. The comparison with
  aarchmrs: `RUSTC_BOOTSTRAP=1 nix develop <checkout> -c cargo build --release` in scratch
  library-swaps/l3/bench, then `l3-encode-bench <words>`. The mutants: copy the crate's src and
  tests into l3/mut/wcontainers-arm64, then `python3 -I mutants.py mut/wcontainers-arm64 --lib` (or
  `--test encode`) in l3 inside `nix develop <checkout>`.
- Lint only this track's packages with `cargo clippy --no-deps -p <package> --all-targets -- -D
  warnings`: without `--no-deps` clippy stops on the kernel interface's own warnings.
- The surface test needs Docker with `alpine:3.22` for linux/arm64: `cargo test -p
  wcontainers-executor-wasmtime --test surface -- zstd_twin` (run on the ARM machine through
  arm-run).
- Shared Cargo.lock: other tracks leave uncommitted lines in it. Only this track's lines were
  committed, through a separate index file (`GIT_INDEX_FILE` with `read-tree HEAD`, `update-index
  --cacheinfo`, `commit-tree`, `update-ref` with the old head), then the same blobs were put in the
  real index for those paths.
