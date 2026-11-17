# Status: graphics, shaders track (G6)

Branch `phase-2-graphics-shaders`. Plan: `docs/phase-2-graphics.md`, sections 6, 7, 8 and G6 in 12.

## Where things stand

Milestone G6a (first slice): done in run 3; runs 4, 5 and 6 fixed what the independent checks of runs
3, 4 and 5 found (listed under "Run 4", "Run 5" and "Run 6" below). Run 6 merged origin/phase-2-graphics
and every origin/phase-2-graphics-* branch again (no conflicts). Run 6's fixes are in and tested on
the laptop; their drawing on the desktop is not done yet (first item of Next).

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
- Not finished: drawing the two fixes. The desktop refused every SSH session for the last hour of
  the run (connection reset after the key exchange), so nothing could be compiled or drawn there.
  Written and committed: `tests/own/route.hlsl` (`ps_rows`: a constant buffer indexed by the pixel's
  column, bound at its declared 80 bytes; `ps_clip`: discards inside a loop, then an
  implicit-derivative sample) and `compile.cmd` lines that compile it and `ps_textured` (`clip`) by
  fxc and dxc. Not yet compiled; the draw cases for them are in scratch
  `graphics-shaders/run6/draw-cases.patch` (apply it after compiling, then add the verdicts to
  `tests/expected/own.txt` and `own-dxil.txt`).
- Fuzz: the check's run (seed 31337, 6000 cases) found one DXIL case killed for memory and showed that
  under an address-space cap the DXBC memory cases end as `Rejected` (the C++ `bad_alloc` caught at
  the boundary); recorded under Known gaps. Its 26 refusals by our own checks (a caught panic of
  naga's layouter on a too-large type, a lowering of mutated IR giving a u32 times f32 multiply) are
  refusals of signed but mutated bytecode, as Known gaps says.

Next (G6b and on, in order):
- First, finish run 6 on the desktop: run `tests/own/compile.cmd` through `tools/graphics-rig/remote`,
  copy back `ps_rows.*`, `ps_clip.*` and `ps_textured.dxil`, apply the scratch patch above, record
  their verdicts, run the corpus suite, Chrome's `web/check` and `tests/draw.rs` (the DXIL builds of
  `ps_rows`, `ps_clip`, `ps_textured`, and `ps_cases` now on WebGL 2 too), then the fuzz again (the
  new own files change its cases).
