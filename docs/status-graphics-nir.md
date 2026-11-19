# Status: graphics, NIR route (G8)

Branch phase-2-graphics-nir. Plan: docs/phase-2-graphics.md, sections 15.2, 15.3, 15.4 and 15.8 (G8).

## Where it stands

G8 run 3 (2026-11-19). Not complete: candidate B, GLSL, the rest of the corpus, the choice and the
15.4 parts remain (Next).

Done:
- Mesa pinned and building on the rented x86-64 machine with llvmpipe and lavapipe (and Zink, whose
  NIR to SPIR-V compiler is candidate A's first step), by one command (How to run). Pin:
  `crates/wcontainers-nir/mesa/pin.toml` (mesa-26.3.1, commit fec8830e9c94); meson options:
  `crates/wcontainers-nir/mesa/options.ini`; script: `crates/wcontainers-nir/mesa/build`.
- `crates/wcontainers-nir`: the crate, its C bridge (`bridge/bridge.c`, `bridge.h`) and its link to
  Mesa's compiler libraries (`build.rs`). Built when `WCONTAINERS_MESA` names the Mesa build folder;
  without it the crate builds without the NIR parts (cfg `mesa`), so the workspace builds everywhere.
  `build.rs` takes everything about Mesa from Mesa's own build (Meson's `meson-info`, the compile
  commands, the introspected dependencies and targets, among them lavapipe's file).
- The NIR the routes take is shaped by one `nir_shader_compiler_options` in the bridge (Zink's list
  plus all 64-bit integer lowering and the instance index counting from the first instance); unused
  inputs, outputs and system values are removed. The drivers (G9, G10) must hand over the same.
- `Nir::to_common_form` (`wc_nir_to_common_form`), every route's step: sampled array layers rounded
  to the nearest even integer (Vulkan's rule; the common form's layer is an integer, and naga's
  SPIR-V reader truncates the float layer); the binding rule of 6.4 (each combined image sampler
  split into a texture and a sampler at the same set and binding; every resource renumbered into its
  stage's set kind by kind in the program's (set, binding) order; the reflection as
  `ReflectedBinding`, program set in `space`, program binding in `register`; unsized arrays of
  resources and overfull ranges refused by name).
- The cache key (15.3): `Nir::key`, BLAKE3 of `nir_serialize`'s bytes and `CONVERTER_VERSION` (this
  crate's version, the Mesa commit and a hash of `mesa/build`'s record of commit and options). Taken
  from the NIR as the driver hands it over. Held by `tests/route.rs`.
- Candidate A (`zink-spirv-naga`): Zink's `ntv_shader_prepare` and `nir_to_spirv` (native Vulkan,
  SPIR-V 1.3), cleaned of Zink's `Index 0` decorations and its unneeded `DrawParameters`, read by
  naga's SPIR-V reader and validated with what the common form allows.
