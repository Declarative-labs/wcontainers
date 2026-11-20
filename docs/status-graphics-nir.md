# Status: graphics, NIR route (G8)

Branch phase-2-graphics-nir. Plan: docs/phase-2-graphics.md, sections 15.2, 15.3, 15.4 and 15.8 (G8).

## Where it stands

G8 run 6 (2026-11-20). Not complete. Both candidates are measured on the SPIR-V corpus and candidate
B leads on both counts (The choice, below). Run 6: GLSL reaches the bridge (Mesa's GLSL compiler
and linker, then st/mesa's steps after a link) and route B takes what comes out, OpenGL's buffers
as arrays of words; held by a run on lavapipe against GLSL's own layout and arithmetic. The GLSL
reference on llvmpipe and the GLSL corpus, the rest of the corpus, the 15.4 rewrites and
robustBufferAccess remain (Next).

Done:
- Mesa pinned and building on the rented x86-64 machine with llvmpipe and lavapipe (and Zink, whose
  NIR to SPIR-V compiler is candidate A's first step), by one command (How to run). Pin:
  `crates/wcontainers-nir/mesa/pin.toml` (mesa-26.3.1, commit fec8830e9c94); meson options:
  `crates/wcontainers-nir/mesa/options.ini`; patches: `crates/wcontainers-nir/mesa/patches/` (applied
  in name order, again from the pinned commit whenever they change; they are part of the converter's
  version); script: `crates/wcontainers-nir/mesa/build`. One patch:
  `zink-matrix-arrays.patch` (Zink's `get_glsl_type` decorates a member that is an array of matrices
  too, with the type's explicit stride).
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
  SPIR-V reader truncates the float layer); compute dispatches counted from workgroup 0 with no
  global offset (`nir_lower_compute_system_values` with no bases); the binding rule of 6.4 (each combined image sampler
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
- Candidate B (`nir-naga`, `src/direct.rs`, `bridge/flat.c`, `flat.h`): the bridge takes the
  shader out of SSA (phi webs into registers; every other value used outside its block, or by an
  `if`, into a register of its own; derefs, and buffer descriptors (`vulkan_resource_index`,
  `load_vulkan_descriptor`), made again in each block) and flattens it into plain
  arrays (types, struct fields, variables, then the instructions in order with `if`, `else`,
  `loop` and their ends as markers). Mesa's numbering stays in C: every kind and operation crosses
  as the bridge's own enums (`WC_ALU_OPS`, `WC_INTRINSICS`, mirrored in order in Rust). Rust walks
  it into naga: stage inputs and outputs as entry point arguments and a result struct through
  locals (the position's y negated on the way out: the common form's clip space is WGSL's, as naga's
  SPIR-V reader makes it for A), buffers through `vulkan_resource_index` chains, registers as u32
  or bool locals, NIR's untyped values kept with the kind their instruction gives and bitcast at
  each use, constants made at each use in the kind it needs, a texture sampled with a comparison
  made a depth texture; storage images (load, store, size; multisampled loads), atomics on buffers
  and workgroup variables (each place an atomic acts on, found by walking its deref back, gets an
  atomic type, as naga requires), `fmod` (a - b floor(a / b)), `frem`, point coordinates, storage
  buffers always readable (the common form has no write-only buffers). What it does not take is
  refused by name ("not taken: ..."). Held by
  `tests/route.rs` (both candidates bind a sampling program alike; a compute program).
- Stage values in route B are lowered IO, one form for every source: `wc_nir_lower_io` (bridge.c)
  runs Mesa's `nir_lower_io_passes` on a Vulkan program (after splitting gl_PerVertex), which the
  OpenGL linker has already run on a GLSL one. `flat.c` gathers every slot `load_input`,
  `load_interpolated_input` and `store_output` reach into one variable a slot (the widest access;
  one kind, u32 where accesses disagree; clip distances folded into one array; a built-in keeps its
  own type), with a fragment input's interpolation from its barycentric (`load_input`: flat); Rust
  reads them as `LOAD_STAGE` and `STORE_STAGE` on that variable's local. Refused by name: a slot
  indexed at run time, other than 32 bits, a second blend source, interpolation at a sample or an
  offset, a slot interpolated two ways. The corpus gives the same results as with variables (run 5).
- alphaToOne's two steps (15.4): `Nir::alpha_to_one` (`wc_nir_alpha_to_one`) lowers IO and runs
  Mesa's `nir_lower_alpha_to_coverage` and `nir_lower_alpha_to_one`. Route B turns
  `demote_samples` into a local of the samples kept (all ones at the start, ANDed with the
  complement of each demoted set) that becomes the sample mask output, ANDed with the program's
  own where it writes one; the pass's 16-bit operations (u2u16, u2u32, mov, and, or, multiply,
  shifts) are carried in 32-bit words holding the zero-extended value (multiply and shift left
  masked to 16 bits, shift counts to their low 4 bits); every other 16-bit operation is refused by
  name. Held by `tests/route.rs` (a program with and one without its own sample mask: one sample
  mask output, alpha one, validated) and `tests/narrow.rs`: a SPIR-V program with native 16-bit
  integers (written word by word in the test) run by lavapipe natively and through route B, both
  held to Rust's 16-bit arithmetic, over every 16-bit value and every shift count 0 to 31 (2,097,152
  invocations, 14,680,064 results, seed 0x5eed for the second operand and the upper bits; 0 differ).
  Checked to see differences: with the multiply's and the shift count's masks taken out (temporary),
  6,151,775 results differed.
- GLSL (`Nir::from_glsl`, `wc_glsl_to_nir` in `bridge/glsl.cpp`, a C++ file built by a second
  `cc::Build` with the flags of Mesa's `standalone.cpp`): Mesa's GLSL compiler and linker on the
  standalone scaffolding (`libglsl_standalone`, no gallium screen), one context for every source:
  OpenGL 4.6 compatibility, GLSL 4.60, the ES 3.x compatibility extensions, and the limits the
  OpenGL driver is to report (named `WC_GL_*` in `glsl.cpp`; G9 reports the same). Then, per stage,
  st/mesa's steps after a link (`st_link_glsl_to_nir`, `st_glsl_to_nir_post_opts`,
  `st_finalize_nir`, MIT) for that driver's capabilities: dead IO variables removed;
  `gl_nir_lower_buffers` (which ends in `nir_lower_explicit_io`: every buffer access is a block
  index and a byte offset); fragment coordinates and point coordinates transformed for the common
  form's origin (upper left, centres at half: `nir_lower_wpos_ytransform` and
  `nir_lower_pntc_ytransform`, their state values added to the parameters); system values; state
  references; atomic counters into storage buffers (no hardware counters); 64-bit integers lowered;
  rectangle textures and gathers with offsets lowered (`nir_lower_tex`: no rectangles, no gather
  offsets); uniform places assigned in words (packed storage, `PackedDriverUniformStorage`) and the
  default block made uniform buffer 0 (`nir_lower_uniforms_to_ubo`, every block one index up);
  samplers as derefs; `gl_FragColor` written to every draw buffer (`nir_lower_fragcolor`, the
  driver's own step, as Zink's). A built-in uniform structure of the compatibility profile (the
  lighting state) is refused by name. Compile and link errors come back with Mesa's log
  (`ConvertError::Glsl`).
