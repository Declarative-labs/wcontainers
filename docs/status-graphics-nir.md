# Status: graphics, NIR route (G8)

Branch phase-2-graphics-nir. Plan: docs/phase-2-graphics.md, sections 15.2, 15.3, 15.4 and 15.8 (G8).

## Where it stands

G8 run 2 (2026-11-19). Not complete.

Done:
- Mesa pinned and building on the rented x86-64 machine with llvmpipe and lavapipe (and Zink, whose
  NIR to SPIR-V compiler is candidate A's first step), by one command (How to run). Pin:
  `crates/wcontainers-nir/mesa/pin.toml` (mesa-26.3.1, commit fec8830e9c94); meson options:
  `crates/wcontainers-nir/mesa/options.ini`; script: `crates/wcontainers-nir/mesa/build`.
- `crates/wcontainers-nir`: the crate, its C bridge (`bridge/bridge.c`, `bridge.h`) and its link to
  Mesa's compiler libraries (`build.rs`). Built when `WCONTAINERS_MESA` names the Mesa build folder;
  without it the crate builds without the NIR parts (cfg `mesa`), so the workspace builds everywhere.
  `build.rs` takes everything about Mesa from Mesa's own build: the source folder from Meson's
  `meson-info.json`, the defines and C standard from `compile_commands.json` (those of Zink's
  `nir_to_spirv.c`), and the zlib and thread link arguments from `intro-dependencies.json`.
- Candidate A runs end to end on one program (`tests/route.rs`): SPIR-V (written by naga from WGSL) to
  NIR by `spirv_to_nir`, NIR's serialised bytes (`nir_serialize`, the cache key's source), then Zink's
  `ntv_shader_prepare` and `nir_to_spirv` in its native Vulkan mode (`ntv_info.is_native_vulkan`, SPIR-V
  1.3), read by naga's SPIR-V reader and validated. Nothing is compared yet.
- The corpus run (`tests/corpus`, a test target without the test harness, one command under How to
  run): every `.spv` under `WC_NIR_CORPUS`, each shader and route in a child process of its own (Mesa
  aborts on some inputs; a crash inside a route is that route's failure, with Mesa's last lines of
  errors as its reason), one JSON line a shader. Knobs: `WC_NIR_SHADERS` (a sample of that many,
  drawn with `WC_NIR_SEED`, default 0x5eed), `WC_NIR_JOBS`, `WC_NIR_RESULTS`, `WC_NIR_DUMP` (each
  route's intermediate form, for diagnosis). Today it converts only (no execution, no reference).
- The corpus's sources are pinned in `crates/wcontainers-nir/corpus/sources` and fetched by
  `corpus/fetch`. Only Sascha Willems' Vulkan samples so far (1005 SPIR-V files: GLSL, HLSL and Slang
  compiled by their authors).
- The NIR the routes take is shaped by one `nir_shader_compiler_options` in the bridge: what the
  common form lacks is lowered by NIR itself (fsat, extract and insert of bytes and words, carry and
  borrow, saturating adds, mul_high, all 64-bit integer operations, uniforms to a uniform block).
  The list is Zink's, which serves the same purpose for Vulkan; the drivers (G9, G10) must report
  the same options, or the NIR they hand over differs. Unused inputs, outputs and system values are
  removed.
- Candidate A's output is cleaned of `Index 0` decorations (Zink writes one on every fragment output;
  it says what no decoration says, and naga's reader took it for dual-source blending).

Candidate A on the corpus, conversion only (run 2, x86-64 machine, load 10 to 20 from other jobs,
192 children at once, so the times are not yet a measurement for the choice): 1005 shaders; 138
have a stage the common form lacks (ray tracing 81, tessellation 30, geometry 10, mesh and task 6,
nothing to convert); 9 crash Mesa's `spirv_to_nir` steps before any route (5 in
`nir_lower_vars_to_ssa` on HLSL's variable rate shading sample, 4 in `align` on the descriptor heap
samples); of the 858 left, 493 convert and validate, 365 fail. Their reasons, largest first:
- 161 `invalid id`: combined image samplers (`OpTypeSampledImage` variables), which naga's reader
  does not take. This is 15.4's combined image sampler rewrite; it has to happen in NIR (or SPIR-V)
  for this route, before naga reads anything.
