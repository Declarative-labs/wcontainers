# Status: graphics container readers

Plan: docs/plan-graphics-container-readers.md. On `main` in the one shared checkout.

## Where things stand

C1 (one DXBC container reader), run 1, 2026-11-21: done and pushed, ready for its check.
- One reader: crates/wcontainers-shaders/src/container.rs, now `pub mod container` (f4302b4a). It
  gives the chunk list (`chunk_list`, `Chunk::tag`, `size`, `contents`, `is_container`,
  `declared_size`), signature elements of every form (`signature_elements`,
  `signature_direction`, `Element` with the component type), a program's instructions
  (`instructions`, `Instruction::operand_indices`), the token fields both fronts read (`token`),
  dxbc-spirv's opcode table (`dxbc_op`, re-exported) and the one checksum (`checksum`, on md-5's
  MD5 core with DXBC's own last block). The crate's own reads (shader model, program type, control
  points, PSV0 fields, signatures for the converters) sit on the same pieces.
- Direct3D 12 (767eab22): dxbc.rs's chunk lookup, program chunk and instruction walk, and root.rs's
  RTS0 lookup and checksum are gone; they use the reader. Its declaration scan (views placed by
  space through the root signature's ranges, unbounded arrays) stays its own on the shared walk.
  md-5 left its manifest. tests/front.rs's checksum test calls `container::checksum`.
- The fuzz test (a1b731f1) signs with `container::checksum`; its own MD5 code is gone.
- Direct3D 11 (dcf9fc21): shader.rs's chunk walk, signature reader and instruction walk are gone;
  its declaration scan (registers, constant buffers indexed at run time) stays its own on the
  shared walk and operand reader. Its `SignatureElement` is made from the shared `Element`.
- Opcode and token constants both fronts defined (DCL_RESOURCE, DCL_UAV_TYPED, the custom data
  opcode, dimensions, return types, shifts and masks) now come from the reader.
- literals.baseline: 123 lines of these files gone (7090247d); the check prints no line of the
  track's files.
- Lines (wc -l, before and after): container.rs 373 to 653, d3d11 shader.rs 1127 to 1083, d3d12
  dxbc.rs 452 to 395, root.rs 441 to 406, fuzz.rs 298 to 270. The reader grew by what the fronts
  needed from it (chunk list with each cut reported, the element iterator, the instruction walk,
  the token fields, the checksum); the four copies are gone.

Nothing of C1 is left. What the independent check should look at is under "Behaviour" below.

## Behaviour

Same outputs for every well-formed container. Measured by a comparison of the old readers (copied
from the commit before f4302b4a) with the new ones, over 1,690 real containers (the rig's DXBC 79
and DXIL 72, the shaders crate's own 334, the corpus in scratch graphics/corpus dxbc and dxil, the
games 972 and godot 69 in scratch graphics-shaders, and two root signatures the d3d12 crate writes)
and 20,000 seeded mutations of them (seed 1): on all 1,690 unchanged inputs no function differs.
The checksum, all of Direct3D 12 (typed views and multisampled arrays with and without ranges,
unbounded arrays, root signature reading with its error texts) and the shaders crate's shader
model, program type, control points and PSV0 fields agree on every mutated case too.

They differ only on malformed containers, where the old readers disagreed with each other and the
one reader takes the format's rule (the chunk's own size bounds what is read in it, as Microsoft's
and dxbc-spirv's parsers read it). Counts are of the 20,000 mutated cases (compare-2.txt):
- Direct3D 11 signatures (input 589, output 535, writes_layer_or_viewport 439): the old reader read
  past a chunk's end into the rest of the bytes, took the first element at 8 whatever the chunk
  header said and read a mask as a whole word. Now an element, its name or the chunk past the
  chunk's end is refused (E_INVALIDARG, as the old reader refused one past the bytes). Example:
  `169_ps_code_23.dxbc` cut at 387: old gave `SV_Position`, new refuses. Where a shortened chunk
  still holds the element, a name is read to the chunk's end (`7600a99b00813253.dxbc` with a chunk
  size lowered by 7: old `SV_TARGET`, new `SV_TA`), as the converters' reader always did.
- Direct3D 11 stage, model, declarations (stage 175, check_model 258 of 86,760 calls, typed views
  27, indexed constants 31, multisampled arrays 31): the program chunk is read within its size, so
  a program chunk whose size runs past the bytes is refused (`1ea526a5bb636445.dxbc`, byte 556
  and the size at 284 raised by 3: old `Pixel`, new E_INVALIDARG); a declaration's operands are
  read within the instruction.
- The converters' signatures (dxbc_signature 543 of 43,380, ISG1/OSG1 131 of 43,380): an element
  whose name offset lies past the chunk ends the list; before it was listed with an empty name
  (`hs_tri_even_cw.dxil` with words at 1136 and 101 replaced: old one element `""`, new none).
  On DXBC dxbc-spirv reads the same signature first; on DXIL the ISG1 and OSG1 chunks only place
  system values.
- Direct3D 12's program chunk is the first SHEX or SHDR in the chunk list (before: the first SHEX,
  else the first SHDR); the two differ only for a container holding both, which no compiler
  writes, and none of the cases above has one.

## How to run