- OpenGL's buffers in route B (`wc_nir_buffers_as_words`, bridge.c): each uniform block becomes a
  variable of `vec4<u32>` words (the common form's uniform arrays step by 16 bytes; its size the
  block's, the default block's from the parameters), each storage block a runtime-sized array of
  `u32`, writable only where the program writes it; uniform block i binds at i, storage block j at
  the number of uniform blocks plus j (the program's set and binding in the reflection);
  `load_ubo`, `load_ssbo`, `store_ssbo` (by its write mask), the storage atomics and
  `get_ssbo_size` become 32-bit derefs of those words at offset / 4, which route B already takes.
  A block index not constant after folding, or an access not 32 bits wide, is left as it is and
  refused by name. This is also 15.4's robustBufferAccess step 2 (storage as runtime-sized arrays
  of words, 32-bit accesses). Held by `tests/route.rs` (a vertex and fragment program with default
  uniforms, a std140 block, std430 blocks, a sampler, an atomic: both stages convert and validate,
  bindings counted by kind; a program that does not compile is refused with its log) and
  `tests/glsl_buffers.rs`: a GLSL compute program reading a std140 block (vec3, float, float array,
  mat3, ivec2) and writing a std430 block (atomic, runtime array, `.length()`) runs on lavapipe
  through route B and naga's SPIR-V writer and matches what GLSL's layout rules and arithmetic give,
  computed in the test (exact values). Checked to see differences: with the word's component within
  its vector forced to 0 (temporary), 10 of 17 values differed.