- DXIL inputs and outputs as the DXBC route lays them out: one four-component value per register
  (now each value keeps its own width, so a Direct3D 12 vertex shader writing a float4 that a pixel
  shader reads as float2 fails WebGPU's pipeline check), and elements dxc packs into one register
  (dxil-spirv gives them a SPIR-V component; naga's reader drops it, so two values claim one
  location; refused as `PackedStageValues` until then). Both fit one wrapper, the one `draw_bases`
  already makes.
- DXIL binding numbers by root signature (6.4, `VariantKey::layout`); today Direct3D 11's rule.
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

```rust
use wcontainers_shaders::{convert, Source, ConvertError};
let converted = convert(Source::Dxbc(bytecode), &variant_key, None)?;
let module: &wcontainers_gpu::ShaderModule = &converted.modules[0];  // hand to Backend::create_shader
let reflection = &converted.reflection;
```
- `variant_key` is `wcontainers_gpu::VariantKey`; `VariantKey::default()` for the plain program. Fields
  built now: `draw_base` (`None`, `Helpers`) and `output_interpolation`. Every other field set to
  anything but its default, and a root signature, is refused by name: `Unsupported` with
  `VariantFlags`, `FilterInShader`, `BorderInShader`, `MirrorOnceInShader`, `LodBiasInShader`,
  `ConstantsAsStorage`, `ConstantsAsTexture`, `VertexPulledInShader`, `StorageFormats`,
  `TexelBufferRoute`, `StoragePacked`, `TexelsThroughBuffer`, `NormAsInteger`, `TargetsInShader`,
  `MinLodClamp`, `SampleCount` or `RootSignatureLayout`.
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
  numbered as for Direct3D 11 for now (stage set, kind range + register; `space` kept in the
  reflection), not yet by root signature. `draw_base` `None` (bases read as zero) and `Helpers` (the
  vertex helper block as for DXBC above) are taken; `Arguments` is
  `Unsupported("DrawBaseFromArguments")` on both routes. Inputs and outputs are filled as for DXBC
  (semantic names from the container's signatures, location = register); a pixel shader's unread
  `SV_Position` may be listed where the DXBC route leaves it out. Sampled pairs are listed as for
  DXBC, by register (a pair's register spaces are not in the pair).
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
- Fuzz (long, one command): `tools/graphics-rig/linux sh -c 'WCONTAINERS_SHADER_CORPUS=$HOME/wcontainers-graphics/corpus/dxbc
  WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000 cargo test -p wcontainers-shaders --test fuzz
  -- --ignored --nocapture'`. Each case is printed before it runs; `WCONTAINERS_FUZZ_FIRST=n` starts
  at case n. It ends at the first case that crashes (Known gaps); scratch
  `graphics-shaders/fuzz/resume.sh <seed> <cases>`, run on the laptop after one build, restarts past
  each crash and lists them; the cases our own checks refused go to
  `~/wcontainers-graphics/fuzz-ours-<seed>.txt`.
- WebAssembly in one command: scratch `graphics-shaders/run5/wasm.sh` (copied to
  `~/wcontainers-graphics/wasm-run5.sh`), run through `tools/graphics-rig/linux bash -c
  '$HOME/wcontainers-graphics/wasm-run5.sh'` after a native corpus run has filled
  `~/wcontainers-graphics/out`: builds `wasm_imports` and the corpus test, runs the test under wasmtime
  and counts the written files that differ from the native ones.
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
- The content key is BLAKE3 over the source, `CONVERTER_VERSION` and the variant key's debug text, until
  the cache (section 8) gives the variant key a serialised form.
- The WebAssembly build imports ten WASI functions, not two (above).
- The corpus test writes GLSL with the WebGL 2 back end's options copied (its `write_glsl` is private
  to that crate); if that back end changes them, the test follows by hand.
- dxil-spirv's converter library is not free of dxbc-spirv as 6.2 says: its bitcode module
  (`bc/module_dxbc_ir.cpp`) reads dxbc-spirv's IR. It is built against our one copy of dxbc-spirv
  (newer than the commit dxil-spirv pins; it compiles), and all of dxil-spirv is built as C++17, as
  its CMake does for that file.

## Known gaps found so far

- Signed but malformed DXIL can end the process natively: dxil-spirv's bitcode reader faults or calls
  `std::terminate` (`bc/cast.hpp`) on it (10 of 3000 fuzz cases in run 5, 15 in run 4,
  Measurements); malformed DXBC exhausted memory in 3 of 3000 (1 in run 4), and the check of run 5
  found one DXIL case too (seed 31337, case 4995). The exhaustion is inside the converters: with the
  address space capped (`ulimit -v 3000000`) the DXBC cases end as `Rejected`, the C++ `bad_alloc`
  caught at the boundary, but natively nothing bounds it without such a cap; on WebAssembly the
  module's memory limit ends the instance. Only bytecode whose checksum matches reaches the converters, so this needs a
  program that signs malformed bytecode. dxil-spirv is used unmodified, so the fix belongs upstream
  (not yet reported). `tests/fuzz.rs` stays failing at those cases.
- Mutated but signed bytecode that dxbc-spirv accepts can give IR our lowering refuses (`Malformed`)
  or a module naga's validation refuses (`Invalid`, for example a constant index past an array's
  end, which Direct3D would read as zero): refused, not converted (Measurements).
- GLSL ES 3.00 has no `noperspective`: a pixel shader reading one is not written for WebGL 2.
- The WebGL 2 back end's fragment depth: check run 3 measured a pixel shader reading `SV_Position.z`
  wrong through it (z arrives as (z+1)/2); that is the WebGL track's clip depth. `ps_depth` is drawn
  through it in `tests/draw.rs` (result in Measurements).
- Direct3D links stages by register here; programs whose vertex outputs and pixel inputs use different
  registers for one semantic need remapping by semantic (also a variant to add).
- Out-of-bounds texture loads give what WebGPU's robustness gives, not Direct3D's zero. So does a
  dynamically indexed constant read past the size the bytecode declares (WebGPU clamps the index).
- Found while drawing, for other tracks: the WebGL 2 back end keeps GL's previous viewport when a
  draw names none (the 1 by 1 pbuffer's at first), where the WebGPU back end uses the whole target;
  the interface does not say what an empty `viewports` means. And its read back later calls
  `glGetBufferSubData`, which ANGLE's GLES lacks (`read_back_now` works).

## Interface changes

- `VariantKey::output_interpolation: Vec<(u32, Interpolation)>` (commit "interface: gave the variant
  key the interpolation ..."): how the pixel program interpolates each vertex output, by location,
  where not `Perspective`; the vertex program writes them so. A variant field, as 4.4 has all
  rewrites that depend on what else is bound; plan text in section 4 says so.
