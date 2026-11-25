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

## SECURITY

- Lines with unsafe, unchanged from graphics-layout: gpu-webgl 160, opengl 353, nir 113, shaders 71, gpu-webgpu 19,
  test-window 12, shader-converters 5; gpu and gpu-test 0.
- Marked exceptions in crates with their own deny table: shader-converters linked 5, test-window appkit 8 and win32 4.
- No new dependency. No hostile-guest suite exists.