- Whether lavapipe runs `demote_samples` (checked first, as G8 says): it does not. In the pinned
  Mesa only broadcom, freedreno, asahi, KosmicKrisp and imagination take it; llvmpipe, gallivm and
  lavapipe have no code for it (searched in the source). So the fragment programs of
  `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` after the two passes are held by G10's
  CTS cases, not by lavapipe; they are to be listed so when they are in the corpus.
- The corpus run: every `.spv` under `WC_NIR_CORPUS`, each
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

The corpus (Sascha Willems' samples, GLSL, HLSL and Slang compiled to SPIR-V): 1005 shaders. 138
have a stage the common form lacks (ray tracing 81, tessellation 30, geometry 10, mesh and task 6); 3
crash Mesa's `spirv_to_nir` before any route (HLSL's variable rate shading); 25 need what our Vulkan
1.1 does not report (buffer device addresses 8, sparse residency 5, descriptor indexing 3, shading
rate 3, minimum LOD 3, barycentrics 2, ray query 2). 839 reach both routes.

Run 4's results, held to lavapipe (x86-64 machine, 192 children at once, load 9.2 at the start and
25.2 at the end, shared with other tracks; 293 s with the Mesa rebuild; Zink patched). Run 5, with
route B on lowered IO, gave the same counts and the same reasons (x86-64 machine, load 0.07 at the
start and 5.96 at the end, 256 s; `results/run5/nir-results-r5a.jsonl`):
- Candidate A: 728 match, 0 differ, 8 not compared, 103 fail by name.
- Candidate B: 773 match, 0 differ, 8 not compared, 58 fail by name.
- Shader by shader: 727 match on both routes, 1 on A only, 46 on B only.
- The 8 not compared, on both: 6 `irradiancecube.frag` (a loop whose step comes from the uniforms
  never ends on a negative step: the reference hangs), 1 multisampled texture (the harness does not
  fill those), 1 fragment program of the mesh shading sample with a block input.
- A's failures: 29 the draw's bases (`DrawParameters`); 40 crashes in Zink's compiler (31 in
  `emit_deref_array` on a variable mode it takes only after its own lowering of buffers and push
  constants to words, which this route does not run; 2 arrays of samplers; 7 `.length()` of runtime
  arrays); 7 naga reader `As` on HLSL vertex inputs and 1 on HLSL's row-major array of matrices in
  `shadowmappingcascade/scene.frag` (it differed before the patch; now it fails by name); 6 integer
  stage outputs without `flat`; 6 input attachments; 3 multiview; 3 vertex programs without a
  position; 3 formatless storage; 2 naga type resolution in order-independent transparency; 1 each:
  coherent image, arrays of combined image samplers indexed at run time, a uniform block's alignment.
- B's failures: 23 `load_first_vertex` and 6 `load_base_instance` (the draw's bases, as A); 11
  subpass inputs; 5 binding arrays the common form does not allow (3 textures and samplers, 2
  buffers: the descriptor heap sample); 3 `load_view_index` (multiview); 3 `image_deref_atomic`
  (order-independent transparency; A fails them too); 3 vertex programs without a position (as A);
  3 storage images without a format (15.4 rewrite); 1 uniform block alignment (HLSL's
  `texturearray/instancing.vert`, as A). The full list: `scripts/reasons.py` (in scratch) on
  `results/run4/nir-results-r4b.jsonl`.
- Zink also writes `gl_ClipDistance` and `gl_CullDistance` outputs a glslang program never writes, so
  A's modules ask a back end for clip distances they do not use; B drops them.

