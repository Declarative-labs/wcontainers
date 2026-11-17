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

- G0, run 3 (2026-11-17): every finding of the second independent check fixed:
  - Feature levels (plan 9) now state what each level needs of a back end's numbers (Microsoft's table
    and the SDK headers, read on the desktop) and how each is met: texture sizes are conditions (11_0
    needs 16384 2D and cube, 2048 3D and layers; else 10_1, 10_0); input slots past the limits are
    fetched in the program (`VertexPulledInShader`); constant buffers past WebGPU's uniform slots are
    read as storage (`ConstantsAsStorage`), past WebGL 2's blocks or 16 KiB block size through a texture
    (`ConstantsAsTexture`); stage values are packed, then refused (`VaryingsBeyondLimit`); BC formats are
    decoded where not native (`BlockDecoded`, with `bcdec_rs`), with the blocks kept on the host for
    copies and read backs (`BlockCopyReadBack`). WebGL 2's 10_0 conditions now include 2048 3D, 512
    layers and the cube size.
  - Sampler LOD bias: removed from `SamplerDesc` (no browser sampler has one); fronts pass it in the
    helper constants and bind a variant (`LodBiasInShader`, plan 4.12).
  - Cube textures: `TextureDesc::cube` and `own_view()`; WebGL 2 picks the GL target from it; other view
    kinds come from a front-kept second texture (`ReinterpretCopy`). 1D textures removed: fronts make them
    2D, one texel high.
  - The brief's "build a shader with the most common states when it arrives" is written as step 2 of
    plan 4.4, with a combination tracker helper for G1.
  - The Direct3D family's module split: one module per back end, recorded as a departure (plan 2, 10).
  - Program variants moved from back ends to fronts and the shaders crate. `VariantKey`, `Reflection`,
    the helper constants (`HelperValue`) and the slot ranges (`SlotKind`) are now Rust in
    `wcontainers-gpu` (`reflect.rs`); the conversion and stage helper API is written out in plan 6.4.
    Milestones in plan 12 restated with what each needs; a rule for interface changes after G0.
  - Minor: `write_buffer_ring` aligns empty parts; `ViewportDepthFlip` for near beyond far;
    `ExecuteIndirect` in plan 5.2; texture UAV clears by dispatch; `DEPTH_BIAS_CLAMP` and
    `DepthBiasInShader` (was `LineBiasInShader`); the loop and fan doc strings corrected.
  - New capability bits `DEPTH_BIAS_CLAMP`, `BLOCK_COMPRESSION`; limit `max_texture_cube` (the 1D limit
    removed). 57 slow path reasons.

