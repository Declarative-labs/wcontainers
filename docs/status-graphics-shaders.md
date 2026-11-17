# Status: graphics, shaders track (G6)

Branch `phase-2-graphics-shaders`. Plan: `docs/phase-2-graphics.md`, sections 6, 7, 8 and G6 in 12.

## Where things stand

Milestone G6a (first slice), run 1, in progress.

Done:
- naga carried in the tree with the GLSL writer changes G3 needs (below). Other tracks get it by
  merging this branch: the workspace `Cargo.toml` patches `naga` to `third_party/naga`.
- dxbc-spirv built into `crates/wcontainers-shaders` (git submodule, upstream, pinned) natively and
  for WebAssembly (`wasm32-wasip1`, wasi-sdk 25).
- `convert` (6.4) for DXBC by the direct route: dxbc-spirv's legalised IR, serialised, read in Rust
  (`src/ir.rs`) and lowered straight into naga (`src/lower.rs`). Vertex, pixel and compute shaders.
  Every corpus shader converts or fails with a named reason; the WGSL is read back and validated by
  naga with WebGPU's core capabilities, the GLSL ES 3.00 compiled by glslangValidator.
- The module is the same on every run and host: the corpus test converts each shader twice and
  compares, and the WebAssembly build's output files equal the native build's byte for byte.
- dxil-spirv built in too (submodule, pinned), and `convert` for DXIL: dxil-spirv's SPIR-V, read by
  naga's SPIR-V reader (`src/dxil.rs`), then given 6.4's binding rule. 11 of the rig's 14 DXIL shaders
  convert, WGSL valid and GLSL ES compiled by glslang; the other 3 stop at the reader's known gaps
  (6.2): typed buffers ("unsupported image dimension", twice) and a storage image without a format.

Not done in this milestone yet (in order): the five own HLSL shaders compiled on the desktop and
checked for meaning; Chrome's own compile of the written WGSL and GLSL (the plan's final verdict; so
far naga and glslang only); the rig's shaders drawn through a back end (no back end branch is on the
remote yet: only phase-2-graphics and phase-2-graphics-rig); the DXIL draw bases from the helpers.

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
  reflection), not yet by root signature. Only `draw_base: None` is taken (the bases read as zero);
  anything else is `Unsupported("DxilDrawBase")`. The reflection has bindings and extras; inputs,
  outputs and sampled pairs are still empty for DXIL.
- Errors: `Rejected` (dxbc-spirv or dxil-spirv refused the bytecode, with its message),
  `Unsupported(name)` (a named feature not provided yet, for example `GeometryShader`, `StorageImage`,
  `TypedBuffer`, `Atomics`), `Reader(text)` (a gap of naga's SPIR-V reader, DXIL only), `Malformed`
  and `Invalid` (our bugs: the corpus test fails on any of these).

## Measurements

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
converters linked) is 4,842,413 bytes (2,356,683 with dxbc-spirv alone) and imports from `wasi_snapshot_preview1`: environ_get, environ_sizes_get, fd_close,
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
Not yet sent upstream.

## How to run

- Linux laptop (no GPU), one command from the Mac: `tools/graphics-rig/linux <command>`. It copies the
  working tree (submodules included) to `~/wcontainers-graphics/src/<branch>` and builds into
  `~/wcontainers-graphics/target/<branch>`, with 5 cargo jobs (2 when the laptop's load is 8 or more).
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
- Out-of-bounds texture loads give what WebGPU's robustness gives, not Direct3D's zero.

## Interface changes

None.