The choice, on this corpus (not final: GLSL and the other sources are still to come):
`corpus/choose`, both routes optimised (`--release`), one shader and route at a time in a child of
its own, on ten cores (`taskset -c 0-9`), x86-64 machine, load 5.5 at the start and 3.2 at the end
(another track's tests ran beside it), before the Zink patch: A 726 match, B 773 match; on the 725
shaders both match, time per shader (to the common form, validated) A median 219 us, mean 261 us; B
median 107 us, mean 125 us. B has more matching shaders and less time per shader: the route is B
(`nir-naga`, `src/direct.rs`). The patch moved A to 728, which changes neither order.

Next, in this order:
1. The GLSL reference on llvmpipe and the GLSL corpus mode. The corpus run takes GLSL programs
   (vertex and fragment pairs, compute alone) through `Nir::from_glsl` to both routes; the
   reference is llvmpipe running the same GLSL through EGL surfaceless (`libEGL` and
   `libgallium` of the pinned build, a GL 4.6 compatibility context), on the same seeded inputs:
   default uniforms set with `glUniform*` from the seed and, on the route's side, written into
   uniform buffer 0 at each parameter's word (the bridge must hand the parameter list's layout out:
   name, type, `ValueOffset`, and the state values, `STATE_FB_WPOS_Y_TRANSFORM` and
   `STATE_FB_PNTC_Y_TRANSFORM`, which the route's side fills as st/mesa does for the target's
   height); blocks bound with the same bytes; vertex outputs by transform feedback, fragment
   outputs into the same targets. Then pin the GLSL sources (piglit's `.shader_test` files and
   dEQP's GLES programs, Godot Compatibility, bgfx, Dolphin) in `corpus/sources`. Legacy varyings
   (`gl_TexCoord`, `gl_FrontColor`: `VARYING_SLOT_TEX0`, `COL0`) reach `builtin_at` as "another
   built-in" and are refused; decide (locations past the generic ones) when the corpus shows how
   many.
2. The rest of the corpus (piglit and dEQP GLSL, Godot Compatibility, bgfx, Dolphin; Godot Forward+
   SPIR-V; the shaders of `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` and
   `dEQP-VK.robustness.buffer_access.*`), each pinned in `corpus/sources`. Then `corpus/choose` again
   over all of it for the final choice.
3. The alpha_to_one programs into the corpus: the fragment programs of
   `dEQP-VK.pipeline.monolithic.multisample.alpha_to_one.*` (VK-GL-CTS makes them in its C++;
   take them from a CTS build's shader dump, pinned), each through `Nir::alpha_to_one` then the
   routes, converted and validated, and listed as held by G10's CTS cases (lavapipe does not run
   `demote_samples`).
4. The rewrites of 15.4 (formatless storage, gathers with run-time offsets, arrays of textures,
   samplers and combined image samplers indexed at run time), which need an entry point in the
   shaders crate (Requests; not there at shaders' tip 2e8abb3e, run 6); robustBufferAccess's three steps
   (needs `HelperValue::BufferRange`).
5. The harness's own limits: multisampled textures, block inputs, programs whose reference does not
   end on the seeded inputs.
6. Candidate A is not developed further (B is chosen); its failures stay listed as they are.

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
  only, one folder up); run 4's in `results/run4/` (`nir-results-r4b.jsonl` the last, with the
  patch; `nir-choice-r4.jsonl` the timed one); `scripts/reasons.py <results> [n]` counts the reasons.
