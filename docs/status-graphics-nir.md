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
- Candidate A's output is cleaned of what Zink writes that says nothing and that naga's reader
  refuses: `Index 0` on every fragment output (read as dual-source blending), and the
  `DrawParameters` capability where no base vertex, base instance or draw index built-in is used
  (Zink declares it for any vertex program reading its instance index).
- The binding rule of the common form (6.4), in NIR, for both routes (`wc_nir_bind`, Rust
  `Nir::bind`, `src/bind.rs`): each combined image sampler is split into a texture and a bare sampler
  of the same array shape at the same set and binding (every sampling instruction's sampler moved to
  it); then every resource is renumbered into its stage's set (vertex and compute 0, fragment 1), each
  kind (`SlotKind` uniform, sampler, texture, storage) from its range's first binding in the order of
  the program's (set, binding), buffer accesses (`vulkan_resource_index`) pointed at the new numbers;
  the reflection (`ReflectedBinding`) has the program's set as `space` and its binding as `register`.
  Refused by name: more resources of a kind than its range holds, and unsized arrays of resources
  (they need `RESOURCE_TABLES`). Held by `tests/route.rs`.
- The meaning of the instance index: the routes' options set `instance_id_includes_base_index`, so
  NIR's instance id is Vulkan's `InstanceIndex` and WGSL's `instance_index`, which both count from
  the draw's first instance; `nir_lower_system_values` runs in the routes' common clean-up.
- The cache key (15.3): `Nir::key`, BLAKE3 of `nir_serialize`'s bytes and `CONVERTER_VERSION` (this
  crate's version, the Mesa commit and a hash of `mesa/build`'s record of commit and options, handed
  over by `build.rs`). Taken from the NIR as the driver hands it over, before any route changes it.
  Held by `tests/route.rs` (same NIR, same key; different NIR, different key).

Candidate A on the corpus, conversion only (run 2's last run, x86-64 machine, load 6 at the start,
192 children at once, so the times are not a measurement for the choice): 1005 shaders; 138 have a
stage the common form lacks (ray tracing 81, tessellation 30, geometry 10, mesh and task 6);
9 crash Mesa's `spirv_to_nir` steps before any route (5 in `nir_lower_vars_to_ssa` on HLSL's
variable rate shading sample, 4 in `align` on the descriptor heap samples); of the 858 left, 739
convert and validate (231 at the run's first try), 119 fail. Conversion time of those converted:
median 230 us, mean 284 us (load as above). The reasons, largest first:
- 31 crashes in Zink's `emit_deref_array` on push constants: Zink's compiler takes push constants
  only as its own words (`load_push_constant_zink`), never a program's block. The way out for this
  route is the interface's spilled immediates (`AddedBinding::SpilledImmediates`, a uniform buffer,
  counted slow path); candidate B can write `Immediate` globals directly.
- 29 `DrawParameters` that is needed: Slang's and HLSL's vertex index subtracts the base vertex.
  The common form has the draw's bases as helper values (`HelperValue::DrawBase`), a rewrite to take
  from the shaders crate (Requests).
- 21 more crashes in Zink's compiler: `deref_buffer_array_length` (7: `.length()` of a runtime array,
  which Zink lowers itself first), sparse residency (3), `nir_instr_as_intrinsic` (4, buffer device
  addresses), barycentrics (2), shading rate (1), `emit_vulkan_resource_index` (2, ray query),
  arrays of samplers (2).