- 74 bindings conflict: Zink splits a separate texture and sampler that the source put on one
  binding (HLSL's) onto one binding. Same rewrite: the common form's numbering (6.4) is not made yet.
- 43 `DrawParameters`: Zink declares the capability for `InstanceIndex`; naga's reader refuses it.
- 45 crashes in Zink's `nir_to_spirv` (26 + 5 `emit_deref_array` of an unsupported variable mode, 7
  `deref_buffer_array_length`, 3 sparse residency, 3 `nir_instr_as_intrinsic`, 2 barycentrics, 1
  shading rate, 1 `emit_vulkan_resource_index`, 3 arrays of samplers): Zink's own lowering
  (`zink_compiler.c`) is not run before it; the route has to run the part it needs.
- The rest, each under 10: input attachments (6, the Vulkan driver lowers them to texture loads),
  arrays of textures indexed at run time (5, 15.4's rewrite), `StorageImageReadWithoutFormat` (3,
  15.4's formatless storage), multiview (3), and 11 small reader and validation gaps (7 `As` of a
  vertex input in HLSL's samples, 4 integer outputs without `flat`).
- Not a failure but wrong: 64 vertex shaders convert only because the module is validated with
  `CLIP_DISTANCES` (`wcontainers_shaders::capabilities` adds it for a clip distance output). Zink
  writes `gl_ClipDistance` and `gl_CullDistance` outputs the shader never writes (glslang's
  `gl_PerVertex` members, still in the NIR after dead variables are removed), so these programs would
  ask a back end for clip distances they do not use. To fix with the rest.
- The module is validated with what the common form allows (`wcontainers_shaders::capabilities`,
  plus `IMMEDIATES` where it has an `Immediate` global: Vulkan's push constants), not WebGPU's core
  capabilities alone; 49 shaders with push constants convert since.

Next, in this order:
0. Candidate A's failures above, largest first; and the corpus classified by what the program needs:
   a shader whose SPIR-V declares a capability our Vulkan 1.1 does not report (15.4's list: ray query,
   buffer device addresses, descriptor heaps, sparse residency, shading rate, barycentrics) is
   counted apart with that reason, not as a route failure.
1. GLSL into the bridge: Mesa's standalone GLSL compiler (`libglsl_standalone`) and `gl_nir_link_glsl`,
   the linker st/mesa uses, giving each stage's linked NIR; then the lowering st/mesa does after
   linking for the caps the OpenGL driver will report (decide those caps here and write them down: G9
   must report the same, or the NIR shape differs).
2. The harness: each corpus shader run on seeded inputs (seed, number of input sets: knobs, recorded)
   by llvmpipe (GLSL, through EGL on the surfaceless platform, which this build has) or lavapipe
   (SPIR-V, `install/share/vulkan/icd.d/lvp_icd.x86_64.json`), and the route's module by
   `wcontainers-gpu-test`'s interpreter (`program.rs`), shaders past its limits through the rig;
   compared exactly for integers and within the format's rounding for floats.
3. Candidate A over the corpus, then candidate B: NIR walked in Rust straight into naga. For B the
   bridge will need NIR's structures in Rust; bindgen was tried and dropped in this run (Departures);
   the choice between bindgen with a named bindings path and a flattened form written by the C side
   is open.
4. The corpus (piglit and dEQP GLSL, Godot Compatibility, bgfx, Dolphin; Vulkan SPIR-V from Godot
   Forward+ and Sascha Willems' samples; the shaders of `dEQP-VK.pipeline.monolithic.multisample.
   alpha_to_one.*` and `dEQP-VK.robustness.buffer_access.*`), in scratch, test data only.
5. The choice by the numbers (count matching, then time per shader), with both counts and times.
6. The cache key (NIR serialised plus converter version); the rewrites of 15.4 (formatless storage,
   gathers with run-time offsets, arrays of textures, samplers and combined image samplers indexed at
   run time), which need an entry point in the shaders crate (Requests); alphaToOne's two steps with
   the exhaustive 16-bit test; robustBufferAccess's three steps (needs `HelperValue::BufferRange`).

## How to run

- The corpus: `crates/wcontainers-nir/corpus/fetch` (into `$CARGO_TARGET_DIR/nir-corpus`), then with
  `WCONTAINERS_MESA` set: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus WC_NIR_RESULTS=<file> cargo
  test -p wcontainers-nir --test corpus`. From the Mac: `orchestrator/cloud/x86-run <checkout> --name
  graphics-nir --jobs 64 -- 'crates/wcontainers-nir/corpus/fetch && export
  WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build && WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus
  WC_NIR_RESULTS=$CARGO_TARGET_DIR/nir-results-a.jsonl cargo test -p wcontainers-nir --test corpus'`,
  then `x86-run --fetch graphics-nir target/nir-results-a.jsonl <scratch>`. 1005 shaders take 12 s.

- Build Mesa, from the Mac: `orchestrator/cloud/x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/mesa/build path:/work/flake/graphics-nir'`. On any Linux machine with Nix:
  `crates/wcontainers-nir/mesa/build` (its argument, the flake, defaults to the checkout). The tools
  come from Nixpkgs' own `mesa` package (`nix print-dev-env --inputs-from <flake> nixpkgs#mesa`), so the
  repository's flake is unchanged and the tools are those of its locked nixpkgs. Output in
  `$CARGO_TARGET_DIR/mesa/{src,build,install}` (target/mesa without it). The script refuses a tag whose
  commit is not the pinned one, makes the build folder again when the commit or the options change
  (Meson reads a machine file only when a folder is made), and fails unless llvmpipe and lavapipe were
  built.
- Test with Mesa (on the x86 machine, after the build):
  `export WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build; cargo test -p wcontainers-nir` and
  `cargo clippy -p wcontainers-nir --all-targets -- -D warnings`; and both again without the variable.
- The literal check needs the git history, which the rented machine's copy lacks: it runs on the Mac
  (`RAYON_NUM_THREADS=3 cargo run -j 4 -q -p wcontainers-literals -- check`). With
  `crates/wcontainers-shaders/dxbc-spirv`'s own submodule checked out the check fails on its Python
  scripts (the baseline holds it not checked out): fetch only the shaders crate's two submodules and
  dxil-spirv's `third_party/spirv-headers`, which the shaders crate's build needs.

- Kept between runs: a shallow clone of the pinned Mesa for reading on the Mac
  (`pyro-parked/wcontainers-scratch/graphics-nir/mesa-src`, 560 MB, no build output; logs beside it in
  `logs/`), and the Mesa build and cargo target on the x86 machine (`/work/target/graphics-nir`).
  No scratch copy of the checkout and no cargo target was made on the Mac, the laptop or the desktop.

## Measurements

- Mesa 26.3.1 clean build (configure, 1239 steps, install), x86-64 machine (c7i.metal-48xl, 192
  threads), `-j 64`: 45.9 s wall, 11 min 37 s user, load average 4.96 at the start and 10.51 at the
  end (other tracks' jobs). An earlier clean build of the same options with SPIRV-Tools: 45.9 s wall
  at load 0.43 at the start.
- No route timings yet.

## Routed decisions (15.9, owner not yet answered; followed as the orchestrator set them)

- Our own OpenGL driver on the interface, as 15.2 recommends. gex_wgpu and wvk are not adopted; nothing of
  theirs is copied or paraphrased (their repository has no licence; a related project is AGPL-3.0).
- Crate and module names are the plan's suggestions: `wcontainers-nir`, `wcontainers-opengl`.
- Mesa under its MIT licence with the permissive files 15.2 lists; dEQP, VK-GL-CTS and piglit for testing only.
- No change to the interface crate: what the second batch needs from it (15.7) is written below as requests.

## Licences brought in (for the owner to confirm)

- Mesa 26.3.1 (fetched by the build script, not in this repository). Its `docs/license.rst` says each
  file's SPDX tag governs; the core is MIT. Linked into wcontainers-nir's tests today: Mesa's static
  libraries `libzink`, `libglsl_standalone`, `libglsl`, `libglcpp`, `libmesa`, `libvtn`, `libnir`,
  `libcompiler`, `libmesa_util`, `libblake3`, `libmesa_util_c11` (the linker keeps only the objects
  used). The per-file SPDX list of what is kept is not made yet; it comes from the link map once the
  bridge is complete.
- zlib (Zlib licence), linked as Mesa's build found it (Nixpkgs' static zlib).
- `serde_json` 1 and `serde` 1 (both MIT OR Apache-2.0), build- and dev-dependencies of
  wcontainers-nir; `cc` (MIT OR Apache-2.0), already in the workspace.
- `spirv` 0.4 (Apache-2.0; the SPIR-V grammar's names, already in the workspace through naga), a
  dependency. `rand` 0.9 and `rand_chacha` 0.9 (MIT OR Apache-2.0, already in the workspace),
  dev-dependencies.
- Corpus, test data only, fetched into the build folder and never in the repository or shipped:
  Sascha Willems' Vulkan samples at d2d49bf7 (MIT, `LICENSE.md`).
- Build tools only, from the repository's locked nixpkgs: Meson 1.12.1 (Apache-2.0), Ninja (Apache-2.0),
  LLVM 21.1.8 (Apache-2.0 WITH LLVM-exception; llvmpipe and lavapipe use it, wcontainers-nir does not
  link it), GCC 16.2.0 (GPL-3.0 with the GCC runtime library exception).

## Departures from the plan

- Mesa is not a git submodule at `third_party/mesa` (15.2). It is pinned by commit in
  `crates/wcontainers-nir/mesa/pin.toml` and fetched by the build script. Reasons: the literal check
  scans every Rust file and script of a submodule (Mesa has hundreds of Python scripts and Rust files),
  any new baseline or policy line is refused as growth, and an unchecked-out submodule is itself a
  finding; and the fork 15.2 describes holds no patch yet (G8 needs none), while where the fork is kept
  is the owner's decision (15.9). When G9 needs Mesa patches, the pin moves to the fork's commit.
- The Mesa build runs on the rented x86-64 machine, not the laptop (15.8 says laptop): the brief of
  2026-11-19 makes the rented machines the default.
- Zink is built beside llvmpipe and lavapipe: its `nir_to_spirv` is candidate A's first step.
  SPIRV-Tools is off (Mesa takes it when present; nothing of ours needs it).
- The bridge's eight functions are declared by hand in `src/mesa.rs`, not by bindgen: bindgen's output
  is included from Cargo's `OUT_DIR`, whose name the literal check refuses as a new code string here
  (its one constant, `CARGO_OUT_DIR`, is private to wcontainers-literals).

- The corpus run executes nothing yet: 15.3's comparison against llvmpipe and lavapipe is the next
  harness step (Next).

## Interface changes

None.

## Requests to other tracks

- Interface track (15.7): everything 15.7 lists; G8 needs `HelperValue::BufferRange` and the slow path
  names `RobustAccessInShader` and `AlphaToOneInShader` before its robustness and alphaToOne parts can
  be reflected.
- Shaders track: a public entry point applying `variant::apply` and its kin to a naga module already in
  the common form (stage, reflection, variant key), so the NIR route takes the Direct3D rewrites from
  one place (15.4) instead of a copy.
- Shaders track: `wcontainers_shaders::capabilities` to add `Capabilities::IMMEDIATES` for a module
  with an `AddressSpace::Immediate` global, as it adds the other extensions a front asks for only
  where the back end has them; until then the NIR route adds it itself.
- Literals track: `CARGO_OUT_DIR` public (in a public module), so a build script can name `OUT_DIR`
  through it.

## Literal check

Adopted with the branch (merged from the shaders branch, which carries it). wcontainers-nir has no
baseline lines: the check passes with none for it (run on the Mac, 2026-11-19 runs 1 and 2, exit 0). The list of
changes that could add lines is the 18 the shaders branch brought; this branch added none. C files are
not scanned; the bridge names its one version by `#define`s all the same.
