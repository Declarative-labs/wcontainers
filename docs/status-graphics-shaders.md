# Status: graphics, shaders track (G6)

Branch `phase-2-graphics-shaders`. Plan: `docs/phase-2-graphics.md`, sections 6, 7, 8 and G6 in 12.

## Where things stand

Milestone G6a (first slice): done in run 3. Run 3 merged origin/phase-2-graphics and every
origin/phase-2-graphics-* branch again (only Cargo.lock conflicted; the shared line's was taken).

Done:
- naga carried in the tree with the GLSL writer changes G3 needs (below). Other tracks get it by
  merging this branch: the workspace `Cargo.toml` patches `naga` to `third_party/naga`.
- dxbc-spirv built into `crates/wcontainers-shaders` (git submodule, upstream, pinned) natively and
  for WebAssembly (`wasm32-wasip1`, wasi-sdk 25).
- `convert` (6.4) for DXBC by the direct route: dxbc-spirv's legalised IR, serialised, read in Rust
  (`src/ir.rs`) and lowered straight into naga (`src/lower.rs`). Vertex, pixel and compute shaders.
  Every corpus shader converts or fails with a named reason; the WGSL is read back and validated by
  naga with WebGPU's core capabilities, the GLSL ES 3.00 compiled by glslangValidator.
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
  Through the direct3d-12 track's d3d12.dll only the triangle passes, and that front does not call
  `convert` yet (it answers "DXIL shaders need the shaders crate's converter" for every other
  shader), so DXIL is proven by `tests/draw.rs` instead.
- dxil-spirv built in too (submodule, pinned), and `convert` for DXIL: dxil-spirv's SPIR-V, read by
  naga's SPIR-V reader (`src/dxil.rs`), then given 6.4's binding rule. 11 of the rig's 14 DXIL shaders
  convert, WGSL and GLSL ES both compile in Chrome; the other 3 stop at the reader's known gaps
  (6.2): typed buffers ("unsupported image dimension", twice) and a storage image without a format.
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

Next (G6b and on, in order):
- DXIL inputs and outputs as the DXBC route lays them out: one four-component value per register
  (now each value keeps its own width, so a Direct3D 12 vertex shader writing a float4 that a pixel
  shader reads as float2 fails WebGPU's pipeline check), and elements dxc packs into one register
  (dxil-spirv gives them a SPIR-V component; naga's reader drops it, so two values claim one
  location). Both fit one wrapper, the one `draw_bases` already makes.
- DXIL binding numbers by root signature (6.4, `VariantKey::layout`); today Direct3D 11's rule.
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
  implemented now: `draw_base` (`None`, `Helpers`). Asking for any other field's rewrite is not yet
  honoured (constants as storage or texture are refused as `Unsupported("ConstantsVariant")`).
- Bindings follow 6.4 for Direct3D 10 and 11: set 0 for vertex and compute shaders' slots, set 1 for
  pixel shaders' (including their UAVs), binding = `SlotKind::first_binding()` + register. Each is
  listed in `reflection.bindings` (kind, register, set, binding). Resource arrays and slots beyond a
  kind's range are refused.
- Inputs and outputs: every user value lives at location = its register, as a four-component value
  of its scalar type (so a vertex and a pixel shader converted apart always match, whatever
  components each uses). `reflection.inputs` and `outputs` give semantic, index, register, mask,
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
- Constant buffers are declared with the size the bytecode gives them, so binding the program's own
  buffer (or any range at least that long) satisfies WebGPU.
- Errors: `Rejected` (dxbc-spirv or dxil-spirv refused the bytecode, with its message),
  `Unsupported(name)` (a named feature not provided yet, for example `GeometryShader`, `StorageImage`,
  `TypedBuffer`, `Atomics`), `Reader(text)` (a gap of naga's SPIR-V reader, DXIL only), `Malformed`
  and `Invalid` (our bugs: the corpus test fails on any of these).

## Measurements

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
- rig DXIL: WGSL 11 of 11, GLSL ES 11 of 11 (measured before run 2's DXIL entry point wrapper; the
  re-run after it was started at the end of run 2 and its result not read). Own: WGSL 5 of 5, GLSL ES 3 of 5 (bitfield operations).
- Each run of the 196 files' compile took about 2 s of GPU job time.

All on the Linux laptop (i7-8750H, Ubuntu 20.04), load average below 4 during the runs, 2026-11-17.
Counts only, no timings.

Direct route (this crate) on the plan's 191-shader DXBC corpus (`WCONTAINERS_SHADER_CORPUS`):
- 127 convert and their WGSL validates (naga, WebGPU core capabilities).
- Of those, 100 are written as GLSL ES 3.00 by naga and 85 compile in glslangValidator 8.13. The 15
  that do not: bitfield extract, insert, reverse and find-MSB (8), `textureGather` (4),
  `gl_SampleID` and `gl_SampleMask` (2, which need OES_sample_variables, which naga does not enable;
  Chrome's verdict, with the extensions the back end enables, is still to run), and one more. The 27
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
Not yet sent upstream.

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
- Unit and corpus tests: `tools/graphics-rig/linux 'WCONTAINERS_SHADER_CORPUS=$HOME/wcontainers-graphics/corpus/dxbc
  WCONTAINERS_SHADERS_OUT=$HOME/wcontainers-graphics/out GLSLANG=glslangValidator cargo test -p
  wcontainers-shaders -- --nocapture'`. Prints the verdict counts; the written WGSL and GLSL and
  `verdicts.txt` land in `~/wcontainers-graphics/out/{rig,corpus}`. Without `WCONTAINERS_SHADER_CORPUS`
  only the rig's shaders run; without `GLSLANG` the GLSL is written but not compiled.
- WebAssembly: add `WASI_SDK_PATH=$HOME/wcontainers-graphics/tools/wasi-sdk` and `--target
  wasm32-wasip1`; run tests with `~/wcontainers-graphics/tools/wasmtime/wasmtime run --dir
  corpus/dxbc::/corpus --env WCONTAINERS_SHADER_CORPUS=/corpus <test>.wasm`.
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
- `blake3` 1.8: as in the plan's section 3.
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

- naga is carried as a copy in the tree (`third_party/naga`, patched in by the workspace), not as a fork
  by git revision (6.2): a fork needs a new repository on the owner's account, and this track commits
  only to its own branch. Moving to a git revision later changes one line of the workspace manifest.
- `isnan` is lowered by `convert` itself (a bit test on the float's bits), not by the WGSL back end
  (6.5): it is exact on every back end and needs no back-end fix.
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

- WebGPU requires a vertex output's interpolation to equal the pixel input's; a vertex shader converted
  alone writes floats as perspective, center. A pixel shader reading `noperspective`, `centroid` or
  `sample` inputs will fail pipeline creation until `VariantKey` carries the pixel shader's
  interpolation per location (an interface change to propose).
- Direct3D links stages by register here; programs whose vertex outputs and pixel inputs use different
  registers for one semantic need remapping by semantic (also a variant to add).
- Out-of-bounds texture loads give what WebGPU's robustness gives, not Direct3D's zero. So does a
  dynamically indexed constant read past the size the bytecode declares (WebGPU clamps the index).
- Found while drawing, for other tracks: the WebGL 2 back end keeps GL's previous viewport when a
  draw names none (the 1 by 1 pbuffer's at first), where the WebGPU back end uses the whole target;
  the interface does not say what an empty `viewports` means. And its read back later calls
  `glGetBufferSubData`, which ANGLE's GLES lacks (`read_back_now` works).

## Interface changes

None.
