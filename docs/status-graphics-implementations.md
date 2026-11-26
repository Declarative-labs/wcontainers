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

## SECURITY

- Lines with unsafe, unchanged from graphics-layout: gpu-webgl 160, opengl 353, nir 113, shaders 71, gpu-webgpu 19,
  test-window 12, shader-converters 5; gpu and gpu-test 0.
- Marked exceptions in crates with their own deny table: shader-converters linked 5, test-window appkit 8 and win32 4.
- Second run: unchanged (733 lines in the nine crates; this run added none and removed none). No marked exception is
  written by this track any more (ruling of 2026-11-25).
- No new dependency. No hostile-guest suite exists.
