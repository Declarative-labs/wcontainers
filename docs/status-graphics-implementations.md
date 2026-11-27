# Status: graphics implementations

The brief is orchestrator/restart-2026-11-25/brief-graphics-implementations.md in the scratch folder. Logs of every
run named here are in the scratch folder graphics-implementations/.

## GI1, the graphics tree on main (2026-11-25)

- Base: main 95bceadf0. The graphics folders, third_party and five status files come from graphics-layout 97e1daafe
  by path, unchanged except the nine lints tables, which inherit the workspace as main's did, and their TODO lines.
- `cargo check --all-targets`, x86-64 and ARM64, Mesa off: every graphics package exits 0 except
  wcontainers-gpu-webgl (its library holds unsafe code, which the workspace forbids; 161 lines by the one counting
  method of the SECURITY section, 160 as first written) and
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
  examples/common/angle.rs, with the one Windows `unsafe` line (`add_window`); webgl's count is unchanged (161 by the one counting method; 160 as first
  written).
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

- Settled: the config as the group's value does not compile (E0284: nothing names the implementation when it appears
  only in a projection). No `Default` was added to a config, his interface did not change, and the `tests!` macro did
  not change. GI4 stays not reached with its two findings counted (MEASURED at 49976d73c, linter da55b9d7, x86-64, a
  copy with gpu-webgl's old lints table so its compiler view exists):
  `crates/gpu/interface.rs:917 conformance_untested_method: `StandaloneGpu::new` is never called with the value it
  returns used, on a value or type bounded by `StandaloneGpu`, in code the suite functions of
  `wcontainers_gpu_conformance` reach` and `crates/gpu/implementation/trace/conformance_run.rs:1 conformance_not_run:
  it names another implementation, `wcontainers_gpu_recording`, outside a type argument of a type of
  `wcontainers_gpu_trace``. The same `new` finding stands on his shader-converters (interface.rs:139) and test-window
  (interface.rs:19) interfaces.

**The one question for the owner.** How should the shared suite open a back end from a config, so that it calls
`StandaloneGpu::new`, when it may neither give configs `Default` nor take the config as the group's value? The
smallest example and the two shapes tried, each measured at 49976d73c as a scratch patch (never committed):

```rust
// interface (his, unchanged): pub trait StandaloneGpu: Gpu { type Config; type NewError: Debug;
//                                fn new(config: Self::Config) -> Result<Self, Self::NewError>; }

// Shape A (graphics-implementations/check-gi4r2/out/experiment-default-config.patch): configs gain Default.
pub fn opens_from_a_default_config<B: StandaloneGpu>(b: B)
where
    B::Config: Default,
{
    let second = B::new(B::Config::default()).expect("a second back end");
    drop((second, b));
}
// arm: $crate::opens_from_a_default_config($value);     every run: `StandaloneGpu: yes`
// linter: conformance_untested_method 0, conformance_not_run 0, and four new findings, e.g.
//   crates/gpu/implementation/recording/implementation.rs:50 public_outside_interface: exported `RecorderConfig`
//   derives `core::default::Default`, which none of StandaloneGpu::Config requires as written here; it widens what
//   the type offers   (the same for TracedConfig, GlConfig and WebGpuConfig)

// Shape B (graphics-implementations/gi5r/config-as-value.patch): the config is the group's value.
pub fn opens_from_its_config<B: StandaloneGpu>(config: B::Config) {
    let mut b = B::new(config).expect("a back end from its config");
    ok(&mut b);
}
// arm: $crate::opens_from_its_config($value);   run: `StandaloneGpu: yes(wcontainers_gpu_recording::RecorderConfig { caps: None })`
// compiler (cargo check --all-targets, x86-64), in the recording, trace and webgpu conformance targets:
//   error[E0284]: type annotations needed
//    --> crates/gpu/implementation/recording/conformance_run.rs:1:233
//     = note: cannot satisfy `<_ as StandaloneGpu>::Config == _`
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
  at the pushed commit 49976d73c on both machines (sixth run): it passes with the hunk, and with the hunk's one line
  taken out it fails ("invocation 96, past the 96 points, wrote no point").
- It needs shaders to dev-depend on wcontainers-gpu-webgpu and wgpu on every native target. Before, only Windows did,
  and both were already in the lock. Corrected in the sixth run (MEASURED, linter at 948af1837 against 49976d73c): it
  adds no depends_on_implementation finding (shaders 5 before and after); it adds one stray_source (the new
  src/stage/helper/tests.rs) and one constant_defined_twice (`PATCHES = 8` in tests/tessellation_draw.rs:27 and
  src/stage/helper/tests.rs:15).

### For the Metal track (two lines; current as of the eighth run)

- The Metal back end is being restarted by another agent on branch `graphics-metal` (READ origin/graphics-metal
  f2560d2d7, 2026-11-26: no crates/gpu/implementation/metal yet); this track does not touch that crate. gpu-metal goes to `crates/gpu/implementation/metal` (package name kept, `[lib] path = "implementation.rs"`,
  exporting only `MetalGpu` and its config), and its conformance_run.rs is the one line
  `wcontainers_gpu_conformance::tests!(<wcontainers_gpu_metal::MetalGpu<..> as wcontainers_gpu::StandaloneGpu>::new(..config..).expect(".."); StandaloneGpu: yes);`.
- It implements `wcontainers_gpu::Gpu` with the six required methods (`dispatch`, `capture_vertices`,
  `create_resource_table`, `write_resource_table`, `write_timestamp`, `timestamps_disjoint`, answering
  `GpuError::Absent` where it lacks one; a window its host never named is `GpuError::Invalid`) and
  `wcontainers_gpu::StandaloneGpu` (a plain-data `Config` with pub fields and no `Default`, `NewError`, `new`), taking
  its data types from `wcontainers_gpu::{caps, error, format, resource, ..}`.

## The sixth run (2026-11-26): the fifth run pushed and measured, layout work in the moved implementations, GI7 prepared

Logs in the scratch folder graphics-implementations/gi6/ (one per job, named by machine and commit); the job script
is tools/gi6-job.sh, the linter's tools/gi4r-lint.sh and tools/layout_by_crate.py.

### What the fifth run left, and what this run did first

- The fifth run had ended with nothing pushed: 758f63f57 and 8d30641a4 and its status section were only in the
  worktree, and its jobs had all ended (`arm-run --status`: gi5r-arm and gi5r-x86 exit 0; no job of this track
  running on either machine). Its folders on both machines were deleted, the untracked helpers (gi-linter-src/,
  gi4r-lint.sh, gi5r-job.sh; copies kept in tools/) removed, the section committed as 49976d73c and pushed.
- His main: `git fetch` of his checkout's main gives ac696ea19, already in this branch (`git merge-base
  --is-ancestor FETCH_HEAD HEAD` exit 0, `git log HEAD..FETCH_HEAD` empty), so there was nothing to merge.

### Measured at the pushed commit 49976d73c, and again at a856bf204 (both machines alike unless named)

- `cargo check --all-targets` under the workspace lints: 17 graphics packages exit 0 (gpu, conformance, recording,
  trace, webgpu, opengl, nir, direct3d-11, direct3d-12, directx, directx-graphics-infrastructure, graphics-test,
  graphics-test-forward, shader-converters with linked and contained, test-window); gpu-webgl exits 101 (166 error lines,
  165 of them unsafe; at a856bf204 one more, its lib test's `could not compile`) and shaders through it. With the old lints table in a scratch copy: all 19 exit 0.
- Tests: gpu 37, recording 215, trace 55, directx-graphics-infrastructure 13, graphics-test 16, direct3d-11 69,
  shader-converters linked 7 and contained 7, all pass. clippy `-D warnings` of gpu, conformance, recording, trace,
  webgpu: exit 0.
- Mesa built from the pinned source (x86-64 65 s, ARM64 99 s). webgpu on lavapipe: 135 passed, 4 failed, 3 ignored
  (GI2's four: bc_textures_read_back_as_written_and_bc6h_and_bc7_sample_exactly,
  a_copy_within_one_depth_or_stencil_subresource_moves_the_region, depth_and_stencil_parts_copy_and_read_back_exactly,
  present_shows_the_frame); its conformance run 53 passed, 1 failed (depth24plus rounding); with lavapipe loaded
  through a manifest of another name the run refuses it ("the adapter is the software llvmpipe").
- Scratch copy (old lints table for gpu-webgl, nir, opengl): webgl on Mesa's EGL 69 passed, 2 failed
  (a_copy_within_one_depth_or_stencil_subresource_moves_the_region, edges_through_pixel_centres_follow_the_top_left_rule,
  as in the fourth run); wasm32 checks of the libraries and of the scenes, browser and replay pages exit 0 (without the
  old table the library check and the browser and replay pages fail through gpu-webgl, the scenes page passes);
  shaders 158 passed, 5 ignored (GI1's 157 and the tessellation test); with `WCONTAINERS_MESA` set, nir 41 passed and
  2 ignored on x86-64 and opengl 11 passed on both; nir's tests on ARM64 still do not link against Mesa (multiple
  definitions in the state tracker's objects, as recorded for GI8).
- The tessellation test (`domain_invocations_past_the_point_total_write_no_point`): passes; with the hunk's one line
  taken out in the copy (`HUNK-LINE before=1 after=0`) it fails with "invocation 96, past the 96 points, wrote no
  point".

### Linter counts (da55b9d7, x86-64, gpu-webgl's old lints table in the copy so its compiler view exists)

Last run's commits, 948af1837 against 49976d73c (without the old table): webgpu public_outside_interface 83 to 80
(the fence is plain data); shaders stray_source 56 to 57 and constant_defined_twice 5 to 6 (the GI6 test, above);
nothing else in the layout rules changed.

This run's layout work, 49976d73c against a856bf204:

| crate | rule | before | after | how |
|---|---|---|---|---|
| webgpu | unparsed | 55 | 1 | the tests take `scene` from wcontainers-graphics-test (a dev-dependency on every native target; it was Windows-only) instead of including graphics-test/src/scene.rs, and the scenes page's modules moved from examples/scenes to tests/scenes (`[[example]] scenes`, `path = "tests/scenes/page.rs"`), so every test includes them below its own folder |
| trace | public_outside_interface | 15 | 13 | the stream file and the unwritten frame are a private `Out`, whose `Drop` writes the last frame; `id`, `run`, `make`, `void` are functions |
| recording (ac7a97aaf) | public_outside_interface | 4 | 1 | the three `impl Recorder` blocks (compute.rs, draw.rs, recorder.rs: 63 methods) are functions of their modules taking the recorder (tools/recorder_functions.py, then by hand: `Bindings::texture` kept, an unused parameter dropped); names a local would hide got another (`buffer` is `buffer_obj`, `view` `view_obj`, the test constructor `new` is `full`, ...). Left: `Id` is pub (it fills the associated types; exported it would need every field pub and its derives would widen it) |

Every other layout count in the nine crates is unchanged: webgpu 80, webgl 177 public_outside_interface; webgl
unparsed 7; conformance constant_defined_twice 27. All rules in the nine crates: 22,387 at 49976d73c, 22,317 at
a856bf204 (unparsed -54, public_outside_interface -2, text rules -14), 22,326 at ac7a97aaf (public_outside_interface
-3, name_unanswered +17: the renamed functions' names wait on the linter's naming answers, which only the Mac's key
can ask for; grammar_unclear -3, verb_not_first -2). At ac7a97aaf, MEASURED on both machines: every check as above
(17 exit 0, gpu-webgl and shaders 101), recording 215 and trace 55 and the other tests as above, clippy exit 0, webgpu
on lavapipe 135/4/3 with the same four.

Tried and taken out: webgpu's handles without `PartialEq`/`Eq` (28 findings) do not compile:
`error[E0369]: binary operation == cannot be applied to type wcontainers_gpu_webgpu::Blend` at tests/scenes.rs:134
(and five more there and at tests/bindings.rs:125). Those tests check that equal blocks made twice are one object;
they need the identity from outside the crate, so the findings stay until the tests can see it another way.

Left, by structure: the inherent constructors and accessors of the exported back ends (webgpu `new`,
`device_descriptor`, `INSTANCE_FLAGS`, `add_window`, `counters`; webgl `new`, `on_angle`, `add_window`) are called by
the fronts (direct3d-12, directx, opengl, shaders' tests), so they go when the fronts build their back ends through
`StandaloneGpu`; the helper impl blocks on `WebGpu` (8) and `Gl` (2) become functions as the recorder's did; the handles' private fields fill associated types (webgpu 14, webgl 45) and need handles
that are plain data. Note: 23ec9ae7a holds the scenes files' renames that belong to a856bf204, so 23ec9ae7a alone does
not build its webgpu tests; the pushed tip does (measured above).

### GI7 prepared (READ origin/graphics-api-crates 90b14f72a: no OpenGL ES trait yet; AC10 comes after Direct3D 12)

- The list of what the WebGL back end needs (scratch NEEDED-FROM-API-CRATES.md) re-measured at 49976d73c: the moved
  crate calls the same 145 distinct glow methods 313 times (`diff` with the list: empty); its unsafe lines by form
  (tools/gi7_sites.py): 39 one-call lines that become the plain call, 104 lines of blocks with several calls, 17
  `unsafe fn`/`impl` items. nir's 52 + 5 + 39 + 17 lines unchanged.
- Nothing more can be built before the trait exists: every one of those calls is an `unsafe fn` of glow or EGL.

### Milestones

- Reached this run: none newly. GI1 to GI3 and GI5 as recorded; the tessellation part of GI6 measured at a pushed
  commit; GI4 not reached (the question above); GI6's removal of the linked route waits on the cost question of the
  brief; GI7 waits on the API track's OpenGL ES trait (AC10); GI8 part B on its Gallium and Mesa compiler traits.
- Next: webgpu's eight private helper blocks as functions, as the recorder's (waits on nothing), then the handles
  as plain data; webgl's two with GI7.

## The seventh run (2026-11-26): webgpu's own code as functions, its handles as plain data, nir linked on ARM64

Logs and lint outputs in the scratch folder graphics-implementations/r7/ (one per job, named by machine and step); the
jobs are tools/r7-job.sh, r7-lint.sh, r7-nirlink.sh, r7-nirwhy.sh, r7-nirfix.sh; the conversion is
tools/webgpu_functions.py; counting is tools/count-unsafe.sh. The independent check of the sixth run held.

### His main

- `git fetch` of his checkout's main gives ac696ea19 ("verus compat"), already in this branch (`git merge-base
  --is-ancestor` exit 0, `git log HEAD..FETCH_HEAD` empty): nothing to merge. CORRECTED in the ninth run: this
  run also wrote that the instruction-interface changes of the day were not on that main; they are, and this branch
  holds them (ninth run, "His main", quotes the commands). No graphics crate names those interfaces.

### webgpu's eight helper impl blocks are functions (a9ddff2f4)

- blocks.rs, draw.rs, edge.rs, fill.rs, memory.rs, packed.rs, vertex.rs and implementation.rs's second block: 92
  methods became functions of their modules taking `gpu: &mut WebGpu` or `&WebGpu`; every call `x.name(..)` on a
  `WebGpu` is `name(x, ..)`. Only the block with the inherent constructor and accessors is left (it is the
  `new`/`add_window` finding below).
- Layout findings, linter da55b9d7 with gpu-webgl's old lints table in the copy (MEASURED, both machines; the tables
  of 221883a43 are identical on ARM64 and x86-64, 80,088 findings each):

| rule (crates/gpu/implementation/webgpu) | 221883a43 | a9ddff2f4 | 9fba25913 + 42d75efa2 | 58cc43187 (tip) |
|---|---|---|---|---|
| public_outside_interface | 80 | 72 | 30 | 30 |
| unparsed | 1 | 1 | 1 | 1 |
| all rules, whole workspace | 80,088 | 80,080 (ARM64 total, x86-64 table) | 80,061 | 80,051 |

  Every other graphics crate's layout counts are unchanged (tables identical but for the webgpu line). At the tip the
  tables and the per-rule totals are identical on both machines.

### webgpu's handles are plain data (9fba25913)

- The fourteen handle types filling `Gpu`'s associated types are `pub struct Buffer(pub u64)` and so on, deriving
  only `Clone` and `Debug`: the number the back end keeps the object under. The back end holds every object in a
  table per kind until `destroy` (the recorder's design); the `Rc` each handle held is now the table's.
  Blocks and binding sets that equal ones share keep a count of the creations that gave them out and leave the table
  when the last is destroyed; a read back leaves it when finished; views, samplers, shaders, query sets and surfaces,
  whose destroy did nothing before, leave it at their destroy. Textures the back end makes for itself (depth staging,
  the raw blocks of converted formats, a depth read drawn into a colour texture) leave the table once nothing names
  them by number; recorded work keeps its own hold, as before.
- A handle naming a destroyed object, or a number the back end never gave, is refused: `Invalid` where the call
  answers a result, the device lost with `Error` where it answers nothing (no panic path was added).
- The two tests that compared handles (`pipelines_and_bind_groups_are_cached`,
  `sets_differing_only_in_a_dynamic_offset_share_a_bind_group`) compare the numbers. The 28 `PartialEq`/`Eq`
  findings and the 14 private-field findings are gone (table above). Left of webgpu's 30: the inherent constructor
  and accessors the fronts call (`new`, `device_descriptor`, `INSTANCE_FLAGS`, `add_window`, `counters`, `depth24`,
  `frame_report`, `held`), the exported `Counters` and its 15 pub fields, `Depth24`, `UNORM24_MAX`, `FRAME_REPORT`:
  they go when the fronts build their back end through `StandaloneGpu` and the counters are read another way.
- New text findings of this run's code: name_unanswered +6 and summary_unanswered +6 (the new names and summaries
  wait on the model's answers, which need the key on the Mac), grammar_unclear +1 (`held`'s summary, same cause),
  number_constant_without_link +1 (`ONE_CREATION = 1`); magic_literal -1.

### nir on ARM64: the cause, and the repair (42d75efa2, 58cc43187)

- CORRECTED: the sixth run's line ("multiple definitions in the state tracker's objects") was half of it. MEASURED at
  221883a43 (tools/r7-nirlink.sh, both machines, Mesa on, `cargo test -p wcontainers-nir --no-run`):

| machine, linker | exit | lines "undefined reference" (symbols) | lines "multiple definition" (symbols) |
|---|---|---|---|
| ARM64, default (ld.bfd 2.46 through gcc) | 101 | 628 (8) | 70 (14) |
| ARM64, rust-lld forced | 101 | 0 | 5 (1: `blake3_compress_in_place_portable`) |
| x86-64, default (rust-lld) | 0, tests 41 passed, 2 ignored | 0 | 0 |
| x86-64, ld.bfd forced | 101 | 624 (8) | 65 (13) |

  So the difference is the linker, not the CPU: Rust links x86-64 Linux with rust-lld and ARM64 with the system
  ld.bfd. ld.bfd's map (r7/nirwhy-x86/why/bfd.map, "Archive member included") shows the chain: glcpp's generated
  parser (libglcpp) references `_mesa_get_shader_include_cursor`; its stand-in in libglcpp_standalone comes earlier in
  the link line, so ld.bfd, taking members in one pass in order, takes libmesa's main/shaderapi.c instead, which pulls
  main/shaderobj.c (defining `_mesa_reference_shader` and others the GLSL stand-ins define too: the multiple
  definitions, 13 symbols on x86-64 and 14 on ARM64 with blake3's), main/state.c, texobj.c and on into the state tracker (st_atom_array.cpp, st_atom_image.c), whose
  `cso_*`, `tc_*` and `u_upload_*` (7 symbols, libgallium.a) and `_mesa_glapi_tls_Context` (libglapi.a) are linked by
  nobody: the 628 undefined references. rust-lld (--why-extract, r7/nirwhy-x86/why/lld-why.txt, the `narrow` test) takes no
  member of libmesa at all. The second cause is ARM64's alone: the blake3 crate's NEON code exports the C function
  `blake3_compress_in_place_portable`, which Mesa's own BLAKE3 (libblake3, linked with Mesa) defines too.
- Repair (tools/r7-nirfix.sh tried each in a copy, both machines): libmesa is no longer linked (crates/nir/build.rs;
  "reorder", the stand-ins after their users, also linked), and on ARM64 nir takes blake3 with the crate's own
  `no_neon` feature (crates/nir/Cargo.toml, a target-specific dependency; a build that holds nir on ARM64 hashes
  without NEON, the product's wasm32 build is not touched). MEASURED in the copy: ARM64 links and nir passes 41 with
  2 ignored; x86-64 passes the same with rust-lld and with ld.bfd forced.
- MEASURED at the pushed commits 42d75efa2 and 58cc43187 (scratch copy with the old lints table, `WCONTAINERS_MESA`
  set): nir 41 passed, 2 ignored and opengl 11 passed on both machines.

### Measured at the pushed tip 58cc43187 (both machines alike)

- `cargo check --all-targets` under the workspace lints: 17 graphics packages exit 0; gpu-webgl exits 101 (167 error
  lines, 165 of them unsafe, one its lib test's `could not compile`) and shaders through it (166), as before.
- Tests: gpu 37, recording 215, trace 55, directx-graphics-infrastructure 13, graphics-test 16, direct3d-11 69,
  shader-converters linked 7 and contained 7 pass. clippy `--all-targets --no-deps -D warnings` of gpu, conformance,
  recording, trace, webgpu exit 0; `cargo fmt --check` of webgpu and nir exit 0; Cargo.lock's hash is the same before
  and after the build.
- webgpu on lavapipe: 135 passed, 4 failed, 3 ignored, the same four as GI2 (bc_textures_read_back_as_written_and_
  bc6h_and_bc7_sample_exactly, a_copy_within_one_depth_or_stencil_subresource_moves_the_region,
  depth_and_stencil_parts_copy_and_read_back_exactly, present_shows_the_frame); its conformance run 53 passed, 1 failed
  (depth24plus rounding); lavapipe through a manifest of another name is refused.
- Scratch copy: wasm32 checks of the libraries and of the scenes, browser and replay pages exit 0; nir and opengl as
  above.

### Unsafe counts: one method from now on

- The SECURITY section is CORRECTED to one counting method (non-comment lines matching the whole word): the earlier
  runs mixed the root manifest's command (which skips attribute lines) with per-file greps. 735 at GI1, 733 since the
  fourth run, as tabled there.

### GI7 and GI8 part B: what the API track should build first

- READ docs/status-graphics-api-crates.md at origin/graphics-api-crates 3ccf0fd5a: no OpenGL ES, Mesa compiler or
  Gallium trait yet (AC10 and AC11 come after Direct3D 12; AC8 waits on where our driver logic lives). The scratch
  folder's graphics-implementations/NEEDED-FROM-API-CRATES.md now opens with the order we ask for and the exact first
  methods: AC11's trait first (B1's 36 methods and B2's flat form: nir's library is the one that fails with Mesa on),
  then AC10's (EGL without the window surface, the 145 GL ES methods with a copying buffer read, the ten extension
  calls; MEASURED that the library compiles only whole: even `new`, `caps` and `status` alone reach 74 of the 145),
  then AC8's two sides (the 95 of 234 Gallium function pointers our driver fills, and the 23 Mesa helper functions it
  calls, besides our two C field readers); desktop GL and Vulkan for nir's corpus last.
- Nothing more of GI7 can be built before that trait: every remaining call is an `unsafe fn` of glow or khronos-egl.

### The second question for the owner (the present test)

The present test cannot tell a back end that refuses every window, because no call of the interface gives a back
end a window (webgpu's `add_window(u32, wgpu::Surface)` and webgl's `unsafe fn add_window(u32, *mut c_void)` are
inherent, outside it), so no value given to `tests!` has one. **May `Gpu` take the host's window through one more
method?** In crates/gpu/interface.rs (not changed by this track):

```rust
// before: pub trait Gpu, "Frames", as on this branch
    // ---- Frames ----

    /// Attaches to a host window, given by the number the host's graphics hook uses for it.
    fn create_surface(&mut self, window: u32) -> Result<Self::Surface, GpuError>;

// after
    // ---- Frames ----

    /// Gives the back end the host's window under the number the host's graphics hook uses for it. The
    /// window lives until the process ends, so a surface made on it may outlive this call.
    fn add_window(
        &mut self,
        window: u32,
        host: &'static (impl raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + Sync),
    ) -> Result<(), GpuError>;
    /// Attaches to a host window given with [`Gpu::add_window`], by its number.
    fn create_surface(&mut self, window: u32) -> Result<Self::Surface, GpuError>;
```

With it, a conformance run on a machine with a display (Windows, macOS: test-window's two implementations) gives its
value window 1 before `tests!`, and `presents_cleared_frames` fails any refusal of a window the back end accepted;
the `'static` window also removes webgl's one loan (the window outliving its EGL surface, NEEDED-FROM-API-CRATES.md
A1). Around it, until he answers: the gap stays listed, each implementation's own window test covers it, and the build
machines (no display) keep today's check.

### Milestones

- Reached this run: none newly. GI4 stays not reached, waiting on his answer to the first question (above, fifth
  run); not worked around. The second question is new.
- Done and measured at a pushed commit: webgpu's helper blocks as functions and its handles as plain data (GI4's
  layout work, not its bar), nir linking on ARM64 (GI8's), the unsafe count corrected, GI7's ordering written for the
  API track.
- Next: GI6's removal of the linked route waits on the cost question of the brief; GI7 waits on the API track's
  OpenGL ES trait (AC10); GI8 part B on its Mesa compiler (AC11) and Gallium (AC8) traits. What waits on nothing:
  webgpu's last 30 (the fronts building their back end through `StandaloneGpu`), and webgl's handles and helper blocks
  with GI7.

## The eighth run (2026-11-26): the seventh run's check finished, nir's link without `no_neon`

Logs in the scratch folder gi-check9/ (the check: arm.log, x86.log, out-*/) and graphics-implementations/r8/ (the
repair: blake3c-arm.log, blake3c-x86.log; the job is tools/r8-blake3.sh).

### His main

- `git fetch` of his checkout's main gives ac696ea19, an ancestor of this branch (`git merge-base --is-ancestor
  FETCH_HEAD HEAD` exit 0): nothing to merge. CORRECTED in the ninth run: this run also wrote that the
  instruction-interface changes of the day were not on that main; they are, and this branch holds them (ninth run,
  "His main", quotes the commands). No graphics crate names those interfaces.

### The check of the seventh run, finished (MEASURED at the pushed tip 29f39d382)

- A fresh clone of the pushed tip with its submodules (gi-check9/src), the check's own script (gi-check8/tools/chk.sh,
  its linter folder renamed only), run on both machines with `--jobs 12` (its first x86-64 call had used the
  machine's default of 96). Both jobs ran to their end: exit 0 after 1,255 s (ARM64) and 1,344 s (x86-64).
- Both machines alike:
  - linter da55b9d7 (gpu-webgl's old lints table in the copy): 80,051 findings; the per-rule table and webgpu's line
    are identical on the two machines; webgpu public_outside_interface 30, unparsed 1, as the seventh run said.
  - `cargo check --all-targets`: 17 graphics packages exit 0; gpu-webgl and shaders exit 101 (166 error lines, 165
    unsafe).
  - Tests: gpu 37, recording 215, trace 55, directx-graphics-infrastructure 13, graphics-test 16, direct3d-11 69,
    shader-converters linked 7 and contained 7 pass. clippy `-D warnings` exit 0, fmt exit 0, Cargo.lock's hash the
    same before and after.
  - Mesa built (99 s ARM64, 104 s x86-64). nir links as pushed (0 undefined, 0 multiple definitions) and passes 41
    with 2 ignored; opengl 11 pass.
  - webgpu on lavapipe: 135 passed, 4 failed, 3 ignored, GI2's four; its conformance run 53 passed, 1 failed. wasm32
    exit 0.
- The check's three plants in nir's link, ARM64: libmesa back gives 628 undefined references (8 symbols) and 65
  multiple definitions (13); libmesa back with the NEON change undone gives 628 (8) and 70 (14, `blake3_compress_in_
  place_portable` the fourteenth); the NEON change undone alone gives 5 (1, that symbol). x86-64 (rust-lld) links all
  three. So the repair and each of its two parts were needed, as the seventh run said.
- What the check named by reading: the `no_neon` feature reaches every crate in a whole-workspace ARM64 build.
  MEASURED at 9034444b8 with the old dependency put back in a copy (`cargo tree --workspace -e normal -f '{p} [{f}]'`,
  ARM64): blake3 resolves to `[default,no_neon,std]`; the translator built alone keeps `[default,std]`. Repaired below.

### nir's link without `no_neon` (b9fd01638, bc48ee898, 9034444b8)

- nir's build script now links Mesa's BLAKE3 from a copy: each member object of Meson's thin archive (objcopy cannot
  copy a thin archive whole) is renamed with `objcopy --redefine-syms`, every inner function (`blake3_*` and the
  `_blake3_*` aliases x86-64's assembly defines, but not the `blake3_hasher_*` functions Mesa's other libraries call)
  taking the prefix `wcontainers_nir_mesa_`, and the copies are archived anew in the build's output folder. The
  target-specific `no_neon` dependency is gone. It touches crates/nir/build.rs and crates/nir/Cargo.toml only; the
  Visual C++ target (no nm or objcopy) links Mesa's library as before.
- MEASURED at the pushed 9034444b8 (scratch copy with the old lints table for nir and opengl, Mesa built in the job):

| | ARM64 (ld.bfd) | x86-64 (rust-lld) | x86-64 (ld.bfd forced) |
|---|---|---|---|
| nir tests link | exit 0, 0 undefined, 0 multiple | exit 0 | exit 0 |
| nir tests | 41 passed, 2 ignored | 41 passed, 2 ignored | 41 passed, 2 ignored |
| opengl tests | 11 passed | 11 passed | |
| plant: rename off | exit 101, 5 multiple (`blake3_compress_in_place_portable`) | exit 0 | exit 0 |
| plant: rename off, `no_neon` back (the seventh run's repair) | exit 0 | exit 0 | exit 0 |
| plant: libmesa back | exit 101, 628 undefined, 65 multiple | exit 0 | exit 101, 624 undefined, 65 multiple |
| whole-workspace blake3 features | `[default,std]` | `[default,std]` | |

- `cargo fmt --check -p wcontainers-nir` and `cargo clippy --all-targets --no-deps -p wcontainers-nir -D warnings`
  exit 0 on both machines. Cargo.lock: unchanged by the commits (READ `git diff 29f39d382..9034444b8 -- Cargo.lock`
  empty). A first try (bc48ee898) renamed only names starting `blake3_`; on x86-64 the renamed references then pulled
  Mesa's assembly objects, whose `_blake3_hash_many_sse41` and ten more aliases the crate defines too (55 multiple
  definitions): 9034444b8 renames those aliases as well.
- Not measured: the hashing speed the other crates lost under `no_neon` (it no longer applies).

### webgpu's last 30 public_outside_interface findings: not reduced this run

READ at 9034444b8, each with what still calls it:

| findings | items | callers | what stops it |
|---|---|---|---|
| 1 | `add_window` | directx com/chosen.rs:117, opengl src/device.rs:72, direct3d-12 examples/directx/gpu.rs:60, trace replay.rs:104, webgpu tests/host | hosts give windows after the back end exists and the interface has no call for it: his second question (seventh run) |
| 4 | the impl block, `new`, `device_descriptor`, `INSTANCE_FLAGS` | those hosts, and direct3d-12 tests (answers, gpu, registers), directx tests/com.rs, shaders tests/common, webgpu tests/scenes/draw.rs and timestamps.rs | a host that adds a window must make its surface on the device's own wgpu instance, so it makes instance, adapter and device itself; the window-less callers could move to `StandaloneGpu` now, but the four stay public while any window host needs them, so they follow the second question |
| 20 | `Counters`, its 15 fields, `counters`, `frame_report`, `FRAME_REPORT`, `held` | 36 reads in webgpu's tests (bindings, writes, scenes and the scenes modules the browser page includes), directx tests/com.rs:957, examples/frames.rs | no interface call reports them; next step (waits on nothing): those tests become unit tests inside the crate, as the recorder's did, and the page stops printing them |
| 5 | `Depth24` (2), `depth24`, `UNORM24_MAX` (2) | webgpu tests copies.rs and depth.rs, the scenes page, direct3d-12 tests/answers.rs | `Caps`'s `DEPTH24_UNORM` answers only part (not float storage); goes with the unit-test move above |

### Milestones

- Reached this run: none newly. The seventh run's check finished and held (above). GI8's ARM64 link now holds
  without turning NEON off for other crates.
- GI4 still waits on his answer to the first question (fifth run); GI6's removal on the cost question; GI7 on the API
  track's OpenGL ES trait (AC10); GI8 part B on its Mesa compiler (AC11) and Gallium (AC8) traits.
- Next, waiting on nothing: webgpu's counter-reading tests as unit tests (20 + 5 findings), then the window-less
  callers on `StandaloneGpu`.

## The ninth run (2026-11-26): webgpu's counters inside the crate, window-less callers on `StandaloneGpu`, the Metal back end merged

Logs, lint outputs and the experiment in the scratch folder graphics-implementations/r9/ (before-*, after-*, m-*.log,
f16probe*); the job is tools/r9-job.sh, the counts tools/r9_rules.py and r9_filediff.py. Commits 8e20476d8 (Metal) and
ad62381ac (webgpu), both pushed; every number below MEASURED at ad62381ac on both machines unless named.

### His main (CORRECTED; the seventh and eighth runs were wrong here)

- `git fetch /Users/thomaseaton/Documents/GitHub/wcontainers +main:refs/owner/main` gives ac696ea19; `git
  merge-base --is-ancestor refs/owner/main HEAD` exit 0; `git log HEAD..refs/owner/main` empty: nothing to merge.
- That main holds the instruction-interface changes of the day, so this branch does too: `git grep` on refs/owner/main
  gives crates/processor-architecture/interface.rs:18 `pub trait ProcessorArchitecture: Sized + Clone`, :47
  `fn meets_expectations(`, :71 `fn simulate_state<'memory>(`, :137 `pub struct MemorySnapshot<...>`,
  crates/behavior-source/interface.rs:26 `fn execute_single_instruction_in_place(`, and `git show
  refs/owner/main:Cargo.toml` line 37 `type_complexity = "allow"`. No graphics crate names those interfaces (`git grep -l
  processor.architecture` over the graphics folders: 0 files).

### The Metal back end and its twenty shared programs (8e20476d8)

- From origin/graphics-metal-on-gpu-interface 1e837dfa6 (merge base with this branch 29f39d382; this branch changed
  none of these paths since): crates/gpu/conformance/suite.rs (+1,884, the twenty programs) and
  crates/gpu/implementation/metal (20 files), taken by `git checkout <branch> -- <paths>`, not by a merge, so the
  other branch's content that branch's merge carries is not taken. Taken beside them because gpu-metal does not
  resolve without them: crates/metal (interface, conformance, implementation/direct, byte for byte), the naga MSL
  patch it is built against (third_party/naga PATCHES and src/back/msl, this track's path) and the root manifest's two
  `overflow-checks` lines for wcontainers-metal-direct and wcontainers-gpu-metal. Cargo.lock regenerated on ARM64
  (`cargo metadata`): only the four workspace packages added, no outside crate added or changed.
- `cargo check --all-targets` of wcontainers-gpu-metal, -metal, -metal-direct and -metal-conformance exit 0 on both
  machines (their Metal code is `cfg(target_os = "macos")`; the Mac itself is not used).
- The shared suite (74 programs): on the recorder 74 passed (`--test conformance`; the recorder's whole package 235,
  was 215; trace 75, was 55). On webgpu over lavapipe 72 passed, 2 failed: GI2's depth24 rounding
  (`a_copy_within_one_depth_or_stencil_subresource_moves_the_region`, "Depth24Plus depth of texel 5:
  0.6249999180436085, not 0.625") and the new `programs_compute_in_16_bit_floats`.
- `programs_compute_in_16_bit_floats` on lavapipe: "a third in 16 bits: 0.33333334", the 32-bit value. Its cause,
  MEASURED with a scratch wgpu program (never committed; r9/f16probe.rs, same on both machines): converted to 16 bits
  and back in one program, every value comes back unrounded (1/3 as 0x3eaaaaab, 2049 as 2049), also through a 16-bit
  multiply by one; the same `f16(x)` stored into a 16-bit storage buffer is rounded right (0x3555, 0x6800, 0x7bff,
  0x0001), `quantizeToF16` rounds (0.33325195, 2048), and 2048 + 1 in 16 bits gives 2048. So lavapipe keeps a 16-bit
  value held in a register at 32-bit precision and rounds it only where it is stored as 16 bits. READ in the pinned
  Mesa: no NIR algebraic rule drops `f2f32(f2f16(x))` (only the exact `fquantize2f16` lowering names the pair), and
  gallivm's `nir_op_f2f16` emits a real `fptrunc` (lp_bld_nir_soa.c:3292); INFERRED: the pair is removed after NIR, in
  the LLVM code llvmpipe builds. A device's precision, not a defect of ours: the recorder and Metal (216 of 216 on his
  Mac) pass it. Listed beside GI2's four; no allowance added to the suite.

### webgpu's counter-reading tests are unit tests (ad62381ac)

- bindings.rs, writes.rs, scenes.rs, copies.rs and the counting halves of prepare.rs, queries.rs and pulled.rs moved
  from tests/ into unit_tests/ (as first_draws.rs, query_sets.rs, pulled_costs.rs), declared by unit_tests.rs under
  `cfg(all(test, not(target_arch = "wasm32")))`, as the recorder's are; the depth probe's test left tests/depth.rs for
  unit_tests/depth_probe.rs. The scene modules they share with the scenes page name `super::` instead of `crate::`,
  and read counts through their includer's `made`: the unit tests give the back end's counters, the page gives none
  and its checks leave the counts out (web/draw.mjs accepts "(not counted here)"; the page no longer prints counters
  or the probe's value, it shows whether `DEPTH24_UNORM` is declared). The pool check moved from renames.rs to its
  own scenes/pool.rs so tests/features.rs and the Windows tests/direct3d.rs need no `made`.
- `Counters`, `counters`, `held`, `depth24`, `Depth24`, `UNORM24_MAX` and `FRAME_REPORT` are crate-private (the
  test-only ones under `cfg(test)`); examples/frames.rs keeps the report line's prefix as its own constant.
- crates/directx/tests/com.rs's ignored `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu`,
  which read `counters()`, became the unit test
  `constants_as_the_direct3d_11_front_makes_them_rewritten_per_draw_make_no_bind_group_per_draw` (the front's buffer
  usage, intent and `Discard` writes), still ignored as the known failure; run with `--ignored` on lavapipe it fails as
  its note says (2 bind groups after the first draw, pictures right).

### Window-less callers on `StandaloneGpu` (ad62381ac)

- `WebGpuConfig` gained `software` (the system's software adapter, WARP on Windows, asked for and taken), `required`
  and `without` (wgpu features the device must have or lack). direct3d-12's tests/answers.rs and tests/registers.rs
  (lavapipe taken), tests/gpu.rs (GPU only, as before), and the shaders' tests/common/mod.rs (`webgpu_on`,
  `webgpu_with`, `webgpu_on_warp`) now build webgpu through `StandaloneGpu`; answers.rs keeps its own `UNORM24_MAX`.
- Not moved, with the reason: crates/opengl/src/device.rs is a window host, not window-less (READ :49-75: it makes each
  window's surface on the device's own wgpu instance and calls `add_window`); crates/directx/src/com/chosen.rs,
  direct3d-12's examples/directx/gpu.rs, trace's replay.rs and webgl's browser examples are window or browser hosts;
  webgl's tests/against_webgpu.rs is `#![cfg(windows)]` and cannot be compiled here.
- direct3d-12 on lavapipe (new in this run): 61 passed, 3 failed: tests/gpu.rs's three, which refuse a CPU adapter
  ("the adapter is the software llvmpipe ..., not a GPU"), as their old `assert_ne!` did.

### Findings by rule (linter da55b9d7, gpu-webgl's old lints table in the copy; tables identical on both machines)

| | afe71ff74 (before) | ad62381ac (after) |
|---|---|---|
| whole workspace | 80,067 | 81,969 |
| webgpu public_outside_interface | 30 | 5 |
| webgpu all rules | 2,835 | 2,815 |
| gpu conformance all rules | 2,905 | 3,722 (the twenty programs: magic_literal +670, text rules) |
| crates/gpu/implementation/metal | 0 | 548 (public_outside_interface 33, conformance_not_run 3, summary/name_unanswered 508) |
| crates/metal | 0 | 558 (trait_not_implemented 2, conformance_untested_method 6, conformance_not_run 2, ...) |
| direct3d-12 / directx / shaders | 4,984 / 4,961 / 4,714 | 4,986 / 4,957 / 4,713 |

- webgpu's other changes: constant_defined_twice +2 (`FRAME_REPORT` in examples/frames.rs), summary_unanswered +7
  (new summaries wait on the model's answers; the key stays on the Mac), the moved files' text findings unchanged in
  number. direct3d-12 constant_defined_twice +2: `UNORM24_MAX` in answers.rs, and registers.rs's `TARGET_SIDE` now
  also defined in crates/metal/conformance.
- webgpu's 5 left: the inherent impl block, `new`, `device_descriptor`, `INSTANCE_FLAGS`, `add_window`; every caller
  left is a window or browser host (above), so they wait on his second question (seventh run).

### Measured at ad62381ac (both machines alike)

- `cargo check --all-targets`: 21 graphics packages exit 0 (the four Metal ones included); gpu-webgl and shaders exit
  101 (166 error lines, 165 unsafe), as before. clippy `--all-targets --no-deps -D warnings` of gpu, conformance,
  recording, trace, webgpu, gpu-metal, direct3d-12, direct3d-11 exit 0; rustfmt `--check` of the 47 changed files and
  `cargo fmt --check -p wcontainers-gpu-webgpu` exit 0; Cargo.lock hash the same before and after.
- Tests: gpu 37, recording 235, trace 75, directx-graphics-infrastructure 13, graphics-test 16, direct3d-11 69,
  shader-converters linked 7 and contained 7 pass.
- webgpu on lavapipe: 154 passed, 5 failed, 4 ignored (was 135, 4, 3): GI2's four
  (`unit_tests::copies::depth_and_stencil_parts_copy_and_read_back_exactly`, the BC test, the depth24 copy in the
  suite, `present_shows_the_frame`) and `programs_compute_in_16_bit_floats`; its unit tests 65 passed, 1 failed, 1
  ignored.
- Scratch copy (old lints tables): wasm32 checks of the libraries and of the scenes, browser and replay pages exit 0;
  shaders' tests 158 passed, 5 ignored on lavapipe; Mesa built (109 s ARM64, 76 s x86-64), nir 41 passed with 2
  ignored and opengl 11. gpu-webgl's tests ran without an EGL named: 17 passed, its 74 suite programs failed to open
  a context (not a measurement of webgl).

### Milestones

- Reached this run: none newly (GI4 still waits on his answer to the first question, fifth run). Done and measured at
  a pushed commit: webgpu's counter tests inside the crate and the window-less callers on `StandaloneGpu` (GI4's layout
  work), the Metal back end and its twenty programs on this branch, run on the recorder and on lavapipe.
- Next: GI4 waits on him; webgpu's last 5 on his second question; GI6's removal on the cost question; GI7 on the API
  track's OpenGL ES trait (AC10); GI8 part B on its Mesa compiler (AC11) and Gallium (AC8) traits.

## The tenth run (2026-11-27): GI7, WebGL 2 on the OpenGL ES crate's trait

Logs in the scratch folder graphics-implementations/r10/ (before-*, before2-*, before3-*, after-*, after2-*, after3-*,
profile-*, exp-keep-arm.log, dev*.log); the job is tools/r10-job.sh, the draw counter tools/r10_drawcount.py, the
mechanical passes tools/gi7_port.py, gi7_calls.py, gi7_fix.py, gi7_functions.py, gi7_handles.py, gi7_mirrors.py,
gi7_split.py. Every number below MEASURED at a pushed commit, both machines, unless named.

### His main

- `git fetch /Users/thomaseaton/Documents/GitHub/wcontainers main` gives ac696ea19; `git merge-base --is-ancestor
  FETCH_HEAD HEAD` exit 0 and `git log HEAD..FETCH_HEAD` empty: nothing to merge (the day's instruction-interface
  changes are in it, as the ninth run recorded; no graphics crate names those interfaces).

### What came from the API track (24c372f21)

- `crates/egl` and `crates/opengl-es` (interfaces, constants, `implementation/direct`, the OpenGL ES conformance
  crate) from origin/graphics-api-crates as at 1571b4566, the docs-only rerun of 9429dd43f (`git diff 1571b4566
  origin/graphics-api-crates -- crates/egl crates/opengl-es` empty), by `git checkout <branch> -- <paths>`. No
  generator path was taken: the crates build without one (the generator is its own workspace, tools/
  api-crate-generator, and nothing in the two crates reads it); their files name it in their first line only.
- MEASURED: all five packages check with `--all-targets`, clippy `-D warnings` exits 0, and the OpenGL ES suite on
  the direct crates over the pinned Mesa's EGL passes 4 of 4 (`TEST-OPENGL-ES-DIRECT`).

### GI7: gpu-webgl on the trait (19feea113, 900024812, 0833bd8d2, d0f51528a)

- `Gl<O: OpenGlEs>`: every glow call is a call of the trait (`O` is the OpenGL ES context); glow and khronos-egl are
  gone from webgl's dependencies (nir still uses them). Natively `Gl<wcontainers_opengl_es_direct::OpenGlEsContext>`
  implements `StandaloneGpu` with `GlConfig { egl: Option<PathBuf>, off }`: it loads the EGL library (once per
  process and folder: below), makes an OpenGL ES 3 context with a 1 by 1 pixel buffer, and keeps the EGL objects in
  a guard destroyed after the context's last call. In the browser `Gl<O>` implements `StandaloneGpu` for any `O`
  with `GlConfig { gl: O, off }`, waiting for the OpenGL ES crate's implementation on WebGL.
- platform.rs is the OpenGL ES calls in the forms the back end uses (names, logs, strings, extensions, the copying
  buffer read) and the EGL context; the old native and browser platform modules (function pointers, transmutes, the
  JSPI import, khronos-egl) are gone. The extension calls are the trait's `*_oes`, `*_ext`, `*_angle` methods.
- What the trait lacks (NEEDED-FROM-API-CRATES.md, section "After GI7", for the API track): glStencilFuncSeparate
  and glStencilOpSeparate (missed by our own list; until then a block whose faces differ loses the device as not
  built, no test meets it), eight extension values (defined in platform.rs with the registry's numbers until then),
  the WebGL implementation, eglCreateWindowSurface, an owned EGL context, the cost of the glue's checks, and the
  library unloaded with its last handle.
- What it costs: no window surface natively, so `create_surface` refuses every window (as it did on Linux, where no
  host ever gave one); the Windows ANGLE test `present_shows_the_frame` is ignored with that reason; directx's
  `webgl` feature (Windows only) would not build: the change it needs is graphics-implementations/r10/
  directx-webgl.patch, not applied (directx is not this track's). The browser pages (webgl's `browser` and
  `draw-cost`, trace's `replay`) do not build for wasm32 until the WebGL implementation exists; the browser's
  `#[unsafe(no_mangle)] extern "C" fn suite_run` is a `#[wasm_bindgen]` export now (INFERRED to keep the plain
  WebAssembly export the page's `WebAssembly.promising` calls; not run, the page cannot build yet).
- Its two helper impl blocks are functions taking `gpu: &mut Gl<O>` (900024812, tools/gi7_functions.py): 66 methods;
  the constructor and the six public accessors stay in one inherent block.
- Its handles are plain data (d0f51528a): `Buffer`, `Texture`, `TextureView`, `Sampler`, `Shader`, `Program`,
  `Blend`, `DepthStencil`, `Raster`, `VertexLayout`, `BindingSet`, `QuerySet`, `Readback`, `Surface` are `pub struct
  X(pub u64)`, deriving only `Clone` and `Debug`; `Fence<F>` is `{ sync: Option<F>, done: Rc<Cell<bool>> }` with pub
  fields, as webgpu's. The back end keeps each object in a table per kind until `destroy` (a read back until it is
  finished; shaders and programs of equal text are one object and leave with their last handle); a number it never
  gave or already destroyed is refused (`Invalid` where the call answers a result, the device lost where it answers
  nothing). The Gpu methods the back end also called itself are split into a method that looks the handles up and an
  internal function on the objects it holds (`make_texture`, `write_texture_held`, `read_back_held`, ...); a draw's,
  a pass's and a copy's objects are looked up once at the method (`Drawn`, `HeldPass`, `Region`, `Place`, `Source`).
  Textures the back end makes for itself never enter the table.
- The EGL library once per process (0833bd8d2): the speed comparison below found the whole suite in one process
  31% slower after the port, every program about 55 ms more (in-process profile, ARM64: sum 12,084 ms before, 15,881
  after); the glue unloads the library with its last handle and Mesa loads its driver again. A scratch copy keeping a
  second load alive gave 12.2 s (exp-keep-arm.log); the commit keeps the loaded library in a process-wide list.

### Unsafe lines (the track's one method, tools/count-unsafe.sh)

| crate | d19c23829 (before) | d0f51528a (after) |
|---|---|---|
| gpu-webgl | 159 (implementation.rs 96, platform.rs 41, state.rs 8, shader.rs 5, depth.rs 4, caps.rs 3, examples/browser.rs 1, examples/common/angle.rs 1) | 0 |
| every other crate of the track | unchanged (webgpu 19, shaders 71, shader-converters 5, nir 113, opengl 354, test-window 12, the rest 0) | unchanged |
| total | 733 | 574 |

Brought in, not written here (the API track's `implementation/direct` crates, each denying unsafe code and marking
each use): egl-direct 56 lines, 39 `#[expect(unsafe_code, reason)]` marks; opengl-es-direct 474 lines, 318 marks
(the API track's own count, 95 and 792, counts comment lines too); egl, opengl-es and the conformance crate 0.

### Measured at the pushed tip d0f51528a (both machines alike unless named; after3-*.log)

- `cargo check --all-targets`: egl, egl-direct, opengl-es, opengl-es-direct, opengl-es-conformance, gpu-webgl,
  shaders, gpu-trace, directx exit 0 under the workspace lints, `forbid` included (before: gpu-webgl 101 with 166
  error lines, shaders through it). clippy `--all-targets --no-deps -D warnings` and `cargo fmt --check` of gpu-webgl
  exit 0; clippy of the webgl library for wasm32 exits 0 (dev22). Cargo.lock: the five API packages added, glow and
  khronos-egl no longer webgl's (still nir's); the same hash before and after each job on both machines.
- wasm32: the libraries of gpu-webgl, gpu-webgpu, gpu, gpu-trace, opengl-es and egl exit 0; webgpu's scenes page 0;
  webgl's `browser` and `draw-cost` and trace's `replay` 101 (no WebGL implementation of the trait yet).
- gpu-webgl on the pinned Mesa's EGL (llvmpipe, surfaceless, no old lints table any more): 89 passed, 2 failed, its
  17 own tests and 72 of the 74 suite programs; the two are the ones that failed before the port, the same way
  (`a_copy_within_one_depth_or_stencil_subresource_moves_the_region`: depth24 rounding, 0.6249999 for 0.625;
  `edges_through_pixel_centres_follow_the_top_left_rule`: texel (0, 0) not drawn). Before (d19c23829, scratch copy
  with the old table, both machines): the same 89 and 2.
- shaders 158 passed, 5 ignored (lavapipe); the OpenGL ES suite on the direct crates 4 of 4.
- Not run: ANGLE (Windows only, the desktop is not used in this run), the browser page (cannot build yet).

### Speed before and after (release, llvmpipe with LP_NUM_THREADS=4, one test thread)

The draw-heaviest suite programs by the number of interface draws each makes (counted on the recorder through the
trace crate, tools/r10_drawcount.py: 55, 8, 6, 5, 4, 3, 3), each run alone 7 times, median ms; and the whole suite in
one process, 3 runs. "absent" is `absent_abilities_answer_absent`, which draws nothing: the process, the context and
the probes.

| ARM64 | d19c23829 (two runs) | 19feea113 | 0833bd8d2 | d0f51528a |
|---|---|---|---|---|
| every_vertex_format_reads_its_element (55 draws) | 717, 726 | 727 | 723 | 722 |
| the_occlusion_counter_counts_every_sample_that_passes (8) | 376, 380 | 379 | 378 | 377 |
| missing_components_read_their_defaults (6) | 445, 450 | 449 | 448 | 445 |
| stencil_operations_and_tests (5) | 250, 252 | 253 | 251 | 250 |
| copies_between_depth_stencil_textures_move_both_aspects (4) | 245, 249 | 247 | 250 | 246 |
| vertices_pulled_from_storage_match_the_fetched_ones (3) | 175, 177 | 176 | 177 | 175 |
| textured_draw_with_depth_and_blending (3) | 266, 269 | 268 | 266 | 266 |
| absent | 176, 179 | 177 | 177 | 176 |
| whole suite, one process (s) | 12.12 to 12.25 | 16.00 to 16.11 | 12.11 to 12.13 | 12.08 to 12.11 |

| x86-64 | d19c23829 (three runs) | 19feea113 | 0833bd8d2 | d0f51528a |
|---|---|---|---|---|
| every_vertex_format_reads_its_element | 667, 718, 666 | 669 | 668 | 727 |
| the_occlusion_counter_counts_every_sample_that_passes | 244, 262, 243 | 247 | 248 | 265 |
| missing_components_read_their_defaults | 272, 270, 272 | 274 | 278 | 298 |
| stencil_operations_and_tests | 204, 202, 204 | 204 | 208 | 225 |
| copies_between_depth_stencil_textures_move_both_aspects | 201, 199, 201 | 203 | 205 | 222 |
| vertices_pulled_from_storage_match_the_fetched_ones | 169, 168, 172 | 178 | 172 | 176 |
| textured_draw_with_depth_and_blending | 217, 216, 223 | 233 | 223 | 221 |
| absent | 170, 169, 172 | 187 | 174 | 171 |
| whole suite, one process (s) | 10.34 to 11.05 | 13.89 to 14.72 | 10.31 to 10.33 | 10.35 to 10.39 |

- Reading: on ARM64, the quieter machine, every draw-heavy program is within 1% of before at every commit: the glue's
  checks (an eglGetCurrentContext per command, the per-draw attribute queries, the pixel store reads) cost nothing
  visible next to llvmpipe's own work, and the plain-data handles nothing either. On x86-64 the numbers move by up
  to 8% between runs of the same commit (before: 667 and 718 for the first program), and the tip's run met a
  one-minute load of 8 against 4 to 5 for the others; its whole suite equals before's. The one real loss, the
  library reloaded for every back end, is gone since 0833bd8d2.
- The x86-64 before run (before-x86.log) built with 96 jobs: its job omitted `--jobs 12`, against the run's rule
  (every later job passed it). Its times are kept because the runs that followed with 12 agree with it.

### Linter (da55b9d7, x86-64, no model key; r9/after-x86 at ad62381ac, the code of d19c23829, against r10/after3-x86)

| crates/gpu/implementation/webgl | before | after |
|---|---|---|
| all rules | 6,724 | 6,649 |
| public_outside_interface | 177 | 88 (the handles' private fields and `PartialEq`, the helper blocks, `pub mod platform` and the Rc handle types gone) |
| conformance_not_run | 0 | 2 (below) |
| unparsed | 7 | 7 |
| name_unanswered, summary_unanswered | 0, 4 | 20, 18 (new names and summaries wait on the model's answers; the key stays on the Mac) |

- New, for the orchestrator: conformance_not_run 2: "`wcontainers_gpu::StandaloneGpu` is implemented for
  `wcontainers_gpu_webgl::Gl<wcontainers_opengl_es_direct::OpenGlEsContext>`, which is not an exported type, so the
  conformance suite, run on the exported one, never runs this implementation" (and its twin on the run's line). The
  native `StandaloneGpu` names the direct implementation because the EGL type cannot be named by `Gl<O>` alone (a
  generic impl over `O: OnEgl<E>` leaves `E` unconstrained). It runs: conformance_run.rs builds that type and its 74
  programs run (above).
- crates/egl 141 and crates/opengl-es 1,377 findings come with the API track's crates (their own status counts them).

### Milestones

- GI7 reached by its brief's bar where it can be measured here: the crate and shaders build under `forbid` with all
  targets on both machines; gpu-webgl's 17 and the shared suite pass as before the port (on Mesa's EGL, not ANGLE:
  ANGLE is the desktop's, not used in this run); shaders 158 (157 and the tessellation test); wasm32 check of the
  library exit 0 under `forbid`. Not met: the browser pages, which wait on the trait's WebGL implementation. The
  independent check is owed.
- Also done: webgl's two helper blocks as functions, its handles as plain data, gpu-webgl's 159 unsafe lines gone.
- Next: GI4 and the window question wait on the owner; GI6's removal on the cost question; GI8 part B on the API
  track's Mesa compiler (AC11) and Gallium (AC8) traits; the browser route and two-sided stencil on the API track
  (NEEDED-FROM-API-CRATES.md, "After GI7").

## The eleventh run (2026-11-27): GI8 part B on the Mesa compiler's trait, the bridge's home, the two webgl failures

Logs in the scratch folder graphics-implementations/r11/ (at-383cea56b-arm.log and -x86.log are the measurement
at the pushed tip; dev*.log the work; probe*-*.log the experiments; lint-* the linter outputs); the job is
tools/r11-job.sh, the port of direct.rs tools/gi8b_direct.py with gi8b_flat.rs.txt, the rule tables
tools/r11_rules.py, the experiment r11/probe_r11.rs (never committed: put into a scratch copy only). Commits
8570ccee4, 915486ad5, 383cea56b, pushed.

### His main

- `git fetch /Users/thomaseaton/Documents/GitHub/wcontainers main` gives ac696ea19; `git merge-base --is-ancestor
  FETCH_HEAD HEAD` exit 0, `git log HEAD..FETCH_HEAD` empty: nothing to merge (MEASURED). The day's
  instruction-interface changes are in it, as the ninth run recorded; no graphics crate names those interfaces.

### The API track's proposal: the bridge's C and its build move into crates/mesa-compiler/implementation/direct

**Yes.** The bridge is the outside API's C; it belongs beside the only crate that calls it, and nir then holds no
C. A second reason, MEASURED this run: nir's tests link today only because the direct crate's undefined `wc_*`
references are met by the archive nir's build makes (both linkers, ARM64 ld.bfd and x86-64 rust-lld); nir itself no
longer names a bridge function, so with ld.bfd's one pass this rests on the order rustc lists the two crates'
archives (INFERRED). With the build in the direct crate its own archive follows its own references.

What moves (paths as on this branch):
- crates/nir/bridge/: bridge.c, bridge.h, bridge_private.h, flat.c, flat.h, glsl.cpp, spirv.c, to
  crates/mesa-compiler/implementation/direct/bridge/.
- From crates/nir/build.rs into the direct crate's own build.rs: the bridge's compile (the `cc` build of bridge.c
  and flat.c, spirv.c with `standalone`, and the GLSL front glsl.cpp: lines 144 to 171 and 194 to 202), the include
  folders and the flags read from Mesa's compile commands (`INCLUDES`, `FLAGS_FROM`, `CPP_FLAGS_FROM`), the static
  link of Mesa's compiler libraries (`LIBRARIES`, lines 204 to 219), the renamed copy of Mesa's BLAKE3
  (`renamed_blake3`, `tool` and their constants), `mesa_dependencies` and `SYSTEM_LIBRARIES`.
- nir's `standalone` feature becomes the direct crate's (nir's feature turns it on), since the OpenGL driver linked
  into Mesa's own library takes only bridge.c and flat.c and links no Mesa library twice.

What nir keeps: src/ (bind.rs, direct.rs, lib.rs, mesa.rs, spirv.rs, the routes over the trait), tests/ and
corpus/, and a build.rs that only sets `cfg(mesa)` and the converter version's parts and the reference paths:
`WCONTAINERS_MESA_MADE_FROM`, `WCONTAINERS_NIR_SOURCES`, `WCONTAINERS_LAVAPIPE`, `WCONTAINERS_EGL`. The pinned Mesa
build itself (crates/nir/mesa: the flake, `build`, meson_build.rs) stays where it is for now: opengl's build.rs,
the webgpu suite on lavapipe and the webgl suite on Mesa's EGL use it too; moving it is a separate step.

Conditions, each measured before as needed: (a) the ARM64 link repairs stay (no libmesa; Mesa's BLAKE3 from the
renamed copy: seventh and eighth runs); (b) with no `WCONTAINERS_MESA` the direct crate compiles nothing and links
nothing, so every build without Mesa still checks; (c) the converter version still changes when the bridge's C
changes (cache keys): the direct crate hands its bridge's hash to nir (a `links` key and `DEP_` variable, as
shader-converters-linked does) or nir hashes the moved folder; (d) the standalone-only functions are declared
only with the feature (NEEDED-FROM-API-CRATES.md, "After GI8 part B's first step", item 3).

### GI8 part B, first step: nir on the Mesa compiler's trait (8570ccee4, 915486ad5, 383cea56b)

- crates/mesa-compiler and crates/mesa-compiler/implementation/direct taken from origin/graphics-api-crates
  051b56c5f by `git checkout <commit> -- crates/mesa-compiler` (only that path; nothing else of that branch), byte
  for byte. Cargo.lock: the two workspace packages added and nir's and opengl's new edges, no outside crate
  (14 added lines; hash 7c2859246daceed2 the same before and after every job on both machines).
- `Nir<M: MesaCompiler>` holds the compiler and its shader; its methods (`impl<M: MesaCompiler + Clone>`) call the
  trait; `route`, `route_patches` and `module` are generic. nir's own types (`Stage`, `Swizzle`, `Wrap`,
  `TextureSwizzle`, `NearestSampler`, `PositionTransform`, `StreamedOutput`, `GlslName`) stay and are converted to
  the trait's at the call, so opengl's code changed only in naming `Nir` (an alias over the direct crate's
  `Bridge` in src/state.rs). `from_raw` and `compiler_options` are gone (the Gallium side, AC8).
- Every number the bridge gives is read by a checked conversion (zerocopy's `TryFromBytes` on the enumerations, in
  their C order), refused by name as `ConvertError::Unknown(enumeration, value)`: the flat form's stage, type
  kinds, scalars, dimensions, variable modes, built-ins, interpolations, storage formats, instruction kinds,
  geometry and tessellation enumerations; `to_common_form`'s result code and each binding's kind (the kind was an
  unchecked array index before); the GLSL names' kind and interpolation. This closes the second open SECURITY
  finding (enumerations from the C bridge transmuted with no range check) on our side.
- Hostile test, tests/hostile_compiler.rs: a stand-in implementation of the trait giving numbers past every
  enumeration (0x80000001); 7 tests, each refusal by name, no panic, no Mesa reached.
- MEASURED at the pushed 383cea56b, both machines alike (Mesa built in the job, 101 s x86-64):

| | ARM64 (ld.bfd) | x86-64 (rust-lld) |
|---|---|---|
| check, Mesa on, workspace lints (`forbid`): mesa-compiler, mesa-compiler-direct | exit 0, 0 errors | exit 0, 0 errors |
| check, Mesa on, `forbid`: nir `--lib` | exit 0 (before: 59 errors, GI1) | exit 0 |
| check `--all-targets`, Mesa on, `forbid`: nir | exit 101: 43 unsafe errors, all in the corpus tests' llvmpipe.rs and lavapipe.rs (A6, A7) | the same |
| check without Mesa: nir, opengl | exit 0 | exit 0 |
| nir `--lib --test route --test hostile_compiler` under `forbid` | 35 passed | 35 passed |
| nir, whole package, old table in the scratch copy | 48 passed, 2 ignored (41 + 7 hostile) | 48 passed, 2 ignored |
| clippy `-D warnings` (nir, mesa-compiler, -direct; lib, route, hostile_compiler) | exit 0 | exit 0 |
| `cargo fmt --check -p wcontainers-nir` | exit 0 | exit 0 |
| opengl, Mesa on, old table in the scratch copy | does not compile: `compiler_options` not found (screen.rs:578), `from_raw` not found (state.rs:1138), 3 errors following from them | the same |
| gpu-webgl on Mesa's EGL (unchanged code) | 89 passed, 2 failed (the two below) | 89 passed, 2 failed |

- What it costs: opengl's Mesa build (scratch copy, old table) passed 11 tests before this run and does not compile
  now, until the Gallium crate gives the screen's options and the shader states' owned shaders
  (NEEDED-FROM-API-CRATES.md, "After GI8 part B's first step", item 1). It was never under `forbid`. GI8's bar (the
  GLES lists, at least 110,431 of 110,703) needs opengl, so it waits on AC8.
- Linter da55b9d7, no model key, tables identical on both machines (83,865 findings each), against the same linter
  on 7e53ee79e (ARM64): crates/nir 984 to 953 (magic_literal 121 to 105, name_unanswered 58 to 40, verb_not_first 28
  to 14; summary_unanswered 6 to 16 and the summary-form rules +3 to +6 for the new items, which wait on the model's
  answers; depends_on_implementation 1 to 2: nir's tests depend on the direct crate, as they must to reach Mesa);
  crates/opengl 1,327 to 1,329 (depends_on_implementation +1: it names the direct crate, being Mesa's driver;
  summary_unanswered +1); crates/mesa-compiler 477 (the API track's crates: number_constant_without_link 209,
  summary_unanswered 79, magic_literal 67, name_unanswered 47, public_outside_interface 8, the rest below 20).
  Also new, from the added crate: shaders' constant_defined_twice +1 (`NARROW_BITS = 16` in
  src/dxil/widths.rs equals the bridge's; shaders cannot name it). compiler_view_missing 53 to 43 is the before
  run's own compile gap (wcontainers_arm64 did not compile in that copy), not this change.

### The two suite programs that fail on webgl over Mesa: causes (MEASURED, both machines alike)

Experiment r11/probe_r11.rs, run in a scratch copy of webgl as unit tests against Mesa's EGL (llvmpipe, LLVM
21.1.8; ARM64 128-bit and x86-64 256-bit give the same lines).

**`edges_through_pixel_centres_follow_the_top_left_rule`: Mesa's GL state tracker, under the upper-left clip
origin the back end chooses; the test's expectation is right (Direct3D's rule, which the fronts need).** The suite's
two triangles drawn in plain GL, each route of the back end, memory row 0 (Direct3D's top) first:

| EXT_clip_control upper-left, y as given (the back end's route where the extension exists) | lower-left, y negated in the program (its route without it) | the suite's expectation |
|---|---|---|
| `....` `AAB.` `ABB.` `BBB.` | `AAA.` `AAB.` `ABB.` `....` | `AAA.` `AAB.` `ABB.` `....` |

With the upper-left origin the top row and left column are dropped and the bottom row taken: the bottom-left
rule. READ in the pinned Mesa, src/mesa/state_tracker/st_atom_rasterizer.c:277-283: `bottom_edge_rule` is set for
a top-origin framebuffer and toggled again when `ClipOrigin == GL_UPPER_LEFT`, and llvmpipe applies it
(lp_setup_tri.c:312, 505). GL does not fix which edge rule a rasterizer uses, so this is allowed GL; the back end
assumes upper-left gives Direct3D's rule (true on ANGLE over Direct3D 11, the route it was built on). Fix, next
step: the back end probes the rule once (these two triangles) and, where upper-left gives the bottom-left rule,
draws with the lower-left origin and y negated (keeping `ZERO_TO_ONE` depth through EXT_clip_control), which the
second column shows gives Direct3D's rule on llvmpipe. Not built this run.

**`a_copy_within_one_depth_or_stencil_subresource_moves_the_region` (Depth24Plus texel 5, 0.6249999 for 0.625):
the back end's write and read arithmetic, which Mesa's compiler and sampler do not meet; the expectation is right.**
The texel was not copied: it is the write and the read alone. Per value v (24-bit), the float the write program
makes (E), what llvmpipe stores for it and what the read program gives back:

| v (texel) | v / (2^24 - 1), nearest float | E, the write program's float | stored | read back |
|---|---|---|---|---|
| 10,485,759 (0.625, texel 5) | 0x3f200000 | 0x3f1fffff | 10,485,758 | 10,485,758 (fails) |
| 12,582,911 (0.75) | 0x3f400000 | 0x3f3fffff | 12,582,910 | 12,582,910 |
| 14,680,063 (0.875) | 0x3f600000 | 0x3f5fffff | 14,680,062 | 14,680,063 |
| 16,777,215 (1.0) | 0x3f800000 | 0x3f7fffff | 16,777,214 | 16,777,215 |
| 14,680,063 written from 0x3f600000 | | | 14,680,063 | 14,680,064 |

- Write: E is v 2^-24 exactly, one float step under the nearest: the program's `v * 2^-24 + v * 2^-48` (convert.rs;
  webgpu's fill.rs writes the same sum in WGSL) is regrouped by Mesa's NIR into `v * (2^-24 + 2^-48)` (READ:
  nir_opt_algebraic.py:246, the inexact rule `fadd(fmul(a, b), fmul(a, c))` to `fmul(a, fadd(b, c))`), and
  2^-24 + 2^-48 rounds to 2^-24. llvmpipe then stores round(E (2^24 - 1)) (READ: lp_bld_conv.c:307-323), one low
  for every v above 2^23. GLSL ES and WGSL let a compiler regroup; the comment's "rounded once" does not hold.
- Read: llvmpipe samples a stored k as k times the float nearest 1/(2^24 - 1) (READ: lp_bld_conv.c:416-427), one or
  two steps above k 2^-24, so the read program's round(d (2^24 - 1)) gives k + 1 in the upper quarter of each
  binade (k = 14,680,063 reads 14,680,064 in the last row). For 0.875 and 1.0 the two errors cancel; for 0.625 and
  0.75 only the write's shows (the suite stops at the first, texel 5).
- The same two steps explain GI2's lavapipe failure of this program and of webgpu's
  `depth_and_stencil_parts_copy_and_read_back_exactly` (identical message; webgpu's write is the same sum).
  INFERRED for webgpu's read: not probed.
- Fix, next step, both halves together (the write alone turns 0.875 into a failure: k + 1 on read): write the
  nearest float with integers only (`uintBitsToFloat(floatBitsToUint(float(v) * 2^-24) + (v != 0 ? 1 : 0))`: v
  2^-24 is exact and the remainder is more than half and at most one step; MEASURED above: the "E + 1" write of
  0x3f200000 stores 10,485,759), and read by inverting the device's own conversion, chosen by a probe at start
  (nearest float, as a Direct3D device gives, against k times the rounded reciprocal, as llvmpipe gives), since the
  two give the same float for different k in the upper quarter. The write half is saved as
  r11/depth-write-fix/write-half.patch, not committed.

### shader-converters' 5 lines and opengl's 354: which API crate takes them, and what each waits on (READ)

- shader-converters, 5 (implementation/linked/implementation.rs: the `BoundaryFunction` type, the `unsafe extern
  "C"` block of our C boundary over dxbc-spirv and dxil-spirv, the call, the config's `PointerAndLength`, the
  output read). The outside API is our C boundary (cpp/) over the two converters; by the one-crate-per-API ruling
  they go to the API track's crate for it (the generator's trait over that boundary, linked/ becoming its
  `implementation/direct`), or go entirely with the linked route if GI6 removes it. Waits on: the owner's question
  1 (may the linked route stay, given the contained route's measured cost: 150 to 330 ms once per process, about
  twice per shader, 22 to 32 s more on the worst game shader, third run); only if it stays does the API track
  write the crate.
- opengl, 354 (MEASURED by kind with the counting method: 249 lines in `unsafe` blocks reading and writing Gallium's
  structures through pointers and calling Mesa's helpers, 82 `unsafe extern "C" fn` callbacks Mesa calls through
  `pipe_screen` and `pipe_context`, 13 `unsafe fn` of ours, 8 `Box::from_raw` of driver objects Mesa hands back, 1
  `extern` block, 1 `#[unsafe(no_mangle)]` entry point `wcontainers_screen_create`). All go to the Gallium crate
  (AC8): its trait of the 95 callbacks our driver fills, its trait of the 23 helper calls and our two field readers
  (NEEDED-FROM-API-CRATES.md, seventh run), with the screen's NIR options and owned shader states of item 1 above;
  opengl then implements the callbacks' trait over `Gpu`. Waits on: AC8, which the API track's ORDER line has
  waiting on agreement of where the OpenGL driver lives (the back-ends track).

### Milestones

- Reached: none of the brief's bars newly. Done and measured at a pushed commit: GI8 part B's first step (nir on
  the Mesa compiler's trait, its library under `forbid` with Mesa on, nir 113 to 56 `unsafe` lines, the enum
  SECURITY finding closed on our side with a hostile test); the proposal answered; both webgl failures' causes found
  by experiment. GI7's independent check held (orchestrator's note).
- Next: the two webgl fixes above (wait on nothing; the next run's first work); GI8 part B's rest (opengl as the
  Gallium trait's implementation, the GLES lists) waits on AC8 and on the bridge's move (the API track); nir's
  corpus tests' 56 lines on A6 (a desktop OpenGL crate) and A7 (Vulkan); GI4 and the window question on the owner;
  GI6's removal on question 1.
- Not done: the hostile-guest suite (none exists; the thread-test design review is still owed to the owner).

## SECURITY

CORRECTED in the seventh run: every count below is by the track's one counting method, used from now on: lines of
`.rs` files under each crate folder that match the whole word `unsafe` and are not comment lines (after leading blanks
they do not start with `//`); attribute lines, strings and `reason =` lines count, so `unsafe_code` does not match
and `#[unsafe(no_mangle)]` does (scratch graphics-implementations/tools/count-unsafe.sh). The root manifest's command
that the runs before used leaves attribute lines out, so it missed webgl's `#[unsafe(no_mangle)]`
(examples/browser.rs) and opengl's (src/screen.rs:591): its 733 and 731 were 735 and 733. MEASURED with the one method:

| crate | graphics-layout 97e1daafe | GI1 9843ce0ce | third run 1282b3928 | fourth to sixth runs (948af1837, 221883a43) | seventh run (a9ddff2f4, 9fba25913, 42d75efa2, 58cc43187) |
|---|---|---|---|---|---|
| gpu (interface and shared code) | 0 | 0 | 0 | 0 | 0 |
| gpu-test / conformance, recording, trace | 0 | 0 | 0 | 0 | 0 |
| gpu-webgpu | 19 | 19 | 19 | 19 | 19 |
| gpu-webgl | 161 | 161 | 161 | 159 | 159 |
| shaders | 71 | 71 | 71 | 71 | 71 |
| shader-converters | 5 | 5 | 5 | 5 | 5 |
| nir | 113 | 113 | 113 | 113 | 113 |
| opengl | 354 | 354 | 354 | 354 | 354 |
| test-window | 12 | 12 | 12 | 12 | 12 |
| total | 735 | 735 | 735 | 733 | 733 |

- Marked exceptions in crates with their own deny table: shader-converters linked 5, test-window appkit 8 and win32 4.
  No marked exception is written by this track (ruling of 2026-11-25); none added in any run.
- The fourth run's two fewer webgl lines are the ANGLE-only `impl Traced<Gl>`, removed with it.
- New dependencies, by run: third run none from outside the workspace (wcontainers-volatile-bytes in gpu, webgpu and
  conformance; byte-sink and byte-source in webgpu and conformance); fourth run bytemuck (already in the lock through
  webgpu) as a webgl dev-dependency, and the trace crate uses postcard, serde and naga as webgl did; sixth run none
  (wcontainers-graphics-test, already a Windows dev-dependency of webgpu, on every native target); seventh run none
  (nir takes the blake3 it already had with its `no_neon` feature on ARM64; Cargo.lock unchanged, MEASURED by hash
  before and after the build on both machines); eighth run none (the `no_neon` feature dropped; nir's build script
  calls nm, objcopy and ar from the toolchain, no new crate). Unsafe lines at 9034444b8: the same 733 (gpu-webgpu 19,
  gpu-webgl 159, shaders 71, shader-converters 5, nir 113, opengl 354, test-window 12, the rest 0).
- Ninth run (8e20476d8, ad62381ac): the track's crates hold the same 733 (MEASURED with count-unsafe.sh at afe71ff74
  and ad62381ac). Brought in with the Metal back end, not written here: crates/metal/implementation/direct 44 lines,
  its `unsafe` blocks under 37 `#[expect(unsafe_code, reason)]` marks in a crate whose own table denies unsafe code
  (an outside API's `implementation/direct`, the place the rules allow); crates/gpu/implementation/metal 0. No new
  outside crate (Cargo.lock: the four Metal workspace packages only).
- No hostile-guest suite exists, so none was run in any run (the thread-test design review is still owed to the owner).
- Tenth run (24c372f21 to d0f51528a): gpu-webgl 159 to 0 lines (MEASURED with count-unsafe.sh at d19c23829 and
  d0f51528a); the track's total 733 to 574. No marked exception written here. Brought in with the API track's crates,
  not written here: egl-direct 56 lines under 39 marks and opengl-es-direct 474 under 318, each in a crate whose own
  table denies unsafe code (an outside API's `implementation/direct`). New dependencies: the five workspace crates
  wcontainers-egl, -egl-direct, -opengl-es, -opengl-es-direct, -opengl-es-conformance (libloading 0.8.9 through
  egl-direct, already in the lock); webgl dropped glow and khronos-egl. No hostile-guest suite exists, none was run.
- Eleventh run (8570ccee4 to 383cea56b): nir 113 to 56 lines (src/mesa.rs 52 and src/direct.rs 5 to 0; the corpus
  tests' llvmpipe.rs 39 and lavapipe.rs 17 left), MEASURED with count-unsafe.sh at 7e53ee79e and 383cea56b; the
  track's total 574 to 517 (webgpu 19, shaders 71, shader-converters 5, nir 56, opengl 354, test-window 12, the
  rest 0). No marked exception written here. Brought in, not written here: crates/mesa-compiler/implementation/direct
  49 lines under 48 `#[expect(unsafe_code, reason)]` marks, in a crate whose own table denies unsafe code (an
  outside API's `implementation/direct`); crates/mesa-compiler 0. Closed on our side: enumerations from the C bridge
  read with no range check (every one now a checked conversion, hostile test tests/hostile_compiler.rs, 7 passing on
  both machines). New dependencies: the workspace crates wcontainers-mesa-compiler and -direct only (no outside
  crate; Cargo.lock hash 7c2859246daceed2 before and after every job). The SAMPLES_PASSED abort finding is opengl's
  and waits with it on AC8. No hostile-guest suite exists, none was run.
