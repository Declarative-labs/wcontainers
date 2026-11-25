# Status: graphics container readers

Plan: docs/plan-graphics-container-readers.md. On `main` in the one shared checkout.

## Where things stand

C2 (one DXIL resource-list reader): done in run 3, 2026-11-21 (commits 260d36d8, 99735dce,
4043e0cd, 537e3bf7, 31080b61); its independent check's seven findings settled in run 4 (below,
"Run 4"). Direct3D 12's llvm-bitcode reader and the llvm-bitcode dependency are gone.
- Shaders side (260d36d8): `container::dxil_views` (one C call, `wcs_dxil_views` in cpp/dxil.cpp)
  gives each shader resource and unordered access view's shape, space, register range and element
  value kind. The cache file has a third entry kind, `cache::Kind::Views`, keyed by `views_key`
  (the container's bytes and the converter version), filled by `cache::dxil_views_cached`. And
  cpp/dxil.cpp keeps the last five modules its resource list readers parsed on each thread (one
  per stage a graphics pipeline has, with the container each came from; run 4, before one), so a
  stage's views, declared registers and constant buffer sizes come from one dxil-spirv parse of
  its module in whatever order a pipeline's stages are read. The conversion itself parses the
  module once more through dxil-spirv's own entry point, as before C2.
- Direct3D 12 (99735dce): `formats::typed_key` reads a stage's views once (`dxil::declared`,
  through `cache::dxil_views` in the Direct3D 12 crate's cache.rs, the process's store) and gives
  the typed texture slots `convert_stage` needs beside the key. dxil.rs keeps only its numbering of
  slots. tests/cache.rs now checks that the file holds each conversion and the views of the
  triangle's two DXIL modules once each.
- Why a cache: dxil-spirv decodes and builds the whole module (function bodies included), about four
  times the old reader's cost per read; run 2's per-process memo alone left the first pipeline of
  a process starting from a cache file 32 % slower. With the views in the cache file and the parse
  shared, every measured case is faster than before (Measurements).
- Pass bar: Direct3D 12 tests 51 passed (run 4); pipeline creation measured faster than before C2
  in all four cases: converting compute, a process's first graphics pipelines converted cold, first
  use from a cache file, made again (Measurements; the last two from run 3, whose paths run 4 did
  not change).
- For the check: the tests that covered the old reader are dxil.rs's (the five of before, now on
  the new reading, and `a_container_read_again_declares_what_its_module_does`), the Direct3D 12
  front and device tests and tests/cache.rs; new: container.rs
  `a_module_s_views_are_read_once_through_the_cache` and run 4's (below). Behaviour: as C1, the
  same outputs for every well-formed container, and on malformed DXIL the module dxil-spirv reads
  (Behaviour).

Run 4 (2026-11-21), the independent check's findings (commits a8b3ccec, f8f226d0):
- Literal baseline: run 3's 31080b61 had rewritten the accepted line of the cache warning
  (`convert` to `Store::append`), which counts as an added line, and run 3's status wrongly said
  the check had no finding in the track's files. The warning's text is now the constant
  `APPEND_REFUSED` in the Direct3D 12 cache.rs, and f8f226d0 deletes the line.