- The choice between the candidates, one command: `crates/wcontainers-nir/corpus/choose [results]`
  (fetches the corpus, builds optimised, runs one shader and route at a time on ten cores; its last
  lines give each route's counts and its median and mean time on the shaders every route matches).
  From the Mac: `x86-run <checkout> --name graphics-nir -- 'WC_NIR_TIMING_CORES=0-9
  crates/wcontainers-nir/corpus/choose $CARGO_TARGET_DIR/nir-choice.jsonl'` (about 30 minutes for 1005 shaders, most of it the six hanging
  `irradiancecube.frag` runs). `WC_NIR_SHADERS=<n>` takes a seeded sample (for the independent check).
- The licence of every linked Mesa file: `crates/wcontainers-nir/mesa/linked-licences [file]` (after
  the Mesa build; the route test linked with a map, each kept object traced to its source).

- Build Mesa, from the Mac: `orchestrator/cloud/x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/mesa/build path:/work/flake/graphics-nir'`. On any Linux machine with Nix:
  `crates/wcontainers-nir/mesa/build` (its argument, the flake, defaults to the checkout). The tools
  come from Nixpkgs' own `mesa` package (`nix print-dev-env --inputs-from <flake> nixpkgs#mesa`), so the
  repository's flake is unchanged and the tools are those of its locked nixpkgs. Output in
  `$CARGO_TARGET_DIR/mesa/{src,build,install}` (target/mesa without it). The script refuses a tag whose
  commit is not the pinned one, makes the build folder again when the commit or the options change
  (Meson reads a machine file only when a folder is made), and fails unless llvmpipe and lavapipe were
  built.
- GLSL tests: `cargo test -p wcontainers-nir --test route --test glsl_buffers` (with
  `WCONTAINERS_MESA`, on the x86 machine).
- Test with Mesa (on the x86 machine, after the build):
  `export WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build; cargo test -p wcontainers-nir` and
  `cargo clippy -p wcontainers-nir --all-targets -- -D warnings`; and both again without the variable.
- The literal check needs the git history, which the rented machine's copy lacks. Run 6 ran it on
  the x86 machine from a git bundle of the branch (`git bundle create <scratch>/nir.bundle
  phase-2-graphics-nir`, sent with `x86-run --ssh 'cat > /work/probes/graphics-nir.bundle'`,
  cloned there, `cargo fetch`, then the check in `nix develop`; the clone and its build deleted
  after). Runs 1 to 5 ran it on the Mac (`RAYON_NUM_THREADS=3 cargo run -j 4 -q -p
  wcontainers-literals -- check`). With
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
  machine `/work/target/graphics-nir` (11 GB after run 4: Mesa, the corpus, cargo's debug and release output, the last results) is kept.

## Measurements

- Mesa 26.3.1 clean build (configure, 1239 steps, install), x86-64 machine (c7i.metal-48xl, 192
  threads), `-j 64`: 45.9 s wall, 11 min 37 s user, load average 4.96 at the start and 10.51 at the
  end (other tracks' jobs). An earlier clean build of the same options with SPIRV-Tools: 45.9 s wall
  at load 0.43 at the start.
- Corpus run with the comparison, 1005 shaders, 192 children, x86-64 machine at load 0.3 rising to
  5.3: 124 s including an incremental build (run 3).
- The whole suite with the SPIR-V corpus compared (run 6, after the GLSL work; x86-64 machine,
  192 children, load 1.0 at the start and 12.5 at the end, 245 s with the build;
  `results/run6/nir-results-r6.jsonl`): the same counts as runs 4 and 5, A 728 match, B 773
  match, 0 differ, 8 not compared on each.
- `tests/glsl_buffers.rs` and `tests/route.rs` (run 6): 0.08 s and 0.01 s, x86-64 machine, load
  0.5 to 4.2.
- `tests/narrow.rs` (2,097,152 invocations, both programs on lavapipe): 0.47 s, x86-64 machine,
  load 0.08 (run 5).
- The choice's timing (run 4, `corpus/choose`, x86-64 machine, ten cores, one child at a time, load
  5.5 at the start and 3.2 at the end with another track's tests running): 1781 s in all; time per
  shader to the validated common form on the 725 shaders both match: A median 219 us, mean 261 us;
  B median 107 us, mean 125 us. The same at 192 children at once (load 9 to 25): A median 383 us, B
  median 196 us.

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
  `libcompiler`, `libmesa_util`, `libblake3`, `libmesa_util_c11`, and since run 6 `libglsl_util`
  and `libglcpp_standalone` (the linker keeps only the objects used). Run 6, with GLSL linked
  (`results/run6/nir-linked-licences-r6.txt`): 243 files, 77 of them from the GLSL libraries
  (`libglsl` 68, `libglsl_util` 4, `libglcpp` 3, `libglcpp_standalone` 1, `libglsl_standalone`
  1), all with the MIT permission text. Four of those are generated and carry a second notice:
  `glsl_parser.cpp` and `glcpp-parse.c` are Bison's output (Bison's skeleton is GPL-3.0-or-later
  with the Bison special exception, which lets the output be distributed under any terms),
  `glsl_lexer.cpp` and `glcpp-lex.c` are flex's output (flex's own BSD-style licence). The other
  166 are run 4's files and licences, unchanged. Kept by the linker in run 4 (`mesa/linked-licences`; list in scratch
  `results/run4/nir-linked-licences.txt`): 129 files, from `libnir` 65, `libmesa_util` 37, `libvtn`
  14, `libblake3` 7, `libcompiler` 3, `libzink` 2, `libmesa_util_c11` 1. Their licences: 89 carry
  the MIT permission text, 28 the SPDX tag MIT, 2 are generated by Mesa's build from MIT scripts
  (`nir_opcodes.c`, `nir_opt_algebraic.c`), 1 BSL-1.0 (`c11/impl/threads_posix.c`), and 9 carry no
  text in the file: the 7 of `util/blake3` (BLAKE3 1.8.2 copied verbatim, its README says;
  upstream's licence is CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception),
  `util/softfloat.c` (Berkeley SoftFloat 3e, BSD-3-Clause text in the file, which the script's MIT
  match does not count) and `util/format/u_format_etc.c` (MIT text in the header it includes,
  `texcompress_etc_tmp.h`). All permissive. GLSL's libraries are not linked yet; the list is made
  again when they are.
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
  finding; and where the fork 15.2 describes is kept is the owner's decision (15.9). Until then the
  patches live as files in `crates/wcontainers-nir/mesa/patches/` (one so far, Zink's), applied by
  the build script; they move into the fork when there is one.
- The Mesa build runs on the rented x86-64 machine, not the laptop (15.8 says laptop): the brief of
  2026-11-19 makes the rented machines the default. The choice's times (15.3: "measured on the
  laptop") are taken there too, alone on ten cores, as that machine's notes take timings.
- The choice is made on the SPIR-V corpus first; it is final only when GLSL and the other sources
  are in and `corpus/choose` is run again over all of them.
- Zink is built beside llvmpipe and lavapipe: its `nir_to_spirv` is candidate A's first step.
  SPIRV-Tools is off (Mesa takes it when present; nothing of ours needs it).
- Candidate B reads NIR through a flattened copy the bridge makes (`bridge/flat.c`), not through
  NIR's own structures from Rust: bindgen is out (below), and the copy keeps Mesa's numbering and
  layout on the C side, so a Mesa update changes C only.
- The bridge's functions are declared by hand in `src/mesa.rs` and `src/direct.rs`, not by bindgen: bindgen's output
  is included from Cargo's `OUT_DIR`, whose name the literal check refuses as a new code string here
  (its one constant, `CARGO_OUT_DIR`, is private to wcontainers-literals).

- Lowered IO keeps a stage value's interpolation only where a fragment input is loaded. Route B
  gives a vertex output Perspective (Flat for an integer one); where a fragment program reads it
  another way, the pipeline (G9, G10) must give the vertex side the fragment side's. The plan does
  not say; the comparison runs each stage alone, so it does not see this.
- The 16-bit test's reference is lavapipe running the same program with native 16-bit integers and
  Rust's 16-bit arithmetic, both, rather than a corpus mode (the program is the test's own).
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
- GLSL is compiled on Mesa's standalone scaffolding with st/mesa's steps after a link written out
  in `glsl.cpp` for one set of capabilities, not by st/mesa itself (which needs a gallium screen
  and context: G9's driver). The steps and their order are st/mesa's; the shader-variant steps
  st/mesa takes at draw time (clamped colours, flat shading, two-sided colour, user clip planes,
  point size), which depend on the draw's state, are not taken here: G9 hands its variants over
  and its own corpus run holds them.
- OpenGL's buffer accesses reach route B as arrays of words, not as typed blocks: after
  `gl_nir_lower_buffers` only byte offsets remain (the block types are gone from the
  instructions), so the bridge makes the words form, the one 15.4 names for robustBufferAccess.
  Program binding for OpenGL: uniform block index, storage block index after them.
- Vulkan capability classes: a program needing what our Vulkan 1.1 will not report is counted apart,
  not as a route failure (the pass bar's "named reason").

## Interface changes

None.

## Requests to other tracks

- Interface track (15.7): everything 15.7 lists; G8 needs `HelperValue::BufferRange` and the slow path
  names `RobustAccessInShader` and `AlphaToOneInShader` before its robustness and alphaToOne parts can
  be reflected.
- Shaders track (a finding): naga's SPIR-V reader fails on HLSL's row-major array of matrices once
  it is decorated (`shadowmappingcascade/scene.frag`, "invalid access As" on candidate A); not needed
  by route B.
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
- Interface or shaders track (later, when the Vulkan driver reports `vkCmdDispatchBase`): a helper
  value for the base workgroup; the routes now count dispatches from workgroup 0.
- Literals track: `CARGO_OUT_DIR` public (in a public module), so a build script can name `OUT_DIR`
  through it.

## Literal check

Run 6: exit 0 at 4294fcf0 (x86-64 machine, from a git bundle; marks 0; the 18 changes that could
add lines are the shaders branch's, as before). The bridge's numbers are `#define`s, named once;
the Rust added has no new bare literals (test data apart). Adopted with the branch (merged from the
shaders branch, which carries it). wcontainers-nir has no
baseline lines: the check passes with none for it (run on the Mac, runs 1 to 4, exit 0; in run 4 the
two new scripts' numbers were taken out (the timing cores are passed in, the load read by `read`), and
one baseline line the Direct3D 11 merge left stale was deleted, as the check asked;
it needs the git history, which the rented machines' copies lack). The list of
changes that could add lines is the 18 the shaders branch brought; this branch added none. C files are
not scanned; the bridge names its one version by `#define`s all the same.