- G0, run 4 (2026-11-17): every finding of the third independent check fixed:
  - Texture atomics (required at 11_0 on `R32_UINT`, `R32_SINT`, exchange on `R32_FLOAT`): new
    `FormatUse::STORAGE_ATOMIC` (no browser format has it) and the exact route `AtomicsThroughBuffer`
    (the view's texels in a buffer, copied around other uses; plan 4.12).
  - 16-bit normalised and rendered snorm formats: native with `texture-formats-tier1` (WebGPU) or
    EXT_texture_norm16 and EXT_render_snorm (WebGL 2), otherwise `NormAsInteger` (stored as the integer
    format of the same bits, decoded, filtered and encoded in the program, blended in it:
    `BlendInShader`). 32-bit float blending, which 10_0 also requires, takes `BlendInShader` without
    `float32-blendable` or EXT_float_blend.
  - `STENCIL_PER_FACE` removed from WebGL 2's list (WebGL forbids it); `StencilFacesSplit` there.
  - Sample mask: new `Features::SAMPLE_MASK` (WebGPU); WebGL 2 writes it in the program with
    OES_sample_variables (`SampleMaskInShader`), and without it the Direct3D fronts report no
    multisampling.
  - Stencil sampling on WebGL 2: WEBGL_stencil_texturing, otherwise `StencilCopiedByDraw`. Every back end
    serves both aspects of a depth-stencil format (documented on `FormatUse::SAMPLE` and the view).
  - Per-target blends without OES_draw_buffers_indexed: `TargetsSplit`; `BlendDesc::same_for_all` doc
    corrected.
  - `VariantKey` now holds every back-end-dependent rewrite (`texel_buffers`, `storage_packed`,
    `atomics_through_buffer`, `decoded`, `targets_in_shader`, the sample-mask flag); `convert` reads
    nothing else, so cache keys name one module everywhere (plan 4.5, 8).
  - `clear_texture` now takes the texture, levels and layers and clears every 3D slice and every layer
    in a pass of its own; `generate_mips` reads the level above through whole-array, cube or cube-array
    views (new built-in programs) so it needs no `TEXTURE_VIEWS`. Tests for both. Built-in programs are
    now made one at a time when a default first draws with each, so WebGL 2 never builds the cube-array
    one.
  - Overlapping queries (`QueriesSegmented`, plan 4.7), append and counter views and
    `CopyStructureCount` (`SlotKind::Counter`, plan 5.1), copies within typeless groups
    (`CopyThroughBuffer`, `DepthWrittenByDraw`), WebGL 2 copies and read backs of formats it cannot
    attach (`HostCopyKept`), and surface formats and colour space (`FormatUse::PRESENT`, SDR only,
    plan 4.9).
  - Renamed: `LogicOpPerPrimitive` to `TargetReadPerPrimitive`, `BlockCopyReadBack` to
    `HostCopyReadBack`. 66 slow path reasons.

## Partly done

- Nothing in G0. Designed for G1, not in the crate yet: the pass tracker (plan 4.7), the occlusion
  counter (plan 4.12) and the combination tracker (plan 4.4).

## Next

- G0's independent check, again.
- G1 to G6 as in plan section 12; they can run in parallel. G1 is short and every other milestone builds
  its tests on it, so start it first.

## How to run

- Interface crate: `nix develop -c cargo test -p wcontainers-gpu` (20 tests) and
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
- Chrome's WebGPU limits and features and WebGL 2 parameters and extensions:
  `nix develop -c node <scratch>/tools/all-limits.mjs` (headless Chrome through the flake's Playwright);
  the run-3 output is `corpus/chrome-limits-run3.json`.

## Measurements

- No timings were taken in G0. The shader verdicts (counts only) were taken on the Mac (Apple M5) on
  2026-11-17 with load average 12.51, 20.81, 20.69 when the builds started; Chrome as installed, headless,
  through the flake's Playwright. Results: plan section 6.3, files in `corpus/`. Run 2 took no new
  measurements (load average 13.02, 14.76, 17.39 when its tests ran). Run 4 took no new measurements
  (load average 27.52, 27.98, 24.45 when its tests ran); facts it used were read in Microsoft's format
  tables in `downloads/msdocs/` and the WebGPU specification copy in `check-g0/spec/` (scratch).
- Run 3, on the Mac, load average 14.71, 16.16, 16.39: Chrome's adapter limits (plan 4.11), counts not
  timings. WebGPU: 16384 2D, 2048 3D and layers, 8 vertex buffers, 30 attributes, 12 uniform buffers, 10
  storage buffers, 48 sampled textures, 28 inter-stage slots, 64 immediate bytes, BC and BC sliced 3D
  present. WebGL 2: 16 attributes, 16 uniform blocks of 16384 bytes, 16 units, 2048 3D and layers,
  every extension the plan names. Direct3D's numbers (14 constant buffers, 32 input slots and elements,
  32 vertex outputs; 16 at 10_0; 512 layers at 10_0) read from the desktop's SDK 10.0.26100 headers,
  read only.

## Dependencies added and their licences

- `naga` 30.0.2 (MIT OR Apache-2.0), with its `wgsl-in` feature since run 2, used by `wcontainers-gpu`.
  Its tree, from `cargo tree`: all MIT or Apache-2.0 except `foldhash` (Zlib), `zerocopy`
  (BSD-2-Clause OR Apache-2.0 OR MIT), `unicode-ident` ((MIT OR Apache-2.0) AND Unicode-3.0),
  `codespan-reporting` (Apache-2.0), `libm` (MIT).
- Read in its repository, to be added by G2 and G3: `bcdec_rs` 0.2.1, MIT (Cargo.toml and the
  ScanMountGoat/image_dds LICENSE), a port of bcdec (MIT or Unlicense).
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
- Direct3D 11 and 12 on WebGPU report feature level 11_0 (when the adapter has 11_0's texture sizes;
  otherwise a lower level), not the top level the brief asks for: 11_1
  requires logic ops, target-independent rasterisation and forced sample counts up to 16 (Direct3D 11)
  and binding tier 3 (Direct3D 12), none of which WebGPU can serve exactly.
- Multisample counts per format fall short of Microsoft's 10_1 and 11_0 format tables (8 samples on 47
  formats at 11_0; 4 samples on `R32_UINT` and others; on WebGPU adapters without
  `texture-formats-tier1`, the 16-bit normalised formats too). They are reported through the API's
  per-format queries; no exact emulation exists. This is the one place reported answers fall short of a
  level's table, and the owner should know.
- On WebGL 2 without OES_sample_variables (10.3% of WebGL 2 browsers, nearly all Firefox) the Direct3D
  fronts report no multisampling at all, because a partial sample mask has no route there. With it,
  4 samples as before.
- WebGL 2's 10_0 also needs EXT_color_buffer_float (99.94%), since 10_0 renders to float formats.
- Two listed precision losses remain: denormal handling in shaders, and Direct3D clip depth on WebGL 2
  without EXT_clip_control (6.8% of WebGL 2 browsers). Indexed vertex fetch past a buffer's end gets
  WebGPU's robust-access value, not zero.
- A program needing more sampled textures than the back end has is refused at creation
  (`BindingsBeyondLimit`, counted and warned); no exact route exists.
- A new crate `wcontainers-directx` (not in the brief's list) builds the one library that holds the
  Direct3D family, so DXGI, Direct3D 11 and 12 share one back end and one device thread; `d3d11.dll`,
  `d3d12.dll` and `dxgi.dll` forward to it (plan 10).
- Bytes the API lets the program reuse when the call returns are copied once by the stand-in (plan 2
  rule 1, 4.6), not read in place as the brief says, because the device thread reads them only after the
  frame's call buffer crosses.
- The Direct3D family (DXGI, 10, 11, 12) is one module per back end, loaded at the first call into any
  of them, not one module per API as the brief says: they share one device and DXGI's objects, which
  separate modules could only do through run-time dispatch or separate devices (plan 10). The unused
  back end is never loaded. Its cost to a Direct3D 11 program is measured at G4.
- Program variants (filtering, border, mirror once, LOD bias, limits, clip depth) are chosen by fronts
  and made by the shaders crate; back ends never rewrite programs.
- One more counted refusal beside `BindingsBeyondLimit`: `VaryingsBeyondLimit` (more stage values
  than the back end has after packing). Texture sizes beyond the adapter's lower the reported level.
- `wcontainers-gpu` gained naga's `wgsl-in` feature, for the built-in programs of the fast-path
  defaults.
- The workspace root `Cargo.toml` gained the `naga` workspace dependency (outside the crates this track
  owns, needed to add one).

## Known failures

- None in the code.