- The corpus run (`tests/corpus`, one command, How to run): every `.spv` under `WC_NIR_CORPUS`, each
  shader and route in a child process of its own; a crash or hang is named and attributed to the step
  it happened in (the route, or the comparison). One JSON line a shader. Knobs: `WC_NIR_SHADERS` (a
  seeded sample), `WC_NIR_SEED` (the sample and every shader's inputs; default 0x5eed),
  `WC_NIR_INPUT_SETS` (vertices or fragments a run, each with inputs of its own; default 64),
  `WC_NIR_JOBS`, `WC_NIR_RESULTS`, `WC_NIR_DUMP`.
- Counted apart before any route (`tests/corpus/needs.rs`): a program declaring a SPIR-V capability
  beyond Vulkan 1.1's core, or one of 1.1's whose feature our Vulkan driver reports absent (15.4's
  list of 26).
- The comparison (`tests/corpus/compare.rs`, `lavapipe.rs`, `interface.rs`): the program as given
  run by lavapipe (the reference), and the route's module written back to SPIR-V by naga's writer
  and run by lavapipe the same way, on the same seeded inputs; the reference runs twice and must
  agree with itself. Lavapipe is loaded from the pinned build by its own `vk_icdGetInstanceProcAddr`
  (no loader), every Vulkan 1.0 to 1.3 feature it has enabled, with transform feedback and
  robustness2 (accesses outside a binding read zero and write nothing, in both runs), one thread
  (`LP_NUM_THREADS=0`, so racing programs repeat). Inputs: uniform, storage and immediate data filled
  by type (floats in [-2, 2), integers in [-4, 4) or [0, 4): programs use them as indices and
  counts), runtime arrays of 64 elements, 4-texel textures of one level (2 layers for arrays, 6
  faces for cubes) with nearest clamped samplers, vertex attributes per input set. Compute: 2
  workgroups, storage compared. Vertex: transform feedback of the position and every located output
  (decorations added to both modules' words), rasteriser off. Fragment: a vertex program made with
  naga feeds one point a pixel of a row, one input set each; 32-bit colour targets cleared to a
  marker, depth where the program writes it, storage. Integers compared exactly; floats equal when
  within 1024 units in the last place or 2^-20 apart (both runs on one driver and processor: only
  the order and fusing of operations differ).
- The comparison is checked to see differences: with every float constant of the route's module
  raised by 1 (a temporary change, not kept), 37 of the 43 compared shaders of a 60-shader sample
  differed (the 6 others use no float constant on what they output).

Candidate A on the whole corpus, held to lavapipe (run 3's last run; x86-64 machine, 192 children at
once, load 0.3 at the start, 5.3 at the end; 124 s with the build): 1005 shaders.
- 138 have a stage the common form lacks (ray tracing 81, tessellation 30, geometry 10, mesh and
  task 6); 3 crash Mesa's `spirv_to_nir` steps before any route (HLSL's variable rate shading).
- 25 need what our Vulkan 1.1 does not report: buffer device addresses 8 (with descriptor heaps and
  untyped pointers in 4), sparse residency 5, descriptor indexing 3, shading rate 3, minimum LOD
  3, barycentrics 2, ray query 2.
- Of the 839 left: 726 convert and match the reference; 3 convert and differ; 8 convert and were not
  compared; 102 fail to convert.
- The 3 that differ: HLSL's `gears.vert`, `shadowmappingcascade/depthpass.vert` and `scene.frag`.
  Cause: Zink's `get_glsl_type` writes `MatrixStride` and `RowMajor` only for a block member that is
  a matrix, never for an array of matrices, and takes the stride from the column's size instead of
  the type's explicit stride; HLSL's row-major arrays of matrices are then read column-major. A
  candidate A defect (Zink's compiler expects its own lowering of buffers to words, which this route
  does not run).
- The 8 not compared: 6 `irradiancecube.frag` (the program loops with a step from its uniforms; with
  a negative step it never ends: the reference hangs), 1 multisampled texture (the harness does not
  fill those yet), 1 fragment program of the mesh shading sample with a block input.
- The 102 failures: 29 `DrawParameters` needed (Slang's and HLSL's vertex index minus the base
  vertex; the draw's bases are a rewrite to take from the shaders crate); 40 crashes in Zink's
  compiler (31 in `emit_deref_array` on a variable mode it does not take, push constants among
  them, which it takes only as its own words; 2 arrays of samplers; 7 `.length()` of runtime
  arrays, which Zink lowers itself first); 7 naga reader `As` on HLSL
  vertex inputs; 6 integer stage outputs without `flat`; 6 input attachments; 3 multiview; 3 vertex
  programs without a position output; 3 formatless storage reads (15.4 rewrite); 2 naga type
  resolution in HLSL's and Slang's order-independent transparency; 1 each: coherent image, arrays of
  combined image samplers indexed at run time, a uniform block's alignment.
- Conversion time of those converted: mean 380 us (192 at once, not a measurement for the choice).
- Not a failure but wrong, still: Zink writes `gl_ClipDistance` and `gl_CullDistance` outputs a
  glslang program never writes, so such modules ask a back end for clip distances they do not use
  (validated with `CLIP_DISTANCES` today).

Next, in this order:
1. Candidate B: NIR walked into naga in Rust. The bridge will need NIR's structures in Rust; bindgen
   was tried and dropped in run 1 (Departures); a flattened form written by the C side (types,
   variables, then each function's control flow and instructions, out of SSA into registers) and
   read by Rust is the plan. Run it over the corpus beside candidate A (`ROUTES` in `run.rs`).
2. Candidate A's fixable defects, so that the choice compares each at its best: Zink's matrix
   decorations (a patch to the pinned Mesa: decorate arrays of matrices, take the explicit stride);
   push constants as spilled immediates; the unused clip and cull outputs.
3. GLSL into the bridge: Mesa's standalone GLSL compiler (`libglsl_standalone`; `standalone.cpp`
   shows the calls: `initialize_context_to_defaults`, `standalone_add_shader_source`,
   `_mesa_glsl_compile_shader`, `link_shaders_init`, `gl_nir_link_glsl`; the linked NIR is
   `_LinkedShaders[stage]->Program->nir`; a C++ file, so a second `cc::Build`), then st/mesa's
   after-link lowering for the caps the OpenGL driver will report (decide and record them: G9 must
   report the same); the reference is llvmpipe (EGL surfaceless).
4. The rest of the corpus (piglit and dEQP GLSL, Godot Compatibility, bgfx, Dolphin; Godot Forward+
   SPIR-V; the shaders of `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` and
   `dEQP-VK.robustness.buffer_access.*`), each pinned in `corpus/sources`.
5. The choice by the numbers (matching count, then time per shader, timed alone: `WC_NIR_JOBS=1`
   under `taskset`), with both routes' counts and times.
6. The rewrites of 15.4 (formatless storage, gathers with run-time offsets, arrays of textures,
   samplers and combined image samplers indexed at run time), which need an entry point in the
   shaders crate (Requests); alphaToOne's two steps with the exhaustive 16-bit test (first check
   whether lavapipe runs `demote_samples`); robustBufferAccess's three steps (needs
   `HelperValue::BufferRange`).
