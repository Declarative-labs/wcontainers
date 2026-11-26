# Status: graphics implementations

The brief is orchestrator/restart-2026-11-25/brief-graphics-implementations.md in the scratch folder. Logs of every
run named here are in the scratch folder graphics-implementations/.

## GI1, the graphics tree on main (2026-11-25)

- Base: main 95bceadf0. The graphics folders, third_party and five status files come from graphics-layout 97e1daafe
  by path, unchanged except the nine lints tables, which inherit the workspace as main's did, and their TODO lines.
- `cargo check --all-targets`, x86-64 and ARM64, Mesa off: every graphics package exits 0 except
  wcontainers-gpu-webgl (its library holds 160 lines of unsafe code, which the workspace forbids) and
  wcontainers-shaders (only through its dev-dependency on gpu-webgl). Mesa on, x86-64: wcontainers-nir's library
  fails (59 errors), wcontainers-opengl through it.
- Tests, both machines: gpu 37, gpu-test 202, shader-converters linked 7 and contained 7, directx-graphics-infrastructure
  13, graphics-test 16, direct3d-11 69 pass; gpu-webgpu 8 pass and 121 stop at the test helper, which refuses an
  adapter that is not a GPU.
- With the old lints table in a scratch copy only: shaders 157 (5 ignored), gpu-webgl 17, nir 41 (2 ignored),
  opengl 11 pass, and the wasm32 check exits 0, as on graphics-layout.

## GI2, the WebGPU suite on lavapipe (2026-11-25)

- The build machines have no GPU. Mesa's lavapipe, from the pinned Mesa that crates/nir/mesa builds (108 s on the
  x86-64 machine), is their Vulkan driver: the WebGPU tests now take its software adapter when `VK_DRIVER_FILES` or
  `VK_ICD_FILENAMES` names its manifest, and still refuse a software adapter otherwise.
- `cargo test -p wcontainers-gpu-webgpu` with `WEBGPU_API=vulkan`, both machines alike: 124 passed, 4 failed,
  2 ignored (before: 8 passed, 121 stopped at the adapter). The 4 failures, none ours:
  - `bc_textures_read_back_as_written_and_bc6h_and_bc7_sample_exactly`: lavapipe's own BC decoder differs from ours
    by up to 1.1e-2 (BC4 signed), 3.9e-3 for the 8-bit formats.
  - `depth_and_stencil_parts_copy_and_read_back_exactly` and the shared suite's
    `a_copy_within_one_depth_or_stencil_subresource_moves_the_region`: lavapipe stores `depth24plus` scaled by
    2^24 - 1 with rounding, so 4 of 32 texels are one step of 2^-24 off (0.6249999 for 0.625), a last-bit
    difference of the kind two real devices show.
  - `present_shows_the_frame`: the test reads a window only on Windows and macOS.
  - Ignored as on the desktop's Vulkan: `wireframe_fill_matches_direct3d` (Vulkan's line rules; the solid cell
    matches) and the BC one-step comparison.
- Found by it and fixed: a NaN copied from `r32float` into `depth32float` by a draw stayed whatever the device's
  depth clamp made of it (lavapipe: one); the copy's program now writes zero, as plan 4.12 lists and as the
  RTX 3080 and Apple M5 did.
- fmt and clippy (`--all-targets --no-deps -D warnings`) of wcontainers-gpu-webgpu exit 0.

## GI3, gpu becomes the interface (started 2026-11-25)

- Step 1 done (decision D3 of the brief): `dispatch`, `capture_vertices`, `create_resource_table`,
  `write_resource_table`, `write_timestamp` and `timestamps_disjoint` have no default; each implementation that
  lacked one now answers `GpuError::Absent` itself (Gl, WebGpu, the recorder and the three test recorders; gpu-webgl's
  tracer already forwarded each). No answer changed. The front hunks are the two test recorders of direct3d-11 and
  directx-graphics-infrastructure only.
- Both machines: checks as after GI1; gpu 37, gpu-test 202, directx-graphics-infrastructure 13, direct3d-11 69 pass;
  in the scratch copy shaders 157, gpu-webgl 17, opengl 11 pass and wasm32 exits 0; WebGPU on lavapipe as GI2;
  fmt and clippy `-D warnings` exit 0 on the six crates touched. nir's 41 pass on x86-64.
- Found, not caused by this: on ARM64 nir's tests do not link with Mesa on (ld.bfd: `_mesa_reference_shader` and
  others defined in both libmesa and Mesa's glsl standalone scaffolding). The graphics tests had only ever run on
  x86-64. Left for GI8.
