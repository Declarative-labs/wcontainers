# Graphics track status

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.

## Done

- G0, run 1 (2026-11-17): the plan, and `crates/wcontainers-gpu` with the interface as real Rust.
- G0, run 2 (2026-11-17): every finding of G0's independent check fixed, in the plan and the crate:
  - Feature levels (plan 9) follow Microsoft's tables: Direct3D 11 on WebGPU reports 11_0 (11_1 needs
    logic ops, target-independent rasterisation and forced sample counts up to 16, none exact on
    WebGPU); Direct3D 12 on WebGPU reports 11_0 with binding tier 1; Direct3D 11 on WebGL 2 reports
    10_0 with multisample load absent for every format. The inexact `MultisampleReadCopy` is gone.
  - Exact occlusion counts by a counting replay (plan 4.12, `OcclusionCountReplay`); `QueryKind` has
    `OcclusionAny` (every back end) and `Occlusion` (exact, needs `OCCLUSION_COUNT`). Queries spanning
    passes, other query kinds, `DrawAuto` and timestamps between draws have stated routes.
  - Binding layouts for WebGPU (plan 4.5): derived from the program, plus `BindingLayoutKey` for what
    the bound views and samplers decide; `FilterInShader` for unfilterable formats sampled linearly;
    `PrepareDesc::layouts` and the cache carry the keys. Binding numbers are fixed per kind.
  - Guest memory lifetime (plan 4.6): bytes the API lets the program reuse are copied once by the
    stand-in into the call buffer or a staging ring; mapped memory is front-owned shadow blocks. The
    page-tracking source is now described correctly (first-write tracking of shared pages).
  - All fast paths are in the crate with defaults built from the base and tested: `clear_rects`,
    `generate_mips` (2D, arrays, cubes, 3D) and `write_buffer_ring`, beside the earlier ones. Their
    built-in programs are WGSL read by naga's WGSL reader. A `helpers()` base method holds their state.
  - Fans keep the provoking vertex the API names (new `provoking` argument); loops are line lists, so a
    guest's largest index is never taken as a restart; depth-stencil passes name only the aspects the
    format has (`AspectOps`); `ColorAttachment::depth_slice`; `PassDesc::empty` for passes without
    targets.
  - New capability bits `MIRROR_ONCE`, `CLIP_DEPTH_ZERO_ONE`; new limits `color_bytes_per_sample`,
    `buffer_copy_alignment`; every vertex format of every component count; texture writes take a
    `WriteMode`; buffer clears take a pattern; handles are `Clone`.
  - New named slow paths: `MirrorOnceInShader`, `InstanceRateInShader`, `OcclusionCountReplay`,
    `TimestampSplitPass`, `UnalignedBufferEdge`, `TargetsSplit`, `FilterInShader`, `StreamOutRepacked`,
    `LogicOpPerPrimitive`, `LayerSplit`, `VertexFetchZeroed`, and the counted refusal
    `BindingsBeyondLimit` (49 reasons in all).
  - Licences of what dxil-spirv ships, its C++ standard, the Chrome flag used for verdicts, and the
    unmeasured direct DXBC lowering are written into the plan (sections 3, 6.2, 6.3, 13).

## Partly done

- Nothing in G0. Designed for G1, not in the crate yet: the shared pass tracker (plan 4.7) and the
  occlusion counter helper (plan 4.12). G6 writes the `rewrite` module (plan 4.5).

## Next

- G0's independent check, again.
- G1 to G6 as in plan section 12; they can run in parallel. G1 is short and every other milestone builds
  its tests on it, so start it first.

## How to run

- Interface crate: `nix develop -c cargo test -p wcontainers-gpu` (13 tests) and
  `nix develop -c cargo clippy -p wcontainers-gpu --all-targets -- -D warnings`, and
  `RUSTDOCFLAGS="-D warnings" nix develop -c cargo doc -p wcontainers-gpu --no-deps`. It also builds for
  `wasm32-unknown-unknown`.
- Shader corpus (scratch, `/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/graphics`):
  - dxbc-spirv built with `nix shell nixpkgs#meson nixpkgs#ninja nixpkgs#clang -c sh -c 'meson setup
    build/dxbc-spirv -Denable_tools=true -Dbuildtype=release && ninja -C build/dxbc-spirv'` from
    `src/dxbc-spirv`. The scratch copy of `tools/dxbc_compiler.cpp` was changed to drop the float-control
    modes (round and denorm modes set to none), as plan 6.3 says.
  - dxil-spirv built with CMake and Ninja from `src/dxil-spirv` into `build/dxil-spirv`.
  - Corpus: `python3 -I tools/extract_dxbc.py downloads/wine-tests/d3d11.c corpus/dxbc`, then
    `build/dxbc-spirv/tools/dxbc_compiler X.dxbc --spv corpus/spv2/X.spv` per file.
  - Naga and Chrome verdicts: from the checkout,
    `nix develop -c <scratch>/tools/run-corpus.sh <scratch>/corpus/spv2 <scratch>/corpus/out-via-spv2
    <scratch>/corpus/dxbc-via-spv` writes `corpus/dxbc-via-spv-naga.txt` and `-chrome.txt`. Chrome runs
    headless with `--enable-unsafe-webgpu`.
  - DXIL: HLSL in `corpus/hlsl`, DXC from `nix build nixpkgs#directx-shader-compiler`, dxil-spirv with
    `--no-bda --ssbo-uav --ssbo-srv` (plus `--bindless` for the SM 6.6 shader), then `run-corpus.sh` on
    `corpus/dxil-spv`.