7. The harness's own limits: multisampled textures, block inputs, programs whose reference does not
   end on the seeded inputs.
8. The SPDX tag of every Mesa file actually linked, from the link map.

## How to run

- The corpus: `crates/wcontainers-nir/corpus/fetch` (into `$CARGO_TARGET_DIR/nir-corpus`), then with
  `WCONTAINERS_MESA` set: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus WC_NIR_RESULTS=<file> cargo
  test -p wcontainers-nir --test corpus`. From the Mac: `orchestrator/cloud/x86-run <checkout> --name
  graphics-nir --jobs 64 -- 'crates/wcontainers-nir/corpus/fetch && export
  WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build && WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus
  WC_NIR_RESULTS=$CARGO_TARGET_DIR/nir-results-a.jsonl cargo test -p wcontainers-nir --test corpus'`,
  then `x86-run --fetch graphics-nir target/nir-results-a.jsonl <scratch>`. 1005 shaders take about
  2 minutes with the comparison (192 at once). Run 3's results are in
  `pyro-parked/wcontainers-scratch/graphics-nir/results/run3/nir-results-a.jsonl` (run 2's, conversion
  only, one folder up); `scripts/reasons.py <results> [n]` counts the reasons.

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

- A quick look at one shader: copy it alone into a folder and point `WC_NIR_CORPUS` there with
  `WC_NIR_DUMP` set; `nix shell nixpkgs#spirv-tools -c spirv-dis` reads the dumps.