- Lint (Mac, own build folder): `CARGO_TARGET_DIR=$PWD/target/graphics-container-readers nix
  develop -c cargo clippy -j 4 -p wcontainers-shaders -p wcontainers-direct3d-12 -p
  wcontainers-direct3d-11 --all-targets -- -D warnings`; rustfmt --edition 2024 on each file.
- Tests: the same with `cargo test -j 4 -p wcontainers-shaders -p wcontainers-direct3d-12 -p
  wcontainers-direct3d-11 -- --test-threads 3`.
- The seeded fuzz: `cargo test -p wcontainers-shaders --test fuzz -- --include-ignored`
  (WCONTAINERS_FUZZ_SEED, WCONTAINERS_FUZZ_CASES, WCONTAINERS_FUZZ_FIRST).
- Literal check: `RAYON_NUM_THREADS=3 nix develop -c cargo run -q -p wcontainers-literals -- check`.
- The comparison (scratch graphics-container-readers/compare, not in the repository): build with
  `cargo build --release --manifest-path <scratch>/compare/Cargo.toml` and the same
  CARGO_TARGET_DIR, run `COMPARE_ROOT=<checkout> COMPARE_CASES=20000 COMPARE_SEED=1
  COMPARE_EXTRA=<dir>:<dir> target/graphics-container-readers/release/container-compare`. The old
  readers are in compare/src/old_*.rs. It leaves out the Direct3D 12 scan where a mutated 5.1
  range is longer than 65,536 (40 cases at seed 1): both old and new list every element (For the
  owner).

## Measurements (Apple M5 Max, shared; load averages at the time)

- Shaders suite after f4302b4a's code (load about 40): 111 passed, 0 failed, 6 ignored (the three
  corpora, cache, geometry, the long fuzz). Direct3D 12: 49 passed (unit 16, cache 1, device 18,
  front 12, gpu 2). Direct3D 11: 66 passed (unit 5, cache 1, front 60), the counts its status
  gives. The final run of all three at the tip: see "Last run" below.
- Comparison, 20,000 cases and 1,690 inputs: 0.9 s wall, release build, load 60.
- Seeded fuzz at its defaults (seed 1, 2,000 cases; load 12 to 29): the three plain tests pass;
  the long one ends the process at case 98 (`views.ps_level0.dxil`, bytes flipped at 879 and 3078)
  with `libc++abi: terminating` inside the DXIL converter: gap A11 of the shaders track (dxil-spirv
  ends the process on such DXIL), known before this change. The case's bytes are signed the same
  as before (the checksums agree on every case) and the reads before the conversion (program type,
  PSV0) agree, so it is reached as before. Not run past it.
- Not run: the WebAssembly build of the shaders crate (no wasm32-wasip1 target or wasi-sdk in this
  Mac's shell; md-5 is no_std pure Rust and was already built for the fuzz test); the Windows and
  GPU-rig runs of the fronts (the front tests above run on the recording back end and Metal).

Last run, at the tip (296ddb6e, with this track's commits; load 7 at the start, 75 at the end),
the three crates' whole suites: 227 passed, 0 failed, 6 ignored (suite-final.log in scratch).

## Departures from the plan and the brief

- Order: the shaders crate's container.rs went first, as both fronts' changes need the reader
  public; then Direct3D 12, the fuzz test, Direct3D 11 last.
- The container.rs commit does more than make the reader public and add the checksum: the
  fronts' needs (each cut of a chunk list reported, an element iterator that reports a cut
  element, the instruction walk, the shared token fields and the opcode table) went in with it,
  as one commit.
- Outside the track's files, one line each, needed for the reader to be reachable or its checksum
  used: crates/wcontainers-shaders/src/lib.rs (`pub mod container`), its Cargo.toml (md-5 from
  dev-dependencies to dependencies), crates/wcontainers-direct3d-12/Cargo.toml (md-5 gone),
  crates/wcontainers-direct3d-12/tests/front.rs (its checksum test calls the shared one), and one
  hunk of Cargo.lock.
- dxbc-spirv does not back the reader: its container and signature classes are C++ behind the
  converter's boundary, and reaching them from both fronts and the root signature reader would add
  extern functions and copies for a reader of a few hundred lines.

## Dependencies and licences

No new dependency. md-5 0.11.0 (MIT OR Apache-2.0), already in the tree, is now a normal
dependency of wcontainers-shaders and no longer one of wcontainers-direct3d-12.

## For the owner

- Behaviour on malformed containers changed as listed under "Behaviour" (the Direct3D 11 front
  refuses more of them; the converters' reflection drops a signature element whose name lies past
  its chunk). Example: a pixel shader whose container is cut inside its input signature chunk was
  accepted by the Direct3D 11 front's signature reader and is now refused with E_INVALIDARG.
- crates/wcontainers-shaders/examples/containers.rs (the shaders track's corpus tool) still walks
  a container's header and chunks itself, with its own plausibility rules (at most 64 chunks,
  alphanumeric tags). Not this track's file; it could use `container::chunk_list`.
- Not changed, found by the comparison: the Direct3D 12 declaration scan (dxbc.rs
  `element_slots`) lists every element of a shader model 5.1 range, whose length comes from the
  bytecode (up to 2^32). A mutated range made one comparison run take 45 GB on this Mac before it
  was stopped. The shaders crate refuses such a range by name (`BindingsBeyondLimit`); the
  Direct3D 12 front does not yet. Fixing it changes behaviour, so it was left for its own item.
