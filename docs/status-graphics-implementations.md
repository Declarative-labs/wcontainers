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

## SECURITY

- Lines with unsafe, unchanged from graphics-layout: gpu-webgl 160, opengl 353, nir 113, shaders 71, gpu-webgpu 19,
  test-window 12, shader-converters 5; gpu and gpu-test 0.
- Marked exceptions in crates with their own deny table: shader-converters linked 5, test-window appkit 8 and win32 4.
- No new dependency. No hostile-guest suite exists.