- Kept between runs: a shallow clone of the pinned Mesa for reading on the Mac
  (`pyro-parked/wcontainers-scratch/graphics-nir/mesa-src`, 560 MB, no build output; logs beside it in
  `logs/`), and the Mesa build and cargo target on the x86 machine (`/work/target/graphics-nir`).
  No scratch copy of the checkout was made on the Mac, the laptop or the desktop. The literal check's
  own build in the checkout's `target` (282 MB) is deleted at the end of each run. On the x86
  machine `/work/target/graphics-nir` (8.5 GB: Mesa, the corpus, cargo's output) is kept.

## Measurements

- Mesa 26.3.1 clean build (configure, 1239 steps, install), x86-64 machine (c7i.metal-48xl, 192
  threads), `-j 64`: 45.9 s wall, 11 min 37 s user, load average 4.96 at the start and 10.51 at the
  end (other tracks' jobs). An earlier clean build of the same options with SPIRV-Tools: 45.9 s wall
  at load 0.43 at the start.
- Corpus run with the comparison, 1005 shaders, 192 children, x86-64 machine at load 0.3 rising to
  5.3: 124 s including an incremental build. Route timings for the choice are not taken yet (they
  need `WC_NIR_JOBS=1` and both candidates).

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
- `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception, already in the workspace), a
  dependency and build-dependency (the cache key and the converter's version).
- `wcontainers-gpu` (ours), a dependency (`SlotKind`, `ReflectedBinding`, `ShaderStage`).
- `ash` 0.38 (MIT OR Apache-2.0) and `libloading` 0.8 (ISC), dev-dependencies (lavapipe in the
  corpus run); both already in the lock file through wgpu.
- `spirv` 0.4 (Apache-2.0; the SPIR-V grammar's names, already in the workspace through naga), a
  dependency. `rand` 0.9 and `rand_chacha` 0.9 (MIT OR Apache-2.0, already in the workspace),
  dev-dependencies.
- Corpus, test data only, fetched into the build folder and never in the repository or shipped:
  Sascha Willems' Vulkan samples at d2d49bf7 (MIT, `LICENSE.md`).
- Build tools only, from the repository's locked nixpkgs: Meson 1.12.1 (Apache-2.0), Ninja (Apache-2.0),
  LLVM 21.1.8 (Apache-2.0 WITH LLVM-exception; llvmpipe and lavapipe use it, wcontainers-nir does not
  link it), GCC 16.2.0 (GPL-3.0 with the GCC runtime library exception).

## Departures from the plan

- 6.4 states the binding rule for Direct3D. For Vulkan and OpenGL programs the NIR route reads it as
  Direct3D 12's: within the stage's set each kind's resources in the order of the program's own
  (set, binding), with the set in `ReflectedBinding::space` and the binding in `register`. A combined
  image sampler is a texture and a sampler reflected at the same set and binding.
- The NIR the routes take is fixed by one set of NIR compiler options in the bridge (Zink's list plus
  all 64-bit integer lowering and the instance index counting from the first instance), which 15.3
  does not name; G9 and G10 must hand over NIR made with the same options.
- The corpus is fetched by `corpus/fetch` into the build folder on the rented machine (each source
  pinned by commit), not kept in the Mac's scratch (15.8): the runs happen there, and the pins make
  it the same corpus everywhere.
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

- The route's module is run by lavapipe through naga's SPIR-V writer, not by the recording back
  end's interpreter (`wcontainers-gpu-test`) or the WebGPU back end on the RTX 3080 (15.3). Reasons:
  the interpreter's limits (no derivatives, implicit levels only on one-level views, no atomics or
  workgroup memory) would leave most real fragment programs to the GPU queue, while lavapipe runs
  every program the reference runs, on the same machine, with one harness for both sides. What this
  holds is the conversion (NIR to the common form); naga's SPIR-V writer is the same one wgpu's
  Vulkan back end uses. The WebGPU back end's own conversion is the shaders track's.
- The plan says "within the format's rounding" for floats; the targets are 32-bit, and two programs
  that differ only in the order of operations differ by more than one rounding. The tolerance is
  1024 units in the last place or 2^-20 absolute (`compare.rs`), stated and checked to still catch a
  change of constants (Where it stands).
- Vulkan capability classes: a program needing what our Vulkan 1.1 will not report is counted apart,
  not as a route failure (the pass bar's "named reason").

## Interface changes

None.

## Requests to other tracks

- Interface track (15.7): everything 15.7 lists; G8 needs `HelperValue::BufferRange` and the slow path
  names `RobustAccessInShader` and `AlphaToOneInShader` before its robustness and alphaToOne parts can
  be reflected.
- Shaders track: a public entry point applying `variant::apply` and its kin to a naga module already in
  the common form (stage, reflection, variant key), so the NIR route takes the Direct3D rewrites from
  one place (15.4) instead of a copy.
- Interface track: `pub const fn stage_set(stage: ShaderStage) -> u32` in `reflect.rs` beside
  `helper_binding` (vertex and compute 0, fragment 1, plan 6.4). The shaders crate writes the rule as
  a literal in `lower.rs`; the NIR route defines `wcontainers_nir::bind::stage_set` until the
  interface has it, and will import it then.
- Shaders track: the draw's bases rewrite (`HelperValue::DrawBase`) as part of the public entry point
  below, for Vulkan programs reading `BaseVertex` or `BaseInstance`.
- Shaders track: `wcontainers_shaders::capabilities` to add `Capabilities::IMMEDIATES` for a module
  with an `AddressSpace::Immediate` global, as it adds the other extensions a front asks for only
  where the back end has them; until then the NIR route adds it itself.
- Shaders track (a finding, not a request for this route): naga's SPIR-V reader turns a sampled
  array layer into an integer with a truncating `As` (`front/spv/image.rs`), where Vulkan and
  Direct3D round to the nearest even integer; a layer of 1.6 reads layer 1 instead of 2. The NIR
  route rounds in NIR first; dxil-spirv's and dxbc-spirv's output reaches the same reader.
- Literals track: `CARGO_OUT_DIR` public (in a public module), so a build script can name `OUT_DIR`
  through it.

## Literal check

Adopted with the branch (merged from the shaders branch, which carries it). wcontainers-nir has no
baseline lines: the check passes with none for it (run on the Mac, 2026-11-19 runs 1 to 3, exit 0;
it needs the git history, which the rented machines' copies lack). The list of
changes that could add lines is the 18 the shaders branch brought; this branch added none. C files are
not scanned; the bridge names its one version by `#define`s all the same.