- Step 2 done (decision D4, second run): `StandaloneGpu: Gpu` with `type Config`, `type NewError: Debug` and
  `fn new(config) -> Result<Self, Self::NewError>`. Implemented by the recorder (`RecorderConfig { caps }`, its
  default the full caps, never fails), WebGpu (`WebGpuConfig { adapter, device, queue }`, never fails) and Gl
  (`GlConfig { angle, off }` natively, `{ context, off }` in the browser; the error is ANGLE's text). The inherent
  constructors stay until GI4 moves every caller onto the suite. Two tests build a recorder from its config.
- Both machines, after main e6672b3eb was merged in: every graphics package checks with `--all-targets` exactly as
  after GI1 (gpu-webgl's library 166 errors, 165 of them `unsafe`; shaders only through it; its library exits 0);
  gpu 37, gpu-test 204 (202 and the 2 new), shader-converters linked 7 and contained 7,
  directx-graphics-infrastructure 13, graphics-test 16, direct3d-11 69 pass; clippy `-D warnings` and fmt of gpu,
  gpu-test, gpu-webgpu exit 0; Mesa builds (120 s ARM64, 132 s x86-64); in the scratch copy shaders 157 (5 ignored),
  gpu-webgl 17, opengl 11 pass, nir 41 (2 ignored) on x86-64 and does not link on ARM64 as before; wasm32 exits 0;
  WebGPU on lavapipe 124 passed, 4 failed, 2 ignored, the same four as GI2.
- Cargo.lock: main's lock lacks the two observation-collector packages; cargo adds them (15 lines), the same on both
  machines. No graphics package's version changed.
- Next: steps 5 and 6, the crate made by the tool's layout (`[lib] path = "interface.rs"`): the trait, its data and
  constants in interface.rs and `pub mod` data files; the shared code (`Helpers`, `PassTracker`,
  `OcclusionCounter`, the inherent methods of `FormatCaps`, `TextureDesc`, `ShaderModule`) as split impl blocks in
  sibling files, each `stray_source` they raise listed; the three free functions become methods
  (`generate_mips_by_draws` of `Helpers`, `color_target_bytes` and `helper_binding` of their data types). Public
  paths move into the data modules, so the users' imports change: 34 files outside gpu use those three functions and
  about 230 files import from the crate; each front hunk is an import or path only.

## The second run (2026-11-25)

- ORCHESTRATOR RULINGS taken in: the graphics API crates track owns the OpenGL ES with EGL crate and the Mesa
  compiler crate, so this track writes no glue and no marked `unsafe`. What each trait must offer, call by call,
  with what is lent and for how long, is in the scratch folder's graphics-implementations/NEEDED-FROM-API-CRATES.md
  (A: OpenGL ES, 145 GL ES calls of gpu-webgl, 10 extension calls, the EGL calls, the browser route, the reference
  renderer's desktop GL and Vulkan for the API track to place; B: Mesa's compiler, its 45 bridge functions, 43 in
  mesa.rs and 2 in direct.rs, and the flat form). Where code that implements no trait lives is not the owner's question: shaders stays a plain library crate;
  its `stray_source` findings stay listed below.
- What does not build under `forbid` until those traits exist, and why: gpu-webgl's library (every glow call is an
  `unsafe fn`, and so are the EGL calls taking pointers and the extension calls; no safe route exists without the
  OpenGL ES crate), shaders' tests and directx's `webgl` feature through it, and with Mesa on nir's library (59
  errors: mesa.rs and direct.rs's bridge calls) and opengl through it. They are not hidden behind a feature or a
  lowered lint.
- The check's other blocking and non-blocking points were not found: neither this file nor any page of the scratch
  folder holds them (searched for the branch name, GI1 and the check's folder, which holds only its job logs). Its
  logs show only what is known (gpu-webgl and shaders under `forbid`). They are repaired when the orchestrator
  passes them on.
- Linter da55b9d7 (x86-64 machine, no model key, so the model's rules report unanswered): layout findings in the
  nine crates are as at GI1 (stray_source 169, conformance_missing 7, crate_not_member 1; also
  depends_on_implementation 14, shader-converters' conformance_untested_method 1, test-window's 6, none from this
  run). The new code adds no layout finding; its text findings are the model's unanswered summaries and names,
  which need the key on the Mac.
- For the Metal track (`graphics-metal`, `MetalGpu<M>`): add the six required methods (`dispatch`,
  `capture_vertices`, `create_resource_table`, `write_resource_table`, `write_timestamp`, `timestamps_disjoint`),
  answering `GpuError::Absent` where it lacks one; implement `StandaloneGpu` with a plain-data config holding its
  Metal value (`MetalGpuConfig<M> { metal: M }`, `NewError = Infallible`); after GI3's steps 5 and 6 change its
  imports to the data modules; at GI4 move to `crates/gpu/implementation/metal`, export only `MetalGpu` and its
  config, drop the inherent `new`, and run the conformance crate's `tests!` from its suite.

## The third run (2026-11-26): GI3, GI4 and GI5 done, GI6 measured

### The last check's blocking point

- It found one: the run did one step and stopped although nothing waited. This run took GI3 to its end and went on
  through GI4 and GI5 and GI6's measurement. The check's own jobs (gi-check-r2) were stopped by the machines after
  51 s and 58 s (exit 143), so they hold no other finding.
- His main ac696ea19 ("verus compat", crates/executor/interface.rs only) was merged in first (65104afb4); no
  graphics crate names the executor.

### GI3, gpu as the interface crate (bcf7e0175)

- crates/gpu: `[lib] path = "interface.rs"` (the tool's template, written by hand: the tool is not run on the Mac).
  interface.rs holds `Gpu`, `StandaloneGpu`, the call data (`IndexSource`, `RingKind`, `FanOrLoop`, `ClearValue`,
  `PrepareDesc`, `Object`) and the four constants. Twelve `pub mod` data files: caps, draw, error, format, helpers,
  occlusion, passes, reflect, resource, shader, slow, state. Users import from them (`wcontainers_gpu::format::Format`).
- The shared code is impl blocks of the interface's own types in sibling files (the 14:10 ruling): caps_methods,
  defaults (the provided methods' helpers and `Helpers::generate_mips_by_draws`), format_methods
  (`Format::color_target_bytes`), helpers_methods, occlusion_methods, passes_methods, reflect_methods
  (`ShaderStage::helper_binding`), resource_methods, shader_methods; tests in defaults/tests.rs,
  helpers_methods/tests.rs and slow_tests.rs. The free functions became associated functions of those types.
- Plain data needs public fields, so `Helpers`, `PassTracker`, `OcclusionCounter` and their parts are public now:
  `Builtin`, `MipRead`, `Builtins`, `HelperRing`, `KeptRange`, `KeptDraw`, `PooledTexture`, `TemporaryTarget`,
  `OpenPass`, `PassQueries` (renamed from the private names so that no glob import meets `Range` or `Open`).
- Front hunks: imports and paths only (MEASURED: every changed line outside the graphics crates is a `use` line or
  a path, or the same statement rewrapped by rustfmt), in direct3d-11, direct3d-12, directx,
  directx-graphics-infrastructure.
- MEASURED, both machines: every graphics package checks with `--all-targets` exactly as after GI1 (gpu-webgl's
  library 166 errors, 165 of them unsafe; shaders through it; nir with Mesa 59; opengl through nir); gpu 37,
  gpu-test 204, direct3d-11 69, directx-graphics-infrastructure 13, graphics-test 16, shader-converters 7 and 7
  pass; scratch copy: shaders 157 (5 ignored), gpu-webgl 17, opengl 11, nir 41 (x86-64; ARM64 does not link, as
  before), wasm32 exit 0; lavapipe 124 passed, 4 failed (GI2's four); clippy `-D warnings` and fmt exit 0.
- Linter da55b9d7 on crates/gpu: interface_item 0, item_out_of_order 0 (`cargo reviewable format` changed nothing),
  stray_source 12 (the sibling files above, by the 14:10 ruling, which the linter does not know yet; the linter's
  branch `ruled-2026-11-25` accepts own-type impl blocks), explicit_shared_error 1 (`GpuError` is the error of 31
  methods; the rule's exclusion needs his word, so it is listed for his review of interface.rs).

### GI4, the conformance crate and three implementations (44f91fef7)

- crates/gpu/conformance (`wcontainers-gpu-conformance`, only target suite.rs): gpu-test's suite and its draws and
  routes in one file (the linter takes suite functions only from suite.rs), each suite function taking its back end
  by value; `suite!` is `tests!($value; StandaloneGpu: yes)` in the tool's fixed shape. gpu-test's `Host` trait is
  gone. `present_shows_the_frame` needed the host to read its window, so the suite has `presents_cleared_frames`
  (window 1, four surface calls, device still working) and each implementation reads its own window in its own test
  (recorder: unit test; webgpu: tests/surface.rs and tests/present.rs; webgl: tests/suite.rs).
- Seven suite functions added so the suite calls every method (`slow_paths_are_counted_and_warned_once`,
  `ring_writes_place_each_part_at_an_aligned_offset`, `binding_sets_of_one_layout_share_its_key`,
  `fences_are_done_once_waited_on`, `fans_cover_their_triangles`, `clear_texture_fills_every_texel`,
  `absent_abilities_answer_absent`). 51 suite tests.
- crates/gpu/implementation/recording (`wcontainers-gpu-recording`, was gpu-test): `Recorder` and `RecorderConfig`
  are declared in implementation.rs, their fields crate-private; the old tests are unit tests (unit_tests/);
  conformance_run.rs is one invocation. The package wcontainers-gpu-test is gone.
- crates/gpu/implementation/webgpu and webgl (packages keep their names): moved as folders, every path out of them
  two levels deeper. `WebGpuConfig` is plain data now (`backends`, `software`): `new` opens the adapter and device
  itself (a native wgpu request answers at once), so conformance_run.rs is one invocation. The browser builds from its
  own device through the inherent constructor.
- webgl keeps a private `Host` trait in examples/common/checks.rs (its own checks make two devices and read a window;
  moved there from gpu-test, not new): a `trait_outside_interface` to settle at GI7. Its ANGLE host moved to
  examples/common/angle.rs, with the one Windows `unsafe` line (`add_window`); webgl's count is unchanged (160).
- MEASURED, both machines: recording 212 (161 unit, 51 suite), gpu 37, direct3d-11 69, dxgi 13, graphics-test 16;
  webgpu on lavapipe 131 passed, 5 failed, 2 ignored: GI2's four and `presents_cleared_frames`, which fails for the
  same reason as `present_shows_the_frame` (no window on the build machines: "no window of that number was
  registered"); scratch copy shaders 157, gpu-webgl 17, opengl 11, nir 41 (x86-64), wasm32 exit 0; clippy and fmt
  clean.
- Linter on crates/gpu at GI4: trait_not_implemented 0, conformance_missing 0. conformance_not_run 3:
  webgl/conformance_run.rs starts with `#![cfg(windows)]` (ANGLE is Windows only; without it the suite fails on
  Linux), and the two browser pages (webgpu examples/scenes/suite.rs, webgl examples/browser.rs) call suite functions
  one per page. conformance_untested_method 1: `StandaloneGpu::new`, which the tool's fixed shape gives no group test
  (the same finding his shader-converters interface has). Not in GI4's bar and listed: public_outside_interface
  (the implementations' inherent constructors and methods; recording 123, webgpu 175), implementation_folder 2
  (webgpu's and webgl's own test and example targets), unparsed 63 (`#[path]` into graphics-test).

### For the Metal track (what section 4 of its status lists, checked against this layout)

- `crates/gpu/interface.rs` exists; the imports become `wcontainers_gpu::{Gpu, StandaloneGpu, caps::Features,
  error::GpuError, resource::BindingEntry}`.
- crates/gpu/implementation/metal is the folder; members already match `crates/*/implementation/*`.
- conformance_run.rs must be one invocation, e.g. `wcontainers_gpu_conformance::tests!(<wcontainers_gpu_metal::
  MetalGpu<..> as wcontainers_gpu::StandaloneGpu>::new(..config..).expect(".."); StandaloneGpu: yes);`. The shared
  suite's six new programs go into crates/gpu/conformance/suite.rs (one file), each suite function taking `mut b: B`,
  and into `tests!` in file order. `presents_cleared_frames` presents into window 1, so a value made for the suite has
  window 1 where it can.
- New provided methods (GI5) have defaults, so Metal need add nothing.

### GI5, guest bytes through VolatileBytes (b62be23ec)

- `Gpu::write_buffer_from`, `write_texture_from` and `finish_read_into` take `&impl VolatileBytes`; the defaults copy
  out once and call today's methods. gpu depends on wcontainers-volatile-bytes (workspace crate).
- WebGpu overrides `write_buffer_from` (a write the queue takes now goes from the bytes into wgpu's staging memory
  through `read_to` and a sink calling `queue.write_buffer`, one copy) and `finish_read_into` (a mapped read back of
  one run goes from the mapping into the bytes, one copy). Texture writes keep the default: wgpu has no write-with
  for textures.
- Suite: three tests with bytes that change during the call (`Drifting`: every byte goes up by one after each read):
  `buffer_writes_from_changing_bytes_land_as_held`, `texture_writes_from_changing_bytes_land_as_held`,
  `read_backs_finish_into_changing_bytes`. MEASURED pass on recording (215) and webgpu on lavapipe (134 passed, the
  same 5 failed) on both machines.
- MEASURED (webgpu tests/volatile.rs, release, lavapipe, 64 x 4 MiB, three runs):

| machine | write, copied out first | write, copied once | saved | finish read, copied out | finish read, once | saved |
|---|---|---|---|---|---|---|
| x86-64 | 295,234 to 301,859 ns/MiB | 164,437 to 172,549 | 122,685 to 137,116 | 190,685 to 195,191 | 76,049 to 86,009 | 109,182 to 119,023 |
| ARM64 | 135,026 to 144,851 | 77,292 to 79,729 | 55,296 to 66,126 | 94,008 to 105,011 | 37,849 to 39,010 | 56,159 to 66,643 |

### GI6, shaders on the contained converters: the measurement

- conversion_time (crates/shaders/examples) now reports the 99th percentile beside the 95th. Release, the corpora
  on the machines (/work/probes/shaders-corpus: dxbc 191, games 973, runner 3,020 files), each converted shader's
  first conversion (what a program pays; conversions are cached), median / 99th percentile / worst, MEASURED:

| x86-64 | linked | contained |
|---|---|---|
| process's first conversion | 0.7 to 1.0 ms | 149 to 160 ms (the module's compile) |
| corpus DXBC (164) | 122 / 356 / 408 us | 311 / 641 / 782 us |
| games DXBC (467) | 286 / 15,421 us / 16.7 s | 520 / 28,816 us / 38.2 s (bf4b9f4770dfd7ca.dxbc) |
| games DXIL (350) | 493 / 39,095 / 43,247 us | 867 / 49,167 / 51,277 us |
| runner DXBC (1,399) | 157 / 556 / 1,001 us | 304 / 867 / 1,695 us |
| runner DXIL (1,327) | 230 / 562 / 687 us | 482 / 876 / 1,048 us |
| all first conversions summed | games DXBC 17.2 s, DXIL 0.76 s; runner 0.57 s | games DXBC 39.0 s, DXIL 1.07 s; runner 1.12 s |

| ARM64 (95th percentile; the run started before the 99th was added) | linked | contained |
|---|---|---|
| process's first conversion | 0.5 to 0.9 ms | 316 to 328 ms |
| corpus DXBC (164) | 113 / 266 (95th) / 411 us | 285 / 540 (95th) / 849 us |
| games DXBC (467) | 289 / 2,706 (95th) us / 16.4 s | 528 / 5,303 (95th) us / 48.7 s |
| games DXIL (350) | 487 / 5,240 (95th) / 41,953 us | 927 / 9,564 (95th) / 60,100 us |
| runner DXBC (1,399) | 131 / 322 (95th) / 921 us | 231 / 532 (95th) / 1,950 us |
| runner DXIL (1,327) | 166 / 315 (95th) / 609 us | 384 / 591 (95th) / 942 us |

- The two routes' outputs: MEASURED (x86-64), the corpus test over all nine corpora with every written file kept
  (`WCONTAINERS_SHADERS_OUT`), linked and contained: 13 of 13 tests pass on each, 4,557 files each, `diff -r` 0
  lines. So the 122 of 1,284 converter calls whose bytes differ (graphics-layout pass 2: SPIR-V ids numbered
  differently, DXBC IR instructions in another order, lengths equal) give the same WGSL, GLSL, verdicts and cache
  records: INFERRED from that, the differences are in numbering and order only (C++ leaves the order of
  evaluation of call arguments unspecified, and the host compiler and wasi-sdk's clang choose differently).
- Fuzz, contained route, ARM64, 12,000 cases at the seeds 141421, 2236067 and 1732050: each seed passes (10 of 10 tests; 4,584 s, 6,091 s and 4,757 s), 0 process ends in 36,000 cases; every former process end is now a named refusal: "dxbc-spirv ran past its memory bound of 268435456 bytes" (33, 24, 27), "dxbc-spirv stopped: wasm trap" (105, 90, 78), "dxil-spirv ran past its memory bound" (9, 7, 4) among them (MEASURED, f6-*.err).
- The saved stage.rs and helper.rs hunks (SCRATCH/rename-crates/uncommitted/wcontainers-agents.patch lines 177 to
  276) READ: the domain stage's dispatch reads the tessellator's point total (`tess::POINT_TOTAL`, copied into a
  buffer at `INDICES_BINDING`) and sets `word::LISTED_TOTAL` to 1, so invocations past the total write records
  outside every clip volume, as the vertex stage already does for a listed strip (src/stage.rs reads
  `LISTED_TOTAL`). Not brought yet: it needs a tessellation draw test on a device (lavapipe), which is the next
  step of GI6.
- The removal of the linked route and its 5 `unsafe` lines waits on his question 1: the contained route costs
  150 to 330 ms once per process, about twice per shader, and 22 to 32 s more on the worst game shader, a cost a
  user would see.

### GI8 part A (READ, nothing to change)

- Both SECURITY findings were already fixed on graphics-layout, which this branch carries: the enum values from the
  C bridge are read by zerocopy's checked `TryFromBytes` (nir src/direct.rs `variant`, `deref_kind`), with the
  test `a_value_the_bridge_does_not_write_is_no_variant`; `PIPE_QUERY_OCCLUSION_COUNTER` (SAMPLES_PASSED) is
  counted (opengl src/query.rs), and `refuse` unwinds into the context's own reset instead of aborting (tests
  `a_refused_call_answers_its_dropped_value_and_its_context_reports_its_own_reset` and two more). MEASURED in the
  scratch copy: nir 41 and opengl 11 pass (x86-64).
- direct.rs's five `unsafe` lines are the bridge calls themselves (`wc_flat_free`, `from_raw_parts`,
  `CStr::from_ptr`, `wc_nir_flatten`), not enum reads: they go with the Mesa compiler API crate (part B).
- Not added: a hostile test that makes a SAMPLES_PASSED query through st/mesa needs a GL context over our Gallium
  driver, which part B's tooling gives.

## The fourth run (2026-11-26): the check of GI4 did not hold; its repair

### What the check measured and what this run measured first

- The check (linter da55b9d7 on a copy with gpu-webgl's old lints table, so its compiler view exists) found GI4
  reported done with its bar not met: conformance_not_run 4, conformance_untested_method 1. MEASURED again here at
  1282b3928 the same way: conformance_not_run 4, conformance_untested_method 1, public_outside_interface 124
  (recording), 175 (webgpu), 425 (webgl), implementation_folder 2, trait_outside_interface 2, crate_not_member 1,
  constant_defined_twice 1 in webgpu and 1 in webgl. GI4 is not done; this run did not call it done either.
- His main is ac696ea19, already merged into the branch (`git merge-base --is-ancestor`), so there was nothing to
  merge; no graphics crate names the instruction interfaces that changed.

### What changed, by finding (counts before and after, linter da55b9d7, x86-64, MEASURED)

| rule (crates/gpu) | before | after | how |
|---|---|---|---|
| conformance_not_run | 4 | 1 | below |
| conformance_untested_method | 1 | 1 | blocked: below |
| implementation_folder | 2 | 0 | webgpu's and webgl's library moved from src/lib.rs to implementation.rs at the folder root, its modules beside it (`[lib] path`) |
| trait_outside_interface | 2 | 0 | webgl's checks take their host from a `host` module the including file gives (the browser page, the ANGLE test), not a trait; the scenes' `Plain` trait became bytemuck's `Pod` |
| crate_not_member | 1 | 0 | the ANGLE DLL tool (webgl/angle) is no crate any more: build.cmd writes its two-line package into the build folder |
| constant_defined_twice (implementations) | 2 | 0 | webgpu's edge program uses the interface's `STORAGE_WORD`; draw-cost's `SIZE` is `TARGET_SIDE`, the fact it is |
| public_outside_interface | 124 / 175 / 425 | 4 / 83 / 177, trace 15 | below |

- conformance_not_run: the two browser pages (webgpu examples/scenes/suite.rs, webgl examples/browser.rs) no longer
  name the conformance crate: webgpu's suite page and its runner (web/suite.html, suite.mjs, the steps of web/check)
  are gone; webgl's page keeps its own checks only. The shared suite therefore no longer runs in Chrome (a WebAssembly
  runner of conformance_run.rs would bring it back). webgl's conformance_run.rs has no `#![cfg(windows)]`: its config
  names the folder of any EGL library in `WEBGL_ANGLE_DIR` (default ANGLE's on the desktop), and the back end takes an
  off-screen-only configuration where the display has no windows, so it runs on Mesa's EGL (llvmpipe, surfaceless) on
  the build machines. `Traced` left webgl: it is the implementation crates/gpu/implementation/trace
  (`wcontainers-gpu-trace`), exported, built through `StandaloneGpu` from `TracedConfig { inner, file }`, its suite run
  on `Traced<Recorder>`; the stream format and `Replay` came with it, and the replay page and a record-then-replay
  test (now on the recorder, so it runs anywhere) moved there. The ANGLE-only `impl Traced<Gl>` (on_angle, add_window)
  went, so directx's `Chosen` is `Gl`; recording a real program's frames through directx waits until directx builds
  `Traced<Gl>` itself.
- The one conformance_not_run left is the trace crate's run: its value must write `RecorderConfig { .. }`, which names
  the recording crate outside a type argument. It and conformance_untested_method (`StandaloneGpu::new`) have one
  cause, for the orchestrator (smallest example in the run's report): no suite function and no wrapper's run can make
  a value of `StandaloneGpu::Config` without naming an implementation, and the tool's `tests!` shape gives the group
  arm no config. His shader-converters, test-window and win32 interfaces have the same finding.
- public_outside_interface: every `pub` item no other crate can reach is `pub(crate)` (rustc's `unreachable_pub`,
  applied by `cargo fix`), and so is every `pub` field of a crate-private struct; the recorder's config is plain data
  without `Default` or `Clone` (`RecorderConfig { caps: None }` declares everything it records), and neither config
  derives `Clone`. Left, each a design step of its own: the inherent methods and impl blocks on the exported back ends
  that tests, examples and directx call (`add_window`, `on_angle`, `counters`, `device_descriptor`, the inherent `new`),
  the handle types filling associated types (private fields, `PartialEq`/`Eq` derived), webgl's `pub mod platform` and
  `glsl`, the trace crate's `Frame` and `Replay` (the replay page uses them).

### The fifth lavapipe failure, and D6 in the conformance value

- `presents_cleared_frames` failed because the suite presented into window 1 while every value given to `tests!` is
  built by `StandaloneGpu::new`, which names no window: the interface has no call that gives a back end a window, so
  it failed on any machine, not for want of a GPU (the recorder passed because it accepts any window number). The test
  now presents where `create_surface` succeeds and otherwise checks that the refusal loses no device. MEASURED on
  lavapipe, both machines: it passes; webgpu 135 passed, 4 failed (GI2's four), 3 ignored.
- D6: `WebGpuConfig { backends, lavapipe }` takes a CPU adapter only when it is lavapipe and `VK_DRIVER_FILES` or
  `VK_ICD_FILENAMES` names its manifest; the test helper asks the back end itself. MEASURED, both machines: with
  lavapipe named, the webgpu conformance run 53 passed, 1 failed (the depth24plus rounding); with lavapipe loaded
  through a manifest of another name, the run refuses it ("the adapter is the software llvmpipe ...").

### Measured at this commit (both machines alike unless named)

- `cargo check --all-targets` under the workspace lints: every graphics package exits 0 except gpu-webgl (167 errors,
  165 of them unsafe) and shaders through it, as before; the new trace crate exits 0.
- Tests: gpu 37, recording 215, trace 55 (51 suite, 3 volatile-bytes, 1 record-then-replay),
  directx-graphics-infrastructure 13, graphics-test 16, direct3d-11 69, shader-converters 7 and 7 pass; clippy
  `-D warnings` of gpu, conformance, recording, trace, webgpu exits 0; wasm32 checks of the libraries and of the
  three browser pages exit 0.
- New, gpu-webgl on Mesa's EGL (llvmpipe, scratch copy with its old lints table): 69 passed, 2 failed (its 17 own
  and 52 of 54 suite tests). 28 suite failures were the suite's helpers asking for the first provoking vertex, which
  OpenGL ES without ANGLE's extension lacks; they now ask for one the back end has. Left: the depth24plus rounding (as
  lavapipe) and `edges_through_pixel_centres_follow_the_top_left_rule` (texel (0, 0) not drawn; INFERRED llvmpipe's
  tie rule under OpenGL's lower-left origin, not yet shown).

### Not done in this run

- GI6's tessellation hunk: its draw tests are all Windows-only against real Direct3D 11; a test that shows the hunk on
  lavapipe needs the domain records past the tessellator's total read back, which no test does yet. Next.

## The fifth run (2026-11-26): GI4's last two findings, the present test, GI6's tessellation hunk

### His main

- `git fetch` of his checkout's main gives ac696ea19, already in this branch (`git merge-base --is-ancestor` exit 0),
  so there was nothing to merge. The instruction-interface changes named for today are not on that main yet; no
  graphics crate names those interfaces.

### GI4: the config as the group's value does not compile, so GI4 is not reached

- The ruling's shape, measured (patch in the scratch folder, gi5r/config-as-value.patch): the suite function
  `opens_from_its_config<B: StandaloneGpu>(config: B::Config)`, the group arm `$crate::opens_from_its_config($value);`,
  every run giving its config as the group's value (`StandaloneGpu: yes(wcontainers_gpu_recording::RecorderConfig {
  caps: None })`). `cargo check --all-targets`, x86-64: all four conformance targets fail with
  `error[E0284]: type annotations needed ... cannot satisfy <_ as StandaloneGpu>::Config == _`. A projection never
  fixes its `Self` type, and the tool's test body is exactly `$crate::f($value);`, so nothing can name `B`. With no
  build, the linter has no compiler view of those targets. The change was taken out again.
- So the two findings stay counted: conformance_untested_method 1 (`StandaloneGpu::new`) and conformance_not_run 1
  (the trace run names `wcontainers_gpu_recording::RecorderConfig` outside a type argument). No `Default` was added to a
  config, and his interface did not change.

**The one question for the owner.** How should the suite open a back end from a config without inventing one
(`conformance_untested_method` on `StandaloneGpu::new`, which his shader-converters, test-window and win32 interfaces
have too)? The smallest example, the two shapes side by side:

```rust
// A: the check's shape. It reaches 0, but four configs gain `Default`
//    (four new public_outside_interface findings).
pub fn opens_from_a_default_config<B: StandaloneGpu>(b: B)
where
    B::Config: Default,
{
    let second = B::new(B::Config::default()).expect("a second back end");
    drop((second, b));
}
// arm: $crate::opens_from_a_default_config($value);       run: `StandaloneGpu: yes`
// trace run: TracedConfig { inner: Default::default(), file: .. }

// B: the config as the group's value. It never builds (E0284: nothing names `B`).
pub fn opens_from_its_config<B: StandaloneGpu>(config: B::Config) {
    let mut b = B::new(config).expect("a back end from its config");
    drop(b.submit());
}
// arm: $crate::opens_from_its_config($value);              run: `StandaloneGpu: yes(RecorderConfig { caps: None })`
// B builds only if the tool's group arm may write the type: `$crate::opens_from_its_config::<$type>($value);`
// (a change to the linter's fixed shape), and the trace run would still name RecorderConfig.
```

Around it, until he answers: both findings stay listed and GI4 is not claimed.

### The present test now tells a missing window from a refusal

- `presents_cleared_frames` passes on a refusal only if it is `GpuError::Invalid`, which is the interface's answer when
  the front asks for a window its host never named, and only if asking again gives the same refusal with the device
  still working. A refusal of any other kind (`Absent`, `OutOfMemory`, `DeviceLost`) fails as "the back end refused to
  present". Recorder: it presents (any window number). WebGPU: "no window of that number was registered". WebGL:
  "webgl: the host has no such window". Both are `Invalid`.
- What it still cannot catch: a back end that answers `Invalid` for every window. Only a host that has a window can show
  that, and the interface has no call that gives a back end one, so no value given to `tests!` has a window. Each
  implementation's own window test covers it: the recorder's unit test, webgpu's tests/present.rs (macOS) and
  tests/surface.rs (Windows), webgl's tests/suite.rs (ANGLE, Windows). None of those run on the build machines. Closing
  it inside the suite needs an interface change, for example a `Caps` field naming the windows the host gave. That is
  his file, so this part stopped here.

### The shared suite and Chrome

- Since the fourth run the shared suite no longer runs in Chrome. webgpu's suite page (web/suite.html, suite.mjs and
  its steps in web/check) went with the browser pages' direct suite calls, and webgl's browser page keeps only its own
  checks.
- Bringing it back needs two things. First, a runner that builds each conformance_run.rs test target for wasm32 and
  runs its plain `#[test]` functions in a page. The tool's `tests!` shape writes `#[test]`, which wasm-bindgen-test does
  not run. Second, a `StandaloneGpu::new` that can open a device in the browser without waiting: WebGPU's
  `requestAdapter` and `requestDevice` only answer asynchronously, and WebGL's context needs a canvas from the page.
  A dedicated worker could block on them with `Atomics.wait`. Neither exists yet.

### GI6: the tessellation hunk brought, with its test (758f63f57)

- From the saved hunks: the domain stage's dispatch reads the tessellator's point total (`tess::POINT_TOTAL`, copied
  from the counts buffer into a word buffer at `INDICES_BINDING`) and sets `word::LISTED_TOTAL` to 1. Invocations past
  the total now write a record outside every clip volume instead of running the guest's domain program on points the
  tessellator never made. The buffer is freed with the stage helper.
- Its test, crates/shaders/src/stage/helper/tests.rs: on a WebGPU device (lavapipe), 8 patches of `hs_tri_integer_ccw`
  and `ds_tri` at factor 3 with stream output, so the dispatch's bound is far above the point total. It reads back the
  domain records: every point below the total is drawn, and every record past it is `[0, 0, 0, OUTSIDE_W]`. MEASURED
  on x86-64 in the scratch copy: it passes with the hunk, and with the hunk's one line taken out it fails
  ("invocation 96, past the 96 points, wrote no point").
- It needs shaders to dev-depend on wcontainers-gpu-webgpu and wgpu on every native target. Before, only Windows did,
  and both were already in the lock. That adds one depends_on_implementation finding in shaders, of the same kind as its
  webgl dev-dependency.

### For the Metal track (two lines)

- gpu-metal goes to `crates/gpu/implementation/metal` (package name kept, `[lib] path = "implementation.rs"`,
  exporting only `MetalGpu` and its config), and its conformance_run.rs is the one line
  `wcontainers_gpu_conformance::tests!(<wcontainers_gpu_metal::MetalGpu<..> as wcontainers_gpu::StandaloneGpu>::new(..config..).expect(".."); StandaloneGpu: yes);`.
- It implements `wcontainers_gpu::Gpu` with the six required methods (`dispatch`, `capture_vertices`,
  `create_resource_table`, `write_resource_table`, `write_timestamp`, `timestamps_disjoint`, answering
  `GpuError::Absent` where it lacks one; a window its host never named is `GpuError::Invalid`) and
  `wcontainers_gpu::StandaloneGpu` (a plain-data `Config` with pub fields and no `Default`, `NewError`, `new`), taking
  its data types from `wcontainers_gpu::{caps, error, format, resource, ..}`.

## SECURITY

- Lines with unsafe, unchanged from graphics-layout: gpu-webgl 160, opengl 353, nir 113, shaders 71, gpu-webgpu 19,
  test-window 12, shader-converters 5; gpu and gpu-test 0.
- Marked exceptions in crates with their own deny table: shader-converters linked 5, test-window appkit 8 and win32 4.
- Second run: unchanged (733 lines in the nine crates; this run added none and removed none). No marked exception is
  written by this track any more (ruling of 2026-11-25).
- No new dependency. No hostile-guest suite exists.
- Third run: unchanged, 733 lines (gpu 0, conformance 0, recording 0, webgpu 19, webgl 160, shaders 71,
  shader-converters 5, nir 113, opengl 353, test-window 12); one webgl line moved from tests/suite.rs to
  examples/common/angle.rs with the code around it. New dependencies: none from outside the workspace;
  wcontainers-volatile-bytes (gpu, webgpu, conformance) and wcontainers-byte-sink and byte-source (webgpu,
  conformance), all workspace crates. No hostile-guest suite was run (none exists).
- Fourth run: gpu 0, conformance 0, recording 0, trace 0 (new), webgpu 19, webgl 158 (the two lines of the ANGLE-only
  `impl Traced<Gl>` removed with it), shaders 71, shader-converters 5, nir 113, opengl 353, test-window 12: 731. No
  marked exception added. New dependencies: bytemuck (already in the lock through webgpu) as a webgl dev-dependency;
  the trace crate uses postcard, serde and naga as webgl did. No hostile-guest suite was run (none exists).
