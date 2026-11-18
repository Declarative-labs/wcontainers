# Status: graphics, shaders track (G6)

Branch `phase-2-graphics-shaders`. Plan: `docs/phase-2-graphics.md`, sections 6, 7, 8 and G6 in 12.

## Where things stand

Milestone G6a (first slice): done in run 3; runs 4, 5, 6, 8 and 9 fixed what the independent checks
of runs 3, 4, 5, 7 and 8 found (listed under "Run 4" to "Run 9" below); run 7 drew run 6's fixes on
the GPU. Milestone G6 in full (section 12's pass bar) is not done: run 9 (the first run on it) fixed
the run 8 check's findings and built the cache of section 8; run 10 built ten of the variant key's
rewrites with drawn tests and the corpus of real programs' shaders; run 11 built depth bias and the
draw bases from an indirect draw's arguments, with drawn tests; run 12 built the minimum
level-of-detail clamp with drawn tests; run 13 built clip distances, depth clamping and mirror-once
addressing in the program with drawn tests; run 14 built pulled vertex inputs and constant buffers
read through a texture with drawn tests; run 15 built filtering and border colours in the program
with drawn tests against real Direct3D 11 on the RTX 3080 and against WARP; run 16 began the stage
helper of section 7: geometry shaders (DXBC) as compute, drawn against real Direct3D 11; run 17
built stream output through the stage helper, atomic operations, view counters (append, consume,
`IncrementCounter`) and storage textures, each tested against real Direct3D 11 on the RTX 3080;
run 18 built typed buffers (`texel_buffers`, both routes, DXBC) and storage textures through
buffers (`texels_through_buffer`, DXBC) with tests against real Direct3D 11; run 19 built the
built-in program clearing storage texture views (5.1) with a test against WARP and the GPU's
Direct3D 11, gave the corpus test view formats, and found the cause of the one WebAssembly
difference; run 20 ported Direct3D's tessellator to a compute program (tested against the RTX
3080's tessellator) and converted hull shaders (to compute) and domain shaders (to vertex
programs), not yet drawn through `Stages`; run 21 (the first run of the G6 milestone's
"run 1" series) converted DXIL's typed buffers and storage images without a format, which stopped
Godot's Direct3D 12 compute shaders; run 22 numbered DXIL's resources by Direct3D 12's rule
(register spaces and registers of any size), which stopped Godot's Direct3D 12 graphics shaders
(`BindingsBeyondLimit`) and the games' `RegisterSpace`; run 23 converted vertex shaders without a
position and ran hull and domain shaders through `Stages`, drawn equal to real Direct3D 11; run 24
converted DXIL atomics on typed buffers, storage textures and view counters, gave pixel shaders
their layer and viewport index (46 game shaders), and lowered structured group-shared memory; run 25
gave DXIL storage textures the bound view's format, read DXIL's infinite and NaN constants, kept
raw buffers to one view per binding and split arrays of textures (Godot's Direct3D 12 shaders); run
26 read multisampled array textures as a texture per layer, applied views' component mappings in the
program and made atomic minimums and maximums of the other signedness compare-exchange loops; run 27
made the tessellator compile under fxc, converted geometry shaders without a position, gathers with
run-time offsets and DXIL helper lane queries; run 28 converted wave operations (WGSL's subgroups),
DXIL gathers with run-time offsets and coherent storage accesses at workgroup scope, and gave the
interface `STORAGE_OFFSET`; run 29 built `STORAGE_OFFSET` and `EARLY_TESTS`, numbered shader model
5.1 DXBC by Direct3D 12's rule and fixed draw arguments dropping later helper members; what remains
is under Next. Runs 21 to 29 merged origin/phase-2-graphics-webgpu,
-webgl, -direct3d-11, -direct3d-12 and -rig (run 28: one conflict in the interface's `slow.rs`,
the same three slow paths in two orders, ours kept).

Done:
- naga carried in the tree with the GLSL writer changes G3 needs (below). Other tracks get it by
  merging this branch: the workspace `Cargo.toml` patches `naga` to `third_party/naga`.
- dxbc-spirv built into `crates/wcontainers-shaders` (git submodule, upstream, pinned) natively and
  for WebAssembly (`wasm32-wasip1`, wasi-sdk 25).
- `convert` (6.4) for DXBC by the direct route: dxbc-spirv's legalised IR, serialised, read in Rust
  (`src/ir.rs`) and lowered straight into naga (`src/lower.rs`). Vertex, pixel and compute shaders.
  Every corpus shader converts or fails with a named reason. `tests/corpus.rs` checks each shader's
  verdict against the one recorded in `tests/expected/<corpus>.txt` (written WGSL read back and
  validated by naga with WebGPU's core capabilities; GLSL ES 3.00 written and compiled by
  glslangValidator when `GLSLANG` is set), so any change either way fails until the record changes.
- Chrome's own compile of every written file (run 2): `crates/wcontainers-shaders/web/check <out-dir>`
  compiles each WGSL file with Chrome's WebGPU and each GLSL file with its WebGL 2 on the desktop's
  RTX 3080 (one GPU job, counts below). Every converted shader's WGSL compiles in Chrome (run 3:
  corpus 127, rig DXBC 56, rig DXIL 24, own 6). The GLSL is written with the WebGL 2 back end's options
  (`FORCE_POINT_SIZE`, Embedded 300 for WebGL, no y flip as on a context with EXT_clip_control).
- Fixed on the way (run 2): constants that are infinite or NaN are now read from a local variable
  (Tint refused `bitcast<f32>(4290772992u)` as a constant expression); naga's WGSL writer writes f32
  literals whose shortest digits exceed f32::MAX with their exact digits (Tint refused
  `340282350000000000000000000000000000000f`); naga's GLSL writer writes the sample mask as
  `gl_SampleMask[0] = int(..)`, reads `uint(gl_SampleMaskIn[0])` and enables
  GL_OES_sample_variables for it.
- The module is the same on every run and host: the corpus test converts each shader twice and
  compares, and the WebAssembly build's output files and verdicts equal the native build's byte for
  byte (re-run in run 3 after all its changes: corpus, rig DXBC, rig DXIL and own).
- Drawn and compared (run 3, `tests/draw.rs`, Windows, one GPU job): the own shaders drawn by real
  Direct3D 11 and, converted, through the WebGPU back end (wgpu on Direct3D 12) and the WebGL 2
  back end (ANGLE), all on the RTX 3080: every picture equal texel for texel (4 pairs on WebGPU, 2
  on WebGL 2; the other 2 use bitfield operations GLSL ES 3.00 lacks). The rig's triangle from its
  DXIL and from its DXBC through both back ends: equal to real Direct3D 12's picture, largest
  difference 0. Drawing found and fixed two lowering bugs: dynamically indexed constant buffers were
  declared 4096 vectors long (dxbc-spirv's choice), so WebGPU wanted 64 KiB bound (now the size the
  bytecode declares); and plain samples at level zero used naga's comparison-only `Zero` level,
  which naga's HLSL writer (wgpu's Direct3D 12 path) writes as a method HLSL lacks.
- The rig's Direct3D 11 scenes through the direct3d-11 track's d3d11.dll (which calls `convert`):
  8 outputs pass (triangle, quads, topology, instanced, targets in 4 formats), instanced newly so
  after the constant buffer fix; the other 141 are missing outputs or failures of the front (it
  refuses what it does not serve yet, for example depth-stencil views, 1D and 3D textures, queries).
  Through the direct3d-12 track's d3d12.dll only the triangle passed in run 3, when that front did
  not call `convert` yet; it does now (merged in run 6, `wcontainers-direct3d-12/src/device.rs`,
  `convert` with `VariantKey::default()`), not re-run here. DXIL is proven by `tests/draw.rs`.
- dxil-spirv built in too (submodule, pinned), and `convert` for DXIL: dxil-spirv's SPIR-V, read by
  naga's SPIR-V reader (`src/dxil.rs`), then given 6.4's binding rule. The rig now has 72 DXIL
  shaders: 49 convert (run 4), the rest are named (counts under Measurements).
- DXIL, run 2: `draw_base: Helpers` reads the bases from the vertex helper block exactly as the DXBC
  route lays it out (the entry point becomes a function, called by a new entry point that passes
  the bases, or zeros for `draw_base: None`). Inputs and outputs are in the reflection, named from
  the container's `ISG1` and `OSG1` signatures; test `dxil_reflection_matches_dxbc` checks that the
  rig's five shaders compiled both by fxc and dxc give the same semantics, locations, types and
  interpolation by both routes.
- Independent check: six shaders of our own (`tests/own/own.hlsl`; the sixth, `ps_window`, added in
  run 3 to draw with `vs_fullscreen`), compiled by fxc 10.0.26100.0 on the desktop
  (`tests/own/compile.cmd` through `tools/graphics-rig/remote`; recompiling gave the same bytes for
  the first five). All give WGSL that compiles in Chrome; four compile as GLSL ES in Chrome, two use
  bitfield operations GLSL ES 3.00 lacks. Their meaning is checked by drawing (above).

- DXIL's sampled pairs are in the reflection (run 3); test `dxil_bindings_and_sampled_pairs_match_dxbc`
  checks that the 18 rig shaders compiled by both fxc and dxc that convert by both routes get the same
  bindings and pairs (4 of them sample).

Run 4 (the independent check's findings, each fixed or recorded):
- `convert` refuses every `VariantKey` rewrite it does not build, by name, on both routes
  (`Unsupported("LodBiasInShader")` and so on; a root signature or `layout` is
  `Unsupported("RootSignatureLayout")`). Before, it returned the plain program for them. Test
  `tests/variants.rs`.
- Malformed bytecode: a container whose checksum does not match is `Rejected` before either converter
  sees it, as real Direct3D 11 and 12 refuse it with `E_INVALIDARG` (test
  `direct3d_refuses_a_wrong_checksum_as_convert_does` in `tests/draw.rs`). Natively the converters
  are built with C++ exceptions, their assertions throw (`cpp/assert` stands in for `assert.h`), and
  each boundary turns any exception into `Rejected`; a panic in our lowering becomes `Malformed`;
  `Ir::op` checks its id. What still ends the process, from the seeded fuzz of re-signed mutations
  (`tests/fuzz.rs`): dxil-spirv's bitcode reader on signed but malformed DXIL (segmentation faults
  and its own `std::terminate` in `bc/cast.hpp`), and one DXBC case that exhausts memory; see Known
  gaps. On WebAssembly (no exceptions, panics abort) every such failure traps the shaders module
  instance, which the host must report as a failed conversion and re-instantiate.
- The corpus tests compare every verdict with the recorded one (above); the 191-shader corpus test is
  `#[ignore]`d with its reason and fails when run without `WCONTAINERS_SHADER_CORPUS`.
- The independent check's own shaders (`ps_xtemp`, `ps_index`, `ps_posw`, `vs_flat`, `ps_early2`) are
  in its scratch; ours for the same cases are `ps_local`, `ps_depth`, `vs_shade`/`ps_flat`.
- Interpolation between stages: interface change `VariantKey::output_interpolation` (below). A vertex
  program writes each output as the pixel program reads it (flat, linear, centroid, sample), on both
  routes, so `nointerpolation` floats, `noperspective` and `centroid` link on WebGPU and GLSL ES.
- Local arrays indexed at run time (`dcl_indexableTemp`): dxbc-spirv resolves them and leaves an
  unused declaration without a type, which we now skip (it was named `StructType`).
- Two resources at one register in different register spaces are `Unsupported("RegisterSpace")` on
  both routes under Direct3D 11's numbering, until root signatures number them.
- naga's HLSL writer gives the fragment position's w as 1/w, as the common form and GLSL have it.
- `resinfo` at level 0 no longer asks for the level count (a bound view has at least one level), so
  GLSL ES can write it: 3 more corpus shaders are written as GLSL ES.
- DXIL pixel shaders report early tests, side effects, discards and per-sample running; test
  `dxil_bindings_and_sampled_pairs_match_dxbc` now also compares the extras of both routes. DXIL
  reports `PrimitiveId`, `VertexWithoutPosition` and `Texture1DArray` by name where the module
  would have been invalid.
- `CONVERTER_VERSION` carries a BLAKE3 hash of sources, not revisions written by hand. As run 4 left
  it, the hash missed dxbc-spirv's `ir/passes` and `cpp/assert` (it did not recurse there); run 5
  fixed that (below).
- The dynamically indexed constant buffers' sizes come from dxbc-spirv's own parser through the C
  boundary, matched by space and register; the Rust parser of SHDR is gone.
- naga's WGSL writer writes the module's diagnostic filters (`diagnostic(off,
  derivative_uniformity);`), so Chrome sees what naga validated.
- Six new shaders of our own (`tests/own/more.hlsl`, `spaces.hlsl`) for these cases.

Run 5 (the independent check of run 4, each finding fixed or recorded):
- DXIL integer signedness: dxil-spirv types integers as unsigned, and naga's SPIR-V reader took a
  conversion's, division's or shift's signedness from its types, so `fptosi`, `sitofp`, `sdiv`,
  `srem` and `ashr` became unsigned (wrong for every negative value). The carried reader now reads
  each instruction in the signedness it names (signed and unsigned division, remainder and modulo,
  right shifts, the four float conversions, `SConvert`/`UConvert`, bitfield extracts, and
  GLSL.std.450's `SAbs`, `SSign`, `SMin`, `SMax`, `SClamp`, `FindSMsb` and their unsigned forms):
  operands are reinterpreted, and the result is given the result type's signedness. Atomic and
  subgroup minimum and maximum, which cannot reinterpret memory, are refused by name
  (`Reader("... on integers of the other signedness")`) when the signedness differs.
- Texel offsets on DXIL loads (`OpImageFetch` with `Offset`/`ConstOffset`) were dropped silently; the
  reader now adds them to the coordinate, and refuses any other load operand it does not know by
  name (`unsupported image load operand`) instead of skipping it.
- `isnan`/`isinf` in DXIL modules: naga's WGSL writer (carried) now writes them for f32 as tests of
  the float's bits (the DXBC route still lowers them in `convert`; see Departures).
- DXIL modules get the same `diagnostic(off, derivative_uniformity)` as DXBC ones, so samples and
  derivatives in non-uniform control flow compile in Chrome.
- DXIL values dxc packs into one register are refused as `Unsupported("PackedStageValues")` instead
  of failing validation as `Invalid`.
- DXBC loops left from inside a switch case (`LoopBreakInSwitch`, refused before): the case sets a
  flag and leaves the switch, and a test after the switch leaves the loop.
- The DXIL boundary installs the same error log as the DXBC one, so dxbc-spirv's container errors
  become the returned reason instead of going to stderr (or `fd_write` on WebAssembly).
- The content key: `CONVERTER_VERSION` is now the crate version plus a BLAKE3 hash of everything
  that makes the module: the converters' C++ (every file under the built directories, recursively,
  `cpp/assert` and `build.rs` included), this crate's Rust sources and the carried naga's sources
  (`third_party/naga/src`). `build.rs` reruns when any of them changes and keeps the C++ objects
  while the C++ hash is unchanged, so a Rust change compiles no C++.
- The fuzz test prints every case our own checks refused (`ours: malformed ...`/`invalid ...`) and
  the status reports them (Measurements). On mutated bytecode they are refusals, not crashes; on
  real bytecode the corpus tests fail on them.
- New own shaders, `tests/own/signed.hlsl`, compiled by both fxc and dxc (`-Gis`): `ps_signed`
  (signed division, remainder, shifts, conversions, min, max, abs on negative values, exact as raw
  bits), `ps_offset` (loads and a sample with texel offsets), `ps_special` (NaN and infinity tests,
  scalar and vector), `ps_cases` (a loop with a switch whose cases leave early), `vs_pack`/`ps_pack`
  (values compilers pack). `tests/draw.rs` draws the DXIL builds too (`vs_transform.dxil` with the
  pixel shader's), against real Direct3D 11's picture of the fxc build.

Run 6 (the independent check of run 5, each finding fixed or recorded):
- DXIL constant buffers indexed at run time were still declared as dxil-spirv declares them, 64 KiB
  (4096 vectors), so a Direct3D 12 program binding its own smaller buffer failed WebGPU's minimum
  binding size and lost the device; run 3's fix was DXBC only, and the "How the fronts call" text
  claimed it for both. Now the DXIL route gives such a buffer the size the bytecode declares: the
  sizes come from the DXIL's own resource list (`dx.resources`, read by dxil-spirv's bitcode parser
  through the C boundary, `wcs_dxil_constant_sizes`), matched by space and register, read only when
  a buffer is declared at 64 KiB; the replaced types are compacted away. Test
  `dynamically_indexed_constants_keep_their_declared_size` now checks both routes on the rig's
  `constants.ps_slots` and `constants.ps_window` (16, 256 and 4096 vectors; 64 vectors) and the
  DXBC `instanced.vs`. The independent check's own `ps_cbuf` (scratch `check-g6a-r6/own`), converted
  on the laptop after the fix, declares `array<vec4<f32>, 5>` by both routes.
- Discard from DXIL: dxil-spirv writes `clip`/`discard` as `OpDemoteToHelperInvocation`, which naga's
  reader refused (`unsupported extension SPV_EXT_demote_to_helper_invocation`), so every Direct3D 12
  pixel shader with an alpha test failed. The carried reader now reads it as the IR's `Kill`, which
  WGSL writes as `discard` (a demote in WGSL too) and GLSL as `discard`. `IsHelperLane()` and the wave
  operations dxil-spirv guards with `OpIsHelperInvocationEXT` stay refused by the reader by name
  (naga has no such built-in). The check's `ps_flow` (dxc build, discard in nested loops) now
  converts, and its WGSL and GLSL ES validate (laptop, naga and glslang); not yet drawn.
- `tests/draw.rs`: the WebGL 2 skip reason is per route (fxc build, dxc build), so the dxc build of
  `ps_cases`, whose GLSL ES has no bitfield operations, is drawn on WebGL 2 (not yet run: the
  desktop was unreachable).
- `tests/own/route.hlsl` (`ps_rows`: a constant buffer indexed by the pixel's column, bound at its
  declared 80 bytes; `ps_clip`: discards inside a loop, then an implicit-derivative sample) and
  `compile.cmd` lines that compile it and `ps_textured` (`clip`) by fxc and dxc; compiled and drawn
  in run 7 (below).
