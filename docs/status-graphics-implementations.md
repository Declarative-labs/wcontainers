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
  --is-ancestor` exit 0, `git log HEAD..FETCH_HEAD` empty): nothing to merge. READ: the instruction-interface
  changes named for today (`meets_expectations`, `simulate_state`, `MemorySnapshot`, ...) are not on that main yet;
  no graphics crate names those interfaces.

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
  FETCH_HEAD HEAD` exit 0): nothing to merge. READ: the instruction-interface changes named for today are not on that
  main; no graphics crate names those interfaces.

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
- No hostile-guest suite exists, so none was run in any run (the thread-test design review is still owed to the owner).