- One parse per stage: run 3 kept one module per thread, so a graphics pipeline read the vertex
  stage's views, then the pixel stage's (replacing it), and the vertex conversion's declared
  registers parsed the vertex module again. cpp/dxil.cpp now keeps five, one per stage a pipeline
  has, the latest first; a test reads seven modules in turn (`modules_read_in_turn_each_give_their_own_lists`,
  which also converts one after the others' views evicted it and after its own).
- dxil-spirv's and dxbc-spirv's messages while views are read go into the call's error, not
  stderr (Behaviour).
- An undecodable views record: `dxil_views_cached` returns `ConvertError::Malformed`; the Direct3D
  12 front reads the views again with a warning (`VIEWS_UNDECODED`) and a count
  (`cache::views_reread`). Tests: container.rs `a_views_record_that_does_not_decode_is_malformed`,
  tests/cache.rs's third run.
- New tests: the checksum refusal and the bypass value (`a_module_s_views_are_read_with_its_checksum_or_the_bypass_value_only`);
  a DXIL stage's key formats, typed buffer route, multisampled layers and storage texture slots
  from `formats::typed_key` (device/formats.rs
  `a_dxil_stage_s_typed_views_give_its_key_and_its_storage_texture_slots`).
- The unbounded range size has one definition: boundary.h's `ViewRange::eUnbounded`, which
  build.rs gives Rust as `ir::view_range::UNBOUNDED`, exported as `container::RANGE_WITHOUT_SIZE`
  and used by cpp/dxil.cpp and the Direct3D 12 dxil.rs (the name `UNBOUNDED_RANGE` made the
  literal check match the nir crate's `UNBOUNDED` message).
- The cold graphics pipeline the check measured (break-even after run 3) is now measured faster
  (Measurements).
- Lock (4043e0cd): my commit by path also took two lines of other tracks' uncommitted manifests
  (`git commit -- Cargo.lock` commits the working file, not the staged hunks); 537e3bf7 takes them
  back out, committing the index alone. The working file was never changed.

C1 (one DXBC container reader): done in run 1; the findings of its check were settled in C2 run 1
(f51d47ce, e2ee94e9):
- A signature element's name must end with its NUL inside its chunk, as in dxbc-spirv
  (`readString` fails at the end of the reader and the signature resets). A name cut by its chunk,
  or starting at the chunk's end, is refused: the Direct3D 11 front answers E_INVALIDARG and the
  converters' list ends before the element. Before this the reader took the name up to the chunk's
  end (`SV_TA` in the check's case B2) or an empty name (B3).
- Resource dimensions and return types come from dxbc-spirv's `ResourceDim` and `SampledType`
  (build.rs writes them as `container::dxbc_dimension` and `container::dxbc_sampled`, as it writes
  the opcodes). The hand-written `token::DIMENSION_*` and `token::RETURN_TYPE_*` are gone; both
  fronts use the generated ones.
- The fuzz test's two walks (finding the program by searching every word for `SHEX`, stepping
  with `token::length`) are gone: it finds the instruction through `container::chunk_list` and
  `container::instructions` (new `Instruction::bytes` gives its place). Direct3D 12's test module
  uses `dxbc_op::CUSTOM_DATA` instead of its own 53.
- Malformed containers are pinned by tests: container.rs `a_name_ends_with_its_nul_inside_its_chunk`,
  `an_instruction_past_its_program_ends_the_walk_before_its_opcode_is_read`,
  `a_chunk_list_entry_past_the_container_is_its_last_item`; Direct3D 11 shader.rs
  `a_signature_element_reaching_past_its_chunk_is_refused`.
- literals.baseline: the line for d3d12 resource.rs `DIMENSION_BUFFER = 1` (a redefinition only
  while the container module restated the DXBC dimension) is gone, in its own commit.
- Lines now (wc -l): container.rs 747, d3d11 shader.rs 1116, d3d12 dxbc.rs 390, root.rs 406, fuzz.rs
  254; 2913 in all, against 2691 before C1. The four copies are gone; the reader and the new tests
  are larger than what they replaced (container.rs's tests are 96 of its lines).

Next: C2's independent check again (run 4's fixes). The plan has no milestone after C2.

## Behaviour

C1: same outputs for every well-formed container (the comparison of run 1: 1,690 real containers,
0 differ). The plan's rule is "same outputs for every well-formed container", narrower than the
brief's "same inputs, same outputs, same errors"; the owner has to accept it (For the owner). On
malformed containers the one reader takes the format's rule, the chunk's own size bounding what is
read in it. Counts of run 1's comparison (seed 1, 20,000 mutated cases; compare-2.txt in scratch):
- Direct3D 11 signatures (input 589, output 535, writes_layer_or_viewport 439): the old reader read
  past a chunk's end into the rest of the bytes. Now an element, its name (with its NUL) or the
  chunk past the chunk's end is refused with E_INVALIDARG. Example: `169_ps_code_23.dxbc` cut at
  387: old `SV_Position`, new refused.
- Direct3D 11 stage, model, declarations (stage 175, check_model 258 of 86,760 calls, typed views
  27, indexed constants 31, multisampled arrays 31): a program chunk whose size runs past the bytes
  is refused; a declaration's operands are read within the instruction.
- The converters' signatures (dxbc_signature 543 of 43,380, ISG1/OSG1 131 of 43,380): an element
  whose name lies past its chunk ends the list (before it was listed with an empty name).
- The converters' control point count: an instruction whose length runs past the program ends the
  walk before its opcode is read, so a `dcl_input_control_point_count` cut that way gives none
  (convert then counts 0 control points) where the old walk gave the declared count. Found by C1's
  check at seed 7 (3 of 201,690); not reachable through `convert`, as dxbc-spirv refuses such a
  program first ("Invalid instruction length"). Run 1's list missed it.
- Direct3D 12's program chunk is the first SHEX or SHDR in the chunk list (before: the first SHEX,
  else the first SHDR); they differ only for a container holding both, which no compiler writes.
Not rerun in this run: the seeded comparison against the old readers after the NUL rule (its
change is one rule, covered by the new tests; every real name ends inside its chunk, as run 1's
1,690 unchanged inputs and the corpus tests show).

C2: the same typed views and multisampled arrays as the old reader on every real DXIL container
that dxil-spirv reads (run 3: the rig's 72, the shaders crate's own 143, games 443, godot 69; 727
agree; and the shaders track's runner blobs, 1,732 with the rest: 1,731 agree). The one that
differs, `runner/blobs/hlsl+distance-4-ps.dxil`, is a module dxil-spirv refuses ("Unknown type!"):
the old reader gave empty lists, the new none, and its conversion fails with that error either way.
On malformed DXIL the views follow the module dxil-spirv parses, which the conversion is made from.
Run 3's seeded comparison (seed 1, 8,000 mutated and re-signed cases, games and godot with the
defaults, scratch c2r3-compare.txt, load 13 at the start, about 10 minutes): 4,090 agree, 3,758
differ, 141 ended the process in dxil-spirv (gap A11: SIGSEGV or `std::terminate`), 11 grew past
4 GiB in dxil-spirv's bitcode reader and were stopped (the first, case 1594, reached 74 GB before I
stopped it by hand; the loop now stops a case past LIMIT_KB). The differences:
- 3,526 where dxil-spirv refuses the module and the old reader read it (2,762 with empty lists,
  740 with typed views, 24 with arrays or one list): the conversion parses the same module
  next, so pipeline creation fails as before.