- Fuzz: the check's run (seed 31337, 6000 cases) found one DXIL case killed for memory and showed that
  under an address-space cap the DXBC memory cases end as `Rejected` (the C++ `bad_alloc` caught at
  the boundary); recorded under Known gaps. Its 26 refusals by our own checks (a caught panic of
  naga's layouter on a too-large type, a lowering of mutated IR giving a u32 times f32 multiply) are
  refusals of signed but mutated bytecode, as Known gaps says.

Run 7 (run 6's fixes drawn):
- `compile.cmd` compiled on the desktop (fxc and dxc 10.0.26100.0). fxc refused `ps_clip` as run 6
  wrote it (a `[loop]` writing a vector's component by the loop's index cannot be unrolled), so its
  values now live in a local array (`dcl_indexableTemp` from fxc). Every file compiled before came
  out byte for byte as committed; the new ones are `ps_rows`, `ps_clip` (both builds) and
  `ps_textured.dxil`. All five convert by their route; verdicts recorded.
- Drawn (`tests/draw.rs`, numbers under Measurements): the DXIL builds of `ps_rows` (80-byte
  constant buffer bound, 5 vectors declared by both routes), `ps_clip` and `ps_textured` (discard)
  equal real Direct3D 11's picture texel for texel on WebGPU and WebGL 2, as do the DXBC builds; the
  dxc build of `ps_cases` equals it on WebGL 2 too.
- `ps_depth` on WebGL 2 now draws equal (the WebGL track patched its ANGLE's clip control), so it is
  no longer marked a known failure. `ps_size` on WebGL 2 now differs (texture size 1 by 1 instead
  of 8 by 8, in every covered texel) since that back end answers `textureSize` from a hidden uniform
  of its own (its commit 3ea9af6); the uniform does not reach this program. Marked a known failure
  of that back end with the reason (Known gaps); the WebGPU picture is equal.
- Fuzz re-run with the new own files (Measurements): 9 DXIL cases ended the process, no DXBC case.

Run 8 (the independent check of run 7, each finding fixed or recorded):
- Loops whose `continuing` block reads values the body made (dxc's loops that carry values): naga's
  GLSL and HLSL writers write `continuing` ahead of the body (`if (!loop_init) { .. }`) and wrote
  those values out again there, from variables the body had already changed, so the check's DXIL
  vertex shader drew nothing on WebGL 2 and natively. The carried writers now declare each such
  value before the loop and assign it where the body makes it (`compact::values_for_continuing`
  finds them). Tests in `tests/glsl_writer.rs`; `vs_carry` (both builds) draws equal (below). naga's
  MSL writer had the same shape; run 9 fixed it too.
- DXIL float to integer conversions: naga's `i32(x)`/`u32(x)` is not Direct3D's (NaN gives 0, the
  limits at and past them). Every f32 to i32 or u32 conversion of a DXIL module now calls a function
  that gives Direct3D's answer (`direct3d_float_to_int` in `src/dxil.rs`; the DXBC route already had
  it from dxbc-spirv). f16 or f64 sources are `Unsupported("FloatToIntWidth")`. `ps_ftoi` draws
  equal by both routes.
- Fine derivatives on WebGL 2: GLSL ES 3.00 has none, and naga's GLSL writer wrote `dFdx` for them
  (coarse on ANGLE here). The carried writer now refuses them with `MissingFeatures(DERIVATIVE_CONTROL)`
  where the language lacks them (ES, desktop below 4.50); coarse and plain ones are still the plain
  functions, which GLSL allows to be either. So a pixel shader using `ddx_fine`/`ddy_fine` is not
  created on WebGL 2 (Direct3D's feature levels below 11 have no fine derivatives either). Corpus
  `106_ps_code_ddy_fine` is now not written as GLSL ES.
- DXBC immediate constant buffers holding NaN or infinities (`NonFiniteConstantVector` before): the
  table is kept as u32 bits that loads reinterpret; vector constants holding them are read from a
  variable as scalars were. Arrays of them used as whole values stay `Unsupported("NonFiniteConstantArray")`.
  `ps_table` (fxc) draws equal.
- `CONVERTER_VERSION` no longer hashes `WASI_SDK_PATH` (the objects' stamp still does): the native
  and WebAssembly builds now give the same version (measured below). The corpus test writes it to
  `converter-version.txt` beside its outputs.
- The DXIL paragraph of "How the fronts call" now says that DXIL values keep their own widths.
- Found on the way, both on the DXIL route only, refused by name instead of failing validation
  (Known gaps): dxc's constants that are NaN or infinite (`Unsupported("NonFiniteConstant")`; dxc's
  build of `ps_table`), and a constant buffer component picked at run time, which dxil-spirv reads
  through a second, scalar view of the buffer (`Unsupported("ConstantComponentIndexing")`).

Run 9 (the independent check of run 8, each finding fixed or recorded, then the cache):
- DXBC integer negation of a value typed unsigned (fxc's `imad -b, c, a` for `a - b * c` on uints)
  was naga's `Negate` on u32, which validation refuses: it is now negated as i32 and reinterpreted.
- DXIL `sub 0, x` (integer negation, and the remainder of `x % 3`): dxil-spirv writes `OpSNegate` on
  its unsigned types; the carried reader now negates an unsigned operand as signed and gives the
  result its type's signedness.
- New own shader `tests/own/negate.hlsl` `ps_negate` (both compilers; fxc writes `ineg` and `imad`,
  dxc `sub i32 0, x` and `sub nsw i32 0, x`): converts by both routes and draws equal everywhere.
- `tests/draw.rs`: each DXIL build is now held to real Direct3D 12 drawing that same DXIL (root
  signature: constants at b0 as a root view, the texture in a table, a static linear clamping
  sampler), not to Direct3D 11's picture of fxc's build. The Direct3D 12 picture is also compared
  with Direct3D 11's and printed (not failed on). The back ends are now made before the Direct3D
  devices: with a Direct3D 12 device made first, wgpu's adapter request got Microsoft's Basic Render
  Driver (seen three times; cause not found).
- naga's MSL writer (wgpu's Metal path, the WebGPU back end natively on the Mac) gets the run 8 fix
  for `continuing` blocks that read the body's values; test in `tests/glsl_writer.rs`. Its output
  was read, not compiled: the Mac has no Metal toolchain installed.
- `ps_size` on WebGL 2 draws equal after merging the WebGL track's 4ab7bc84; its known-failure mark
  is gone, so the test fails again if it breaks.
- The run 8 count was wrong: `tests/draw.rs` then had 21 cases, all 21 equal on WebGPU by the DXBC
  route (the Measurements line said 20 of 20; corrected below).
- Bitfield operations on WebGL 2 (fxc's `bfi`/`ubfe` for ordinary arithmetic and the common
  full-screen vertex shader): plan 6.5 gives their lowering to the WebGL 2 back end's GLSL writing;
  until then those programs fail there, and that back end answers the failed link with device lost
  rather than an error. Recorded for the WebGL track (Known gaps).
- The cache of section 8 (`src/cache.rs`): one file, 16-byte header and checked entries, loaded into
  an index, appended to, merged by union; a cut or damaged entry ends the file there and is dropped;
  a key present with another payload is counted (`disagreements`) and the first kept. Converted
  shaders: key `cache::shader_key` (BLAKE3 of the source kind and bytes, `CONVERTER_VERSION` and the
  postcard form of the variant key; `convert` now gives modules this key), value `cache::encode`
  (postcard of the naga modules, entry points, stages and the reflection), `decode` validates again;
  `convert_cached` does both. State combinations: `record_combination` and `combinations` keep any
  serde type a front makes of its tuple (the interface's blocks now derive serde). Tests
  `tests/cache.rs` (round trip, merges in both orders, seeded cuts, a damaged byte, a disagreement,
  other files refused, combinations). The corpus test sends every converted shader (all five
  corpora) through `encode`/`decode` under each key a front asks for today (plain, draw bases from
  the helpers), requires the same bytes back and from a second conversion, and writes `cache.txt`.
- Found by that: DXIL records differed between the native and WebAssembly builds (the written WGSL
  did not): naga's SPIR-V reader made each function's global and constant expressions in the order
  of a hash map keyed by id, which differs between 32-bit and 64-bit targets. It now makes them in
  id order; every record of every corpus is equal between the builds.

Run 10 (variants and the real programs' shaders):
- `src/variant.rs` makes the variant key's rewrites on the common form after either route, so DXBC
  and DXIL get the same ones. Its primitives: the entry point wrapped (moved into a function and
  called by a new entry point that can read its inputs and replace, add or drop its outputs); the
  stage's helper block grown member by member (a block the draw bases made is extended, so
  `DrawBase` stays at 0 and later values follow, each in `reflection.helpers`); and an expression
  replaced by a call to a new function (put ahead of every function in the arena; the Emit range is
  split), which lets a rewrite compute new operands without reordering a function's arena.
- Built, each drawn on the RTX 3080 (`tests/variant_draw.rs`, both routes, numbers under
  Measurements): `VIEWPORT_ADJUST` (WebGPU's fractional viewport natively against the whole-pixel
  one with the variant, on WebGPU and WebGL 2), `CLIP_DEPTH_REWRITE` (WebGL 2 made without
  EXT_clip_control against real Direct3D 11's depth buffer), `VIEWPORT_DEPTH_FLIP` (against the
  swapped range's definition applied to Direct3D 11's depth; see Departures), `SAMPLE_MASK` (WebGPU's
  native mask against the variant, four samples), `COUNTING` (outputs 1.0 at location 0 with target
  0's alpha, keeps depth and sample mask outputs, removes storage writes; its sum equals Direct3D
  11's occlusion query, on both back ends), `DUAL_SOURCE` (targets 0 and 1 as `blend_src` 0 and 1;
  against Direct3D 11's SRC1 blending), `TEXTURE_INFO` (level counts from the helpers; against
  WebGPU's query and Direct3D 11, and on WebGL 2), `samples` (`GetRenderTargetSampleCount` as the
  variant's constant: DXBC directly, DXIL through dxil-spirv's sample count option; without it
  `Unsupported("SampleCount")` on both routes), `lod_bias` (implicit level becomes a bias, a bias or
  explicit level grows by it, gradients scale by its power of two; against Direct3D 11's MipLODBias
  on a 4-level texture, which shows Direct3D applies it to explicit levels too; comparisons refused
  `LodBiasOnComparison`, depth levels `LodBiasOnDepthLevel`), `constants_as_storage` (read-only
  storage at the first storage binding past the program's, `AddedBinding::ConstantsAsStorage`).
- Still refused by name (run 11 built `DepthBiasInShader` and `DrawBaseFromArguments`, run 13
  `ClipDistanceInShader`, `DepthClampInShader` and `MirrorOnceInShader`): the flags
  `ProvokingRewrite`, `EarlyTestsInShader`, `AntialiasedLinesAsQuads`, `PrimitiveIdAsGeometry`,
  and the fields (run 15 built `FilterInShader` and `BorderInShader`) `ConstantsAsTexture`,
  `VertexPulledInShader`, `StorageFormats`, `TexelBufferRoute`, `StoragePacked`,
  `TexelsThroughBuffer`, `NormAsInteger`, `TargetsInShader`, `RootSignatureLayout` (run 12 built `MinLodClamp`).
- The real programs' shaders: `examples/containers.rs` finds every DXBC and DXIL container in the
  files under a directory (header and chunk table checked, distinct ones by BLAKE3) and lists where
  each came from. Run over the rig's real programs (C:\wc\programs) it found 890: 529 DXBC and 361
  DXIL, from bgfx's examples and libraries, SDL3 (Chocolate Doom's) and the DirectX SDK samples'
  compiled shaders (Godot and the SDK tutorials compile theirs at run time from source, so none of
  theirs). Kept in scratch `graphics-shaders/games` and on the laptop `corpus/games`, outside the
  repository like the plan's corpus (bgfx BSD-2-Clause, SDL zlib, DirectX-SDK-Samples MIT; test data
  only). `tests/corpus.rs` `game_shaders_give_their_recorded_verdicts` (ignored without
  `WCONTAINERS_GAME_SHADERS`) holds them to `tests/expected/games.txt` and `games-dxil.txt`, and sends
  each through the cache record as the other corpora.
- Found by them and fixed: naga's SPIR-V reader put an `OpUndef`'s zero value inside an Emit range
  (invalid; dxc's `GetDimensions`), and panicked where an integer texel coordinate's vector type had
  not been made (bgfx's own shaders); multisampled array textures from DXIL are now refused by name
  as the DXBC route refuses them (WGSL has none).
- Shared drawing code: `tests/common/mod.rs` holds what `draw.rs` and `variant_draw.rs` share (the
  inputs, one draw through a back end with a `Setup` of what a case changes, and real Direct3D 11's
  draw with a `Reference` of its changes: texture levels, sampler bias, blending, an occlusion query,
  a depth buffer and range).

Run 11 (variants):
- `DEPTH_BIAS` (pixel programs): the wrapped entry point writes the fragment's depth plus Direct3D's
  bias from `HelperValue::DepthBias` (units times the step, plus the slope scale times the larger
  of `|dpdx(z)|` and `|dpdy(z)|`, clamped by the clamp's sign), computed in a prologue before the
  program runs. A depth the program writes itself is left as written (the fixed function biases only
  the rasterised depth; not measured against Direct3D). The wrapper gained a prologue and a way to
  read an entry input, adding it as an argument where the program has none.
- `DrawBaseSource::Arguments` (vertex programs), on both routes: each route builds the bases' block
  as for `Helpers`, then one rewrite on the common form turns it into `HelperValue::DrawArguments`
  (word offset, indexed, draw number) and replaces each load of the base vertex or first instance by
  a call that reads the argument buffer bound read-only at `DRAW_ARGUMENTS_BINDING` in set 3: word
  offset + 2 (base vertex, as i32) or + 3 (first instance), one more when indexed.
- New own shaders `vs_ids`/`ps_ids` (`variants.hlsl`, fxc and dxc): the vertex and instance numbers
  as colours, the triangle's number so the provoking vertex does not matter. Every other own shader
  recompiled byte for byte as committed.
- Drawn (`tests/variant_draw.rs`, numbers under Measurements): depth bias against real Direct3D 11
  drawing into D24_UNORM_S8_UINT with the rasteriser's bias, ours into Depth32Float taken to 24-bit
  codes, unclamped and clamped (the test also checks that the clamp changes Direct3D's depth); the
  draw bases (direct by the helpers, indirect by the arguments; non-indexed with a first vertex,
  indexed with a first index and base vertex; two instances from a first instance of 1 or 2) against
  real Direct3D 11's `DrawInstanced`, `DrawIndexedInstanced` and their indirect forms.
- The shared harness (`tests/common`) draws with bases, indices and indirect arguments
  (`Setup::bases`, `Reference::bases`) and gives Direct3D 11 a biased 24-bit depth buffer.

Run 12 (variants):
- `min_lod_clamped` (any stage), on both routes, by the functional specification's 5.8 (scratch
  `graphics/downloads/d3d11spec`): one rewrite outlines every sample whose sampler is biased or whose
  texture is clamped into one callee that applies both (`samples` in `src/variant.rs`, which
  replaced `lod_bias`). A clamped texture's samples go through the merged sampler of their pair, a
  new handle at `CLAMPED_SAMPLER_BINDING` + the pair's index in `sampled_pairs` in set 3 (the front
  makes it with its level range starting at the clamp), and read zero when the clamp passes the
  view's last level; gathers (level 0 only) also read zero for a clamp of 1 or more; loads read zero
  when their level is below the clamp's whole part (`loads`). `HelperValue::MinLod` is (the clamp's
  f32 bits, the view's level count) as two u32. Comparisons of a clamped texture are
  `Unsupported("MinLodOnComparison")`: Direct3D compares the out-of-bounds zero with the reference
  by the sampler's function, which the program does not know.
- New own shaders `ps_minlod` (an implicit sample, a sample at level 0.25 and loads of each level, by
  the pixel's place) and `ps_mingather` (a gather), fxc and dxc. Every other own shader recompiled
  byte for byte as committed.
- Drawn (`tests/variant_draw.rs`): against real Direct3D 11 with `SetResourceMinLOD` on a 4-level
  texture made with `RESOURCE_CLAMP` (numbers under Measurements).
- Clip distances natively (the reference the `CLIP_DISTANCE` variant needs, and what real programs
  with `SV_ClipDistance` need where the back end has them): a vertex program's clip distances are the
  common form's `@builtin(clip_distances)` array on both routes (DXBC in `lower.rs`, refused before
  as `ClipDistance`; DXIL already read by naga, now reflected as `SystemValue::ClipDistance`), and
  the module is validated with naga's `CLIP_DISTANCES`. `capabilities(module)` (public) gives the
  capabilities a module needs, WebGPU's core ones plus dual-source blending and clip distances where
  it uses them; `convert` and the corpus test validate with it. Cull distances and clip distances
  read by a pixel shader stay refused by name (`CullDistance`, `ClipDistance`).
- Carried naga: the GLSL writer enables `GL_ANGLE_clip_cull_distance` (WebGL's
  WEBGL_clip_cull_distance) for WebGL programs instead of `GL_EXT_clip_cull_distance`, which
  Chrome's WebGL 2 refuses.
- New own shader `vs_clip` (`vs_transform` with two clip distances), fxc and dxc; drawn natively
  on wgpu's Vulkan path (its Direct3D 12 path has no clip distances, wgpu issue 6236) against real
  Direct3D 11. The shared harness gained `webgpu_on(backends)`.

Run 13 (variants):
- Interface change `VariantKey::linked: LinkedValues { location, clip_distances }` (below): where the
  values `CLIP_DISTANCE` and `DEPTH_CLAMP` add pass from the vertex to the pixel program, which are
  converted apart. Four-component f32 perspective values from `location`, the clip distances first,
  then the unclamped clip z.
- `CLIP_DISTANCE`, both routes: the vertex program's `clip_distances` output is replaced by those
  values (`linked_out`); the pixel program reads them (`linked_in`) and, before the program runs,
  discards where one is negative (`samples` 1), or on four samples takes each distance times the
  fragment position's w (z over w, affine on the screen) to Direct3D's four standard sample positions
  from its derivatives, writes the samples where none is negative into the sample mask (combined with
  the program's own and with `SAMPLE_MASK`'s) and discards where none is (`clipped`). Refused by name:
  `samples` 0 (`SampleCount`), other counts (`ClipDistanceSamples`), per-sample programs on several
  samples (`ClipDistancePerSample`), a vertex count other than the key's (`LinkedClipDistanceCount`),
  a linked location the vertex program already writes (`LinkedLocationTaken`).
- `DEPTH_CLAMP`, both routes: the vertex program keeps clip z in 0..w (`position`) and passes the
  unclamped z; the pixel program writes depth = near + (far - near) * z * w_frag from
  `HelperValue::DepthRange`, plus the depth bias when `DEPTH_BIAS` is set too, clamped to the range
  (`depth`, which replaced `depth_bias`); a depth the program writes is only clamped.
- A wrapped program's own result struct loses its bindings when the wrapper changes the outputs, so a
  replaced output (clip distances) asks nothing of the language (`Wrapped::finish`).
- `mirror_once_in_shader`, both routes, in the one sample callee of run 12 (`mirror_once`): each
  coordinate whose address mode in `HelperValue::SamplerState` is mirror-once is folded to its
  absolute value (the front binds a clamping sampler there), and an implicit level becomes the
  unfolded coordinate's gradients (times two to a bias). Refused by name: comparisons
  (`MirrorOnceOnComparison`), gathers (`MirrorOnceOnGather`, their texels would come mirrored),
  texel offsets (`MirrorOnceWithOffset`), depth textures at an implicit level (`MirrorOnceOnDepth`).
  Cube coordinates are left alone. Interface: the sampler state's codes are the variants' order of
  `Filter` and `AddressMode` (a doc change).
- New own shaders `vs_deep` (`vs_transform` with z/w from -0.2 to 1.15) and `ps_mirror` (a sample at
  uv * 6 - 3), fxc and dxc; every other own shader recompiled byte for byte as committed.
- The harness: `Reference::depth_clip_off` and `address`, `Setup::address`.

Run 14 (variants):
- Interface change: `HelperValue::PulledStream { input }` is four u32 (offset, stride, divisor,
  bias) per pulled input, and `AddedBinding::PulledStream(n)` is the storage binding of the input at
  location n (below).
- `pulled` (vertex programs), both routes (`src/variant/pull.rs`): each pulled input leaves the entry
  point and is fetched in the wrapper's prologue by `pull_<location>(vertex index, instance index)`:
  element = vertex index when the divisor is 0, else (instance index + bias) / divisor; address =
  offset + element * stride; the element reads zero (and the defaults (0, 0, 0, 1) where the format
  has fewer components) unless all its bytes are inside the binding (`arrayLength`); each component
  read from the one word it lies in (Direct3D aligns components to their size). Normalised values are
  made exactly rounded from bits (the expansion of c / (2^n - 1) is c's bits repeated; the GPU's own
  division is c times a reciprocal, one step off, measured), half floats from bits with their NaN
  payloads (`unpack2x16float` on the RTX 3080 gives 0x7fffffff for every NaN; Direct3D keeps the
  payload, measured), 11- and 10-bit floats from bits. The values are given the declared input's
  kind by bit cast and its component count. Inputs in a struct, of 16-bit types or other shapes are
  refused by name (`PulledInputInStruct`, `PulledInputWidth`, `PulledInputType`).
- New own shaders `vs_pull`/`ps_pull` (`pull.hlsl`, fxc and dxc): thirteen inputs of thirteen formats;
  one point per vertex and instance carries one input's bits to an RGBA32_UINT target. Every other own
  shader recompiled byte for byte as committed.
- `tests/pull_draw.rs` (Windows, GPU job): four streams (per vertex, stride 48, 56 elements; per
  instance with step rates 3, 1 (bound at offset 8) and 0) drawn as 64 vertices by 64 instances from
  first instance 4, every input pulled, against real Direct3D 11's input assembler on the same
  seeded bytes (`WCONTAINERS_PULL_SEED`, `WCONTAINERS_PULL_CASES`). It also checks that Direct3D
  reads zeros and the defaults past the per-vertex stream's end.
- Interface change: `CONSTANTS_TEXTURE_WIDTH` (256): a constant buffer read through a texture has its
  16-byte row r at texel (r % 256, r / 256) of an RGBA32 unsigned 2D texture (below).
- `constants_as_texture` (any stage), both routes (`src/variant/constants.rs`): each named buffer's
  binding becomes an added texture binding (`AddedBinding::ConstantsAsTexture(register)`, the stage's
  set, the first texture binding past the program's); every load through it becomes a call that
  computes the byte offset from the types along its access path (struct members, arrays by stride,
  vector components, indices picked at run time as arguments) and fetches the row's texel, giving a
  scalar by its component or a vector by a swizzle where its first component is fixed; the old
  variable becomes private and compaction removes it. Other shapes are refused
  (`ConstantsAsTextureLayout`). The shared harness copies the constants into such a texture where a
  program asks (`constants_texture`).

Run 15 (variants):
- `filter_in_shader` (by texture) and `border_in_shader` (by sampler), any stage, both routes
  (`src/variant/filter.rs`, `src/variant/filter.wgsl`): every sample of such a texture, or through
  such a sampler, becomes a call into a filter library written in WGSL, parsed with naga's `wgsl-in`
  (already in every build through wcontainers-gpu) once per texture shape and imported into the
  module (types copied, the functions it calls first, equal copies made once; `Outlined` puts them
  ahead of the callees). The library reads texels with loads and filters by Direct3D's rules
  (functional specification 7.18): level of detail log2 of the longer of the coordinate's scaled
  gradients (coarse derivatives for implicit levels), plus the instruction's and the sampler's bias
  (`lod_bias`), clamped to the sampler's range and the resource clamp (`min_lod_clamped`);
  magnification at zero or below; point mip levels floor(lod + 0.5), linear ones blended by the
  fraction at 8 bits; linear taps at 8 bits of subtexel precision; every address mode per tap
  (mirror-once included) and the border colour; texel offsets; gathers of the linear footprint at
  level 0; anisotropic filtering along the major axis with up to the sampler's anisotropy in taps.
  The sampler's state, border colour and the texture's format components come from the helper
  block; level 0's size and the layer count are asked of the texture's variable in the callee (so
  the WebGL 2 back end's per-view size uniforms answer them); the level count from `TEXTURE_INFO`'s
  helper where set. The pair leaves `reflection.sampled_pairs` when no native sample uses it.
  Refused by name: comparisons (`FilterInShaderOnComparison`, `BorderInShaderOnComparison`), cubes
  (`FilterInShaderOnCube`), depth and integer textures (`FilterInShaderOnDepth`,
  `FilterInShaderOnInteger`), 1D arrays (`FilterInShaderDimension`).
- Interface change: `HelperValue::TextureComponents { texture }` (below): Direct3D reads a border
  colour as a texel of the view's format, so R32_FLOAT gives (border.r, 0, 0, 1); measured.
- Found and fixed: the helper block placed each member after the block's padded size, not after the
  last member's end, so a member following a small one (the level count after the bias, a resource
  clamp after a bias) sat at another offset than GLSL ES's std140 block reads it: WebGL 2 read the
  wrong value (every DXIL filtered draw read level count 0). Members are now packed as std140 packs
  them; the reflection gives the new offsets (a resource clamp after a bias now at 8, not 16).
- New own shaders `vs_filter`/`ps_filter` (`filter.hlsl`, fxc and dxc): a plane in perspective, the
  pixel shader sampling six ways by a constant (implicit, bias, level, gradients, offset, gather).
  Every other own shader recompiled byte for byte as committed.
- `tests/filter_draw.rs` (Windows, GPU job): seeded sampler states (`WCONTAINERS_FILTER_SEED`,
  `WCONTAINERS_FILTER_CASES`, default 8) on a 16 by 16 texture of five levels, R32_FLOAT (by
  `filter_in_shader`) and RGBA8 (by `border_in_shader`), every mode, both routes, WebGPU and WebGL 2,
  against real Direct3D 11 on the RTX 3080 and against WARP. The two Direct3D implementations differ
  (the GPU blends mip levels otherwise, the trilinear optimisation; at a level of detail of exactly
  .5 the GPU takes the higher level, WARP the lower), so a texel passes when it equals either; one
  that equals neither passes when it does with the level of detail moved by 1/256 either way (a
  texel that close to a decision), or where the two themselves differ (counted as a third answer).
  Where WebGPU has the sampler natively (RGBA8, no border, mirror-once or bias) the variant is also
  held to WebGPU's own sampler (or WARP), and the own sampler to the GPU's Direct3D. Anisotropic
  cases are a known failure (Known gaps).

Run 16 (the stage helper, first part: geometry shaders on WebGPU):
- `src/stage.rs`, `src/stage/helper.rs`, `src/lower/geometry.rs`, `src/build.rs` (a small builder of
  naga functions for the programs the crate makes itself). A DXBC geometry shader converts to two
  modules: its compute form (`modules[0]`) and the vertex program that draws its output
  (`modules[1]`, the pass-through); `reflection.extras` is `StageExtras::Geometry`, its outputs are
  reflected as a vertex stage's (the pixel stage links to them, with `output_interpolation` as for a
  vertex program), its slots are in set 0 as a compute program's. DXIL geometry shaders are refused
  by naga's SPIR-V reader (`spirv-reader unsupported`, recorded).
- The data, in buffers the helper owns (`stage.rs` says it in its module comment): vertex records
  (the vertex stage's outputs, record `instance * count + vertex` in draw order, four-word slots:
  the position, then each location the geometry shader reads), and the geometry output (each
  invocation owns `max_vertices` records, then from `INDEX_BASE` `2 * max_vertices` index slots per
  invocation, strips cut with the restart index, unused slots restarting; point output is drawn
  without indices, unused vertices placed outside the clip volume). Every invocation writes at fixed
  slots, so the output keeps Direct3D's order with no counting across invocations.
- The geometry shader's compute form: one invocation per input primitive and geometry instance (x)
  and API instance (y); it fills the input arrays from the records (lists n per primitive; strips
  step by one, odd triangles (i, i + 2, i + 1)), gives `SV_PrimitiveID` (per API instance),
  `SV_GSInstanceID` and the input vertex count, runs the shader, whose `EmitVertex`/`CutVertex` call
  emit and cut functions writing the current outputs into its slots. Refused by name: streams other
  than 0 (`GeometryStreams`), no position output, inputs of other shapes, clip and cull distances,
  `SV_RenderTargetArrayIndex` and `SV_ViewportArrayIndex` (the rig's `gs_layers` and `raster.gs`),
  and a geometry shader's key asking for the last vertex stage's rewrites (`GeometryVariant`).
- The vertex stage's compute form (`stage::vertex_program`, made by `Stages::new`): the vertex
  program converted with every input pulled (`VariantKey::pulled`), its entry point called by a
  compute entry point that makes the vertex and instance numbers as a draw does (first plus the
  place, or the index there, read from the index buffer as words, plus the base vertex), and writes
  the record. Not pulled inputs are refused (`VertexInputNotPulled`).
- `Stages<B>` (`stage::Stages`): `new(backend, vertex, geometry)` makes both compute programs;
  `run(backend, &StageDraw)` records the two dispatches for one API draw (outside a pass) and returns
  `FinalDraw` (the buffer, the index offset, topology, count), which the front draws in its pass with
  `passthrough()` and its pixel program, `vertex_layout()`, restart on. The helper's own bindings are
  in set 1 of its compute programs (values, index words, records, geometry output), which no compute
  program of the API uses. It counts `SlowPath::GeometryAsCompute`. Refused by name in `run`:
  indirect draws, indexed strips (`GeometryIndexedStrip`: Direct3D restarts them at the largest index,
  which needs a pass numbering the primitives), triangle strips with adjacency, 8-bit indices; a
  topology that does not give the shader's input primitive is `StageError::TopologyMismatch`.
- New own shaders (`geometry.hlsl`, fxc, and dxc for the geometry shaders): `vs_geometry`,
  `ps_geometry`, `gs_quads` (points to quads), `gs_cut` (none, one or two triangles by primitive
  number, a cut and a four-vertex strip), `gs_instances` (`[instance(3)]`), `gs_lines` (line strips),
  `gs_points` (a varying number of points), `gs_adjacent` (triangles with adjacency),
  `gs_line_adjacent` (lines with adjacency). Every other own shader recompiled byte for byte as
  committed.
- `tests/geometry.rs` (any machine): what each declares, both modules and the vertex compute form
  validate, the cache record round-trips with both modules' keys, the refusals.
  `tests/geometry_draw.rs` (Windows, GPU job): thirteen cases (point, line, triangle lists and
  strips, line lists and strips and triangle lists with adjacency, indexed with base vertices, API
  instances with first instances, geometry instances) per seed of 64 vertices, through the WebGPU
  back end against real Direct3D 11 on the RTX 3080.
- Corpus records: the rig's `geometry.gs_adjacency`, `gs_points`, `gs_strips` and `streamout.gs`
  now convert (stream output is declared to the API, not in the bytecode); `gs_layers` is refused
  `RenderTargetArrayIndex`, `raster.gs` `ViewportArrayIndex`.

Run 17 (stream output, atomics, counters, storage textures):
- Stream output through the stage helper (`src/stage/stream_out.wgsl`, `stage::stream_program`,
  `StreamOutput`, `StreamTargets`; `Stages::new(backend, vertex, Option<geometry>,
  Option<&StreamOutput>)`): from the vertex stage alone or the geometry stage, one workgroup of 256
  splits the units (geometry invocations, or API primitives) into runs, counts each run's whole
  primitives, scans, and writes each primitive's vertices element by element in Direct3D's layout
  (entries with a buffer, first component, count, gaps). Writing stops for every buffer at the first
  primitive one cannot hold; the filled sizes advance; words 4 and 5 of the filled buffer are the
  primitives written and needed (the stream output statistics query). Strips give odd triangles as
  (i, i + 2, i + 1), which real Direct3D 11 matches. `passthrough()` and `vertex_layout()` give
  `None` when nothing is drawn. Point output now writes index slots too. Stream 0 only.
- Found by it: DXBC's `mad` was lowered as a multiply then an add; the RTX 3080's Direct3D fuses it,
  so streamed positions were 1 ulp apart. It is now naga's `fma` (as dxbc-spirv's own SPIR-V writer
  emits `Fma`), leaving the device the choice it has for the native program. Every other drawn test
  (`draw`, `variant_draw`, `filter_draw`, `geometry_draw`) still passes after it.
- Atomics (`FunctionLowering::atomic`): shared memory and raw or structured storage buffers that an
  atomic operation uses are declared with atomic scalars, their plain loads and stores becoming
  atomic loads and stores; every operation of the IR (add, subtract, signed and unsigned minimum and
  maximum, and, or, xor, exchange, compare-exchange, increment, decrement). The atomic scalar is
  signed where the declaration takes signed minimums or maximums; one taking both is refused
  `AtomicSignedness`. A compare-exchange always takes its result (naga's HLSL writer has no form
  without one). WGSL's compare-exchange is the weak one (Known gaps).
- View counters: `DCL_UAV_COUNTER` is one `atomic<u32>` storage buffer at the slot's
  `SlotKind::Counter` binding (224 plus the slot), listed in the reflection; `IMM_ATOMIC_ALLOC` is an
  atomic add returning the value before, `IMM_ATOMIC_CONSUME` an atomic subtract returning the value
  after, as Direct3D defines them.
- Storage textures: a typed storage view of a texture takes its format from
  `VariantKey::storage_formats` (the bound view's; refused `StorageFormat` without it,
  `StorageFormatAbsent` for formats WGSL cannot store, `StorageFormatKind` when the declaration's
  type is of another kind); access from the declaration: read-only, write-only, or read-write, which
  WebGPU has for `r32float`, `r32uint` and `r32sint` only (others refused `ReadWriteStorageFormat`
  until the through-buffer route). Loads, stores (the value made four components of the format's
  kind), size and layer queries. Atomics on them are refused `StorageImageAtomics` (WebGPU has none:
  `AtomicsThroughBuffer`).
- New own shaders (`compute.hlsl`, fxc and dxc; every other own shader recompiled byte for byte):
  `cs_atomics` (every atomic on shared memory and a raw buffer, signed ones on a structured buffer,
  compare-exchange loops, a counter view, append and consume views), `cs_decrement` (a decremented
  counter), `cs_images` (rgba8unorm, r32float read-write, an rgba16uint array, rg32sint).
- `tests/compute.rs` (any machine): what each declares, WGSL read back and validated.
  `tests/compute_draw.rs` (Windows, GPU job): both cases per seed against real Direct3D 11 on the
  RTX 3080, every word of every view and every counter (via `CopyStructureCount`) compared; what
  depends on invocation order (the appended values, the last exchange) is put in order first.
  `tests/geometry_draw.rs` gained `stream_output_matches_direct3d`: eight cases (geometry shaders
  with cuts, strips, lines, points, instances; the vertex stage alone with strips, indexed lists
  with a base vertex and instances, lines, points; the first buffer filling up) per seed, every byte
  of both buffers and the statistics query against real Direct3D 11.
- DXIL compute shaders with these views are refused by naga's SPIR-V reader ("unsupported image
  dimension", the typed buffer gap below).

Run 18 (typed buffers):
- Two interface changes (below): `VariantKey::texel_formats` (the bound views' formats by texture
  slot; storage slots keep `storage_formats`), `HelperValue::TexelBufferView { set, binding }` (the
  view's first byte and element count), and `TEXEL_BUFFER_TEXTURE_WIDTH` (2048).
- `TexelBufferRoute::Storage` (WebGPU), DXBC (`src/variant/texel.rs`, declared in `lower.rs`): each
  typed buffer is an `array<u32>` storage buffer at its slot's binding (read-only at a texture slot),
  read and written through functions made once per view: loads decode the view's format (the
  vertex pulling decoders, `pull.rs`, shared), stores encode it as the functional specification
  3.2.2 and 3.2.3 say (integers clamped; normalised values NaN to zero, clamped, scaled, plus or
  minus a half and truncated; floats to half, 11 and 10 bits toward zero, NaN kept quiet, past the
  largest finite giving it, no sign giving zero for negatives), atomics (32-bit integer formats
  only, as in Direct3D) by index. Elements at or past the length read zero in the format's
  components and the defaults in the others (measured: Direct3D gives alpha 1 for a format without
  it), take no writes, and atomics there do nothing and return zero. Elements narrower than a word
  are written by a compare-exchange loop on their word, so those words are atomic.
- `TexelBufferRoute::Texture` (WebGL 2): at texture slots only (`TypedBufferStorageAsTexture`
  otherwise), a 2D texture of the view's format, element e (from the buffer's start) at texel
  (e % 2048, e / 2048), read with a load, the same bounds.
- Refused by name: no route (`TypedBuffer`, the default key), no format (`TypedBufferFormat`), a
  format with no typed buffer form (`TypedBufferFormatAbsent`: sRGB, shared exponent, packed 16-bit,
  depth, compressed), a format of another kind than the declaration (`TypedBufferFormatKind`),
  atomics on a format without them (`TypedBufferAtomicFormat`), typed buffers in geometry shaders
  (`TypedBufferInGeometry`: the stage helper does not fill their helper values yet). DXIL typed
  buffers stay refused by naga's SPIR-V reader.
- The builder (`build.rs`) no longer emits atomic results, and runs variant-style code (`raw`).
- New own shaders (`texels.hlsl`, fxc and dxc; every other own shader recompiled byte for byte):
  `cs_texels` (four views loaded, their lengths, a float view stored, a 32-bit unsigned view with
  atomics, loads and stores; the stored values written to the results too), `ps_texels` (a float and
  an unsigned view read by a pixel shader, and their lengths).
- `tests/compute.rs`: `cs_texels` converts for every loadable format at t0 and u0 and its WGSL reads
  back; every refusal. `tests/compute_draw.rs` `typed_buffers_match_direct3d` (seeded formats from
  those the device has, first elements, lengths and bytes): every result word, every byte of both
  written views against real Direct3D 11 on the RTX 3080, normalised stores within Direct3D's
  permitted 0.6 of a code (below). `tests/variant_draw.rs` `typed_buffers_draw_as_direct3d_does`:
  `ps_texels` by the storage route on WebGPU and the texture route on WebGPU and WebGL 2, every
  texel against real Direct3D 11's picture. The shared harness binds typed buffers by the pixel
  key's route and fills their helper values (`Setup::texel_buffers`, `Reference::texel_buffers`).
- Storage textures through a buffer of their texels (`texels_through_buffer`, by storage slot), DXBC:
  the same functions as typed buffers (`TexelPlace::Image`), the texel at ((z * rows per layer + y)
  * row pitch + x) from `HelperValue::TexelLayout` (interface change: it now also gives the view's
  width, height and layers or depth, for bounds and size queries). Loads decode, stores encode
  (narrow texels by compare-exchange on their word), atomics on 32-bit formats, float exchange as
  u32 words; a texel outside the view reads zero and takes no writes; size queries from the layout.
  1D, 1D array, 2D, 2D array and 3D views; refused by name: multisampled ones, geometry shaders
  (`TexelsThroughBufferInGeometry`), formats without a typed form (`StorageFormatAbsent`), formats of
  another kind (`StorageFormatKind`). Storage textures without the variant still refuse atomics
  (`StorageImageAtomics`).
- New own shader `cs_through` (`texels.hlsl`, fxc and dxc): atomics (add, max, or, with the value
  before) on an R32_UINT texture, an R32_FLOAT texture read and written, a 2D array of a float format
  and a 3D texture of an unsigned format written, their sizes, invocations past the views. Its values
  are exact in f32 and each texel takes one kind of atomic, so the result depends on no order (a
  first version mixed add and max on a texel, and its float arithmetic differed by 1 ulp between
  the two compilers' orderings; both changed in the shader, not in the test).
- `tests/compute.rs` `storage_textures_convert_through_buffers`; `tests/compute_draw.rs`
  `storage_textures_through_buffers_match_direct3d` (seeded formats among those the device has as
  typed storage views, seeded texels; our buffers' rows 3 texels longer and layers a row taller than
  the views, padding seeded): every texel of every view against real Direct3D 11's textures.

Run 19 (the built-in programs, the corpus with views):
- `src/builtin.rs`: `storage_clear(format, dimension)` is the compute program a front dispatches for
  `ClearUnorderedAccessView*` on a texture view: one module per storage format (the 40 WGSL has)
  and view dimension (`D2`, `D2Array`, `D3`; cube views refused `ClearDimension`), parameters in a
  48-byte uniform at `CLEAR_PARAMS_BINDING` (`clear_params`: the value words, then the box from
  origin to end, x, y and layer or slice), the view write-only at `CLEAR_VIEW_BINDING`, set 0,
  workgroups of 8 by 8 by 1 (`clear_groups`). `storage_clear_value(format, values, float)` gives the
  words: float clears pass the floats (denormals made zeros of their sign, as Direct3D's clear
  does, measured) for the store to convert; integer clears give each component's low bits as its
  own bits (normalised codes as the float the store scales back to the code, floats from their
  bits). Refused by name, for the front to clear another way: float clears of integer formats
  (`FloatClearOfInteger`; Direct3D 11 drops them, measured), the most negative code of a signed
  normalised component (`SnormMostNegative`: a store gives -1.0 the code above it), formats with no
  storage form.
- `capabilities(module)` now adds naga's 16-bit normalised storage formats where a module declares
  such a view.
- `tests/builtin.rs` (any machine): every format and dimension, the values (every 8- and 16-bit
  normalised code round trips), the parameters. `tests/clear_draw.rs` (Windows, GPU job): seeded
  formats among those both have as typed storage views, views (a 2D layer, array layers, 3D
  slices), texels and values (`WCONTAINERS_CLEAR_SEED`, `WCONTAINERS_CLEAR_CASES`, default 64),
  against WARP and the RTX 3080's Direct3D 11. The two disagree on integer clears: WARP gives the
  low bits as the functional specification and Microsoft's documentation say; the GPU saturates
  each value to the component's bits, and reads the values of B8G8R8A8_UNORM, R10G10B10A2_UNORM and
  R11G11B10_FLOAT as floats. The test holds integer clears to WARP and counts the GPU's
  differences; float clears to normalised formats round a value half way between codes up on WARP
  and down on the GPU (both within the specification's 0.6 of a code), so a float clear passes
  where it equals either (it equals the GPU's in every case run).
- The corpus test: a shader refused `StorageFormat` or `TypedBuffer` under the plain key is
  converted again with every storage and texture slot given a view of R32_FLOAT, then R32_UINT,
  then R32_SINT (the formats WebGPU reads and writes), typed buffers through storage buffers; the
  verdict is `ok views:<format>` with the first that converts, or the failure with "(with views)".
- The one WebAssembly difference (`d599fbc7efa911be.dxbc`, Known gaps) is C++'s unspecified order
  of evaluating a call's arguments: dxbc-spirv's `ir_pass_arithmetic.cpp` (line 1375) makes the
  divide helper's result as `CompositeConstruct(type, add(UDiv(a, b)), add(UMod(a, b)))`, so gcc
  (native, right to left) appends the modulo first and clang (wasi-sdk, left to right) the divide.
  About ten other statements build operands the same way (`ir_pass_arithmetic.cpp` 937, 1033, 1055;
  `ir_pass_scratch.cpp` 774; `dxbc_registers.cpp` 573, 626; `dxbc_converter.cpp` 3658;
  `dxbc_io_map.cpp` 203). No compiler choice makes every build agree (clang on Windows evaluates
  right to left too, for the Microsoft ABI), so the fix is upstream: each such operand made in its
  own statement. Not changed here (the submodule is upstream's, unpatched); to send upstream.

Run 20 (the tessellator):
- `src/stage/tessellator.wgsl`: Direct3D's fixed-function tessellator as one compute program, ported
  from Microsoft's reference tessellator (MIT, "Copyright (c) Microsoft Corporation", as Mesa carries
  it in src/gallium/auxiliary/tessellator, read at Mesa main on 2026-11-17): triangle, quad and
  isoline domains, the four partitionings (powers of two as integer, as the reference has it), the
  four output topologies, in the reference's 16.16 fixed point. One invocation per patch; three
  passes (`stage::Tessellator::run`): count each patch's points and indices, a one-workgroup scan
  giving each patch its first point and index and writing an indexed indirect draw, then write the
  points (u, v, the patch's number, its primitive number) and the indices. The hull stage's output
  layout (header and patch records) is `stage::tess`; buffers are sized by the largest factor the
  hull shader declares (`tess::patch_bounds`).
- One fix found by the test: a factor's cull test reads its bits, since a comparison on the GPU
  flushed a denormal edge factor to zero and culled a patch both the reference and the GPU keep.
- `tests/tessellation_draw.rs` (Windows, GPU job): patches of seeded factors (ordinary values, past
  the largest, whole numbers and a step either side, halves, NaN, zeros, negatives, infinity,
  denormals) for every domain, partitioning and output topology (32 hull shaders and 3 domain shaders
  in `tests/own/tessellation.hlsl`, fxc), real Direct3D 11's domain shader output streamed (domain
  location and primitive number) against our points and indices read back. 16 seeds, 512 draws: in
  every one each patch's primitives equal the RTX 3080's exactly (every coordinate's bits), patches
  in draw order. The order of primitives inside a patch, and which vertex a triangle starts at
  (about a third start at the same one), differ from the GPU's: the test compares each patch as a
  set of primitives with their winding kept. That start matters for flat-interpolated domain
  outputs (the provoking vertex); not matched (the GPU's order is its own, not the reference's).
- fxc (which wgpu's Direct3D 12 path uses here) fails on the program with an internal error
  ("argument pulled into unrelated predicate"), so the test runs wgpu on Vulkan. Chrome's compile
  not yet checked (Known gaps).

- Hull shaders (DXBC, `src/lower/hull.rs`): a compute program, one invocation per patch (x) and API
  instance (y), reading its input control points from the vertex records (positions from slot 0),
  running the control point function once per output control point (its id stored before each
  call), then the patch constant function, and writing the patch's record: each control point's
  slots (user locations sorted, then `SV_Position` as data), the patch constants' slots, the outer
  and inner factors; patch 0's invocation writes the header (`stage::tess`: words per patch and per
  point, where constants and factors start, each location's slot). Reflection: `StageExtras::Hull`
  (domain, partitioning with powers of two as integer as dxbc-spirv gives it, output, counts).
- Domain shaders: a vertex program whose vertex number picks a point (u, v, patch, primitive);
  `SV_DomainLocation` is (u, v, 1 - u - v) for triangles and (u, v, 0) otherwise, control point data
  and patch constants come from the patch's record through the header's slots (zero for a location
  the hull shader does not write), factors and the primitive number likewise. Outputs as a vertex
  shader's.
- The corpora: all 18 hull and domain shaders of Wine's corpus and the rig now convert, and their
  WGSL and that of the 35 own ones (`tests/own/tessellation.hlsl`) compile in Chrome; GLSL ES has
  no storage buffers or compute, so none is written for WebGL 2 (which has no tessellation).
- Not yet drawn: hull and domain shaders run through `Stages` (Next).

Run 21 (DXIL typed buffers and storage images, for Godot's Direct3D 12 shaders):
- `src/dxil/images.rs`: before naga reads dxil-spirv's SPIR-V, each image type of a typed buffer
  (`DimBuffer`) becomes a 1D image and each storage image without a format gets a 32-bit format of
  its texel's kind (the words are edited in place; the variables' space, register and kind are
  noted). After reading, numbering and the draw bases: storage images take the bound view's format
  from `storage_formats` (refusals as DXBC's: `StorageFormat`, `StorageFormatAbsent`,
  `StorageFormatKind`, `ReadWriteStorageFormat`), their access from what the program does (read,
  write, or both); typed buffers take the DXBC route's forms (`texel_buffers`: `Storage` or
  `Texture`, `Native` refused `TypedBuffer`), each load, size query and store becoming a call of
  `variant::texel`'s functions (signed coordinates bitcast, narrower values padded), any other use
  refused `TypedBufferUse`. The new functions are put before the module's own (`functions_first`,
  shared with the float to integer pass; `place_calls` likewise).
- naga's SPIR-V reader (carried, `third_party/naga`): takes the `NonPrivateTexel` image operand and
  the `NonPrivatePointer` memory operand (every storage access of the IR is non-private), takes
  `MakePointerAvailable`/`MakePointerVisible` with their scope only on workgroup memory (which
  every target keeps coherent within its workgroup; dxil-spirv puts them on every groupshared
  access), refuses them elsewhere (`UnsupportedMemoryAccess`) and refuses image write operands
  other than hints and non-private (`UnsupportedImageWriteOperand`; before, all were dropped with
  a warning, so a coherent write lost its coherence silently). `OpImageQuerySize` on an array now
  gives the layer count as its last component (it gave the size alone, and `GetDimensions` of a
  2D array read past it).
- Corpora (laptop, counts below): games DXIL 47 more convert (WGSL; compute, so no GLSL ES), the
  rig's DXIL 2 more. Still refused, all named: atomics on images and typed buffers (naga's reader
  has no `OpImageTexelPointer`, "storage class %11": 5 games, 4 own, 1 rig), mixed view kinds in
  the corpus test (`TypedBufferFormatKind` 5 games), `RegisterSpace` 10, and the rest as before.
- At the end of run 21 the build folders were deleted (laptop
  `~/wcontainers-graphics/target/phase-2-graphics-shaders`, 23 GB; desktop
  `C:\wc\target\phase-2-graphics-shaders`), so the next run builds from clean; `out` and the logs
  are kept.
- Tests: `storage_textures_match_direct3d` (compute_draw) and `typed_buffers_draw_as_direct3d_does`
  (variant_draw) now also convert dxc's DXIL of the same HLSL and compare it with real Direct3D
  11's result for fxc's DXBC (results under Measurements).

Run 22 (Direct3D 12's numbering, for Godot's graphics shaders and the games' register spaces):
- DXIL resources are numbered by Direct3D 12's rule (plan 6.4 as now written; see Departures):
  within the stage's set, each kind's slots are the program's own resources of that kind in
  (space, register) order from the kind's first binding. Any space and register fits; only a
  program with more resources of a kind than its range holds is `BindingsBeyondLimit`. The front
  finds each binding's descriptor by `ReflectedBinding::space` and `register` (G5's `locate`
  already does), so no root signature enters the key. DXBC keeps Direct3D 11's numbering, so a
  shader model 5.1 DXBC program using a space other than 0 is still `RegisterSpace`.
- `cpp/dxil.cpp` gives dxil-spirv remappers that bind each resource at descriptor set = its
  Direct3D class (constant buffer, shader resource view, unordered access view, sampler, counter)
  and binding = its index in a table (class, space, register) returned beside the SPIR-V, so
  resources of different classes at one register stay apart (before, a `StructuredBuffer` at t3
  and a `RWStructuredBuffer` at u3 took one binding, and counters went to space + 1, colliding with
  space 1). Shader resource views of buffers are now `SlotKind::Texture`, as on the DXBC route and
  as G5's `locate` looks for them (it maps `Storage` to unordered access ranges only).
- Interface: `ReflectedBinding::slot()` (binding minus its kind's first binding); `VariantKey`'s
  per-slot fields and `Reflection::sampled_pairs` name slots, which are the registers under
  Direct3D 11's numbering. The variant code (`slot_of`, constants as storage or a texture, DXIL's
  storage and typed buffer formats) now looks slots up so. A front setting a per-slot field for a
  DXIL program takes the slot from the plain conversion's reflection.
- Tests: `dxil_numbers_register_spaces_by_the_programs_own_slots` (variants.rs: `ps_spaces.dxil`,
  t0 in spaces 0 and 1, gets texture bindings 32 and 33 and pairs (0, 0), (1, 0); its fxc 5.1 build
  stays `RegisterSpace`); `dxil_bindings_and_sampled_pairs_match_dxbc` now compares each resource's
  kind, space, register and set and the pairs by register, and checks each route's own numbering;
  `tests/own/twospaces.hlsl` (`ps_twospaces`, dxc with t0 in spaces 0 and 1, fxc reading t0
  twice) drawn in `tests/draw.rs` against real Direct3D 12 (the test's root signature now has an
  SRV range in each space).
- Results (Measurements): the 10 games' DXIL shaders refused `RegisterSpace` convert (9 written as
  both WGSL and GLSL ES, 1 WGSL only), `ps_spaces.dxil` and `ps_twospaces` convert; no other verdict
  changed in any corpus. Every written WGSL compiles in Chrome. `ps_twospaces` draws equal to real
  Direct3D 12 (DXIL) and 11 (DXBC) through both back ends.
- At the end of run 22 the build folders were deleted again (laptop
  `~/wcontainers-graphics/target/phase-2-graphics-shaders`, 4.9 GB; desktop
  `C:\wc\target\phase-2-graphics-shaders`); the next run builds from clean.

Run 23 (what the Direct3D 11 and 12 tracks waited on: vertex shaders without a position, hull and
domain shaders in `Stages`):
- Merged origin/phase-2-graphics-webgpu, -webgl, -direct3d-11 and -direct3d-12 (no conflicts).
- A vertex (or domain) program writing no `SV_Position` now converts on both routes
  (`stage::clip_unpositioned`, run by `convert` before the variants): the entry point becomes a
  function, and a new entry point adds the position (0, 0, 0, -1), outside every clip volume, so a
  draw of it rasterises nothing and the stage helper's records carry it. Before, such programs
  (stream output from the vertex stage, vertex shaders feeding hull or geometry shaders) were
  `VertexWithoutPosition`. Verdicts that changed, every one to ok (WGSL and GLSL ES written):
  corpus 6, own `vs_patch`, the rig's `tessellation.vs` (DXBC and DXIL). No other verdict changed.
- `Stages::tessellated(backend, vertex, hull, domain)`, `InputTopology::PatchList(n)`: per draw the
  vertex stage as compute writes the records, the hull program (one invocation per patch and
  instance) writes the patch records into a buffer the helper owns (header plus `patch_words` per
  patch, from `StageExtras::Hull`), the tessellator runs, and `FinalDraw::tessellated` (new,
  `TessellatedDraw`) names the indexed indirect draw's arguments and the patches and points the
  domain program reads (how a front draws it: "How the Direct3D fronts call `convert`"). The hull
  stage's sets travel in `StageDraw::geometry_sets` (no new field, so the fronts' struct literals
  still build). Counted as `SlowPath::TessellationAsCompute`. Geometry shaders and stream output
  after tessellation are refused (no constructor takes them yet).
- `Stages::destroy` and `Tessellator::destroy` release programs and buffers (asked by G5).
- Tests (`tests/tessellation_draw.rs`, Windows, wgpu on Vulkan as the tessellator's test):
  `tessellated_patches_draw_as_direct3d_11_does` draws the rig's tessellation scene's seven patches
  (its real fxc shaders: a pass-through vertex shader with pulled inputs, hull shaders passing
  control points with factors from a constant buffer, domain shaders that bend the surface) all
  solid, against real Direct3D 11 drawing the same on the RTX 3080: the triangle (fractional odd)
  and the four quad patches (odd, even, pow2, integer) equal texel for texel; the isoline patch
  (lines) has every texel Direct3D drew, 36 texels only ours drew and 97 colours 1 or 2 steps apart
  (each API's line rule: Vulkan's here). `rig_tessellation_scene_draws_as_direct3d_11_does` runs the
  rig's scene itself (lines for six patches): no texel Direct3D drew is missed and the solid patch
  is equal; per cell 133 to 390 texels differ (36 to 59 drawn only by ours at line ends, 0 to 32
  more than 2 steps apart, not analysed further: where lines cross, which line's colour a texel
  takes follows the line rule too, expected but not checked). The tessellator test still passes (4
  seeds, 128 comparisons).
- Fronts: `cargo check -p wcontainers-direct3d-11 -p wcontainers-direct3d-12 --all-targets` on the
  desktop builds with these changes.
- At the end of run 23 the build folders were deleted (laptop
  `~/wcontainers-graphics/target/phase-2-graphics-shaders`, 4.9 GB; desktop
  `C:\wc\target\phase-2-graphics-shaders`); the next run builds from clean.

Run 24 (what the fronts' real programs wait on: DXIL atomics, the games' layer reads, structured
group-shared memory):
- Merged origin/phase-2-graphics-direct3d-11, -direct3d-12 and -rig (no conflicts; webgpu, webgl
  and the shared line had nothing new).
- naga's SPIR-V reader (carried) takes pointer types of the Image class and `OpImageTexelPointer`:
  the texel is read as the image indexed by its whole coordinate (never emitted), which only atomic
  operations take; a multisampled image is refused (`UnsupportedInstruction`).
- `src/dxil/images.rs`: right after reading, each atomic on a texel pointer becomes a call of a
  function of its own taking the coordinate, the value and any comparand (`texel_atomics`; any
  other use of a texel pointer is `TexelPointerUse`), so the module can be typed. `finish` writes
  those functions by the image's route: typed buffers through `TexelBuffer::atomic` (their words
  atomic of the view's kind), storage textures in `texels_through_buffer` through the texel
  layout (`at`), view counters (dxil-spirv's one-element R32 typed buffers at its counter set) as
  the interface's counter, one atomic word at the counter binding. A storage texture with atomics
  not read through a buffer is `StorageImageAtomics`, as on the DXBC route, which both fronts
  answer by asking for `texels_through_buffer`. Storage textures in `texels_through_buffer` are
  now read and written through buffers on the DXIL route too (loads, stores, size and layer
  queries), as on the DXBC route; this was in Next since run 21.
- `cpp/dxil.cpp`: raw and structured buffers are storage buffers (dxil-spirv's `IDENTITY` made a
  structured buffer of 4-byte elements with a counter a texel buffer, which needed a format no front
  gives), with dxil-spirv's storage buffer alignment set to 4 (each view bound at its own first
  byte, as the fronts bind them). A storage buffer the program only writes is read-write (WebGPU
  has no write-only ones).
- Interface: `HelperValue::Layer` and `HelperValue::Viewport` (below). DXBC pixel shaders reading
  `SV_RenderTargetArrayIndex` or `SV_ViewportArrayIndex` read the draw's run's value from them;
  the reflection lists the inputs as those system values. Writing them from a vertex shader stays
  `RenderTargetArrayIndex`/`ViewportArrayIndex` (reported absent, plan 4.12); from a geometry
  shader the stage helper does not carry them yet. DXIL pixel shaders reading them stop in naga's
  reader ("unsupported builtin %9").
- Structured group-shared memory whose members dxbc-spirv types apart (`StructType` before): an
  array of structs, members at the offsets their alignment allows; an address's index after the
  array's picks the member. Several scalars read as one vector from consecutive members of one
  type; other cases are `StructMembersAsVector`; atomics on struct members `StructAtomic`.
- The corpus test retries a shader refused `StorageImageAtomics` with every storage slot read
  through a buffer (verdict `ok views:<format> through`), as the fronts do.
- New own shaders (fxc 10.0.26100.0 and its dxc, `compile.cmd`): `layers.hlsl` (`gs_layers`,
  `ps_layer`) and `cs_shared` in `compute.hlsl`.
- Verdicts that changed (all named before and after): games DXBC 47 to ok (46 layer reads, 1
  behind structs), games DXIL 2 to ok and 3 to later named reasons (`StorageFormatKind`,
  `TypedBufferFormatKind`: the corpus test's single view kind; `ConstantComponentIndexing`), Wine's
  corpus 4 to ok (2 storage texture atomics through buffers, a layer read, a viewport read), rig
  DXBC `compute.ps_overlap` and rig DXIL `d3d12.cs_add` to ok, own DXIL `cs_decrement` to ok,
  `cs_texels` to `NonFiniteConstant`, `cs_through` to `StorageFormatKind` (mixed view kinds in the
  corpus test; it converts and draws with its real formats, below), `cs_atomics` to
  `AtomicSMin on integers of the other signedness` (dxil-spirv declares buffers of unsigned words
  and takes a signed minimum on them), new `ps_layer.dxbc`, `cs_shared` (both) ok, `gs_layers`
  `RenderTargetArrayIndex`, `ps_layer.dxil` reader-refused.
- Drawn against real Direct3D 11 on the RTX 3080 (Measurements): `cs_through.dxil` (atomics, loads,
  stores and sizes of storage textures through buffers), `cs_decrement.dxil` (a counter),
  `cs_shared` both builds, and `layer_and_viewport_inputs_read_the_draws_run` (variant_draw: real
  Direct3D 11 draws into four layers through `gs_layers`, layer n through viewport n % 2; ours
  draws each layer alone with the run's helper values): all equal, texel for texel and word for
  word. `cs_atomics.dxil` is held in `compute_draw` as a known refusal with its reason, so the test
  fails when that changes.
- At the end of run 24 the build folders were deleted (laptop
  `~/wcontainers-graphics/target/phase-2-graphics-shaders`, 8.1 GB; desktop
  `C:\wc\target\phase-2-graphics-shaders`, 5.9 GB); the next run builds from clean. `out`,
  `outw` and the logs are kept.

Run 25 (what the Direct3D 12 front's real programs wait on: Godot's shaders, sm6-uav-rwtexture):
- Merged origin/phase-2-graphics-webgpu, -webgl, -direct3d-12 and -rig (no conflicts; the shared
  line and direct3d-11 had nothing new).
- DXIL storage textures always take the bound view's format from `storage_formats`. dxil-spirv
  gives a texture the program reads its own `R32f`, which we kept (only formatless ones were
  retyped), so the Direct3D 12 front's `Rgba32Float` variant of vkd3d's sm6-uav-rwtexture still
  came back `r32float`. Now a read and written four-channel view is refused
  `ReadWriteStorageFormat`, which the fronts answer with `texels_through_buffer`. New own shader
  `ps_halves` (`uav.hlsl`, dxc: half writes are a load and a store each); test
  `read_written_four_channel_storage_goes_through_a_buffer`.
- `src/dxil/nonfinite.rs`: an infinite or NaN constant (a literal, a module constant holding one, or a
  private table starting at such values, as dxc writes `static const` arrays) becomes the result of a
  call, at the start of the function using it, of a function that loads the float's bits from a
  local variable and reinterprets them; a table is stored when the entry point starts. Run right
  after reading (it moves functions and compacts). `NonFiniteConstant` remains only for such a value
  in another address space or of a form not copied (refused by name). Own `ps_table.dxil` and
  `cs_texels.dxil` convert; both are now drawn from their DXIL builds (below).
- `cpp/dxil.cpp`: raw and structured buffers are `SSBO_SCALAR_ALIGNED` with dxil-spirv's
  conservative vectorisation on, so dxil-spirv reads them in words and adds no `uvec2`/`uvec4`
  views of the same buffer at the same binding (WGSL takes one variable per binding: 18 of Godot's
  shaders failed validation, "Bindings for [1] conflict"). No other corpus verdict changed and the
  drawn DXIL tests still pass (raw and structured buffers in `compute_draw`).
- `src/dxil/arrays.rs`: an array of textures (`Texture2D t[8] : register(t0)`; sampled or depth,
  not 1D, so typed buffers stay refused) becomes one texture per element at registers r to r + n - 1
  (Direct3D 12's numbering gives each its slot). An element picked by a constant is that texture;
  one picked at run time makes each image operation using it a call of a function switching on the
  index (the last element for an index past the array, which Direct3D leaves undefined). Arrays of
  storage views, samplers, typed buffers and any other use are still `ResourceArray`. The carried
  naga's SPIR-V reader gives an element of an image array the element's type (it refused it as
  "invalid image"). New own shader `ps_array` (`arrays.hlsl`, dxc); test
  `texture_arrays_are_a_texture_per_element` checks the bindings and the switch; not drawn yet.
- Godot's Direct3D 12 shaders in the games corpus: the 69 shaders the Direct3D 12 front's run 23
  dumped as refused (scratch `graphics-direct3d-12/run23/dump`, Godot's own shaders compiled at run
  time), copied as `godot-<stage>-<hash>.dxil` into scratch `graphics-shaders/godot` and `games`
  and the laptop's `corpus/games`. Verdicts now: 58 convert (WGSL written; compute has no GLSL ES),
  4 `StorageFormatKind` (the corpus test's one view format per kind), 2 `ResourceArray` (arrays of
  other views), 3 "memory access 0x28 outside workgroup memory" and 1 "image load operand 0x200"
  (`globallycoherent` views: WGSL has no device-scope coherence, left refused), 1
  `IsHelperInvocationEXT`.
- Drawn on the RTX 3080 (gpu1.log, gpu2.log in scratch `graphics-shaders/run25`): `cs_texels.dxil`
  (DXIL typed buffers with atomics, loads and stores) against real Direct3D 11's run of the fxc build,
  4 seeds, 0 elements apart (6 views of seeded formats each); `ps_table.dxil` against real Direct3D
  12 and 11, both back ends, 0 of 4096 texels differ.

Run 26 (what the fronts' real programs wait on: bgfx's multisampled arrays, Godot's view mappings):
- Merged origin/phase-2-graphics-direct3d-11 and -direct3d-12 (no conflicts; the others had nothing
  new).
- Interface change `VariantKey::texture_layers` and `AddedBinding::TextureLayer` (below). A
  `Texture2DMSArray` is refused `MultisampledArrayTexture` until the key gives its slot's layer
  count; then its variable becomes layer 0 at the slot's binding, layers 1 to n - 1 are added texture
  bindings, each load is a call switching on the layer (zero past the count), the layer count query
  is the key's count (`src/variant/layers.rs`, after every other rewrite, both routes). Own shaders
  `ps_fill` and `ps_msarray` (`msarray.hlsl`, fxc and dxc); tests
  `multisampled_array_layers_are_a_texture_each` (variants.rs) and
  `multisampled_array_layers_read_as_direct3d_does` (variant_draw.rs: real Direct3D 11 fills three
  layers of 4 samples a value per sample and reads them with the layer picked at run time, past the
  count too, and `GetDimensions`; ours fills a texture per layer): both builds 0 of 4096 texels
  apart on the RTX 3080 (run26/gpu1.log).
- Interface change `VariantKey::swizzled`, `ViewComponent` and `SlowPath::SwizzleInShader` (below):
  each load and sample of a mapped slot becomes a call composing its result from the mapping (a
  constant one is 1 or 1.0 by the texel's kind); a gather of component c gathers the component c
  maps to, or is the constant (`src/variant/swizzle.rs`, first of the rewrites, so filtering in the
  program and the layers act inside it). Depth-class textures (comparisons) with a mapping are
  refused `SwizzledDepth`. Tests `swizzled_slots_compose_what_they_read` (variants.rs) and the
  `swizzled` case of `variants_draw_as_the_native_feature_does` (the plain program reading texels
  uploaded already mapped, against the variant on the unmapped ones, both back ends).
- Interface change `Reflection::slow_paths` and `SlowPath::AtomicExchangeLoop` (below). A signed
  atomic minimum or maximum on memory whose other atomics are unsigned (or the reverse) is a
  compare-exchange loop: DXBC in `lower.rs` (`exchange_loop`; the memory is declared unsigned when
  both are used), DXIL in the carried naga's SPIR-V reader (it refused them), reported by scanning
  dxil-spirv's SPIR-V (`atomic_signedness_differs`). Typed views keep `AtomicSignedness`.
  `cs_atomics` now also takes signed minimums and maximums of its raw view's unsigned words; its
  DXIL build converts; test `atomics_of_the_other_signedness_are_exchange_loops`; the drawn result
  against real Direct3D 11 is in `compute_draw` (Measurements).
- The laptop stopped answering (no ping, no SSH) partway through run 26, so this run's tests ran on
  the desktop (Departures). Its build folder (`~/wcontainers-graphics/target/phase-2-graphics-shaders`,
  rebuilt early in this run) could not be deleted: delete it first next run if no build needs it.
- Commit 6898e7cb went out without the own corpus records of `ps_fill` and `ps_msarray`, so
  `own_shaders_give_their_recorded_verdicts` failed on it; b99ca30f records them. At the end of run
  26 the desktop's `C:\wc\target\phase-2-graphics-shaders` was deleted.

Run 27 (what the fronts' real programs wait on: the tessellator under fxc, Godot's helper lanes):
- Merged origin/phase-2-graphics-direct3d-11 and -direct3d-12 (one conflict in the interface's
  `slow.rs`: both sides' new slow paths kept, `ALL` now 105; the others had nothing new).
- The tessellator program compiles under fxc (wgpu's Direct3D 12 path, which the fronts and the
  rig's tessellation scene use): fxc failed with "no storage type for block output" (optimised) and
  "argument pulled into unrelated predicate" (`/Od`), found by writing the program as HLSL with
  wgpu's options and bisecting with fxc on the desktop. `stitch_regular` is now one loop choosing
  each quad's shape (three loops in a switch before), and the switches over the output topology and
  the triangle's edge are `if` chains and selects. `tests/tessellation_draw.rs` now runs on wgpu's
  Direct3D 12 (it ran on Vulkan for this reason): 16 seeds, 512 draws, every patch's primitives
  equal the RTX 3080's; the solid scene equal; the rig's scene: no texel Direct3D drew is missed
  (lines differ by each API's rule, as before) (run27/tess1.log).
- Geometry shaders writing no position (feeding stream output only) convert: the record's position
  slot carries (0, 0, 0, -1), as a vertex program without one does (`GeometryWithoutPosition` is
  gone). Own `gs_unplaced` (geometry.hlsl); `stream_output_matches_direct3d` has a case for it with a
  declaration skipping the position's place: seeds 1 to 4, 0 words apart from real Direct3D 11 and
  27 of 60 primitives written by both (run27/geom1.log).
- Gathers with offsets given at run time (`gather4_po`, `gather4_po_c`; `DynamicOffset` before, the
  fronts' test_gather, test_gather_c and gather files): the coordinate moves by the offset in texels
  of level 0, the offset's low six bits signed as Direct3D takes them (`offset_coordinate` in
  `lower.rs`). Own `ps_gatheroff` (offsets per pixel, some past six bits) drawn equal to real
  Direct3D 11 through WebGPU, 0 of 4096 texels apart (run27/draw3.log); Wine's 159 and 161 now
  convert. Exact where the texture's size is a power of two (the shifted coordinate is then exact);
  other sizes are not drawn in a test.
- `IsHelperLane()` and the helper guards dxil-spirv puts around wave operations: the carried naga
  reader reads `OpIsHelperInvocationEXT` as a private flag (naga as carried), which `dxil.rs`
  (`helper_invocations`) sets in pixel programs from the coverage (zero in helper invocations) and
  at each demotion. dxil-spirv's coverage input, a one-element array the reader kept (refused as
  "the sample mask stays an array"), is now a u32 argument (`sample_mask_input`). Own `ps_helper`
  (values.hlsl; dxc build shader model 6.6 with discard then `IsHelperLane()`, fxc build from the
  coverage): both builds drawn equal to real Direct3D 11's picture of the fxc build, 0 of 4096 texels
  apart. Not against real Direct3D 12: this desktop's Direct3D 12 refuses the 6.6 build's pipeline
  (`E_INVALIDARG`), so the draw test's new `dxil_against_11` holds that build to Direct3D 11. That
  helper invocations have zero coverage is measured on the RTX 3080 only (wgpu's Direct3D 12 and real
  Direct3D 11); WebGPU does not state it.
- Found by drawing `ps_gatheroff`'s dxc build against real Direct3D 12 (2218 of 4096 texels apart,
  run27/draw4.log): naga's SPIR-V reader skipped image sample operands it did not know, a run-time
  `Offset` among them, with only a log warning, so the DXIL build sampled without its offsets. The
  carried reader now refuses them (`unsupported image sample operand`, naga as carried), and the
  DXIL build of run-time gather offsets is refused until it is built as the DXBC route's is (Next).
- Godot's fragment shader with wave operations now stops at `Unsupported("Subgroups")`: WGSL's
  subgroups need naga's WGSL writer to write `enable subgroups;` and the validation here to take
  `Capabilities::SUBGROUP` with the subgroup operations and stages; then the helper guards above
  apply. Wine's 123 and 124 atomics shaders convert (records updated); own `ps_fill` is
  `glsl-compile` (GLSL ES 3.00 has no `gl_SampleID`; WebGL has no OES_sample_variables).

Run 28 (what the fronts' real programs wait on: Godot's wave operations and coherent compute
shaders, Direct3D 12's gathers and views at any first element):
- Merged origin/phase-2-graphics-direct3d-11 and -direct3d-12 (the others had nothing new).
- Wave operations convert (`Unsupported("Subgroups")` is gone): `capabilities()` gives naga's
  `SUBGROUP` for a module with subgroup operations or built-ins (a front creates such a program only
  where the back end has `Features::SUBGROUPS`), the carried naga's WGSL writer writes `enable
  subgroups;` and its WGSL reader takes it, and such modules turn WGSL's `subgroup_uniformity`
  check off as they do the derivative one (WGSL's default for it is an error; spec read in scratch
  `run28/spec`). Refused by name: subgroups in vertex programs (`SubgroupsInVertex`, WebGPU has
  none) and subgroup barriers (`SubgroupBarrier`). The carried reader now reads
  `OpGroupNonUniformBallotBitCount` (`WaveActiveCountBits`, `WavePrefixCountBits`), which it refused:
  a count of a ballot made in the module is a subgroup add of the ballot's predicate (reduce,
  inclusive or exclusive scan); a ballot of unknown origin is counted from its bits, reduce only.
  And `OpGroupNonUniformAllEqual` (`WaveActiveAllEqual`; WGSL has none) as `subgroupAll(x ==
  subgroupBroadcastFirst(x))`, every component of a vector. And `OpGroupNonUniformBallotFindLSB` and
  `FindMSB` as the lowest or highest set bit of the ballot's four words.
  Godot's fragment shader with wave operations now converts (its WGSL validates; GLSL ES has no
  subgroups). Own `ps_wave` (waves.hlsl, dxc only, shader model 6.0: lane counts and indices,
  sums, prefixes, ballots, reads of other lanes, after discards): `draw.rs`'s
  `wave_operations_draw_as_direct3d_12_does` against real Direct3D 12 through the WebGPU back end
  on wgpu's Vulkan (wgpu's Direct3D 12 path gives subgroups only through dxc), its device made with
  `SUBGROUP` (`webgpu_with`), as the back end does not ask for it yet (Next).
- DXIL gathers with run-time offsets: the carried reader reads a gather's `Offset` operand by moving
  the coordinate in texels of level 0, the low six bits signed, as the DXBC route does
  (`gather_offset_coordinate`); other run-time sample offsets stay refused. `ps_gatheroff`'s dxc
  build drawn against real Direct3D 12: 0 of 4096 texels apart (run28/draw1.log).
- Coherent storage (`globallycoherent`; Godot's compute shaders, memory access 0x28 and image operand
  0x200): WGSL's memory model makes every storage and texel access non-private and available and
  visible at workgroup scope (its "Private vs Non-private"), so the carried reader takes
  `MakePointerAvailable`/`Visible` and `MakeTexelAvailable`/`Visible` at workgroup, subgroup or
  invocation scope, and refuses wider scopes by name (`unsupported memory operand .. at scope ..`).
  Two of Godot's compute shaders now convert, two move on to `ResourceArray`.
- `VariantFlags::STORAGE_OFFSET` and `ReflectedBinding::stride` (interface change below): the
  stride is given for every storage binding (`variant/offset.rs`, test
  `storage_bindings_give_their_stride_and_offsets_are_refused`); the flag is refused by name
  (`Unsupported("StorageOffset")`) until built. The rewrite was written (scratch
  `run28/offset-attempt.rs`, and its drawn test against real Direct3D 11, every view of
  `cs_atomics`, `cs_shared` and `cs_decrement` at a seeded first element, as
  `run28/compute_draw-storage-offset.patch`) but gave invalid modules: it pointed each existing
  `Access` at index expressions appended after it, and naga requires an expression's operands to come
  before it in the arena. Next under Next.

Run 29 (what the fronts wait on: views at any first element, early tests, typed views in vertex
shaders under indirect draws, shader model 5.1 register spaces):
- Merged origin/phase-2-graphics, -webgl, -direct3d-11 and -direct3d-12 without conflicts. The
  shared line's new slot test built a `ReflectedBinding` without our `stride`; fixed in the
  interface commit below.
- `STORAGE_OFFSET` is built (`variant/offset.rs`): every index into a storage view's runtime array
  gets the view's offset in elements (`HelperValue::StorageOffset` over the stride) added, and every
  length of it loses them. The carried naga has a new public `compact::reorder_expressions(module,
  function, order)`, which rebuilds a function's expression arena in a given order with every handle
  renumbered (built on compact's own private maps); the rewrite appends each access's expressions
  and then reorders them ahead of it, which is what run 28's attempt lacked. A block with sized
  members before its array is `StorageOffsetOfBlock`. Tests: `storage_bindings_give_their_stride_and_offsets`
  (both routes, a helper value per view, valid modules) and the drawn
  `storage_views_at_any_first_element_match_direct3d` (run 28's patch, applied).
- The draw arguments rewrite (`DrawBaseSource::Arguments`) replaced the whole vertex helper block,
  dropping members after the bases, so a vertex shader reading a typed buffer (its view's place,
  `TexelBufferView`, is a later member) failed validation (the Direct3D 12 track's vertex-views
  case). It now renames the bases' three members and keeps the rest. New own `vs_texels`
  (texels.hlsl, fxc and dxc), test `draw_arguments_keep_the_typed_views_place`.
- `EARLY_TESTS` is built (`variant/early.rs`): the pixel program's prologue reads the read-only
  attachment's depth at `EARLY_DEPTH_BINDING` and stencil at `EARLY_STENCIL_BINDING`, compares as
  `HelperValue::EarlyTests` says (a compare code minus one is the set of passing relations: less 1,
  equal 2, greater 4; code 0 is off) against the fragment's depth, biased where the variant has
  `DEPTH_BIAS`, and discards before the program runs when the test fails. Four samples test each
  covered sample at its standard position (depth moved along its derivatives, as the clip
  distances do) and discard when none passes; per-sample shading (`EarlyTestsPerSample`), other
  counts (`EarlyTestsSamples`) and no count (`SampleCount`) are refused by name. Test
  `early_tests_read_the_attachment_before_the_program`; drawn test `tests/early_draw.rs`
  (`early.hlsl`: a first draw writes seeded depth and stencil over a large triangle, a second
  `[earlydepthstencil]` pixel shader marks each pixel it runs for in a storage buffer, under seeded
  depth and per-face stencil tests, reference and read mask; Depth32FloatStencil8 on WebGPU,
  D32_FLOAT_S8X24_UINT on Direct3D 11): equal to real Direct3D 11 in 32 of 32 scenes. Not shown:
  that the plain program differs in those scenes (expected, as WebGPU runs it before the tests).
- Shader model 5.1 DXBC (which only Direct3D 12 runs) is numbered by Direct3D 12's rule, as DXIL:
  each kind's slots in (space, register) order (`Lowering::slot`, the version read from the
  container's SHEX or SHDR chunk). Shader model 5.0 and below keep the register as the slot and
  refuse other spaces by name (`RegisterSpace`). `ps_spaces.dxbc` now converts with the same
  bindings, slots and sampled pairs as its DXIL build (test
  `register_spaces_are_numbered_by_the_programs_own_slots`).
- Not done: `DynamicOffset` (the Direct3D 12 track's gather and gather_c files): gathers with
  run-time offsets convert since run 27 (DXBC) and 28 (DXIL); what remains raising it is a load
  with a run-time offset, which DXBC encodes only as an immediate, so the next run needs the
  front's failing shader to see which instruction it is.

Next (in order):
- The fronts (theirs), now possible: `VariantFlags::STORAGE_OFFSET` where a raw or structured view's
  offset is not a multiple of `min_storage_offset_alignment` (binding from the largest multiple of
  the alignment and of `ReflectedBinding::stride` below the view), `VariantFlags::EARLY_TESTS` with
  the depth-stencil attachment read-only and its depth and stencil views bound at
  `EARLY_DEPTH_BINDING` and `EARLY_STENCIL_BINDING` (any single-sampled unsigned texture there for
  a format without stencil) and `samples` set, shader model 5.1 DXBC by (space, register) from the
  reflection as DXIL.
- Early tests on 24-bit depth: the program compares the fragment's unrounded depth with the stored
  24-bit value, where the fixed test rounds first (not yet measured; the drawn test uses 32-bit
  float depth). If it differs, round the fragment's depth to the format's step in the helper block.
- The WebGPU back end (theirs): ask wgpu for `SUBGROUP` where the adapter has it and declare
  `Features::SUBGROUPS`; then Godot's 3D platformer through the Direct3D 12 front (its 38 fragment
  pipelines stopped at the helper query before run 27 and at wave operations before run 28).
- Fronts (theirs): `texture_layers` for bgfx's multisampled arrays (G4, and G5's
  multisample_array_texture), `swizzled` for Direct3D 12's component mappings and both fronts'
  stencil views (Godot's 3D platformer), and counting `Reflection::slow_paths`.
- Direct3D 12 front (asked of G5): for DXIL its `typed_texture_slots` and `typed_key` key the
  per-slot fields by register, but DXIL slots are the view's place among the program's views of
  its kind (run 22's numbering); a program with views at u1 and u3 has slots 0 and 1. Take the
  slots from the plain conversion's reflection (`ReflectedBinding::slot()` by space and register).
- DXIL signed minimum and maximum on buffers dxil-spirv declares as unsigned words
  (`cs_atomics.dxil`, the D3D fronts' `AtomicSignedness`): a compare-exchange loop in the program
  (needs a named slow path) or the memory retyped where only signed ones use it.
- DXIL's `ConstantComponentIndexing` (one game shader): the calls of `place_calls` (as
  `nonfinite.rs` and `arrays.rs` use them) can read a scalar of a constant buffer through a helper
  taking the index, so no arena rebuild is needed.
- A drawn test of arrays of textures (`ps_array` or a compute shader picking `t[i]` per thread, against
  real Direct3D 12); implicit-derivative samples of an element picked at run time differ within a
  quad (the switch puts lanes of one quad in different cases): sample with gradients taken before
  the switch if the drawn test shows it. The DXBC route (shader model 5.1 ranges) still refuses
  arrays. Then the Direct3D 12 front's resource_arrays test and Godot rerun.
- Layer and viewport outputs of geometry shaders through the stage helper (`LayerSplit`, the
  rig's 3 game geometry shaders and Wine's layered and multiple-viewport tests), and the DXIL
  pixel shader's layer (naga's reader has no layer built-in).
- The fronts' routes (theirs, now possible): Direct3D 11's six Wine tests and atg-simplebezierpc,
  Direct3D 12's 13 tessellation files and vertex_id. Stream output from a vertex shader without a
  position has no drawn test here yet (the fronts' Wine tests will be the first).
- Tessellation, the rest: geometry shaders and stream output after a domain shader (the domain
  program through `vertex_as_compute`), DXIL hull and domain shaders (naga's SPIR-V reader: the
  tessellation execution models), the hull shader's declared largest factor (the reflection says
  64), indexed and instanced patch
  draws (built, not drawn in a test).
- Godot on Direct3D 12 (G5's real program): run 22's numbering is expected to remove
  `BindingsBeyondLimit` from its graphics shaders (not checked: Godot compiles its shaders at run
  time, so they are not in the corpus); run Godot (and bgfx 37-gpudrivenrendering, which waited on
  run 24's DXIL atomics) through G5's front after merging and read their refusals.
- Typed buffers in geometry shaders (the stage helper fills `TexelBufferView`); the corpora with
  formats (below).
- Corpus: run 19 gave the corpus test one view format per kind (Run 19); shaders whose views mix
  kinds are still refused `StorageFormatKind`/`TypedBufferFormatKind` there (10 games, 4 own, 1 rig).
  To convert them too, the corpus needs each slot's kind: record the formats the rig's real
  programs bind (an API trace) or read each declaration's type from the bytecode in the test.
- The WebAssembly build's module differs from the native one for one game shader (Known gaps):
  find which side orders the divide-modulo helper's operations by platform (dxbc-spirv's IR as
  serialised, compared between the two builds, first) and make it deterministic.
- Storage packing (`storage_packed`): open question first. The plan says only "packed into one
  buffer at offsets from the helper constants"; separate guest buffers become one GPU buffer only if
  the front keeps every storage-capable buffer in one arena (or copies), which is the fronts' memory
  design, and the binding the packed buffer takes is not named. To settle with the interface track.
- Comparisons in the program (shadow maps with a border colour, very common: WebGPU has no border
  colour): `SamplerState` needs the comparison function (an interface change: a word for it), then
  the library compares each tap. Cube filtering in the program (seamless edges). Then the rest of
  the variants (the list above), each with a drawn test as run 10's:
- `PROVOKING_LAST` (open question, not built): a pixel program sees only one value per primitive
  for a flat input, the provoking vertex's, so no rewrite of the programs alone picks the last
  vertex's. The exact routes are the front's: lists drawn with each primitive's indices rotated
  ((a, b, c) as (c, a, b), winding kept), strips and fans made lists first. If that holds, the flag
  has no use and the interface could drop it; until decided it stays refused by name.
- Decoded textures and targets written in the program, line coverage, the primitive
  index from the stage helper, root signature layouts.
- The stage helper of section 7, the rest: stream output from streams other than 0 and rasterized
  together with streaming (a draw of the pass-through as well), geometry by
  replay on WebGL 2, indexed strips into a geometry shader (a pass numbering primitives across
  restarts), triangle strips with adjacency, layer and viewport index outputs, clip distances
  through the stage, DXIL geometry shaders (naga's SPIR-V reader: the geometry execution model),
  the last vertex stage's variants on the pass-through. Tests against real Direct3D.
- The built-in clear (run 19) in the fronts: G4 and G5 dispatch `builtin::storage_clear` where the
  view's format has `STORAGE_WRITE` and no pass draws into it (they refuse such clears today), and
  clear by a copy from a filled buffer where `storage_clear_value` refuses. Its rectangles (Direct3D
  12) are boxes of the same program.
- The real programs' shaders (every converted one compiles in Chrome since run 10), what is left
  after run 24: DXBC 13 of 529 (views of mixed kinds in the corpus test 8, layers written by 3
  geometry shaders and 1 vertex shader, a multisampled array texture), DXIL 13 of 361 (views of
  mixed kinds 7, `PackedStageValues` 2, `ConstantComponentIndexing`, a multisampled array
  texture, two reader refusals).
- The cache in a front: G4 and G5 call `convert_cached` and `record_combination`; the browser host
  keeps the bytes in the origin private file system. Building ahead from `combinations` at start.
- Older items, still open:
- DXIL inputs and outputs as the DXBC route lays them out: one four-component value per register
  (now each value keeps its own width, so a Direct3D 12 vertex shader writing a float4 that a pixel
  shader reads as float2 fails WebGPU's pipeline check), and elements dxc packs into one register
  (dxil-spirv gives them a SPIR-V component; naga's reader drops it, so two values claim one
  location; refused as `PackedStageValues` until then). Both fit one wrapper, the one `draw_bases`
  already makes.
- Send the carried naga changes upstream (below).
- naga SPIR-V reader fixes for DXIL (6.2): typed buffers ("unsupported image dimension"), storage
  images without a format, `OpIsHelperInvocationEXT` (`IsHelperLane()`, wave operations), the sample mask kept as
  an array, geometry and tessellation execution models (the stage helper), and 1D texture arrays
  (the DXBC route reads them as 2D arrays; DXIL refuses `Texture1DArray`).
- dxil-spirv's crashes on signed but malformed DXIL (Known gaps): report upstream with the replay
  cases.
- The remaining `VariantKey` fields and flags of 6.4, the stage helper (section 7), the cache
  (section 8), and the corpus of real game shaders.
- GLSL ES 3.00 lacks bitfield operations, `textureGather` and multisampled samplers: what Chrome's
  WebGL 2 refuses (corpus 13, rig DXBC 6, rig DXIL 1, own 2) is only these. 6.5 gives the first two to the WebGL 2
  back end's writing; multisampled reads are absent there.

## How the Direct3D fronts call `convert`

- Geometry shaders (WebGPU, `Features::COMPUTE`): convert the pixel shader first, then the geometry
  shader with `output_interpolation` from the pixel shader's inputs (as for a vertex shader), and the
  vertex shader with every input pulled (`pulled`, as for pulled inputs) and `draw_base: Helpers`.
  `Stages::new(backend, &vertex, &geometry)` once per pair. Per draw, outside the pass:
  `stages.run(backend, &StageDraw { vertex_sets, geometry_sets, index, topology, range })`, where
  the vertex sets are its set 0 (pulled streams) and 3 (helper block: `PulledStream`s, `DrawBase`)
  and the geometry sets its set 0 (its slots) and 3 (its helper block, if any), all with
  `StageSet::COMPUTE`; an index buffer must have `BufferUsage::STORAGE` and a size that is a multiple
  of 4. Then, in the pass, draw the returned `FinalDraw`: the program of `stages.passthrough()` and
  the pixel shader, `stages.vertex_layout()`, `buffer` as stream 0, indices (u32) from `buffer` at
  `index_offset` when given, its topology, restart on, `count` vertices or indices, one instance.
  `tests/geometry_draw.rs` does exactly this.
- Hull and domain shaders (WebGPU, `Features::COMPUTE`): convert the pixel shader first, the domain
  shader with `output_interpolation` from the pixel shader's inputs, the hull shader plain, the
  vertex shader with every input pulled. `Stages::tessellated(backend, &vertex, &hull, &domain)`
  once per triple. Per draw, outside the pass: `stages.run` as for geometry, with the hull shader's
  sets (its slots in set 0, its helper block in set 3, `StageSet::COMPUTE`) as `geometry_sets` and
  topology `InputTopology::PatchList(n)` (Direct3D's `n_CONTROL_POINT_PATCHLIST`). The returned
  `FinalDraw` has `tessellated: Some(t)`: in the pass draw the domain program (`domain.modules[0]`)
  with the pixel shader, a vertex layout with no streams, the domain shader's own sets, and in set
  3 `t.patches` at `DOMAIN_PATCHES_BINDING` and `t.points` at `DOMAIN_POINTS_BINDING` (read-only
  storage, `StageSet::VERTEX`, beside the domain shader's helper block if it has one); indices u32
  from `FinalDraw::buffer` at offset 0, topology `FinalDraw::topology`, range
  `DrawRange::IndexedIndirect { args: &t.arguments, offset: 0 }`. `tests/tessellation_draw.rs`
  (`scene_ours`) does exactly this. Geometry shaders and stream output after tessellation are not
  built yet.
- `Stages::destroy(backend)` releases its programs and buffers.
- A vertex shader writing no `SV_Position` (feeding stream output, a geometry or a hull shader)
  converts: its program writes the position (0, 0, 0, -1), outside every clip volume, so drawn it
  rasterises nothing (not checked against Direct3D drawing such a shader), and its records carry that
  position. `reflection.outputs` lists no position for it.

```rust
use wcontainers_shaders::{convert, Source, ConvertError};
let converted = convert(Source::Dxbc(bytecode), &variant_key, None)?;
let module: &wcontainers_gpu::ShaderModule = &converted.modules[0];  // hand to Backend::create_shader
let reflection = &converted.reflection;
```
- `variant_key` is `wcontainers_gpu::VariantKey`; `VariantKey::default()` for the plain program. Built:
  `draw_base` (`None`, `Helpers`, `Arguments`), `output_interpolation`, `lod_bias`, `pulled`,
  `constants_as_texture`,
  `constants_as_storage`, `samples`, `linked`, `mirror_once_in_shader`, and the flags
  `CLIP_DEPTH_REWRITE`, `VIEWPORT_DEPTH_FLIP`, `VIEWPORT_ADJUST` (vertex programs; ignored on
  others), `COUNTING`, `SAMPLE_MASK`, `DUAL_SOURCE`, `DEPTH_BIAS` (pixel programs), `CLIP_DISTANCE`
  and `DEPTH_CLAMP` (both), `TEXTURE_INFO` and `min_lod_clamped`. Every other field or flag
  set, and a root signature, is refused by name (the list is under Run 10). Each helper value a variant needs is in `reflection.helpers` at its offset in the
  stage's block (`helper_size` bytes, bound at `helper_binding(stage)` in set 3), after `DrawBase` when
  both are asked: the front writes `ViewportAdjust` as (scale x, scale y, offset x, offset y) of clip
  x and y (offsets times w), `SampleMask` as a u32, `LodBias { sampler }` as an f32,
  `TextureLevels { texture }` as a u32, `DepthBias` as (constant units, slope scale, clamp, the depth
  format's step) in f32, `DrawArguments` as (word offset of the draw's arguments, 1 when indexed, draw
  number) in u32.
- `min_lod_clamped`: for each pair n of `reflection.sampled_pairs` whose texture is clamped, bind a
  sampler like the pair's own but with its level range starting at max(its own, the clamp in the
  view's numbering) at `CLAMPED_SAMPLER_BINDING` + n in set 3; write `MinLod { texture }` as (the
  clamp's f32 bits, the view's level count). A clamped texture that is compared is refused
  (`MinLodOnComparison`).
- Clip distances: a vertex program writing `SV_ClipDistance` converts to a module with
  `@builtin(clip_distances)`, which needs the back end's `Features::CLIP_DISTANCE` (wgpu's
  `CLIP_DISTANCES`; WebGL's WEBGL_clip_cull_distance); without it the front asks for the
  `CLIP_DISTANCE` variant in both programs' keys. `wcontainers_shaders::capabilities(module)` gives
  what a module needs beyond WebGPU's core.
- `CLIP_DISTANCE` and `DEPTH_CLAMP` (without `Features::DEPTH_CLAMP`): set the flag in both keys and
  the same `linked` in both: `location` one past every location of the vertex program's
  `reflection.outputs` and the pixel program's `reflection.inputs` with no `system` value (convert
  both plain first, as `linked_location` in `tests/variant_draw.rs` does), `clip_distances` the vertex
  program's count with `CLIP_DISTANCE` (zero otherwise). The pixel program needs `samples` (1 or 4)
  with `CLIP_DISTANCE`. With `DEPTH_CLAMP` write `DepthRange` as (near, far) f32 of the API's viewport;
  the raster block keeps depth clipping on (the program keeps z inside), and those draws lose early
  depth tests (the program writes depth). Check the vertex program's locations plus the added ones
  against the back end's inter-stage limit.
- `mirror_once_in_shader` (without `Features::MIRROR_ONCE`): bind the sampler with each mirror-once
  axis made `Clamp`, and write `SamplerState { sampler }` (nine words: min, mag, mip filter, the three
  address modes of the API's sampler as `AddressMode as u32`, anisotropy, then the level range as
  f32) at its offset; the member is 48 bytes (three four-word vectors).
- `pulled` (inputs past the back end's vertex buffer or attribute limits, step rates it lacks): list
  `(location, format)` per pulled input (location = the input's register). Each gets a read-only
  storage binding in set 0 (`reflection.bindings`, `added: Some(PulledStream(location))`): bind its
  stream's buffer from the stream's offset rounded down to `min_storage_offset_alignment`, to the
  buffer's end (a multiple of 4 bytes). Write `PulledStream { input }` as four u32: the offset's
  remainder plus the attribute's offset, the stride, then per vertex (0, 0), per instance with step
  rate n (n, (n - 1) * first instance), step rate 0: stride 0, divisor 1, bias 0 and the first
  instance's element added to the offset. The program reads `instance_index`, which counts from the
  first instance, so it needs no other base. Each pulled input takes a storage binding of the vertex
  stage: the front checks the total against the back end's per-stage storage limit (WebGPU's
  default is 8) and refuses past it.
- `filter_in_shader` (a texture whose view's format the back end cannot filter, sampled through a
  sampler with a linear filter: R32_FLOAT and depth read as colour on WebGPU) and
  `border_in_shader` (a sampler with a border address mode where the back end has no border colour,
  every sampler on WebGPU): bind the texture as before (it is only loaded now, so WebGPU's layout
  makes it unfilterable) and the program's real sampler at its slot; write `SamplerState { sampler }`
  as for mirror-once, `BorderColor { sampler }` (four f32), `TextureComponents { texture }` (a bit
  per component the view's format has, red first: R32_FLOAT 1, RGBA 15), `LodBias` with `lod_bias`,
  `MinLod` with `min_lod_clamped`, and with `TEXTURE_INFO` (WebGL 2) `TextureLevels`. The texture's
  pair leaves `reflection.sampled_pairs`. Comparisons, cubes, depth and integer textures are
  refused by name (Run 15).
- `constants_as_texture` (WebGL 2, past its uniform block count or size): the slot's
  `reflection.bindings` entry becomes kind `Texture` with `added: Some(ConstantsAsTexture(register))`;
  bind an RGBA32 unsigned 2D texture holding the buffer's bytes, row r at (r % 256, r / 256)
  (`CONSTANTS_TEXTURE_WIDTH`), copied when the buffer changed.
- `DEPTH_BIAS` writes the depth itself, so the front draws with the raster block's bias at zero and
  without early depth tests for those draws.
- `draw_base: Arguments` reads the bases from the indirect argument buffer, bound read-only (storage)
  at `DRAW_ARGUMENTS_BINDING` in set 3 for the vertex stage; the helper block holds where the draw's
  arguments start.
- `samples` is the targets' sample count where the program reads it (`GetRenderTargetSampleCount`);
  a program that reads it converts only with it set (`Unsupported("SampleCount")` otherwise).
- `COUNTING` programs have no side effects left (`extras.side_effects` false); `DUAL_SOURCE` needs
  WebGPU's dual-source-blending (the module is validated with naga's `DUAL_SOURCE_BLENDING` then) and
  a program writing targets 0 and 1 (`Unsupported("DualSourceWithoutTwoTargets")` otherwise).
- `constants_as_storage` moves each named constant buffer to the storage range: its
  `reflection.bindings` entry becomes kind `Storage`, the first storage binding past the program's
  own, `added: Some(ConstantsAsStorage(register))`; bind the same buffer as read-only storage.
- `SAMPLE_MASK` cannot serve WebGL 2: WebGL has no OES_sample_variables, so its GLSL cannot write
  `gl_SampleMask` (ANGLE refuses the program); a front there has no sample mask on multisampled
  targets.
- Linking a vertex and a pixel shader: convert the pixel shader first, then ask for the vertex
  shader with `output_interpolation` = every `(location, interpolation)` of the pixel shader's
  `reflection.inputs` with no `system` value whose interpolation is not `Perspective` (as
  `tests/draw.rs`'s `ours` does). WebGPU and GLSL ES require the two stages to agree; without it a
  pixel shader reading `nointerpolation`, `noperspective`, `centroid` or `sample` values fails
  pipeline creation or program linking.
- Bindings follow 6.4 for Direct3D 10 and 11: set 0 for vertex and compute shaders' slots, set 1 for
  pixel shaders' (including their UAVs), binding = `SlotKind::first_binding()` + register. Each is
  listed in `reflection.bindings` (kind, register, set, binding). Resource arrays and slots beyond a
  kind's range are refused.
- Inputs and outputs: every user value lives at location = its register, as a four-component value
  of its scalar type (so the types of a vertex and a pixel shader converted apart always match,
  whatever components each uses; their interpolation matches through `output_interpolation`). `reflection.inputs` and `outputs` give semantic, index, register, mask,
  location and component. Pixel shader output n is `@location(n)`, `SystemValue::Target(n)`.
- With `draw_base: Helpers`, a vertex shader reading `SV_VertexID` or `SV_InstanceID` reads the
  draw's bases from its helper block (set 3, `helper_binding(Vertex)`), `HelperValue::DrawBase` at
  offset 0, 16 bytes (`reflection.helpers`, `helper_size`). Without it, `SV_VertexID` is
  `vertex_index` and `SV_InstanceID` is `instance_index` as they are (right only when the bases are 0).
- `reflection.sampled_pairs` lists (texture register, sampler register) pairs in first-use order.
  `reflection.extras` for pixel shaders: early tests, side effects (UAV writes), discards, per sample.
- DXIL: `convert(Source::Dxil(bytecode), &variant_key, None)`, same result shape. Bindings are
  numbered by Direct3D 12's rule (run 22): stage set as for Direct3D 11; within it each kind's
  slots are the program's own resources of that kind in (space, register) order from the kind's
  first binding (shader resource views, buffers among them, are `Texture`, unordered access views
  `Storage`, counters `Counter` at their view's slot). Find each binding's descriptor by its
  `space` and `register`; per-slot variant fields take `ReflectedBinding::slot()`. `draw_base` `None` (bases read as zero), `Helpers` and
  `Arguments` are taken as for DXBC. Inputs and outputs are listed as for DXBC
  (semantic names from the container's signatures, location = register), but unlike DXBC each value
  keeps its own width and type (`vs_transform.dxil` writes `@location(1) vec2<f32>`), so a
  Direct3D 12 vertex shader writing a float4 that the pixel shader reads as float2 fails WebGPU's
  pipeline check: until that is fixed (first under Next), the front must link only stages whose
  shared locations have the same width and type, comparing `reflection.outputs` with the pixel
  shader's `reflection.inputs` (mask and type), and answer a mismatch as the API's refusal. A
  pixel shader's unread `SV_Position` may be listed where the DXBC route leaves it out. Sampled
  pairs are listed as for DXBC, by register (a pair's register spaces are not in the pair).
- DXIL-only refusals: `NonFiniteConstant`, `ConstantComponentIndexing`, `FloatToIntWidth`,
  `PackedStageValues`, `PrimitiveId`, `VertexWithoutPosition`, `Texture1DArray`, and `Reader(..)`.
- On WebGL 2, the back end's GLSL writing refuses programs using fine derivatives
  (`MissingFeatures(DERIVATIVE_CONTROL)`), as GLSL ES 3.00 has none; a front on WebGL 2 reports a
  feature level without them.
- Constant buffers are declared with the size the bytecode gives them, on both routes (DXIL's
  dynamically indexed ones since run 6), so binding the program's own buffer (or any range at least
  that long) satisfies WebGPU.
- Errors: `Rejected` (the container's checksum does not match, which Direct3D answers with
  `E_INVALIDARG` too, or dxbc-spirv or dxil-spirv refused the bytecode, with its message, including
  their failed assertions and exceptions natively), `Unsupported(name)` (a named feature or variant
  not provided yet, for example `GeometryShader`, `StorageImage`, `TypedBuffer`, `Atomics`,
  `RegisterSpace`, `PackedStageValues`), `Reader(text)` (a gap of naga's SPIR-V reader, DXIL
  only), `Malformed` and `Invalid` (our lowering's own checks or naga's validation refused the
  converter's output, a panic in the lowering included; on real bytecode these are our bugs, and
  the corpus tests fail on any of them). A front answers each with the API's own refusal
  (`E_INVALIDARG` at shader creation).
- On WebAssembly a converter failure that is not an error return (dxil-spirv's crashes on signed but
  malformed DXIL, see Known gaps) traps the shaders module; the host reports it as a failed
  conversion.

## Measurements

Run 29, 2026-11-18, counts only (laptop load about 0.2 to 5.6; desktop shared through the queue,
whose hazard slot other tracks' long runs held for most of the run), logs in scratch
`graphics-shaders/run29`:
- Laptop: the full suite of both crates with the corpora (`--include-ignored --skip mutated
  --no-fail-fast`, suite2.log): everything passes except the own-shader records, which lacked the
  four new files of each route; the records were copied from `classes.txt` and the corpus test then
  passed 9 of 9 (corpus3.log). `ps_spaces.dxbc` is now `ok`; no game, rig or Wine corpus verdict
  changed with shader model 5.1 numbering. Variant tests 25 of 25, the interface's 36 of 36, clippy
  of both crates clean (suite2.log, variants4.log).
- Recompiling every own shader on the desktop (`compile.cmd`) gave the committed bytes; new are
  `vs_texels`, `vs_early`, `ps_early` and `ps_mark` (fxc and dxc).
- Desktop (one GPU job each, RTX 3080): `early_draw` 32 seeded scenes (early3.log): in 21 Direct3D
  ran the shader for 44 to 2017 of 4096 pixels, in 11 for none; in every scene the marks and the
  target's texels of the fxc and the dxc build through WebGPU equal real Direct3D 11's (0 apart).
  The first 16 scenes of the first version of the test (small triangles, 11 of them empty) were also
  all equal (gpu3.log). `compute_draw` 5 of 5 (gpu3.log), among them
  `storage_views_at_any_first_element_match_direct3d` (8 seeds, every view of `cs_atomics`,
  `cs_shared` and `cs_decrement` at a seeded first element, 0 words apart by both routes).
- WebAssembly (laptop, wasmtime; wasm1.log): the corpus test passes 9 of 9 under wasmtime, and the
  written files equal the native ones except the known gap (`116_cs_group_index_code` WGSL,
  `d599fbc7efa911be` WGSL and GLSL, and the two `cache.txt` holding them).
- Not rerun this run: Chrome's compile of the written files.

Run 28, 2026-11-18, counts only (laptop load about 1 to 5; desktop shared through the queue), logs in
scratch `graphics-shaders/run28`:
- Desktop (one GPU job each): `draw` 4 of 4 (gpu6.log): `ps_wave` through WebGPU on wgpu's Vulkan 0 of
  4096 texels apart from real Direct3D 12 (2074 texels drawn), `ps_gatheroff`'s dxc build 0 of 4096
  apart from real Direct3D 12 (draw1.log, gpu4.log), the other cases as before; `compute_draw` 4 of 4,
  24 comparisons 0 failed (gpu4.log); the Windows clippy of the crate clean (gpu4.log, gpu6.log).
- Laptop: clippy of both crates clean and the full suite of both crates with the corpora
  (`--include-ignored --skip mutated`; suite7.log): the records of own-dxil and games-dxil were
  copied from `classes.txt` after the deliberate changes (ps_gatheroff.dxil and ps_wave.dxil
  convert; Godot: the fragment shader with wave operations and two compute shaders convert, two
  compute shaders move from the reader to `ResourceArray`).
- Recompiling every own shader on the desktop (`compile.cmd`) gave the committed bytes; only
  `ps_wave.dxil` is new.
- Not rerun this run: the WebAssembly comparison and Chrome's compile of the written files.
- At the end of run 28 the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders` (5.7 GB)
  and the desktop's `C:\wc\target\phase-2-graphics-shaders` were deleted.

Run 27, 2026-11-18, counts only (laptop load about 1; desktop shared through the queue), logs in
scratch `graphics-shaders/run27`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; the full suite of both crates with the corpora (`--include-ignored --skip mutated`): every
  test passes but the own-shaders record of `ps_gatheroff.dxil`, which the reader change made
  `spirv-reader unsupported` and which was then recorded (suite3.log; the record copied from
  `classes.txt`).
- Desktop (one GPU job each): `tessellation_draw` 3 of 3 on wgpu's Direct3D 12, 512 comparisons
  (tess1.log); `geometry_draw` 2 of 2, 52 and 36 comparisons (geom1.log); `draw` 3 of 3 with the
  new cases (draw3.log), and the Windows clippy of the crate clean (draw4.log, which also holds the
  DXIL gather failure that led to the reader change).
- Not rerun this run: the WebAssembly comparison and Chrome's compile of the written files.
- At the end of run 27 the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders` and the
  desktop's `C:\wc\target\phase-2-graphics-shaders` and `C:\wc\probe` were deleted.

Run 26, 2026-11-18, counts only (desktop shared through the queue; the laptop unreachable), logs in
scratch `graphics-shaders/run26`:
- Desktop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu --no-fail-fast` (every test not
  ignored, GPU ones included, default seeds) passes (gpu4.log): `compute_draw` 24 comparisons 0
  failed (`cs_atomics` with its exchange loops, both builds, 0 words apart at seeds 1 to 4),
  `variant_draw` 48 comparisons 0 failed (`swizzled` both builds and back ends 0 of 4096 texels),
  the multisampled array 0 texels apart on both builds, filtering 396 comparisons with the 32 known
  anisotropic failures. `cargo check -p wcontainers-direct3d-11 -p wcontainers-direct3d-12
  --all-targets` builds (fronts.log). Not rerun: the corpus, games and WebAssembly runs and Chrome's
  compile (they need the laptop).

Run 25, 2026-11-18, counts only (laptop load 1 to 4; desktop shared through the queue), logs in
scratch `graphics-shaders/run25`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu -- --include-ignored --skip mutated`
  with the corpus, the games (Godot's added) and glslang: every test passes (corpus test 9 of 9,
  208 s; full.log).
- Desktop: `cargo clippy -p wcontainers-shaders --all-targets -- -D warnings` clean, and `cargo
  check -p wcontainers-direct3d-11 -p wcontainers-direct3d-12 --all-targets` builds (fronts.log).
  `compute_draw` 4 of 4, `draw` 3 of 3, `variant_draw` 3 of 3 at seeds 1 to 4 (gpu2.log, after the
  raw buffer change): every comparison within its allowance, as in run 24; DXIL builds new to the
  drawn tests: `cs_texels` 0 elements apart in 4 seeds, `ps_table` 0 of 4096 texels on both back ends.
- Chrome 157.0.8103.78 on the RTX 3080 (chrome.log): every WGSL file compiles (corpus 181, games
  516, games DXIL 406 with Godot's 58, own 89, own DXIL 33, rig 76, rig DXIL 53); the GLSL ES
  failures are the known classes only (bitfield operations, `textureGather`, multisampled samplers:
  corpus 13, games 10, games DXIL 1, own 5, own DXIL 2, rig 2, rig DXIL 2).
- WebAssembly (laptop, `wasm-run10.sh`, wasm.log): the corpus test under wasmtime 9 of 9 (331 s),
  every verdict as native; written files equal to the native ones except corpus 2 and games 3, the
  counts of the known gap of run 24 (files not listed again by name).
- At the end of run 25 the build folders were deleted (laptop
  `~/wcontainers-graphics/target/phase-2-graphics-shaders`, 7.1 GB; desktop
  `C:\wc\target\phase-2-graphics-shaders`, 6.1 GB); `out` and the logs are kept.

Run 24, 2026-11-18, counts only (laptop load 2 to 9; desktop shared through the queue), logs in
scratch `graphics-shaders/run24`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu -- --include-ignored --skip mutated`
  with the corpus, the games and glslang: every test passes (corpus test 9 of 9, 232 s; full.log).
- Desktop: `cargo clippy -p wcontainers-shaders --all-targets -- -D warnings` clean;
  `compute_draw` 4 of 4 at seeds 1 to 4 (draw3.log): atomics and counters 20 comparisons (fxc and
  dxc builds of `cs_atomics`, `cs_shared`, `cs_decrement`; `cs_atomics.dxil` the known refusal),
  0 words apart; storage textures 4 seeds both builds 0 apart; typed buffers 4 seeds 0 elements
  apart; storage textures through buffers, both builds, 4 seeds, 0 texels apart in every view
  (R32Uint with atomics, R32Float read and written, the seeded array and volume formats).
  `variant_draw` 3 of 3 (draw2.log): the layer and viewport test 0 texels apart in all four layers;
  the run's other variant comparisons 48, 0 failed. `cargo check -p wcontainers-direct3d-11 -p
  wcontainers-direct3d-12 --all-targets` builds (fronts.log).
- WebAssembly (laptop, `wasm-run10.sh`, wasm.log): the corpus test under wasmtime 9 of 9 (330 s),
  every verdict as native; written files equal to the native ones except the known gap's two
  shaders (Known gaps: `116_cs_group_index_code` WGSL, `d599fbc7efa911be` WGSL and GLSL, and their
  lines in each `cache.txt`).
- Chrome 157.0.8103.78 on the RTX 3080 (chrome.log): every WGSL file compiles (corpus 181, games
  516, games DXIL 348, own 89, own DXIL 30, rig 76, rig DXIL 53); the GLSL ES failures are the known
  classes only (bitfield operations, `textureGather`, multisampled samplers: corpus 13, games 10,
  games DXIL 1, own 5, own DXIL 2, rig 2, rig DXIL 2).

Run 23, 2026-11-18, counts only (laptop load 3 to 5; desktop shared through the queue), logs in
scratch `graphics-shaders/run23`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -- --include-ignored --skip mutated` with the corpus,
  the games and glslang: every test passes (corpus test 9 of 9, 278 s), verdicts as recorded
  (test3.log).
- Desktop: `cargo clippy -p wcontainers-shaders --all-targets -- -D warnings` clean;
  `tessellation_draw` 3 of 3 (tess6.log; the tessellator at the default 4 seeds, 128
  comparisons), `geometry_draw` 2 of 2 at 4 seeds (geom2.log); `cargo check -p
  wcontainers-direct3d-11 -p wcontainers-direct3d-12 --all-targets` builds (fronts.log).
- Chrome 157 on the RTX 3080 (chrome.log): every WGSL file compiles (corpus 177, games 469, games
  DXIL 346, own 87, own DXIL 28, rig 75, rig DXIL 52), the 10 newly written programs as GLSL ES
  too; the GLSL ES failures are the known classes only (bitfield operations, `textureGather`,
  multisampled samplers: corpus 13, games 10, games DXIL 1, own 5, own DXIL 2, rig 2, rig DXIL 2).

Run 22, 2026-11-18, counts only (laptop load 1 to 3; desktop shared through the queue), logs in
scratch `graphics-shaders/run22`:
- Laptop, `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu -- --include-ignored --skip mutated`
  with every corpus: all pass once the records took the new verdicts (games-dxil: 10 from
  `unsupported RegisterSpace` to `ok` 9 and `ok-wgsl glsl-write` 1; own-dxil: `ps_spaces.dxil` ok,
  `ps_twospaces.dxil` ok; own: `ps_twospaces.dxbc` ok).
- Chrome 157 on the RTX 3080 (`web/check`, one GPU job): WebGPU compiles every written WGSL file
  (corpus 171, games 469, games-dxil 346, own 86, own-dxil 28, rig 74, rig-dxil 51); WebGL 2
  fails only on bitfield operations, `textureGather` and multisampled samplers as before (corpus
  13, games 10, games-dxil 1, own 5, own-dxil 2, rig 2, rig-dxil 2).
- Desktop, `draw`, `compute_draw` and `variant_draw` (one GPU job, all pass): `ps_twospaces` 0 of
  4096 texels differ on WebGPU and WebGL 2, DXIL against real Direct3D 12 and DXBC against real
  Direct3D 11; every earlier case as before.

Run 21, 2026-11-17, counts only (desktop shared with the other tracks through the queue; laptop
load 1 to 4), logs in scratch `graphics-shaders/run21`:
- `storage_textures_match_direct3d`, seeds 1 to 4 (draw2.log): fxc's DXBC and dxc's DXIL of
  `cs_images` through the WebGPU back end, 32 textures, every one equal word for word to real
  Direct3D 11's on the RTX 3080.
- `typed_buffers_draw_as_direct3d_does`, seeds 1 to 8 (draw2.log): 48 comparisons (DXBC and DXIL,
  each through storage on WebGPU and a texture on WebGPU and WebGL 2), all equal texel for texel.
- Corpora (laptop, corpus3.log, suite1.log): games DXIL 361: 288 ok, 47 ok as WGSL only
  (compute), 26 named refusals; own DXIL 44, rig DXIL 72 (2 more convert than run 20). The whole
  suite passed on the laptop (corpus 9 tests, variants 17, cache 6, the rest).
- Chrome 157 (chrome1.log, one GPU job): every written WGSL compiles (games DXIL 336, own DXIL 26,
  rig DXIL 51); GLSL ES fails only where it did (multisampled samplers 2, `textureGather` 3).

Run 20, 2026-11-17, counts only (desktop shared with the other tracks; laptop load 2 to 6):
- `tessellator_matches_direct3d` (`run20/tess5.log` in scratch `graphics-shaders`), seeds 1 to 16,
  32 topologies each: 512 draws of 8 patches, every patch's primitives equal the RTX 3080's
  (Vulkan through wgpu for ours), about 12 s of GPU job.
- The laptop suite (`run20/suite4.log`): 57 tests pass, none fail (the fuzz test skipped as in How
  to run); the corpora's records changed only for the hull and domain shaders (9 corpus, 9 rig, 35
  own) and `vs_patch` (`VertexWithoutPosition`).
- Chrome 157 on the desktop (`run20/chrome2.log`): every new WGSL file compiles with WebGPU: corpus
  9, own 35, rig 10, and `tessellator.wgsl` and `stream_out.wgsl` as written. A first attempt found
  no WebGPU adapter in Chrome; the retry a few minutes later did.
Run 19, 2026-11-17, counts only (the desktop shared with the other tracks, no timings taken):
- `storage_clears_match_direct3d` (`run19-clear4.log` in scratch `graphics-shaders`), seeds 1 to
  256: every case passes; all 40 storage formats on both (WebGPU on wgpu's Direct3D 12 has
  `STORAGE_WRITE` for each, the device a typed storage view of each); 62 float clears of integer
  formats refused `FloatClearOfInteger`, each left unchanged by Direct3D 11 too; 74 cases where the
  GPU's Direct3D 11 differs from WARP (integer clears saturated or read as floats, float clears
  rounded down at half a code), ours equal to WARP's integer clears and to the GPU's float clears.
  The first runs (`run19-clear1.log` to `-3.log`) found the denormal flush (R32_FLOAT, 1e-40 cleared
  as 0 by both) and the two references' disagreement.
- The corpora with view formats (laptop, `run19-corpus1.log`, `-2.log`, records updated): of the
  shaders refused `StorageFormat` or `TypedBuffer` before, games 51 of 60 now convert (9 refused
  `StorageFormatKind` 1 and `TypedBufferFormatKind` 8: they declare views of more than one kind,
  which one format for every slot cannot give; a front gives each slot its view's format), the
  Wine corpus 19 of 21 (2 refused `StorageImageAtomics`: atomics on a storage texture without
  `texels_through_buffer`), the rig 1 of 2, own shaders 0 of 4 (all four mix kinds). Compute shaders
  among them are `ok-wgsl glsl-write` (WebGL 2 has no compute), so their class does not show the
  views; `verdicts.txt` does.
- Chrome on the desktop (`run19-chrome.log`, one GPU job, RTX 3080): every written WGSL file
  compiles with WebGPU: corpus 162, games 469, games DXIL 289, own 50, own DXIL 26, rig 65, rig
  DXIL 49. WebGL 2 refuses 35 GLSL files (corpus 13, games 10, games DXIL 1, own 5, own DXIL 2,
  rig 2, rig DXIL 2), each for `bitfieldExtract`/`bitfieldInsert`/`bitfieldReverse`/`findMSB`,
  `textureGather` or `sampler2DMS`, which GLSL ES 3.00 lacks (6.5 gives the first two to the WebGL
  2 back end's writing); `web/check` exits 1 on them as before.
- The whole suite on the laptop before the push (`run19-suite2.log`, then the corpus test again
  after the records): every test passes, the corpora with glslang included; clippy with warnings
  denied passes on the laptop and on Windows.

Run 18, 2026-11-17, counts only (the desktop shared with the other tracks, no timings taken):
- `typed_buffers_match_direct3d`: seeds 1 to 64 (`texels-draw-2.txt`, `-3.txt`, `-4.txt` in scratch
  `graphics-shaders`), every result word and every written byte equal to real Direct3D 11 on the
  RTX 3080, every u1 atomic word equal; every loadable format at t0 to t3 and 20 stored formats at
  u0 drawn (half, 11- and 10-bit float stores exact, NaN and infinities included). Normalised stores:
  the RTX 3080 gives the code below where ours (the specification's formula) gives the one above, at
  fractions from 0.514 to 0.548 of a code (6 components in seed 4, 13 in seed 7, 0 to 9 elsewhere);
  the specification permits 0.6, so the test holds both to it (Departures). The first run found that
  past the length Direct3D gives the defaults where the format lacks components (fixed).
- `storage_textures_through_buffers_match_direct3d`: seeds 1 to 16 (`through-2.txt`), every texel
  of all four views equal (formats drawn: R16, R32, RG16, RG32, RGBA16 and RGBA32 floats; R8, RG8,
  RGBA8, RG16, RGBA16, R32 and RG32 unsigned).
- The whole drawn suite after it (`run18-gpu-2.txt`): `compute_draw` 4 tests (4 seeds), `draw` 3,
  `filter_draw` (2 seeds, 108 comparisons, 0 failed), `geometry_draw` 2 (4 seeds), `pull_draw`,
  `variant_draw` 2 (8 typed buffer seeds) pass. The laptop suite with every corpus
  (`run18-tests-2.txt`) passes.
- `typed_buffers_draw_as_direct3d_does`: seeds 1 to 40 (`texels-pixel-1.txt`, `run18-gpu-1.txt`),
  120 comparisons (storage on WebGPU, texture on WebGPU and WebGL 2), every texel equal.
- After the harness change (`run18-gpu-1.txt`): `draw` 3 tests, `compute_draw` 3, `geometry_draw` 2
  (4 seeds), `variant_draw` 2 pass.

Run 17, 2026-11-17, counts only (the desktop shared with the other tracks, no timings taken):
- `tests/geometry_draw.rs` (scratch `run17/gd3.log`), seeds 1 to 8: the geometry pictures 104
  comparisons, 0 texels differ; stream output 64 comparisons, every byte of both buffers and both
  statistics equal to real Direct3D 11's. Before `mad` became `fma` (`run17/gd2.log`): 8 of 32
  stream output comparisons had one or two words 1 ulp apart (all `gs_cut`, the lerp).
- After the `fma` change (`run17/draws1.log`): `draw` 3 tests pass; `variant_draw` passes;
  `filter_draw` seeds 1 to 8, 396 comparisons, 0 failed, 32 anisotropic known failures as before.
- Chrome's compile of every written file after run 17's changes (`run17/chrome.log`, one GPU job):
  every WGSL file compiles (corpus 143, games 418, games DXIL 289, rig 64, rig DXIL 49, own 50, own
  DXIL 26); WebGL 2 refuses only what GLSL ES 3.00 lacks (bitfield operations, `textureGather`,
  multisampled samplers): corpus 13, games 10, games DXIL 1, rig 2, rig DXIL 2, own 5, own DXIL 2.
  Geometry, atomics and counters convert to WGSL only (`ok-wgsl glsl-write`: compute).
- Corpus records after run 17 (laptop, `run17/linux-all3.log`): 56 storage textures now refused
  `StorageFormat` (no format under the default key) where they were `StorageImage`; 10 geometry
  shaders and 13 shaders with atomics or counters now convert; 2 refused `AtomicSignedness`; 3 game
  geometry shaders now refused `RenderTargetArrayIndex`.
- WebAssembly against native after run 17 (laptop, `run17/wasm.log`, scratch script
  `wasm-run10.sh`): the corpus test passes under wasmtime (9 tests); written files equal byte for
  byte in corpus, rig, rig DXIL, own, own DXIL and games DXIL; in games 3 differ, all from one shader
  (`d599fbc7efa911be.dxbc`: its WGSL, its GLSL and its line in `cache.txt`, Known gaps).
- The whole suite before the push (`run17/final-gpu.log`, one GPU job, about 100 s): `draw`,
  `variant_draw`, `filter_draw` (396 comparisons, 32 anisotropic known failures), `pull_draw`,
  `geometry_draw` (52 and 32 comparisons) and `compute_draw` (8 and 4 seeds) all pass; on the laptop
  every test passes with the corpora and glslang (`run17/linux-all3.log`, then the corpus test
  again after the records).
- `tests/compute_draw.rs` (`run17/cd2.log`), seeds 1 to 4: atomics and counters 8 comparisons, 0
  words apart, counters equal (256, 256, 0; 0); storage textures 16 comparisons (4 formats), 0 words
  apart.

Run 16, 2026-11-17, counts only:
- `tests/geometry_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 2 s,
  scratch `run16/geometry7.log`): seeds 1 to 4, thirteen cases each, WebGPU (wgpu's Direct3D 12
  path) against real Direct3D 11: 52 comparisons, 0 of 4096 texels differ in every one (allowance
  0); Direct3D draws 59 to 1468 texels per picture. Before the test gave the vertex stage the draw's
  bases (`run16/geometry5.log`), the cases with a first vertex, base vertex or first instance
  differed, as Direct3D's vertex and instance numbers leave the bases out.
- Laptop (i7-8750H, load average 0 to 0.6): the suite without the ignored corpora passes after
  recording the new own and rig verdicts; clippy with warnings denied passes. Windows clippy with
  warnings denied passes.

Run 15, 2026-11-17, counts only:
- `tests/filter_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 92 s,
  scratch `run15/desktop-final.log`), seeds 1 to 8, each by the DXBC and the DXIL route on WebGPU
  (wgpu's Direct3D 12 path) and WebGL 2 (ANGLE), R32_FLOAT and RGBA8, six sampling modes: 396
  comparisons, 0 failed, 32 known failures (all of seed 8, the one anisotropic seed: gather and
  explicit levels pass there). Of the 336 isotropic pictures: no texel equals neither Direct3D; in
  24 pictures 48 texels equalled neither until the level of detail moved by 1/256; in 8 pictures one
  texel each (one place, seed 3's gradients) is a third answer where the GPU and WARP differ. The
  GPU and WARP themselves differ in 68 of the 84 isotropic pictures, by up to 3758 texels (largest
  0.67). Against WebGPU's own sampler (12 RGBA8 comparisons): 0 texels apart from both it and WARP;
  the own sampler against the GPU's Direct3D 11: 0 texels apart in all 12 (allowance 1/128 per
  channel throughout). Seeds 1 to 16 (`run15/filter12.log`): seeds 1 to 13 pass; seed 14
  (anisotropy 4) lost ANGLE's device (DXGI_ERROR_DEVICE_REMOVED, Known gaps).
- Before the fixes (scratch `run15/filter3.log` to `filter11.log`): mip-point selection at an exact
  .5 rounded down (the GPU rounds up); DXIL on WebGL 2 read level count 0 (the helper packing bug);
  R32_FLOAT borders gave all four border components (Direct3D gives (r, 0, 0, 1)); a quad whose two
  triangles were not one plane put a seam of differing derivatives across the picture (the test's
  own geometry, fixed). Rounding the level of detail to 8 bits, or point coordinates to 8 subtexel
  bits, made more texels differ, so neither is done.
- `tests/variant_draw.rs` and `tests/draw.rs` on the desktop after the helper packing change: both
  pass (`run15/desktop-final.log`). Windows clippy with warnings denied passes.
- Chrome 157 (`web/check`, one GPU job, about 3 s, `run15/chrome.log`): the filter variants of
  `ps_filter` by both routes (by texture with a bias; by sampler with a resource clamp and
  `TEXTURE_INFO`) and of the rig's 2D array, 3D and gather shaders, written by a test kept out of
  the repository: WGSL 7 of 7 compile in Chrome's WebGPU, GLSL ES 2 of 2 in its WebGL 2.
- Laptop (i7-8750H, load average 0.1 to 0.7): the whole suite with every corpus (`--include-ignored
  --skip mutated`, the plan's corpus and the real programs' shaders) passes after recording the new
  own verdicts (`vs_filter` converts; `ps_filter`'s plain GLSL ES has `textureGather`) and four rig
  shaders that now compile as GLSL ES (the rig track recompiled its shaders at lower shader models,
  so fxc no longer writes bitfield operations in them); every other verdict and cache record as
  recorded (corpus test 179 s; `run15/full-suite2.log`). Clippy with warnings denied passes. Not
  re-run: the WebAssembly comparison (plain conversions did not change).

Run 14, 2026-11-17, counts only:
- `tests/pull_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 2.5 s,
  scratch `run14/pull5.log`): seeds 1 to 50, by the DXBC and the DXIL route on WebGPU (wgpu's
  Direct3D 12 path): 0 of 4096 texels differ from real Direct3D 11 in all 100 comparisons
  (allowance 0, the texels are the inputs' bits). Before the exact conversions (seed 1,
  `run14/pull3.log`): 635 of 4096 differed, normalised values by one step (Direct3D's are correctly
  rounded) and half-float NaNs (payload lost). Windows clippy with warnings denied passes.
- Laptop (i7-8750H, load average 0 to 0.4): the suite without the ignored corpora passes after
  recording the new own verdicts (`vs_pull`, `ps_pull`: convert, by both routes); clippy with warnings
  denied passes.
- `tests/variant_draw.rs` on the desktop (one GPU job, about 3.7 s, `run14/draw1.log`): 104
  comparisons, all within their allowances; new: constants through a texture in both stages
  (`vs_transform` with `ps_textured`, and with `ps_rows`, whose row is picked by the pixel's column)
  against the plain programs' uniform blocks, on WebGL 2 and WebGPU, by both routes: 0 of 4096 texels
  differ in all 8 (allowance 0). Every earlier comparison as in run 13. Windows clippy with warnings
  denied passes.
- `tests/draw.rs` on the changed shared harness: all 3 tests pass (`run14/draw-plain.log`).
- Laptop (i7-8750H, load average 0 to 0.4), end of run: the whole suite with every corpus
  (`--include-ignored --skip mutated`, the plan's corpus and the real programs' shaders) passes, every
  verdict and cache record as recorded (corpus test 175 s; `run14/full-suite.log`); clippy with
  warnings denied passes. Not re-run: Chrome's compile and the WebAssembly comparison (the plain
  conversions did not change; only the new variants' paths did).

Run 13, 2026-11-17, counts only:
- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 2.3 s,
  scratch `run13/draw8.log`; 96 comparisons, all within their allowances), each by the DXBC and the
  DXIL route: clip distances in the program (`vs_clip`, `ps_textured`) against real Direct3D 11's
  native clipping: WebGPU (wgpu's Direct3D 12 path, which has no clip distances) and WebGL 2, 11 of
  4096 texels differ, by at most 2 (allowance 2 in any number: the same pixels are drawn, an attribute
  over the unclipped triangle rounds apart; a pixel drawn on one side only would differ by far more);
  four samples on WebGPU against the native distances on wgpu's Vulkan path, resolved: 12 differ by
  at most 2. Depth clamping (`vs_deep`, depth range 0.25 to 0.75) against Direct3D 11 with
  DepthClipEnable false, depth as f32 steps: WebGPU 648 of 4096 within 2 steps, WebGL 2 570 within 2
  (allowance 2); clipping changes 791 of Direct3D's texels. Mirror-once in u and repeat in v on a
  4-level texture (`ps_mirror`) against Direct3D 11's MIRROR_ONCE: WebGPU and WebGL 2, 5 of 4096
  differ by 1 (allowance 1: explicit gradients' level of detail rounds apart); the mode changes 1693
  of Direct3D's texels against clamping. Every earlier comparison of the file as before.
- During the run the desktop's shared ANGLE (`C:\wc\tools\angle-mozangle-0.7.1`, rebuilt by another
  track at 16:30) crashed in libGLESv2.dll at offset 0x1f83f5 on making a context, as did a copy
  built here from the same recipe (`C:\wc\tools\angle-shaders-track`), the WebGL back end's own
  suite too, before and after today's merge of that back end. The new comparisons were first drawn
  on WebGPU alone with the WebGL draws taken out of an uncommitted copy of the test
  (`run13/draw7.log`, the same numbers as above); after the next rebuild of the shared ANGLE (16:59)
  the whole test ran and passed as committed (`run13/draw8.log`).
- Laptop (i7-8750H, load average 0 to 1): the suite without the ignored corpora passes after
  recording the new own verdicts (`vs_deep`, `ps_mirror`: convert), clippy with warnings denied
  passes; Windows clippy with warnings denied passes. At the end of the run the whole suite with
  every corpus (`--include-ignored --skip mutated`, the plan's corpus and the real programs'
  shaders) passes, every verdict and cache record as recorded (corpus test 179 s;
  `run13/full-suite.log`). Not re-run: Chrome's compile and the WebAssembly comparison (the
  plain conversions did not change).

Run 12, 2026-11-17, counts only:
- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, scratch
  `run12/draw2.log`), each by the DXBC and the DXIL route, against Direct3D 11's resource clamp:
  `ps_minlod` with clamps 0, 1.5, 3 and 3.5 on WebGPU and WebGL 2, and `ps_mingather` with clamps
  0.5, 1 and 3.5 on WebGPU: 0 of 4096 texels differ in every case (allowance 1). Each clamp past the
  first changes Direct3D's picture (1.5: 2162 texels, 3: 2530, 3.5: 2442; gather 1 and 3.5: 2071).
  Every earlier comparison of the file as in run 11. Windows clippy with warnings denied passes.
- Clip distances natively (`run12/draw4.log`): `vs_clip` with `ps_textured` on wgpu's Vulkan path,
  0 of 4096 texels differ from Direct3D 11 by both routes (allowance 0); the planes change 1071 of
  Direct3D's texels. Every other comparison of the file as before (68 exact comparisons in the log).
- Chrome 157 (`web/check`, one GPU job, about 3 s, own corpora only, `run12/chrome2.log`): WGSL own
  33 of 33, own DXIL 20 of 20 (`vs_clip` with `enable clip_distances` included); GLSL ES own 26 of
  30, own DXIL 17 of 18: `vs_clip` compiles after the naga change (before it, both builds failed on
  the extension's name), `ps_minlod` compiles, the failures are bitfield operations and
  `textureGather` (`ps_mingather`). The other corpora were not re-run: their verdicts did not
  change.
- Laptop (i7-8750H, load average 0.4 to 3): the suite with every corpus (`--skip mutated`) passes
  after recording the new own verdicts (`vs_clip` converts; glslang 8.13 knows neither clip
  distance extension, so its GLSL ES verdict is a glslang failure), every other verdict unchanged,
  so no corpus or real program shader used clip distances. Earlier in the run, after recording the four
  new own verdicts (`ps_minlod` converts and compiles as GLSL ES; `ps_mingather`'s GLSL ES has
  `textureGather`, which ES 3.00 lacks); every other verdict unchanged. Clippy with warnings denied
  passes.

Run 11, 2026-11-17, counts only:
- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, scratch
  `run11/draw4.log`), each by the DXBC and the DXIL route: depth bias against Direct3D 11's
  D24_UNORM_S8_UINT buffer, in 24-bit codes, on WebGPU and WebGL 2: (2000 units, slope 0.25, no
  clamp) 355 of 4096 texels differ by 1 code, (clamp 0.0002) 471 by 1, (-2000, -0.25, clamp -0.0002)
  478 by 1, never more than 1 (allowance 1); the clamp changes 1610 of Direct3D's texels. Draw bases
  against Direct3D 11, 0 of 4096 differ in every case: vertices from 48 and indexed (first index 6,
  base vertex 48), two instances from 1 or 2, directly on WebGPU and WebGL 2 and indirectly (the
  arguments 4 words into their buffer) on WebGPU. Every run 10 comparison as in run 10. Windows clippy
  with warnings denied passes. `tests/draw.rs` on the changed shared harness: all 3 tests pass
  (`run11/draw-plain.log`).
- Laptop (i7-8750H, load average 0.5 to 4.2): the whole suite with every corpus passes (`--skip
  mutated`; corpus test 214 s), every verdict as recorded; own DXBC and DXIL each two more
  (`vs_ids`, `ps_ids`, converting). Clippy with warnings denied passes.
- Not re-run this run: Chrome's compile of the written files and the WebAssembly comparison (the
  default conversions did not change; only the new variants' paths did).

- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, scratch
  `run10/draw8.log`), each by the DXBC and the DXIL route: equal texel for texel (or, for depth, in
  f32 steps) to the stated reference: viewport adjustment 0 of 4096 on WebGPU and WebGL 2; clip
  depth on WebGL 2 with and without EXT_clip_control and WebGPU natively, 0 steps from Direct3D 11's
  depth buffer; swapped depth range on WebGPU 843 of 4096 within 3 steps of the definition (allowance
  4); sample mask 0 on WebGPU; constants as storage 0; level-of-detail bias 2.5 on a 4-level texture 0
  on WebGPU and WebGL 2 against Direct3D 11; counting 1610 of 1610 samples Direct3D 11's query
  counts, on both back ends; level counts 0 on WebGPU and WebGL 2 (WebGPU's own query 0 against
  Direct3D 11); sample count (DXBC) 0 on both; dual-source blending 0 on WebGPU against Direct3D 11.
  `tests/draw.rs` after its move onto the shared harness: all 3 tests pass, every comparison equal as
  in run 9. Windows clippy with warnings denied passes.
- Laptop (i7-8750H, load average 0.3 to 4): the whole suite with every corpus passes (`--skip
  mutated`), clippy with warnings denied passes. Verdicts: corpus 127 of 191 convert (unchanged), rig
  56 of 79, rig DXIL 49 of 72 (unchanged), own 28 of 30 (`ps_samples` needs `samples`, `ps_spaces`),
  own DXIL 15 of 20; the real programs' shaders: DXBC 418 of 529 convert (named: render target array
  index 47, storage images 39, typed buffers 21, geometry shaders 3, multisampled arrays 1), DXIL 289
  of 361 (reader: storage images without a format 38, typed buffers 19, a load operand 1, viewport
  index from a vertex stage 1; register spaces 10, packed stage values 2, multisampled arrays 1).
  Cache records written and read back equal: corpus 254, rig 112, rig DXIL 98, own 56, own DXIL 30,
  games 836, games DXIL 578.
- Chrome 157 (`web/check`, one GPU job, about 9 s, `run10/chrome.log`): WGSL compiles for every
  converted shader: corpus 127, rig 56, rig DXIL 49, own 28, own DXIL 15, games 418, games DXIL 289.
  GLSL ES: corpus 89 of 102, rig 47 of 53, rig DXIL 46 of 48, own 22 of 25, own DXIL 13 of 13, games
  406 of 416, games DXIL 288 of 289; every failure a bitfield operation, `textureGather` or
  `sampler2DMS`.
- WebAssembly (`wasm-run10.sh`, laptop): `wasm_imports` (release) 5,112,617 bytes; the corpus test
  under wasmtime passes (all 9), every written file equals the native build's except one real
  program shader, `d599fbc7efa911be.dxbc`: its udiv helper function computes the quotient and the
  remainder from the same loads in the other order (the same values; WGSL, GLSL and the cache record
  differ). Known gaps.

Run 9, 2026-11-17, counts only:
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, `run9/draw4.log`, and
  again after the last change to the file, `run9/draw5.log`, all 3 tests passing): DXBC route equal
  to Direct3D 11 texel for texel on WebGPU 22 of 22 and WebGL 2 17 of 17 drawn (`ps_size` included);
  DXIL route equal to Direct3D 12 drawing the same DXIL on WebGPU 12 of 12 and WebGL 2 11 of 11; the
  12 Direct3D 12 pictures equal Direct3D 11's of the fxc builds. Not drawn on WebGL 2: bitfield
  operations 3, `noperspective` 1, fine derivatives 2. Windows clippy with warnings denied passes.
  The rig's triangle from DXIL and DXBC equal to Direct3D 12's picture on both back ends. (Drawn
  before the reader's id-order change, which only orders the expression arena.)
- Laptop (i7-8750H, load average 0.6 to 3.8): the suite with the corpus passes, every verdict as
  recorded (own DXBC and DXIL each one more: `ps_negate`, converting); `tests/cache.rs` 6 of 6 with
  the default 10 seeded cuts (seed 1). Records written: own 52, own DXIL 26, rig 112, rig DXIL 98 and
  the corpus's, each the same bytes after decoding and after a second conversion.
- WebAssembly (`wasm-run5.sh`): the corpus test under wasmtime passes and every written file,
  `cache.txt` included, equals the native build's (corpus, rig, rig DXIL, own, own DXIL: 0 differ),
  after the reader's id-order fix (before it, the DXIL `cache.txt` files differed).
- Chrome 157 (`web/check`, one GPU job, about 4 s, `run9/chrome.log`): WGSL corpus 127 of 127, rig
  DXBC 56 of 56, rig DXIL 49 of 49, own 26 of 26, own DXIL 13 of 13; GLSL ES corpus 89 of 102, rig
  DXBC 47 of 53, rig DXIL 46 of 48, own 21 of 24, own DXIL 12 of 12. Every GLSL ES failure is a
  bitfield operation, `textureGather` or `sampler2DMS`.

Run 8, 2026-11-17, counts only:
- Laptop (i7-8750H, load average 0.2 to 4.2), the suite with the corpus: corpus 127 convert and 64
  named (unchanged), of which 87 compile as GLSL ES in glslang, 15 do not and 25 are not written (one
  more unwritten: `106_ps_code_ddy_fine`, fine derivatives); rig DXBC 56 of 79 and rig DXIL 49 of 72
  (unchanged); own DXBC 25 of 26, own DXIL 12 of 16 (`ps_table` `NonFiniteConstant`, `ps_spaces`
  `RegisterSpace`, `vs_pack`/`ps_pack` `PackedStageValues`).
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job): equal to Direct3D 11
  texel for texel on WebGPU in 21 of 21 by the DXBC route and 11 of 11 by the DXIL route, on WebGL 2
  in 15 of 16 drawn by the DXBC route (`ps_size` the known failure, 2591 of 4096 texels, largest 111)
  and 10 of 10 by the DXIL route; not drawn on WebGL 2: bitfield operations 3, `noperspective` 1,
  fine derivatives 2 (`ps_deriv`, both builds). New and equal everywhere they are drawn: `vs_carry`
  (both builds, the check's loop case), `ps_ftoi` (both builds; NaN, both infinities and 2^31 from
  the constant buffer, finite values past the limits), `ps_table` (fxc's table holding NaN and
  infinities), `ps_deriv` (WebGPU), `ps_coarse`. The rig's triangle from DXIL and DXBC equal to
  Direct3D 12's picture on both. Log in scratch `graphics-shaders/run8/draw1.log`.
- Chrome 157 (`web/check`, one GPU job, about 2 s): WGSL corpus 127 of 127, rig DXBC 56 of 56, rig
  DXIL 49 of 49, own 25 of 25, own DXIL 12 of 12; GLSL ES corpus 89 of 102, rig DXBC 47 of 53, rig
  DXIL 46 of 48, own 20 of 23, own DXIL 11 of 11. Every GLSL ES failure is a bitfield operation,
  `textureGather` or `sampler2DMS`. Verdicts in scratch `graphics-shaders/run8/out`.
- WebAssembly (`wasm-run5.sh`, laptop): `wasm_imports` (release) 4,959,946 bytes, the same WASI
  imports; the corpus test under wasmtime 38.0.4 passes, and every written file equals the native
  build's (corpus, rig, rig DXIL, own, own DXIL: 0 differ), `converter-version.txt` included: both
  builds give `0.1.0+7e003573...` for the same sources (they differed before by the SDK's path).
- Clippy with warnings denied passes on the laptop for the crate; the carried naga's changed files
  give no new clippy warnings with the GLSL, HLSL, WGSL writers and SPIR-V reader built.
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000` under `ulimit -v 4000000` (new own
  files, so new cases; laptop load below 2): 7 cases ended the process, all DXIL in dxil-spirv (4
  segmentation faults: 1604, 1843, 2040, 2597; 3 `std::terminate`: 240, 309, 2637); no DXBC case.
  Our own checks refused 11: `malformed` 5 (`unknown stage` 3, `no instruction 0` 2), `invalid` 6
  (a constant index past an array's end 3, a compute workgroup size out of range 2, `VertexIndex` in
  a pixel shader 1). Log `~/wcontainers-graphics/fuzz-run8-seed7.log`, refusals `fuzz-ours-7.txt`.
  The check of run 7 (seed 4242, 8000 cases) found 39 DXIL cases that ended the process, none DXBC.

Run 7, 2026-11-17, counts only:
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job): WebGPU equal to Direct3D
  11 texel for texel in 16 of 16 by the DXBC route and 7 of 7 by the DXIL route; WebGL 2 in 11 of 12
  drawn by the DXBC route (`ps_size` the known failure, 2591 of 4096 texels, largest 111) and 7 of 7
  by the DXIL route; 4 not drawn on WebGL 2 for the reasons in the cases (bitfield operations,
  `noperspective`). The rig's triangle from DXIL and DXBC equal to Direct3D 12's picture on both.
  Logs in scratch `graphics-shaders/run7/draw*.log`.
- Chrome 157 (`web/check`, one GPU job, about 3 s; the first attempt failed with "no WebGPU
  adapter", the retry a minute later worked): WGSL corpus 127 of 127, rig DXBC 56 of 56, rig DXIL 49
  of 49, own 20 of 20, own DXIL 8 of 8; GLSL ES corpus 90 of 103, rig DXBC 47 of 53, rig DXIL 46 of
  48, own 16 of 19, own DXIL 8 of 8. Every GLSL ES failure is a bitfield operation, `textureGather`
  or `sampler2DMS`. Verdicts in scratch `graphics-shaders/run7/out`.
- Laptop (i7-8750H, load average 0.4 to 4.5), the suite with the corpus: corpus, rig DXBC and rig DXIL
  verdicts unchanged from run 5; own DXBC 20 of 21, own DXIL 8 of 11. Clippy with warnings denied
  passes on the laptop and on Windows (draw test included).
- WebAssembly (`wasm-run5.sh`): `wasm_imports` (release) 4,930,913 bytes, the same 10 WASI imports;
  the corpus test under wasmtime 38.0.4 passes and every written file equals the native build's
  (corpus, rig, rig DXIL, own, own DXIL: 0 differ).
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000` (new own files, so new cases; laptop
  load 3 to 4.5): 9 cases ended the process, all DXIL in dxil-spirv (7 segmentation faults: cases
  517, 553, 1357, 1564, 1705, 2234, 2737; 2 `std::terminate`: 726, 1648); no DXBC case. Our own
  checks refused 17: `malformed` 10 (`no instruction 0` 5, `unknown stage` 4, `opcode 416 result`
  1), `invalid` 7 (a constant index past an array's end 6, an image operation on a signed integer
  texture 1). Log `~/wcontainers-graphics/fuzz-run7-seed7.log`, refusals `fuzz-ours-7.txt`.

Run 6, 2026-11-17, counts only (laptop i7-8750H, load average 1 to 4):
- The whole suite with the corpus (`--include-ignored --skip mutated`), after the run's fixes and
  before the new own shaders: every test passes and every verdict equals its record, unchanged from
  run 5 (corpus 127 convert and 64 named, rig DXBC 56 of 79, rig DXIL 49 of 72; none of the rig's DXIL
  discards, so the demote fix changes no rig verdict).
- WebAssembly (`wasm-run5.sh`): `wasm_imports` (release) is 4,930,913 bytes (naga's compaction is
  linked now), the same 10 WASI imports; the corpus test under wasmtime 38.0.4 passes and every file
  it writes equals the native build's (corpus, rig, rig DXIL, own, own DXIL: 0 differ).
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000`, the same inputs as run 5 (the new own
  shaders not yet in): 11 cases ended the process, the 10 DXIL cases of run 5 exactly (8 segmentation
  faults, 2 `std::terminate`) and DXBC 934 killed for memory; run 5's DXBC memory cases 1539 and
  2228 ended as `rejected` this time (how much memory the laptop had free differs between runs). Our
  own checks refused the same 13 cases as in run 5. Log `~/wcontainers-graphics/fuzz-run6-seed7.log`.
- The check of run 5 (seed 31337, 6000 cases, before run 6): 19 ended the process (9 segmentation
  faults and 5 `std::terminate`, all DXIL; 5 killed for memory, 4 DXBC and 1 DXIL), 26 refused by our
  own checks.

Run 5, 2026-11-17, counts only:
- Laptop (i7-8750H, load average 1 to 5), the whole suite with the corpus (`--include-ignored --skip
  mutated`): every test passes; every verdict of corpus, rig DXBC and rig DXIL equals its record,
  unchanged from run 4 (corpus 127 convert and 64 named, rig DXBC 56 of 79, rig DXIL 49 of 72). Own
  DXBC 18 of 19 (`ps_spaces` `RegisterSpace`; `ps_cases` converts now that loops are left from switch
  cases), own DXIL 5 of 8 (`ps_spaces` `RegisterSpace`, `vs_pack` and `ps_pack`
  `PackedStageValues`).
- Chrome 157 on the RTX 3080 (`web/check`, one GPU job, about 3 s): WGSL corpus 127 of 127, rig DXBC
  56 of 56, rig DXIL 49 of 49, own 18 of 18, own DXIL 5 of 5; GLSL ES corpus 90 of 103, rig DXBC 47
  of 53, rig DXIL 46 of 48, own 14 of 17, own DXIL 5 of 5. Every GLSL ES failure is a bitfield
  operation (`findMSB`, `bitCount` and `findLSB` among them), `textureGather` or `sampler2DMS`.
  Verdicts in scratch `graphics-shaders/run5/out`.
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings): every own pair equal to Direct3D 11
  texel for texel by the DXBC route on WebGPU (14 of 14) and WebGL 2 (9 drawn, `ps_depth` the known
  back-end failure, 2591 of 4096 texels, largest 77, as in run 4), and by the DXIL route
  (`ps_signed`, `ps_offset`, `ps_special`, `ps_cases`) on WebGPU (4 of 4) and WebGL 2 (3 drawn;
  `ps_cases` is not drawn there because fxc's build extracts bits). The rig's triangle from DXIL and
  DXBC equal to Direct3D 12's picture on both. Log in scratch `graphics-shaders/run5-draw.log`.
- WebAssembly (laptop, `run5/wasm.sh`): `wasm_imports` (release) is 4,853,449 bytes, the same 10 WASI
  imports. The corpus test under wasmtime 38.0.4 passes, and every file it writes equals the native
  build's (corpus, rig, rig DXIL, own, own DXIL).
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000` (scratch `fuzz/resume.sh`, laptop load
  below 5). The inputs now include the 13 new own files, so the cases are not those of run 4. 13
  cases ended the process: 10 DXIL in dxil-spirv (8 segmentation faults, 2 `std::terminate`: cases
  434, 743, 1457, 1904, 1938, 1964, 2158, 2600, 2786, 2997) and 3 DXBC killed for memory (934,
  1539, 2228). 13 cases were refused by our own checks: `malformed` 7 (`no instruction 0` 4, `unknown
  stage` 3) and `invalid` 6 (4 constant indices past an array's end, 1 duplicate switch case value, 1 `tanh` of a
  type it does not take;
  scratch `fuzz/run5-seed7-ours.txt`). The independent check's run with seed 20261117 and 12000
  cases (before run 5) ended the process in 55 (0.46%) and refused 38 by our own checks.

Run 4, 2026-11-17, on the Linux laptop (i7-8750H, load average 1 to 2), counts only:
- Corpus test (all verdicts as recorded in `tests/expected`): corpus 127 convert (88 GLSL ES compile
  in glslang, 15 do not, 24 not written: 3 fewer unwritten than run 3, from the level-zero fold), 64
  named failures with the same counts by reason as below; rig DXBC 56 of 79 (unchanged); rig DXIL 49
  of 72 (the rig's DXIL grew from 27; the 23 others: execution models of geometry and tessellation 15,
  typed buffers 2, a storage image without a format 1, an unsupported built-in 1, the sample mask as
  an array 1, `PrimitiveId` 1, `VertexWithoutPosition` 1, `Texture1DArray` 1; run 4's record of
  this line said 17 execution models, which was wrong); own DXBC 12 of 13
  (`ps_spaces` is `RegisterSpace`), own DXIL `ps_spaces` `RegisterSpace`.
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000`, inputs the rig's, our own and the
  corpus, every mutation re-signed so it reaches the converters, restarted after each crash
  (scratch `graphics-shaders/fuzz/resume.sh`): 16 of the 3000 cases ended the process, 15 of them
  DXIL (10 segmentation faults and 5 `std::terminate` in dxil-spirv: cases 751, 943, 1113, 1632,
  1651, 1656, 1732, 1846, 1937, 1955, 2443, 2693, 2774, 2775, 2810) and one DXBC case killed for
  memory (1996, `024_ps_ld_sint8_code_1.dxbc`). No assertion abort, foreign exception or Rust panic
  ended it (run 3's check found 43 foreign exceptions, 25 assertions and 4 panics in 5,950 DXBC
  cases, and 292 assertions in 2,000 DXIL cases, all unsigned mutations, which the checksum now
  refuses). Replay one case: `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_FIRST=751
  WCONTAINERS_FUZZ_CASES=752`.
- Chrome 157 on the RTX 3080 (`web/check`, one GPU job, about 3 s), the files of the run above:
  WGSL corpus 127 of 127, rig DXBC 56 of 56, rig DXIL 49 of 49, own 12 of 12; GLSL ES corpus 90 of
  103, rig DXBC 47 of 53, rig DXIL 46 of 48, own 9 of 11. Every GLSL ES failure is a bitfield
  operation, `textureGather` or `sampler2DMS`. Verdicts in scratch `graphics-shaders/run4/out`.
- `tests/draw.rs` on the desktop (RTX 3080; the desktop is shared, no timings): every own pair equal
  to Direct3D 11 texel for texel on WebGPU (9 of 9) and on WebGL 2 (6 drawn: `ps_flat`, `ps_local`,
  `ps_size` among them; `ps_depth` differs in 2591 of 4096 texels, largest 77, the back end's depth,
  recorded as a known failure of that back end); the rig's triangle from DXIL and DXBC equal to
  Direct3D 12's picture on both. Real Direct3D 11 (`CreatePixelShader`) and 12
  (`CreateGraphicsPipelineState`) answer a container with one flipped byte with `E_INVALIDARG`
  (0x80070057), and `convert` with `Rejected`.
- WebAssembly: `wasm_imports` (release) is 4,837,106 bytes, the same 10 WASI imports. The corpus test
  built for `wasm32-wasip1` under wasmtime 38.0.4 passes (all records met) and writes every file equal
  to the native build's; its verdicts differ only where glslang, which it cannot run, would refuse.

Run 3, 2026-11-17, after all of run 3's changes:
- Laptop corpus test (i7-8750H, load average below 6): corpus 127 convert (85 GLSL ES compile in
  glslang, 15 do not, 27 not written), 64 named failures, the same counts by reason as below; rig
  DXBC 56 of 79 (unchanged); rig DXIL 24 of 27 (the rig now has 27 DXIL shaders; the 3 others are
  the reader's gaps: typed buffers twice, a storage image without a format).
- Chrome 157 on the RTX 3080 (one GPU job, about 2 s): WGSL corpus 127 of 127, rig DXBC 56 of 56,
  rig DXIL 24 of 24, own 6 of 6; GLSL ES corpus 87 of 100, rig DXBC 47 of 53, rig DXIL 23 of 24
  (`textureGather`), own 4 of 6. Verdicts in scratch `graphics-shaders/run3/out-final/*/chrome.txt`.
- `tests/draw.rs` on the desktop (RTX 3080, driver 32.0.16.2101; ANGLE from mozangle 0.7.1): every
  comparison exact, as listed above. The desktop is shared; no timings taken.
- WebAssembly: `wasm_imports` (release) is 4,853,482 bytes, the same 10 WASI imports.

Chrome, run 2, 2026-11-17: Chrome 157.0.8103.78 (the rig's pinned build), headless, on the desktop
(RTX 3080, WebGPU on D3D12, WebGL 2 on ANGLE D3D11), every adapter feature requested and every WebGL
extension enabled. Verdicts per file in `chrome.txt` beside the written files. After run 2's fixes:
- corpus: WGSL 127 of 127 compile; GLSL ES 87 of 100 (13 fail: bitfield operations 8, gather 4,
  `sampler2DMS` 1). Before the fixes WGSL was 125 (the NaN constant and the f32::MAX literal) and
  GLSL 86 (the sample mask).
- rig DXBC: WGSL 56 of 56; GLSL ES 47 of 53 (bitfield insert 3 and extract 1, gather 1,
  `sampler2DMS` 1).
- rig DXIL: WGSL 11 of 11, GLSL ES 11 of 11 (before run 2's DXIL entry point wrapper; run 3's re-run
  after it gave 24 of 24 and 23 of 24, above). Own: WGSL 5 of 5, GLSL ES 3 of 5 (bitfield operations).
- Each run of the 196 files' compile took about 2 s of GPU job time.

All on the Linux laptop (i7-8750H, Ubuntu 20.04), load average below 4 during the runs, 2026-11-17.
Counts only, no timings.

Direct route (this crate) on the plan's 191-shader DXBC corpus (`WCONTAINERS_SHADER_CORPUS`):
- 127 convert and their WGSL validates (naga, WebGPU core capabilities).
- Of those, 100 are written as GLSL ES 3.00 by naga and 85 compile in glslangValidator 8.13. The 15
  that do not: bitfield extract, insert, reverse and find-MSB (8), `textureGather` (4),
  `gl_SampleID` and `gl_SampleMask` (2, which need OES_sample_variables; run 2 made naga enable it,
  and Chrome compiles them, above), and one more. The 27
  naga does not write, by the features it names: texture level queries 14, storage buffers 7, compute
  3, sample counts 2, cube arrays 1 (all absent on WebGL 2, or back-end fixes of 6.5).
- 64 fail with a named reason: StorageImage 15, Atomics 9, GeometryShader 7, VertexWithoutPosition 6,
  TypedBuffer 6, HullShader 5, DomainShader 4, SampleCount 4, StorageCounter 2, DynamicOffset 2,
  RenderTargetArrayIndex 2, StencilRef 1, ViewportArrayIndex 1.
- The SPIR-V route at the same dxbc-spirv commit (6.3, measured in G0): naga read 123, wrote 112 as
  WGSL (111 compiled in Chrome), 95 as GLSL ES (80 compiled in Chrome).
- Decision: the direct route, as the plan chose. It converts 127 to valid WGSL against 112 written,
  and 100 to GLSL ES against 95, with every failure named, and none of the reader's gaps (compute
  shaders, typed buffers' and storage images' formats read wrongly, built-ins WGSL cannot express).

The rig's 79 compiled DXBC shaders (`crates/wcontainers-graphics-test/shaders/dxbc`): 56 convert to
valid WGSL (45 also compile as GLSL ES in glslang); 23 named: GeometryShader 6, HullShader 6,
DomainShader 3, StorageCounter 3, StorageImage 2, Atomics 1, PrimitiveId 1, VertexWithoutPosition 1.

WebAssembly: the conversion alone (`examples/wasm_imports.rs`, release, `wasm32-wasip1`, both
converters linked) was 4,842,413 bytes in run 2 (4,853,482 in run 3) (2,356,683 with dxbc-spirv alone) and imports from `wasi_snapshot_preview1`: environ_get, environ_sizes_get, fd_close,
fd_fdstat_get, fd_prestat_get, fd_prestat_dir_name, fd_read, fd_seek, fd_write, proc_exit (10;
the plan expected proc_exit and fd_write only: libc++'s streams, which dxbc-spirv's logger uses, pull
in wasi-libc's stdio and start-up). The corpus test built for `wasm32-wasip1` and run under wasmtime
38.0.4 gives the same verdicts and the same written files as the native build, for the corpus and
the rig's DXBC and DXIL.

## naga as carried

`third_party/naga` is naga 30.0.2 exactly as published on crates.io (commit "carried naga 30.0.2 in
the tree as published"), plus these changes, each a later commit touching only `third_party/naga`:
- GLSL writer, `BuiltIn::VertexIndex` without `WriterFlags::DRAW_PARAMETERS` is written
  `(uint(gl_VertexID) + naga_vs_base_vertex)`, with `uniform uint naga_vs_base_vertex;` declared in
  vertex shaders that read it (`back::glsl::BASE_VERTEX_BINDING`, feature `VERTEX_INDEX`), as naga
  already does for `instance_index` with `naga_vs_first_instance`. The back end sets it to each indexed
  draw's base vertex (0 otherwise); GL leaves an unset uniform 0. With `DRAW_PARAMETERS` nothing
  changes (desktop GL's `gl_VertexID` already includes the base vertex).
- GLSL writer, `BuiltIn::SampleIndex` is written `uint(gl_SampleID)` (it is a `u32` in naga and an
  `int` in GLSL; 6.3's corpus had one shader failing on it).
- GLSL writer, `BuiltIn::SampleMask`: written `gl_SampleMask[0] = int(...)` and read
  `uint(gl_SampleMaskIn[0])` (GLSL's are `int` arrays), requesting GL_OES_sample_variables on ES.
- WGSL writer, an f32 literal whose shortest digits name a number past f32::MAX (f32::MAX itself) is
  written with the f64 digits of its exact value, which WGSL compilers accept.
- HLSL writer (wgpu's Direct3D 12 path), a fragment entry point's position is passed on as
  `float4(p.xyz, 1.0 / p.w)`: the IR's fragment position has 1/w in w (SPIR-V's and GLSL's rule,
  which dxbc-spirv inverts back to Direct3D's w), HLSL's `SV_Position` has w.
- WGSL writer, the module's diagnostic filters are written as `diagnostic(...)` directives
  (`Severity::to_wgsl_ident` added beside `from_wgsl_ident`).
- SPIR-V reader (run 5): integer instructions read in the signedness they name, the result given its
  type's (`convert::op_signedness`, `glsl_signedness`, `Frontend::retype_int`); atomic and subgroup
  minimum and maximum of the other signedness refused (`Error::UnsupportedSignedness`); texel offsets
  on `OpImageFetch` added to the coordinate; unknown load operands refused
  (`Error::UnsupportedImageLoadOperand`) instead of skipped with a log warning.
- WGSL writer (run 5), `isnan`/`isinf` of f32 scalars and vectors written as bit tests (WGSL has
  neither).
- SPIR-V reader (run 6), `OpDemoteToHelperInvocation` read as `Statement::Kill` (WGSL's `discard`
  demotes too), its extension and capability accepted.
- GLSL and HLSL writers (run 8), a loop's `continuing` block reads the values its body made in that
  iteration from variables declared before the loop (`compact::values_for_continuing`), instead of
  writing their expressions again where the body's variables have changed.
- MSL writer (run 9), the same `continuing` fix as the GLSL and HLSL writers (run 8).
- SPIR-V reader (run 9), `OpSNegate` of an unsigned operand negated as signed; global variables'
  and constants' expressions made in id order (the same module on 32-bit and 64-bit targets).
- SPIR-V reader (run 10), an `OpUndef` in a function ends the running Emit range before its zero value
  (a zero value is never emitted); a texel coordinate's vector type of another scalar kind is made
  where the reader needs it, instead of panicking that it should exist.
- GLSL writer (run 12), WebGL programs with clip or cull distances enable
  `GL_ANGLE_clip_cull_distance` (WEBGL_clip_cull_distance's name) instead of
  `GL_EXT_clip_cull_distance`.
- GLSL writer (run 8), fine derivatives need `Features::DERIVATIVE_CONTROL` (GLSL 4.50, not ES)
  instead of being written as the plain functions.
- SPIR-V reader (run 25), an image taken from an array of images (`Access`/`AccessIndex` on a
  binding array) has the array's element type. This one is in commit 182398b1 together with the
  crate's `arrays.rs`, not a commit of its own.
- SPIR-V reader (run 27), `OpIsHelperInvocationEXT` reads a private bool named
  `front::spv::HELPER_INVOCATION`, made at its first use; the module's user sets it (this crate's
  `dxil.rs` from the coverage and at each demotion). Before, the reader refused the instruction.
- SPIR-V reader (run 27), an image sample operand it does not know is refused
  (`unsupported image sample operand`) instead of skipped with a warning.
- SPIR-V reader (run 28), a gather's run-time `Offset` moves the coordinate by the offset in texels of
  level 0 (low six bits, signed); availability and visibility operands at workgroup scope or less
  taken on storage and texels (WGSL's memory model gives every access that), wider ones refused
  (`Error::UnsupportedMemoryScope`); `OpGroupNonUniformBallotBitCount` read as a subgroup add of the
  ballot's predicate (or the ballot's bits counted, reduce only); `OpGroupNonUniformAllEqual` read as
  an all of each lane's value equal to the first lane's; ballot find LSB and MSB from the words' trailing and leading zeros.
- WGSL writer and reader (run 28), `enable subgroups;` written for modules with subgroup operations or
  built-ins (`Module::uses_subgroups`) and taken by the reader (`ImplementedEnableExtension::Subgroups`).
Not yet sent upstream: that needs a GitHub account and fork the owner chooses, which this track does
not have. Each change is its own commit touching only `third_party/naga`, so each can be sent as it
stands (`git diff d380816 HEAD -- third_party/naga`).

## How to run

- Linux laptop (no GPU), one command from the Mac: `tools/graphics-rig/linux <command>`. It copies the
  working tree (submodules included) to `~/wcontainers-graphics/src/<branch>` and builds into
  `~/wcontainers-graphics/target/<branch>`, with 5 cargo jobs (2 when the laptop's load is 8 or more).
- Chrome's compile of the written files: copy `~/wcontainers-graphics/out` from the laptop to a
  directory here (`rsync -a tom@192.168.2.240:wcontainers-graphics/out/ <dir>/`), then
  `crates/wcontainers-shaders/web/check <dir>`: one GPU job on the desktop, `chrome.txt` comes back
  into each corpus directory, exit 1 if any file fails.
- When the desktop refuses new SSH connections (reset after the key exchange; run 6 lost an hour to
  it, run 7 half an hour, it did so all through run 8, and again in run 9), already open shared connections keep working. Runs 7 and 9 pointed this
  checkout's rig socket (`~/.ssh/wc-rig-<hash of the git dir>`, see `tools/graphics-rig/common.sh`)
  at another checkout's live one with a symbolic link, and removed the link afterwards. The rig also
  notes the desktop's clock once went back 8 hours, so cargo may keep stale objects: run 7's first
  draw build ran `cargo clean -p wcontainers-shaders -p naga -p wcontainers-gpu
  -p wcontainers-gpu-webgpu -p wcontainers-gpu-webgl -p wcontainers-graphics-test` first.
- The laptop's disk (122 GB) filled during run 10 and a build failed with "No space left on device":
  run 10 deleted this branch's `debug/incremental` and three stale `wcontainers-shaders-*` build
  directories under `~/wcontainers-graphics/target/phase-2-graphics-shaders` (16 GB free after). The
  independent checks' directories `~/wcontainers-graphics/check-g6a*` hold about 29 GB and were left
  alone.
- Building on the desktop: the rig's `remote` does not fetch submodules, so a build command that
  needs this crate starts with `git submodule update --init crates/wcontainers-shaders/dxbc-spirv
  crates/wcontainers-shaders/dxil-spirv && git -C crates/wcontainers-shaders/dxil-spirv submodule
  update --init third_party/spirv-headers && cargo build ...` (they stay checked out afterwards).
- Drawing (Windows, needs the GPU and ANGLE at C:\wc\tools\angle-mozangle-0.7.1, which the WebGL
  track's `angle\build.cmd` makes; `WCONTAINERS_ANGLE` names another):
  `S="git submodule update --init -q crates/wcontainers-shaders/dxbc-spirv crates/wcontainers-shaders/dxil-spirv && git -C crates/wcontainers-shaders/dxil-spirv submodule update --init -q third_party/spirv-headers"; tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test draw --no-run" -- "cargo test -j 3 -p wcontainers-shaders --test draw -- --nocapture --test-threads 1"`.
  With `WCONTAINERS_SHADERS_OUT` set in the job the pictures are kept as raw texels under `draw`.
- Windows lint (the draw test is Windows only): `tools/graphics-rig/remote "$S && cargo clippy -j 3
  -p wcontainers-shaders --all-targets -- -D warnings"`.
- The rig's scenes through the direct3d-11 track's library: `tools/graphics-rig/scenes --build "$S &&
  cargo build -q -j 3 -p wcontainers-directx" --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' --api d3d11`.
- Unit and corpus tests: `tools/graphics-rig/linux sh -c 'WCONTAINERS_SHADER_CORPUS=$HOME/wcontainers-graphics/corpus/dxbc
  WCONTAINERS_SHADERS_OUT=$HOME/wcontainers-graphics/out GLSLANG=glslangValidator cargo test -p
  wcontainers-shaders -- --include-ignored --skip mutated'`. Each corpus's verdicts must equal
  `tests/expected/<corpus>.txt`; the written WGSL and GLSL, `verdicts.txt` and `classes.txt` (the
  records' form: copy it over the record after a deliberate change) land in
  `~/wcontainers-graphics/out/<corpus>`. The 191-shader corpus test is ignored without
  `--include-ignored` and fails without `WCONTAINERS_SHADER_CORPUS`; glslang runs only with both
  `GLSLANG` and `WCONTAINERS_SHADERS_OUT`.
- Geometry shaders through the stage helper (Windows, one GPU job, about 2 s for 4 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test geometry_draw --no-run" -- "set WCONTAINERS_GEOMETRY_SEED=1&& set WCONTAINERS_GEOMETRY_CASES=4&& cargo test -j 3 -p wcontainers-shaders --test geometry_draw -- --nocapture --test-threads 1"`.
  Each comparison prints the texels Direct3D drew and how many differ; with
  `WCONTAINERS_SHADERS_OUT` the pictures are kept.
- Stream output: the same job runs `stream_output_matches_direct3d` (about 3 s for 8 seeds of both
  tests together).
- Typed buffers: the `compute_draw` job below runs `typed_buffers_match_direct3d` too (about 1 s for 8
  seeds); `variant_draw` runs `typed_buffers_draw_as_direct3d_does` (`WCONTAINERS_TEXEL_SEED`,
  `WCONTAINERS_TEXEL_CASES`, default 8 seeds, about 1.5 s).
- Atomics, counters and storage textures (Windows, one GPU job, about 2 s for 4 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test compute_draw --no-run" -- "set WCONTAINERS_COMPUTE_SEED=1&& set WCONTAINERS_COMPUTE_CASES=4&& cargo test -j 3 -p wcontainers-shaders --test compute_draw -- --nocapture --test-threads 1"`.
- Early tests in the program against real Direct3D 11 (Windows, one GPU job, under 1 s; seeded scenes,
  default 32): `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test early_draw --no-run" -- "set WCONTAINERS_EARLY_SEED=1&& set WCONTAINERS_EARLY_CASES=32&& cargo test -j 3 -p wcontainers-shaders --test early_draw -- --nocapture --test-threads 1"`.
  Each scene prints how many pixels Direct3D ran the shader for and the marks and texels apart, for
  the fxc and the dxc build.
- The built-in clear against WARP and the GPU's Direct3D 11 (Windows, one GPU job, about 3 s for
  256 cases): `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test clear_draw --no-run" -- "set WCONTAINERS_CLEAR_SEED=1&& set WCONTAINERS_CLEAR_CASES=256&& cargo test -j 3 -p wcontainers-shaders --test clear_draw -- --nocapture --test-threads 1"`.
- The tessellator against the GPU's (Windows, one GPU job, about 12 s for 16 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test tessellation_draw --no-run" -- "set WCONTAINERS_TESSELLATION_SEED=1&& set WCONTAINERS_TESSELLATION_CASES=16&& cargo test -j 3 -p wcontainers-shaders --test tessellation_draw -- --nocapture --test-threads 1"`.
  The same job runs the tessellated patches and the rig's tessellation scene against real
  Direct3D 11 (about 1 s; with `WCONTAINERS_SHADERS_OUT` set the pictures are kept as raw RGBA
  under `draw`, 256 by 128).
- Compiling own shaders: `tests/own/compile.cmd` on the desktop (through `tools/graphics-rig/remote`);
  run 17 fetched the new files back as hex (`certutil -encodehex -f <file> <file>.hex 12`, then
  `type`) and decoded them here.
- Filtering in the program (Windows, one GPU job, about 90 s for 8 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test filter_draw --no-run" -- "set WCONTAINERS_FILTER_SEED=1&& set WCONTAINERS_FILTER_CASES=8&& cargo test -j 3 -p wcontainers-shaders --test filter_draw -- --nocapture --test-threads 1"`.
  Each comparison prints texels apart from both references, from each, and before moving the level
  of detail; with `WCONTAINERS_SHADERS_OUT` the pictures are kept.
- The variant tests (Windows, one GPU job, about 1.5 s):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test variant_draw --no-run" -- "cargo test -j 3 -p wcontainers-shaders --test variant_draw -- --nocapture --test-threads 1"`.
  Each comparison prints its counts and allowance. The WebGL 2 back end leaves its GL context current
  when it is made, so a test makes a second one (without EXT_clip_control) only after the first one's
  last draw.
- The real programs' shaders: `tools/graphics-rig/remote "$S && cargo run -q -j 3 -p wcontainers-shaders --example containers -- C:\wc\shaders-games C:\wc\programs"`,
  then copy C:\wc\shaders-games to scratch `graphics-shaders/games` and the laptop's
  `~/wcontainers-graphics/corpus/games`; the corpus test reads them with
  `WCONTAINERS_GAME_SHADERS=$HOME/wcontainers-graphics/corpus/games` (and `--include-ignored`).
- The cache's seeded cuts: `tools/graphics-rig/linux sh -c 'WCONTAINERS_CACHE_SEED=7
  WCONTAINERS_CACHE_CUTS=1000 cargo test -p wcontainers-shaders --test cache -- --nocapture'` (each
  cut is printed with its seed; defaults seed 1, 10 cuts).
- Fuzz (long, one command; not re-run in run 9): `tools/graphics-rig/linux sh -c 'WCONTAINERS_SHADER_CORPUS=$HOME/wcontainers-graphics/corpus/dxbc
  WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000 cargo test -p wcontainers-shaders --test fuzz
  -- --ignored --nocapture'`. Each case is printed before it runs; `WCONTAINERS_FUZZ_FIRST=n` starts
  at case n. It ends at the first case that crashes (Known gaps); scratch
  `graphics-shaders/fuzz/resume.sh <seed> <cases>`, run on the laptop after one build, restarts past
  each crash and lists them; the cases our own checks refused go to
  `~/wcontainers-graphics/fuzz-ours-<seed>.txt`.
- WebAssembly in one command: scratch `graphics-shaders/run10/wasm.sh` (copied to
  `~/wcontainers-graphics/wasm-run10.sh`; it adds the real programs' shaders to run 5's), run through
  `tools/graphics-rig/linux bash -c '$HOME/wcontainers-graphics/wasm-run10.sh'` after a native corpus run has filled
  `~/wcontainers-graphics/out`: builds `wasm_imports` and the corpus test, runs the test under wasmtime
  and counts the written files that differ from the native ones (`converter-version.txt` among them
  since run 8, so a differing converter version shows as a differing file).
- WebAssembly: add `WASI_SDK_PATH=$HOME/wcontainers-graphics/tools/wasi-sdk` and `--target
  wasm32-wasip1`; run a test with `~/wcontainers-graphics/tools/wasmtime/wasmtime run --dir $S::$S
  --dir corpus/dxbc::/corpus --env WCONTAINERS_SHADER_CORPUS=/corpus <test>.wasm --include-ignored`,
  where `$S` is the source copy (`~/wcontainers-graphics/src/phase-2-graphics-shaders`), which the
  tests read at their build path.
- After cloning: `git submodule update --init crates/wcontainers-shaders/dxbc-spirv
  crates/wcontainers-shaders/dxil-spirv` and `git -C crates/wcontainers-shaders/dxil-spirv submodule
  update --init third_party/spirv-headers`. dxbc-spirv's own SPIRV-Headers and dxil-spirv's other
  submodules (SPIRV-Tools, SPIRV-Cross, its copy of dxbc-spirv) are not needed. The rig's `remote`
  sends a git bundle, which carries no submodules: building this crate on the desktop needs the
  submodules fetched there (not done yet; a matter for the rig's remote script).
- The corpus is in scratch (`wcontainers-scratch/graphics/corpus/dxbc`, from Wine's tests, LGPL, test
  data only) and was copied to `~/wcontainers-graphics/corpus/dxbc` on the laptop.

Installed on the laptop (Ubuntu 20.04) for this track: rust target `wasm32-wasip1` for 1.99.0 (rustup),
`glslang-tools` 8.13 (apt), wasi-sdk 25.0 and wasmtime 38.0.4 (release archives from their GitHub
releases, under `~/wcontainers-graphics/tools`). rustup, the 1.99.0 toolchain, gcc 9.4 and clang 10
were already there.

## Dependencies and licences

- Run 21: `spirv` 0.4.0 (Apache-2.0; SPIR-V's enums, already in the tree under naga), used to read
  and edit dxil-spirv's SPIR-V before naga reads it.
- Run 20: `src/stage/tessellator.wgsl` is a port (to WGSL) of Microsoft's reference tessellator,
  MIT ("Copyright (c) Microsoft Corporation", the licence text at the head of Mesa's
  src/gallium/auxiliary/tessellator/tessellator.cpp, read 2026-11-17); the program's header names it.
  No new crate.
- dxbc-spirv, https://github.com/doitsujin/dxbc-spirv, submodule at 6196cc6c62b7b04acb4f3e713ff3bb1a7e7950fd
  (upstream main's head on 2026-11-17): MIT, "Copyright (c) 2025 Philip Rebohle" (its LICENSE).
- dxil-spirv, https://github.com/HansKristian-Work/dxil-spirv, submodule at
  510991e707cda3aa389305448edad7c7dd76e3e7 (upstream master's head on 2026-11-17): MIT (LICENSE.MIT,
  "Copyright (c) 2019-2022 Hans-Kristian Arntzen for Valve Corporation", SPDX MIT). Of its submodules
  only third_party/spirv-headers (Khronos, MIT-style) is fetched; its glslang SPIR-V builder and
  bitcode reader are in its own tree (licences in the plan's section 3).
- naga 30.0.2 (carried, see above): MIT OR Apache-2.0; its `spv-in` reader is now used.
- `cc` 1.11 (build dependency, compiles the C++): MIT OR Apache-2.0 (its Cargo.toml); with `jobserver`
  0.1 (MIT OR Apache-2.0) for its parallel feature.
- `blake3` 1.8: as in the plan's section 3; also a build dependency now (hashing the C++ sources).
- Run 15: naga's `wgsl-in` feature (naga as above; already built into every program using this
  crate through wcontainers-gpu).
- Run 9, for the cache: `postcard` 1.1 (MIT OR Apache-2.0), `serde` 1.0 with `derive` (MIT OR
  Apache-2.0, also now a dependency of wcontainers-gpu, with `bitflags`' `serde` feature), `crc32fast`
  1.5 (MIT OR Apache-2.0), naga's own `serialize`/`deserialize` features; all already in the
  workspace's lock. Dev-dependency `fastrand` 2.5 (Apache-2.0 OR MIT) for the seeded cuts. Licences
  read from each crate's Cargo.toml.
- Dev-dependencies of `tests/draw.rs`, Windows only, already in the workspace's lock: `windows` 0.62
  (MIT OR Apache-2.0), `wgpu` 30 (MIT OR Apache-2.0), `pollster` 1 (Apache-2.0 OR MIT), and the
  workspace's own wcontainers-gpu-webgpu, wcontainers-gpu-webgl and wcontainers-graphics-test.
- Both C++ projects confirmed in run 3: the submodules' URLs are the upstream repositories, the
  pinned commits are on upstream's main (dxbc-spirv, still its head) and master (dxil-spirv, whose
  head has since moved to a781dce), and neither checkout has local changes.
- Test tools only, never shipped: glslang (BSD-3-Clause and others, Khronos), wasi-sdk (Apache-2.0
  WITH LLVM-exception; its libc++ and wasi-libc are linked into the WebAssembly build: libc++ Apache-2.0
  WITH LLVM-exception, wasi-libc MIT-style and others), wasmtime (Apache-2.0 WITH LLVM-exception).

## Departures from the plan

- Storage offsets (run 28; not built yet, see Next): 4.5 said every converted program adds
  `StorageOffset`; it is a flag (`VariantFlags::STORAGE_OFFSET`) instead, so programs whose views are aligned (most, on Direct3D
  11) pay no load and add per access, and a front that cannot know ahead (Direct3D 12) sets it on
  every program, which is the plan's behaviour. The plan's text now says so.

- Run 26: the laptop stopped answering, so the non-GPU tests ran on the desktop (3 cargo jobs,
  inside the GPU job) instead; the corpus and WebAssembly runs, which need the laptop's corpus and
  tools, were not rerun.

- Vertex programs without a position (run 23): the plan is silent. They convert with the position
  (0, 0, 0, -1) added, which every clip test rejects, so a direct draw of one rasterises nothing
  instead of the conversion being refused; the stage helper's routes (stream output, hull and
  geometry stages) never rasterise it.
- Direct3D 12's numbering (run 22) is the program's own, not the root signature's: each kind's
  slots are the program's resources in (space, register) order, in the stage's set. The plan had
  sets by visibility and ranges in root signature order with the root signature's hash in the key.
  The back ends make each pipeline's layout from its modules, so a root-signature numbering would
  share nothing more, while it would make a program per root signature (more conversions and cache
  entries) and need the root signature at every conversion; the front already finds descriptors by
  space and register. The plan's 6.4 text is changed to match (interface commit). `VariantKey::layout`
  and `convert`'s `root_signature` stay for root constants read as immediates (not built; refused
  `RootSignatureLayout`).

- Stores of normalised formats through typed buffers are held to Direct3D's permitted tolerance
  (0.6 of a code from the exact value, functional specification 3.2.3.4 and 3.2.3.6), not to the
  RTX 3080's codes: the GPU rounds some values 0.51 to 0.55 above a code down, which no exact
  formula of the specification reproduces. Every other stored and loaded value is compared exactly.
- The formats of typed buffer views at texture slots are a variant field (`texel_formats`), so a
  program is made per bound format; decoding by a format code read at run time would cost a branch
  per load.

- The stage helper (6.4's sketch `Stages::draw(backend, call, outputs)`) is `Stages::run`, which
  records the compute work outside the pass and returns what the front draws in its own pass: the
  interface allows dispatches only outside a pass, and the front owns its passes. The helper's own
  bindings are in set 1 of its compute programs (no compute program of the API uses set 1), so the
  interface needed no new binding numbers. Adjacency topologies are the helper's own
  `InputTopology` (they reach only the geometry stage), not the interface's `Topology`.

- Filtering in the program is held to two references, real Direct3D 11 on the RTX 3080 and WARP,
  texel by texel equal to either (or within 1/256 of a level of detail of either, or a third answer
  where the two differ), not to one picture: the two differ from each other in most pictures with
  linear mip filtering (the GPU's trilinear optimisation), which no program reproduces. Anisotropic
  cases are a known failure (Known gaps).
- The filter library is WGSL text in the crate, parsed by naga's `wgsl-in` and imported into the
  converted module, rather than naga IR built by hand: the same input for the same output as a
  hand-built tree, far shorter, and reusable for the stage helper's programs.

- `VIEWPORT_DEPTH_FLIP`'s reference is computed (1 - z of real Direct3D 11's depth buffer, the
  swapped range's definition), within 4 f32 steps (3 measured): no API on the desktop draws a swapped
  range natively (Known gaps). The other variants are held to a native feature or real Direct3D's
  picture, exactly.
- The counting test counts into an rgba8unorm target (the grid's triangles do not overlap, so each
  texel counts at most one), not an r16float one: the WebGL 2 back end reads no float target back.
- The real programs' shaders stay outside the repository (scratch and the laptop), as the plan's
  191-shader corpus does, rather than in it beside the rig's.

- wasi-sdk is not in the flake (6.2 says G6 adds it): the only machine that builds for WebAssembly is
  the laptop, which has no Nix (the Mac must not build). It stays a release archive there (How to
  run) until a Nix machine builds WebAssembly.
- `tests/fuzz.rs` writes MD5's block function out (about 25 lines) to sign mutated containers: the
  DXBC checksum needs MD5's raw block function with its own last block, and the RustCrypto `md-5`
  crate keeps that function private. Test code only.
- DXIL whose checksum is the bypass value dxc writes when it does not sign (sixteen bytes of 1) is
  taken as signed; real Direct3D 12's answer to it is not measured (every DXIL we have is signed).
- Malformed bytecode is contained natively by C++ exceptions at the boundary and `catch_unwind`
  around our lowering; 6.2 builds the C++ with exceptions off everywhere. WebAssembly keeps them off
  (wasi-sdk 25's libc++ has none), where the module instance is the boundary.
- naga is carried as a copy in the tree (`third_party/naga`, patched in by the workspace), not as a fork
  by git revision (6.2): a fork needs a new repository on the owner's account, and this track commits
  only to its own branch. Moving to a git revision later changes one line of the workspace manifest.
- `isnan` from DXBC is lowered by `convert` itself (a bit test on the float's bits), not by the WGSL
  back end (6.5). From DXIL, naga's reader gives the IR's `isnan`/`isinf`, which the carried naga's
  WGSL writer writes as the same bit tests (f32 only; f16 and f64 stay refused by the writer), and
  GLSL writes as GLSL's own `isnan`/`isinf`.
- The content key's converter version (8) is the crate version plus a hash of all the sources that
  make the module (the C++, our Rust, the carried naga), not the crate version and the C++
  revisions: a change to the lowering or to naga changes modules without changing either.
- `[earlydepthstencil]` and conservative depth are not put in the module: WGSL has no form for either
  and naga's validator refuses them under WebGPU's capabilities. Early tests are reported in
  `StageExtras::Pixel::early_tests`; conservative depth is only an optimisation hint and is dropped
  (so GLSL ES loses it too).
- Every user input and output is a four-component value at location = register (6.4 speaks of packing
  by component; WGSL has no component qualifier, and fixed four-component values let two stages
  converted apart always agree).
- The cache stops reading at the first entry that is cut short or fails its CRC, and drops the rest
  (the plan says only that a cut-off last entry is ignored): past a damaged length nothing can be
  trusted, and dropping keeps appends well-formed. The CRC covers the entry's kind, key and length
  as well as its payload.
- A converted shader's record keeps the naga module and the reflection, not naga's `ModuleInfo`
  (naga cannot read it back); `decode` validates the module again.
- The WebAssembly build imports ten WASI functions, not two (above).
- The corpus test writes GLSL with the WebGL 2 back end's options copied (its `write_glsl` is private
  to that crate); if that back end changes them, the test follows by hand.
- dxil-spirv's converter library is not free of dxbc-spirv as 6.2 says: its bitcode module
  (`bc/module_dxbc_ir.cpp`) reads dxbc-spirv's IR. It is built against our one copy of dxbc-spirv
  (newer than the commit dxil-spirv pins; it compiles), and all of dxil-spirv is built as C++17, as
  its CMake does for that file.

## Known gaps found so far

- fxc fails with internal errors on programs with several loops in one switch and on switches
  whose cases call functions inlined many times (run 27, the tessellator): the built-in programs
  avoid them; the tessellation test on wgpu's Direct3D 12 is what catches a new one.
- Tessellator output buffers are sized for every patch reaching factor 64 (about 180 KB a patch),
  since the counts are known only on the GPU; a draw of thousands of patches needs hundreds of MB.
  To size by the counts instead (read back later, grown before the next draw) once draws run.
- Inside a patch the GPU's tessellator gives the primitives in another order than the reference, and
  starts about two thirds of its triangles at another vertex (winding kept); ours follow the
  reference. A domain shader's flat outputs (taken from a triangle's first vertex) can then differ.
- Typed buffers: DXIL's atomics are refused by naga's SPIR-V reader; geometry shaders' are refused
  (`TypedBufferInGeometry`); through a texture, a view reaches at most 2048 times the back end's
  largest texture height elements (32M on ANGLE's 16384), short of Direct3D 10's 2^27, which the
  front must report.

- Conversion is not byte-identical between the WebAssembly and native builds for one game shader
  (`d599fbc7efa911be.dxbc`, run 17): the helper function returning (x / y, x % y) loads its operands
  for the modulo first natively and for the division first in WebAssembly, so the WGSL, GLSL and
  module key differ (the meaning is the same). The same source key on both, so the difference arises
  in conversion; the lowering iterates no hash map in an order that reaches the output (phis are
  sorted), so dxbc-spirv's IR order on a 32-bit target is the first suspect. Run 10's comparison had
  no difference; not investigated further in run 17.

- WGSL has only the weak compare-exchange (`atomicCompareExchangeWeak`), which may fail without
  cause; Direct3D's does not. Where the browser's back end writes it as a strong one (Direct3D 12's
  `InterlockedCompareExchange`, Vulkan's `OpAtomicCompareExchange`) the result is exact; a spurious
  failure elsewhere would return the compared value while not exchanging. Not seen; not tested apart.

- The stage helper (run 16) refuses by name: indexed strips into a geometry shader, triangle strips
  with adjacency, indirect draws, streams other than 0, layer and viewport index outputs, clip and
  cull distances through a geometry shader, the last vertex stage's variants on its pass-through
  program (so a geometry shader draws only where WebGPU has what those variants stand in for), DXIL
  geometry shaders. Stream output and tessellation are not built. Its buffers grow and are never
  shrunk.

- Anisotropic filtering in the program (`filter_in_shader`, `border_in_shader`) is a known failure
  of `tests/filter_draw.rs`: the footprint is the implementation's own (the RTX 3080's and WARP's
  differ from each other, by up to 0.14 in seed 8), and the program's taps along the major axis are
  a third (up to 0.59 from the GPU with gradients scaled 3 and 0.5). Seed 14 (anisotropy 4) lost
  ANGLE's device in a WebGL 2 draw (cause not found; first suspect: the program's tap loop as
  ANGLE's HLSL compiles it).
- Filtering in the program refuses comparisons, so a shadow map sampled with a border colour on
  WebGPU (no border colours there) is refused until `SamplerState` carries the comparison (Next).

- The minimum level-of-detail clamp's zero is (0, 0, 0, 0): Direct3D gives the format's default
  for components the format lacks (alpha 1 for an R8 texture, for example), which the program does
  not know. Drawn only on RGBA8.
- `DEPTH_BIAS` leaves a depth the program writes as it is; that Direct3D does not bias a written
  depth is expected (as Vulkan defines it), not measured.
- wgpu's Direct3D 12 path has no clip distances (wgpu issue 6236), so natively on Windows the
  WebGPU back end refuses programs with them unless made on Vulkan; Chrome's WebGPU compiles them.
- `draw_base: Arguments` serves WebGPU only: WebGL 2 has no storage buffers to read the arguments
  from (nor indirect draws; its front reads them back and draws directly with `Helpers`).

- One real program shader (`d599fbc7efa911be.dxbc`, a bgfx shader) and, since run 24's check, one
  of Wine's (`116_cs_group_index_code.dxbc`; not compared since run 19) convert to modules whose
  expression order differs between the native and WebAssembly builds (run 10): the two results of
  its unsigned divide come out in the other order. Same values, but a cache made natively and one
  made in the browser would disagree on its entry (counted, the first kept). Cause (run 19): C++
  leaves the order of a call's arguments unspecified, and dxbc-spirv builds some operands inside
  one call's arguments (Run 19 lists them); gcc and MSVC evaluate them right to left, clang for
  WebAssembly left to right. Fix upstream; until then the browser's conversion (clang) is the one a
  shared cache should be made with.
- The GPU's Direct3D 11 clears integer values into storage views otherwise than WARP and the
  specification (Run 19); the built-in clear follows the specification, so a program relying on
  the RTX 3080's saturation would see other texels.

- WebGL 2 cannot write the sample mask (no OES_sample_variables in WebGL), so `SAMPLE_MASK` serves
  only WebGPU, which has the mask natively; the plan's `SampleMaskInShader` route for WebGL 2 does not
  exist (recorded for the owner).
- Neither real Direct3D 11 nor ANGLE on it draws a depth range with near beyond far as Vulkan and
  OpenGL define it (both gave the unswapped depths), so `VIEWPORT_DEPTH_FLIP` is checked against the
  definition applied to Direct3D 11's depth, not against a native swapped range.

- Signed but malformed DXIL can end the process natively: dxil-spirv's bitcode reader faults or calls
  `std::terminate` (`bc/cast.hpp`) on it (7 of 3000 fuzz cases in run 8, 9 in run 7, 10 in run 5, 15 in run 4,
  Measurements); malformed DXBC exhausted memory in 3 of 3000 in run 5 (1 in run 4, none in run 7), and the check of run 5
  found one DXIL case too (seed 31337, case 4995). The exhaustion is inside the converters: with the
  address space capped (`ulimit -v 3000000`) the DXBC cases end as `Rejected`, the C++ `bad_alloc`
  caught at the boundary, but natively nothing bounds it without such a cap; on WebAssembly the
  module's memory limit ends the instance. Only bytecode whose checksum matches reaches the converters, so this needs a
  program that signs malformed bytecode. dxil-spirv is used unmodified, so the fix belongs upstream
  (not yet reported). `tests/fuzz.rs` stays failing at those cases.
- Mutated but signed bytecode that dxbc-spirv accepts can give IR our lowering refuses (`Malformed`)
  or a module naga's validation refuses (`Invalid`, for example a constant index past an array's
  end, which Direct3D would read as zero): refused, not converted (Measurements).
- GLSL ES 3.00 has no `noperspective`: a pixel shader reading one is not written for WebGL 2, nor
  one using fine derivatives (run 8).
- DXIL constants that are NaN or infinite are refused (`NonFiniteConstant`): naga's IR takes no such
  literal, and making them at run time needs new expressions ahead of their users. The DXBC route
  reads them from variables.
- A DXIL constant buffer component picked at run time (`cb[i]` of a float4's components) is refused
  (`ConstantComponentIndexing`): dxil-spirv reads it through a second view of the buffer as scalars
  4 bytes apart, which WebGPU's uniform layout and WGSL's one variable per binding refuse.
- WebGL 2 (for the WebGL track): fxc writes ordinary arithmetic and the common full-screen vertex
  shader with `bfi`/`ubfe`, which GLSL ES 3.00 lacks (6.5 gives their lowering to that back end), and
  the back end answers the refused program with device lost instead of an error.
- The MSL writer's `continuing` fix (run 9) is checked by reading its output only; no Metal compile.
- The WebGL 2 back end's fragment depth (z arrived as (z+1)/2) is fixed there: `ps_depth` draws equal
  through it since run 7.
- The WebGL 2 back end's `textureSize` from a hidden uniform (its 3ea9af6) left `ps_size` reading 1
  by 1; fixed by its 4ab7bc84, equal since run 9.
- Direct3D links stages by register here; programs whose vertex outputs and pixel inputs use different
  registers for one semantic need remapping by semantic (also a variant to add).
- Out-of-bounds texture loads give what WebGPU's robustness gives, not Direct3D's zero. So does a
  dynamically indexed constant read past the size the bytecode declares (WebGPU clamps the index).
- Found while drawing, for other tracks: the WebGL 2 back end keeps GL's previous viewport when a
  draw names none (the 1 by 1 pbuffer's at first), where the WebGPU back end uses the whole target;
  the interface does not say what an empty `viewports` means. And its read back later calls
  `glGetBufferSubData`, which ANGLE's GLES lacks (`read_back_now` works).

## Interface changes

- Run 29, `CompareFn`'s variants numbered 1 to 8 (Direct3D's numbers) as the codes
  `HelperValue::EarlyTests` holds, zero there a test that is off (its doc said "u32 codes" without
  naming them), and `EARLY_STENCIL_BINDING`'s doc says a front binds any single-sampled unsigned
  texture there for a format without stencil (WebGPU wants every declared binding bound; the
  program reads it only where the stencil test is on). Commit "interface: numbered the compare
  functions ...", which also gives the shared line's new slot test the `stride` field.

- Run 28, `VariantFlags::STORAGE_OFFSET` and `ReflectedBinding::stride` (commit "interface: ...";
  plan text in 4.5): `HelperValue::StorageOffset` existed with nothing asking for it; storage views
  at any first element (Direct3D 12's buffer_srv, the fronts' refused offsets) need the program to add
  it, and the front needs the program's element size to choose where to bind from.

- Run 26, `VariantKey::texture_layers: Vec<(u32, u32)>` and `AddedBinding::TextureLayer(slot,
  layer)` (commit "interface: gave the variant key the layer counts of multisampled array
  textures ..."; plan text in 4.2 and 6.4): plan 4.2 has fronts hold a multisampled array as a
  texture per layer, which a program can only read through a binding per layer.
- Run 26, `VariantKey::swizzled: Vec<(u32, [ViewComponent; 4])>`, `ViewComponent` and
  `SlowPath::SwizzleInShader` (plan text in 6.4): the default for back ends without
  `VIEW_SWIZZLE` (wgpu 30 has none), as the Direct3D 11 and 12 tracks asked.
- Run 26, `Reflection::slow_paths: Vec<SlowPath>` (with serde derives on `SlowPath`) and
  `SlowPath::AtomicExchangeLoop`: slow paths a program takes by itself whatever its variant, so the
  front can count and warn of them.

- Run 24, `HelperValue::Layer` and `HelperValue::Viewport` (commit "interface: gave fragment
  programs the layer and viewport of the draw's run as helper values"; plan text in 4.12's layered
  drawing): a pixel shader reading its layer or viewport index needs the run's, which only the
  front knows (`LayerSplit`, `MultiViewportSplit`), and zero where no earlier stage writes one;
  WebGPU has no such fragment input. Both fronts refuse helper values they do not write, so they
  build unchanged and refuse such shaders by name until they write them.

- Run 22, `ReflectedBinding::slot()` and the per-slot fields of `VariantKey`, the sampled pairs and
  `AddedBinding::ConstantsAs*` named by slot (commit "interface: named variant slots by a binding's
  place in its kind's range ..."; plan text in 6.4's Direct3D 12 sets): under Direct3D 12's
  numbering registers can be any size and repeat across spaces, so a bit per register cannot name
  them; the slot is the register under Direct3D 11's.

- Run 20, `DOMAIN_PATCHES_BINDING` (13) and `DOMAIN_POINTS_BINDING` (14) in the helper set (commit
  "interface: named the bindings in the helper set where a domain program reads ..."; plan text in
  6.4's set 3 list): a domain program, the final draw's vertex program, reads the hull stage's
  patches and the tessellator's points, which the front binds there for the stage helper.
- Run 20, `StageExtras::Hull` gained `point_locations` and `constant_locations` (commit "interface:
  gave a hull shader's reflection the locations it writes per control point and per patch"): the
  stage helper sizes the patch records from them.
- Run 18, `HelperValue::TexelLayout { slot }` given the view's width, height and layers or depth
  (five u32 in two 16-byte rows; commit "interface: gave the texel layout of a storage view read
  through a buffer the view's size ..."; plan text in 4.12): bounds and size queries need them, and
  the row pitch is not the width.
- Run 18, `VariantKey::texel_formats: Vec<(u32, Format)>` and `HelperValue::TexelBufferView { set,
  binding }` (commit "interface: gave the variant key the formats of typed buffers at texture slots
  ..."), then `TEXEL_BUFFER_TEXTURE_WIDTH` (2048) and the texture route's documentation (commit
  "interface: laid out the texture a typed buffer is read through on webgl 2 ..."); plan text in
  4.5's variant paragraph and 6.4's helper values: a program decoding a typed buffer must know the
  bound view's format, place and length, which only the front knows.

- Run 15, `HelperValue::TextureComponents { texture }` (commit "interface: gave the helper constants
  the components of a texture's format ..."; plan text in 6.4's list of helper values): a border
  colour applied in the program is read as a texel of the view's format, which only the front
  knows; a per-texture value, since one sampler serves textures of several formats.

- Run 14, `CONSTANTS_TEXTURE_WIDTH` (256) and `AddedBinding::ConstantsAsTexture`'s texture described
  (commit "interface: laid out the texture a constant buffer is read through ..."; plan text in 4.12):
  the front's copy and the program's fetch must agree on where each row lies.

- Run 14, `HelperValue::PulledStream { input }` (was `{ stream }`, three u32) as four u32 (offset,
  stride, divisor, bias), and `AddedBinding::PulledStream(n)` documented as the binding of the input
  at location n (commit "interface: gave each pulled vertex input its own binding ..."; plan text in
  4.12): the program then does not depend on how inputs share streams, and a step rate's first
  instance folds into the bias, so no base is needed.

- Run 13, `VariantKey::linked: LinkedValues { location: u32, clip_distances: u8 }` (commit
  "interface: gave the variant key where the values clip distances and depth clamping add pass
  ..."): the two programs are converted apart and must agree where the added values travel; plan
  text in 4.12. And `HelperValue::SamplerState`'s codes numbered as the variants' order of `Filter`
  and `AddressMode` (doc only).

- Run 9, two commits touching only `crates/wcontainers-gpu` (and the lock): serde `Serialize` and
  `Deserialize` derives on the variant key, the reflection and every type they hold (`Format`,
  `VertexFormat`, `ShaderStage` included), and on the state blocks of `state.rs` and `CompareFn`, as
  section 8 plans for the cache's postcard records.

- `VariantKey::output_interpolation: Vec<(u32, Interpolation)>` (commit "interface: gave the variant
  key the interpolation ..."): how the pixel program interpolates each vertex output, by location,
  where not `Perspective`; the vertex program writes them so. A variant field, as 4.4 has all
  rewrites that depend on what else is bound; plan text in section 4 says so.