- Microsoft's feature level and format tables used for plan 9 are in `downloads/msdocs/`.

## Measurements

- No timings were taken in G0. The shader verdicts (counts only) were taken on the Mac (Apple M5) on
  2026-11-17 with load average 12.51, 20.81, 20.69 when the builds started; Chrome as installed, headless,
  through the flake's Playwright. Results: plan section 6.3, files in `corpus/`. Run 2 took no new
  measurements (load average 13.02, 14.76, 17.39 when its tests ran).

## Dependencies added and their licences

- `naga` 30.0.2 (MIT OR Apache-2.0), with its `wgsl-in` feature since run 2, used by `wcontainers-gpu`.
  Its tree, from `cargo tree`: all MIT or Apache-2.0 except `foldhash` (Zlib), `zerocopy`
  (BSD-2-Clause OR Apache-2.0 OR MIT), `unicode-ident` ((MIT OR Apache-2.0) AND Unicode-3.0),
  `codespan-reporting` (Apache-2.0), `libm` (MIT).
- Read in their repositories, not yet dependencies: dxbc-spirv MIT (2025 Philip Rebohle; SPIRV-Headers
  submodule, Khronos MIT-style); dxil-spirv MIT (Hans-Kristian Arntzen for Valve), shipping glslang's
  SPIR-V builder (BSD-3-Clause), RenderDoc's bitcode reader (MIT) and SPIRV-Headers; Fossilize MIT (not
  used).
- Test-only, in scratch, never shipped: Wine's d3d11 test source (LGPL-2.1) as DXBC test data; DXC
  (NCSA) to make DXIL test shaders.

## Departures from the plan's brief

- Writing WGSL and GLSL ES 3.00 from the common form happens in each back end (with naga's writers and
  that language's fixes), not in `wcontainers-shaders` as the brief's crate list says. Reason: back ends
  depend only on the interface crate, and the writing is the back end's own language. The shaders crate
  converts into the common form, lowers extra stages, and owns the cache.
- Fronts depend on the interface plus the two shared helper crates (`wcontainers-shaders`,
  `wcontainers-directx-graphics-infrastructure`); "depend only on the interface crate" is read as "never
  on each other or on a back end".
- Geometry, hull and domain stages and stream output are lowered by the shaders crate's stage helper and
  reach back ends as ordinary dispatches and draws; the interface has no method for them.
- DXIL reaches the common form through dxil-spirv's SPIR-V and naga's SPIR-V reader, which needs fixes
  (made in a naga fork pinned by revision until upstream takes them).
- Target formats are not a block a draw names (the brief lists targets among the blocks): the pass names
  its targets, so a seventh handle per draw would repeat it. `prepare` takes them as `TargetLayout`.
- Direct3D 11 and 12 on WebGPU report feature level 11_0, not the top level the brief asks for: 11_1
  requires logic ops, target-independent rasterisation and forced sample counts up to 16 (Direct3D 11)
  and binding tier 3 (Direct3D 12), none of which WebGPU can serve exactly.
- Multisample counts per format fall short of Microsoft's 10_1 and 11_0 format tables (8 samples on 47
  formats at 11_0; 4 samples on `R32_UINT` and others). They are reported through the API's per-format
  queries; no exact emulation exists. This is the one place reported answers fall short of a level's
  table, and the owner should know.
- Two listed precision losses remain: denormal handling in shaders, and Direct3D clip depth on WebGL 2
  without EXT_clip_control (6.8% of WebGL 2 browsers). Indexed vertex fetch past a buffer's end gets
  WebGPU's robust-access value, not zero.
- A program needing more sampled textures than the back end has is refused at creation
  (`BindingsBeyondLimit`, counted and warned); no exact route exists.
- A new crate `wcontainers-directx` (not in the brief's list) builds the one library that holds the
  Direct3D family, so DXGI, Direct3D 11 and 12 share one back end and one device thread; `d3d11.dll`,
  `d3d12.dll` and `dxgi.dll` forward to it (plan 10).
- `wcontainers-gpu` gained naga's `wgsl-in` feature, for the built-in programs of the fast-path
  defaults.
- The workspace root `Cargo.toml` gained the `naga` workspace dependency (outside the crates this track
  owns, needed to add one).

## Known failures

- None in the code.