- Each under 10: `As` of an HLSL vertex input (7), integer stage outputs without `flat` (6), input
  attachments (6, the Vulkan driver lowers them to texture loads), multiview (3), arrays of textures
  indexed at run time (3, 15.4's rewrite), unsized arrays of resources (3, refused by the binding
  rule), formatless storage reads (3, 15.4's rewrite), HLSL's displacement sample without a position
  output (3), and 5 single reader or validation gaps.
- Several of these need what our Vulkan 1.1 does not report (ray query, buffer device addresses,
  descriptor heaps, sparse residency, shading rate, barycentrics): to be counted apart (Next 0).
- Not a failure but wrong: vertex shaders from glslang convert only because the module is validated
  with `CLIP_DISTANCES` (`wcontainers_shaders::capabilities` adds it for a clip distance output). Zink
  writes `gl_ClipDistance` and `gl_CullDistance` outputs the shader never writes (glslang's
  `gl_PerVertex` members, still in the NIR after dead variables are removed), so these programs would
  ask a back end for clip distances they do not use. To fix with the rest.
- The module is validated with what the common form allows (`wcontainers_shaders::capabilities`,
  plus `IMMEDIATES` where it has an `Immediate` global), not WebGPU's core capabilities alone.

Next, in this order:
0. The corpus classified by what the program needs: a shader whose SPIR-V declares a capability our
   Vulkan 1.1 does not report (15.4's list) is counted apart with that reason, not as a route
   failure. Then candidate A's failures above, largest first (push constants as spilled immediates;
   the unused clip and cull outputs; `.length()`).
1. The harness (the pass bar needs it; nothing is compared yet): each corpus shader run on seeded
   inputs (seed and number of input sets: knobs, recorded) by lavapipe (SPIR-V; load
   `install/lib/libvulkan_lvp.so` and its `vk_icdGetInstanceProcAddr` with `ash` 0.38, already in the
   lock file, through `ash::Entry::from_static_fn`, no loader) or llvmpipe (GLSL, EGL surfaceless),
   and the route's module by `wcontainers_gpu_test::program::run` one invocation at a time (vertex:
   attributes in, outputs out; fragment: varyings and the pixel centre in; compute: invocations of a
   workgroup) with a `Resources` of the same seeded buffers and textures (nearest filtering, one
   level). Vertex outputs of the reference by transform feedback (lavapipe has
   `VK_EXT_transform_feedback`; the capture decorations are added to the SPIR-V words), fragment
   outputs by drawing one point a pixel into 32-bit targets from a generated vertex program.
   Derivatives, workgroup memory, atomics and storage writes of images are past the interpreter's
   limits (run them through the rig's WebGPU back end). Integers compared exactly, floats within a
   stated tolerance.
2. GLSL into the bridge: Mesa's standalone GLSL compiler (`libglsl_standalone`; `standalone.cpp`
   shows the calls: `initialize_context_to_defaults`, `standalone_add_shader_source`,
   `_mesa_glsl_compile_shader`, `link_shaders_init`, `gl_nir_link_glsl`; the linked NIR is
   `_LinkedShaders[stage]->Program->nir`; a C++ file, so a second `cc::Build`), then st/mesa's
   after-link lowering for the caps the OpenGL driver will report (decide and record them: G9 must
   report the same).
3. Candidate B: NIR walked in Rust straight into naga. The bridge will need NIR's structures in Rust;
   bindgen was tried and dropped in run 1 (Departures); a flattened form written by the C side is the
   other choice.
4. The rest of the corpus (piglit and dEQP GLSL, Godot Compatibility, bgfx, Dolphin; Godot Forward+
   SPIR-V; the shaders of `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` and
   `dEQP-VK.robustness.buffer_access.*`), each pinned in `corpus/sources`.
5. The choice by the numbers (count matching, then time per shader, timed alone: `WC_NIR_JOBS=1`
   under `taskset`), with both routes' counts and times.
6. The rewrites of 15.4 (formatless storage, gathers with run-time offsets, arrays of textures,
   samplers and combined image samplers indexed at run time), which need an entry point in the
   shaders crate (Requests); alphaToOne's two steps with the exhaustive 16-bit test;
   robustBufferAccess's three steps (needs `HelperValue::BufferRange`).

## How to run

- The corpus: `crates/wcontainers-nir/corpus/fetch` (into `$CARGO_TARGET_DIR/nir-corpus`), then with
  `WCONTAINERS_MESA` set: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus WC_NIR_RESULTS=<file> cargo
  test -p wcontainers-nir --test corpus`. From the Mac: `orchestrator/cloud/x86-run <checkout> --name
  graphics-nir --jobs 64 -- 'crates/wcontainers-nir/corpus/fetch && export
  WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build && WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus
  WC_NIR_RESULTS=$CARGO_TARGET_DIR/nir-results-a.jsonl cargo test -p wcontainers-nir --test corpus'`,
  then `x86-run --fetch graphics-nir target/nir-results-a.jsonl <scratch>`. 1005 shaders take 12 s.
  Run 2's results are in `pyro-parked/wcontainers-scratch/graphics-nir/results/nir-results-a.jsonl`;
  `scripts/reasons.py <results> [n]` beside it counts the reasons.

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
- `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception, already in the workspace), a
  dependency and build-dependency (the cache key and the converter's version).
- `wcontainers-gpu` (ours), a dependency (`SlotKind`, `ReflectedBinding`, `ShaderStage`).
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
- Interface track: `pub const fn stage_set(stage: ShaderStage) -> u32` in `reflect.rs` beside
  `helper_binding` (vertex and compute 0, fragment 1, plan 6.4). The shaders crate writes the rule as
  a literal in `lower.rs`; the NIR route defines `wcontainers_nir::bind::stage_set` until the
  interface has it, and will import it then.
- Shaders track: the draw's bases rewrite (`HelperValue::DrawBase`) as part of the public entry point
  below, for Vulkan programs reading `BaseVertex` or `BaseInstance`.
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