- 227 where the old reader refused and dxil-spirv reads (16 with typed views): the key now has
  the views' formats the converter needs. Example: case 1040 (`0361b7318e7bb95d.dxil` bits 1 at
  801 and 418, 2 at 908) declares a typed storage texture; converted without its format, as the
  old reader's key had it, it fails `Unsupported("StorageFormat")`.
- 5 where both read and differ (cases 1127, 3862 and three more; in one the new reading refuses a
  view of the views list): dxil-spirv reads another kind than the old reader; converting 1127 and
  3862 ends the process in dxil-spirv (A11) either way.
Not measured beyond 8,000 cases (run 2's 20,000 had no per-case record).
- Messages: where dxil-spirv or dxbc-spirv refuses a module while its views are read, the reason
  goes into the C call's error (run 4; run 3 let dxil-spirv print it to stderr, for example
  `[ERROR]: Unknown type!` for `runner/blobs/hlsl+distance-4-ps.dxil`). `dxil_views` gives `None`
  for it without a message, as the old reader gave its lists without one; the conversion, which
  parses the same module next, reports the reason as before (`Rejected("Unknown type!")`).
- A cache file whose views record does not decode: `cache::dxil_views_cached` answers
  `ConvertError::Malformed`, as `convert_cached` does for a shader record; the Direct3D 12 front
  then reads the module's views again each time, warns ("a views record of the cache does not
  decode") and counts it (`cache::views_reread`). Run 3 read them again silently. Reaching it needs
  a damaged record that passes the entry's CRC, as the key holds the converter version.

## Measurements (Apple M5 Max, 18 cores, shared; load averages at the time)

- C2 run 4, as committed (a8b3ccec), release, recording back end. Before is HEAD f54cca11's
  crates as in run 3 (scratch old-d3d12 on head-shaders). Load 7.9 throughout.
  - A process's first graphics pipelines converted cold (pipeline-bench/cold.sh, the check's
    bench: a fresh process, no cache file, the rig's triangle, then common.vs_color with
    common.ps_color), 80 processes each, alternating, the first pair left out as warm-up
    (c2r4-bench-cold.txt, tallied by cold-tally.sh): triangle median 641.0 us before, 601.0 us
    after, after faster in 77 of 79 pairs; the colour pair median 289.9 us before, 266.8 us after,
    after faster in 78 of 79. The check measured run 3 at break-even on the pair (276.5 against
    278.5 us at load 5), which the second parse of the vertex module explains.
  - Converting (convert.sh, 12 processes each, alternating, c2r4-bench-convert.txt): compute
    per pipeline median 225.2 us before, 189.9 us after, after faster in 12 of 12; triangle
    238.2 to 265.6 us before, 210.0 to 225.3 us after.
