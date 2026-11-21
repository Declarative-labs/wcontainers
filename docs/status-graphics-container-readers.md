# Status: graphics container readers

Plan: docs/plan-graphics-container-readers.md. On `main` in the one shared checkout.

## Where things stand

C2 (one DXIL resource-list reader), run 1, 2026-11-21: stopped, as the brief says for a library that
does not fit. dxil-spirv's module reader makes pipeline creation slower (Measurements), so Direct3D
12's own resource list reader (crates/wcontainers-direct3d-12/src/dxil.rs, on llvm-bitcode) stays and
nothing of C2's code is committed. The attempt, which was complete and passed the Direct3D 12 tests,
is kept as a patch: scratch graphics-container-readers/c2-dxil-spirv-attempt.patch. What it would
take to go on is under "For the owner".

C1 (one DXBC container reader): done in run 1; the findings of its check are settled in this run
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

Next: nothing of C1. C2 waits on the owner (For the owner).

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

C2 attempt (not committed): on all 735 real DXIL containers (the rig's 72, the shaders crate's own,
the corpus 16, games 443, godot 69) the attempt gave the same typed views and multisampled arrays as
the old reader, once DXIL's -1 for an unbounded range was read as the old reader reads it. On
mutated containers, signed again, dxil-spirv ends the process (SIGSEGV at case 24 of seed 1,
`685b1bd13a9db30c.dxil` bit 6 at 3208; also `std::terminate`), where the old reader returns: gap A11
of the shaders track. In pipeline creation the converter parses the same bytes right after, so the
process would end there anyway, but the end would come earlier.

## Measurements (Apple M5 Max, 18 cores, shared; load averages at the time)

- C2, the read alone, release build, 735 DXIL inputs, 20 rounds (dxil-compare in scratch), load
  13 to 18: before, the three reads of a pixel or compute stage, 32 to 38 us per stage; after, one
  call, 125 to 155 us per stage. At load 30 the same ran 56 and 276 us. Split of the one call (load
  18, by switching parts off in a throwaway build): checksum about 15 us, dxil-spirv's bitstream
  decoding (all records, function bodies included) about 85 us, building the module about 55 us.
  The container checksum in Rust alone: 7.7 us per input (inputs 4,556 KiB in all).
- C2, pipeline creation with the conversion in the process cache, recording back end, release
  (pipeline-bench in scratch, built against the Direct3D 12 crate before and after), 20 rounds,
  run before, after, before, after at load 17 to 20: 59 DXIL compute pipelines, median 76.1 and
  76.9 us before, 104.5 and 104.0 us after; the rig's DXIL triangle (vertex and pixel), median 77.4
  and 76.3 us before, 99.5 and 102.8 us after. About 37 % and 30 % slower: the pass bar fails.
- C2 attempt, Direct3D 12 tests (debug, load about 15): 49 passed (unit 16, cache 1, device 18,
  front 12, gpu 2).
- C1 fixes, the three crates' whole suites at f51d47ce's content (load 18 at the start, 15 at the
  end): 231 passed, 0 failed, 6 ignored (the 227 of run 1 and the 4 new tests). c1fix-suite.log in
  scratch. Lint with warnings denied: clean. Literal check: no finding in the track's files.
- Not run: the WebAssembly build of the shaders crate (no wasm32-wasip1 target or wasi-sdk in this
  Mac's shell); the long seeded fuzz (unchanged inputs to it: the fuzz test's own walks only).

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
- C2's tools, only with the attempt's patch applied: scratch dxil-compare (old reader against
  `dxil::declared`; COMPARE_ROOT, COMPARE_EXTRA, COMPARE_SEED, COMPARE_CASES, COMPARE_FIRST,
  COMPARE_ROUNDS) and pipeline-bench/before and /after (`cargo build --release --manifest-path
  <scratch>/pipeline-bench/<before|after>/Cargo.toml`, run with BENCH_ROOT, BENCH_EXTRA,
  BENCH_ROUNDS). The "before" bench builds the copy of the Direct3D 12 crate in scratch old-d3d12.

## Departures from the plan and the brief

- C2 stopped (the brief's rule for a library that does not fit). Example: the rig's DXIL triangle
  pipeline, made again with its conversion cached, takes 77 us with our reader and 100 us with
  dxil-spirv's, because dxil-spirv decodes and builds the whole module, function bodies included,
  where our reader skips every block but the module's constants and metadata.
- C1's check fixes touched crates/wcontainers-shaders/build.rs (two lines in its list of dxbc-spirv
  enums), the shaders track's file, so that the dimensions and return types come from dxbc-spirv as
  the opcodes do; its git status was clean before the edit.
- From run 1, still true: the container.rs commit (f4302b4a) was not the one small commit the brief
  asked for, and a Cargo.lock hunk went into a code commit (767eab22). History is not rewritten;
  this run kept the lock and the baseline in commits of their own.
- From run 1, still true: outside the track's files, one line each in the shaders crate's lib.rs and
  Cargo.toml, the Direct3D 12 Cargo.toml and tests/front.rs.

## Dependencies and licences

No new dependency. md-5 0.11.0 (MIT OR Apache-2.0), already in the tree, is a normal dependency of
wcontainers-shaders. llvm-bitcode 0.4.2 (MIT) stays a dependency of wcontainers-direct3d-12.

## For the owner

- C2: dxil-spirv's module reader does not meet "pipeline creation not slower": a cached pipeline is
  made 30 to 37 % slower (Measurements). Ways on, each the owner's call: accept the slowdown
  (the patch is ready, correct on every real DXIL container); keep our reader (now); or read the
  list from dxc's `STAT` chunk, which holds the module without function bodies, where a container
  has one (308 of 735 here): not measured, and a container whose `STAT` and `DXIL` disagree would
  give a wrong key. Changing dxil-spirv to skip function bodies would be a change to vendored code.
- Behaviour on malformed containers changed with C1 as listed under "Behaviour"; each kind now has
  a test. Example: a pixel shader whose container is cut inside its input signature chunk is
  refused with E_INVALIDARG by the Direct3D 11 front, where it was accepted.
- crates/wcontainers-shaders/examples/containers.rs (the shaders track's corpus tool) still walks
  a container's header and chunks itself, with its own plausibility rules (at most 64 chunks,
  alphanumeric tags). Not this track's file; it could use `container::chunk_list`. With it, "one
  reader in the tree" holds for shipped code and the tests, not for that example.
- Not changed, found by run 1's comparison: the Direct3D 12 declaration scans (dxbc.rs
  `element_slots`, and dxil.rs `Reader::declared` likewise) list every element of a range whose length comes
  from the bytecode (up to 2^32). The shaders crate refuses such a range by name
  (`BindingsBeyondLimit`); the Direct3D 12 front does not yet. Fixing it changes behaviour, so it
  is left for its own item.