- C2 run 4 tests: the three crates' whole suites at a8b3ccec (debug, load 10.9 at the start and
  10.6 at the end): 237 passed, 0 failed, 6 ignored (run 3's 233, three new in container.rs and
  one in device/formats.rs; tests/cache.rs's new run is in its one test); Direct3D 12 alone 51 (unit 18, cache 1,
  device 18, front 12, gpu 2). c2r4-suite.log in scratch. Lint with warnings denied: clean.
  rustfmt: clean on the changed files. Literal check: no finding in the track's files (61 of other
  tracks at the time, c2r4-literals.txt).

- C2 run 3, as committed, pipeline-bench in scratch (release, recording back end, 56 DXIL compute
  pipelines of the rig's and the shaders crate's own inputs, and the rig's triangle). Before is
  HEAD f54cca11's Direct3D 12 crate (scratch old-d3d12) on HEAD's shaders crate (scratch
  head-shaders: the working crate with this track's six files taken from HEAD); after is the tree.
  Load (1 minute) 10 to 13 throughout.
  - converting, the first pass of a process without a cache file, 12 processes each, alternating
    (convert.sh, c2r3-bench-convert-3.txt): compute per pipeline 238.1 to 329.1 us before (median
    256), 200.8 to 293.1 us after (median 216), after faster in 11 of 12 pairs (the twelfth 282.5
    against 290.9 while the load rose); triangle 266.5 to 384.7 us before (median 310), 234.8 to
    372.1 us after (median 255).
  - the first pass of a process starting from a cache file, 10 processes each, alternating
    (first.sh, c2r3-bench-first-3.txt): compute 64.4 to 69.7 us before (median 67.4), 39.3 to
    42.5 us after (median 41.9); triangle 84.1 to 92.1 us before, 49.4 to 59.2 us after.
  - made again, 20 rounds, two runs of each side alternating, each run then again from its file
    (run.sh, c2r3-bench-run-3.txt): compute median 62.4 to 64.0 us before, 34.9 to 43.4 us after;
    triangle 62.8 to 66.6 us before, 28.7 to 35.3 us after.
  - The triangle's first numbers after are not cold reads of its views: the compute pass, which
    tries every input, read them first. The compute numbers are cold on both sides.
  - The same converting run with the before bench on the tree's shaders crate (so sharing the
    parse too) gave before median 207 us, after 189 us (c2r3-bench-convert-2.txt); without the
    shared parse (only the cache entry kind) after was about 6 % slower converting than before
    (median 242 against 227, c2r3-bench-convert.txt), which is why the parse is shared.
- C2 run 3 tests: the three crates' whole suites (debug, load 13 at the start and 21 at the end):
  233 passed, 0 failed, 6 ignored (run 1's 231, container.rs's new cache test, dxil.rs's new
  test); Direct3D 12 alone 50 (unit 17, cache 1, device 18, front 12, gpu 2). c2r3-suite.log in
  scratch. Lint of the three crates with warnings denied: clean. rustfmt: clean. Literal check:
  wrongly reported clean; it had the '>' line of the rewritten baseline entry (Run 4).
- Earlier C2 figures (runs 1 and 2, the reasons for this run's shape): one dxil-spirv read of a
  stage costs 125 to 155 us against 32 to 38 us for the old reader's three (735 inputs, load 13 to
  18): checksum about 15 us, bitstream decoding about 85 us, building the module about 55 us. A
  per-process record alone made a first use from a cache file 32 % slower (60 to 79 us per compute
  pipeline). Logs c2r2-*.txt and dxil-compare-*.txt in scratch.
- C1 fixes, the three crates' whole suites at f51d47ce's content (load 18 at the start, 15 at the
  end): 231 passed, 0 failed, 6 ignored. c1fix-suite.log in scratch.
- Not run: the WebAssembly build of the shaders crate (no wasm32-wasip1 target or wasi-sdk in this
  Mac's shell; cpp/dxil.cpp now has a `thread_local`, as dxil-spirv's own dxil_spirv_c.cpp, which
  that build already compiles); the long seeded fuzz (its inputs did not change in this run).

## How to run

- Lint (Mac, own build folder): `CARGO_TARGET_DIR=$PWD/target/graphics-container-readers nix
  develop -c cargo clippy -j 4 -p wcontainers-shaders -p wcontainers-direct3d-12 -p
  wcontainers-direct3d-11 --all-targets -- -D warnings`; rustfmt --edition 2024 on each file.
- Tests: the same with `cargo test -j 4 -p wcontainers-shaders -p wcontainers-direct3d-12 -p
  wcontainers-direct3d-11 -- --test-threads 3`.
- The seeded fuzz: `cargo test -p wcontainers-shaders --test fuzz -- --include-ignored`
  (WCONTAINERS_FUZZ_SEED, WCONTAINERS_FUZZ_CASES, WCONTAINERS_FUZZ_FIRST).
- Literal check: `RAYON_NUM_THREADS=3 nix develop -c cargo run -q -p wcontainers-literals -- check`.
- C1's comparison (scratch compare): as in run 1, `COMPARE_ROOT=<checkout> COMPARE_CASES=20000
  COMPARE_SEED=1 COMPARE_EXTRA=<dir>:<dir> target/graphics-container-readers/release/container-compare`.
- C2's tools (scratch): dxil-compare (the old reader, kept there as old_dxil.rs, against
  `dxil::declared`; COMPARE_ROOT, COMPARE_EXTRA, COMPARE_SEED, COMPARE_CASES, COMPARE_FIRST,
  COMPARE_ROUNDS; COMPARE_PROBE=<file> prints one container's conversion error and views), run
  over mutated cases with `sh dxil-compare-loop.sh <out>` (starts again after a case that ends the
  process or passes LIMIT_KB of memory), tallied by `sh c2r3-tally.sh <out>.txt`. Build with
  `nix develop <checkout> -c cargo build -j 4 --release` in its folder.
- The pipeline bench: `cargo build -j 4 --release --manifest-path
  <scratch>/pipeline-bench/<before|after>/Cargo.toml`, then `sh pipeline-bench/convert.sh`,
  `first.sh` and `run.sh` (BENCH_ROOT, BENCH_EXTRA, BENCH_ROUNDS, BENCH_CACHE; CONVERT_RUNS,
  FIRST_RUNS). The before side builds scratch old-d3d12 on scratch head-shaders (HEAD f54cca11's
  crates).
- The cold graphics bench (the check's, copied): build `pipeline-bench/cold-before` and
  `cold-after` the same way, then `COLD_RUNS=80 sh pipeline-bench/cold.sh > out.txt` and
  `sh pipeline-bench/cold-tally.sh out.txt`.

## Departures from the plan and the brief

- C2 has two parts the plan did not name, without which the speed bar fails: the cache file's third
  entry kind (crates/shaders/src/cache.rs, the shaders track's file, 32 lines added,
  nothing of it changed; its git status was clean before the edit), and the per-thread record of the
  last parsed modules in cpp/dxil.cpp. The record holds up to five modules (one per stage of a
  graphics pipeline) and their containers' bytes per thread that read one; a sixth module evicts
  the one read longest ago. Their memory was not measured.
- Outside the track's files for C2: the Direct3D 12 crate's device.rs, device/formats.rs,
  device/geometry.rs, cache.rs, Cargo.toml and tests/cache.rs (no track works in that crate). The
  cache test's count of entries changed with the new kind: it now counts each kind.
- literals.baseline: the cache message's accepted line follows the code from `convert` to
  `Store::append` (one line changed, not added); 29 lines of the gone reader and the slot bound are
  deleted.
- 4043e0cd took two lines of other tracks' uncommitted lock changes; 537e3bf7 took them back out
  (Where things stand). History is not rewritten.
- C1's check fixes touched crates/shaders/build.rs (two lines in its list of dxbc-spirv
  enums); C2 added four more lines there (the boundary's view enums, run 4's `ViewRange`), and
  the enums themselves in cpp/boundary.h.
- Run 4, outside the track's files: the shaders crate's cache.rs again (`dxil_views_cached` now
  returns a Result, its git status clean before the edit), and in the Direct3D 12 crate cache.rs
  (named warnings, the reread count `views_reread`, public for its test), device/formats.rs (one
  test module) and tests/cache.rs (a third run).
- From run 1, still true: the container.rs commit (f4302b4a) was not the one small commit the brief
  asked for, and a Cargo.lock hunk went into a code commit (767eab22).
- From run 1, still true: outside the track's files, one line each in the shaders crate's lib.rs and
  Cargo.toml, the Direct3D 12 Cargo.toml and tests/front.rs.

## Dependencies and licences

No new dependency. llvm-bitcode 0.4.2 (MIT) is gone from wcontainers-direct3d-12 and from the lock.
blake3 1.8.7 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception) stays a dev-dependency of
wcontainers-direct3d-12. md-5 0.11.0 (MIT OR Apache-2.0) is a normal dependency of
wcontainers-shaders. serde and postcard, which the views' cache record uses, were already
dependencies of wcontainers-shaders.

## For the owner

- C2 adds a cache file entry kind (`Kind::Views`) to the shaders crate's cache.rs, which is the
  shaders track's file, and keeps up to five parsed modules per thread in cpp/dxil.cpp: without them the
  library read made pipeline creation slower (Departures). Example: the first compute pipeline of
  a shader in a process starting from a cache file took 60 us before and 79 us with the library
  read and a per-process record only (run 2), and 67 us before and 42 us now (run 3).
- C2 behaviour on malformed DXIL: the views now come from the module dxil-spirv parses, the module
  the conversion is made from, so they follow it where the old reader read the bytes otherwise
  (Behaviour). Example: a module dxil-spirv refuses gives no views (the conversion then fails as
  before).
- Behaviour on malformed containers changed with C1 as listed under "Behaviour"; each kind now has
  a test. Example: a pixel shader whose container is cut inside its input signature chunk is
  refused with E_INVALIDARG by the Direct3D 11 front, where it was accepted.
- crates/shaders/examples/containers.rs (the shaders track's corpus tool) still walks
  a container's header and chunks itself, with its own plausibility rules (at most 64 chunks,
  alphanumeric tags). Not this track's file; it could use `container::chunk_list`. With it, "one
  reader in the tree" holds for shipped code and the tests, not for that example.
- Fixed on 2026-11-22 (commits 06a2647d, 8c13643d), found by run 1's comparison: the Direct3D 12
  declaration scans (dxbc.rs `element_slots`, dxil.rs `places`) listed every element of a range
  whose length comes from the bytecode (up to 2^32; one mutated container took 45 GB). Now a
  range is a first slot and an end (`TypedView::slots`, `multisampled_arrays` gives ranges), and
  slots under Direct3D 12's numbering are counted from merged runs of places (`dxil::Places`).
  `formats::typed_key` puts in the key only the slots below their kind's range
  (`SlotKind::slots`), the only ones the converter binds; it refuses the rest
  (`BindingsBeyondLimit`). Same answers as before for the 450 real containers of the shaders
  crate's corpus and the graphics test's shaders: tests/declared.rs, recorded before the change
  in tests/declared.expected. Two behaviour notes, both for malformed bytecode only: a shader
  model 5.1 range whose upper bound is just below its lower bound (2^32 elements counted modulo
  2^32) is no element, as release builds read it before; debug builds panicked on it. An array
  of no elements is no longer listed as an empty typed view (it never gave a slot).

## The semantic name rule (checked 2026-11-22)

The open item "a semantic name with no terminator inside its chunk is accepted by the shared
reader" is already fixed, by f51d47ce (C1's check, above under "Where things stand"). I checked
again against the reference: dxbc-spirv reads the name with `readString` on a reader bounded by
the chunk, and an element whose name has no NUL before the chunk's end fails and resets the whole
signature. container.rs `signature_element` takes the name up to its first NUL inside the chunk's
contents and refuses the element otherwise; `a_name_ends_with_its_nul_inside_its_chunk` pins
it. Nothing changed in this run.

## Requests to other tracks

- Shaders track (crates/shaders/src/container.rs `signature`): where one element of a
  signature chunk is malformed, the converters' list keeps the elements before it. The reference
  (dxbc-spirv `Signature`, `resetOnError`) drops the whole signature then. Only malformed
  containers differ, and dxbc-spirv refuses their conversion itself, so this is not clearly
  worth changing; if the list should follow the reference, `signature` would give no elements of
  a chunk with any malformed element. Not changed by me (not my file).
