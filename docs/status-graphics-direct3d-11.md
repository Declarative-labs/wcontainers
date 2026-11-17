# Direct3D 11 front status

Branch: phase-2-graphics-direct3d-11. Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
Runs so far: two on G4a, the first slice.

## Where things stand

G4a (the rig's three Direct3D 11 scenes drawn through our own d3d11.dll) is done:
- The three scene programs, unchanged, run with our library beside them and match real Direct3D 11
  exactly on the RTX 3080 (tolerance 0, largest difference 0): triangle, quads and topology. Their
  DXBC is converted by the shaders crate (`wcontainers_shaders::convert`, merged from
  origin/phase-2-graphics-shaders); the hand-written WGSL stand-ins are gone.
- They match both ways the library can be placed: the library itself copied in as d3d11.dll, and
  our forwarding d3d11.dll beside wcontainers_directx.dll.
- The same programs through the rig's forwarding library also match, so programs and references
  are unchanged.
- Two programs of our own (examples `own_handled` and `own_refused`, run by tests/programs.rs) run
  beside the system's d3d11.dll and beside ours. `own_handled` (dynamic vertex buffer by
  `Map(WRITE_DISCARD)`, indexed draws at four base vertices, a constant buffer rewritten by
  `UpdateSubresource` between draws, read back and checked texel by texel against the arithmetic)
  passes both ways, and reports that its `D3D11CreateDevice` resolved into C:\Windows\SYSTEM32\d3d11.dll
  and into our wcontainers_directx.dll respectively. `own_refused` gets 0 from real Direct3D for all
  four requests and from ours `E_NOTIMPL` for a geometry shader, a 3D texture and a structured
  buffer, and `DXGI_ERROR_DRIVER_INTERNAL_ERROR` from `GetDeviceRemovedReason` after a draw sampling
  an unbound slot (which Direct3D reads as zeros; we do not serve it yet, so the device is removed).
- Calls from several threads are safe: every COM method and every drop of state the objects share
  runs under one re-entrant library lock (test with 8 threads; the same test aborts without the lock).

What the front serves: device creation at 11_0 (10_0 without compute), buffers, 2D textures (arrays,
cubes, mips, immutable and staging), render target, depth-stencil and shader resource views of 2D
textures, samplers, blend, depth-stencil and rasterizer states, input layouts, vertex and pixel
shaders converted from DXBC, constant buffers, draws (plain, indexed, instanced; every list and strip
topology; adjacency topologies without a geometry shader through `AdjacencyDropped`), clears, copies,
`UpdateSubresource`, maps of staging resources and of dynamic buffers (`MappedUploadWhole`),
`ClearState` and `Flush`.

## What comes next (G4b and on)

1. Wine's Direct3D 11 tests through the library (the rig has them on origin/phase-2-graphics-rig),
   and the rig's other Direct3D 11 scenes; the gap list of section 12.
2. The helper constants (set 3): SV_VertexID and SV_InstanceID with non-zero start or base locations
   (now refused at the draw), sampler LOD bias, and the other variant fields of 6.4; choose the
   `VariantKey` per draw instead of the default.
3. Zeros for shader slots nothing is bound to (Direct3D defines the read; now the draw is refused).
4. The swap chain and presenting through the Direct3D 12 track's infrastructure crate (merged, not
   used yet: the three scenes save their pictures by reading back).
5. d3d12.dll and dxgi.dll forwarders, once the library exports their entry points. Before that,
   solve this: our library draws through wgpu on Direct3D 12, which loads d3d12.dll and dxgi.dll by
   name, so forwarders with those names in the program's folder would load our own library again.
6. The call buffer and device thread (section 4.8); today calls run on the caller's thread.

## How to run

From the checkout root on the Mac (the desktop does all building and testing). The desktop's
checkout needs the shaders crate's submodule, so commands start with `git submodule update --init -q`.
- Front tests (recording back end, no GPU), 11 tests:
  `tools/graphics-rig/remote "git submodule update --init -q && cargo test -q -j 3 -p wcontainers-direct3d-11"`.
- Library tests (COM behaviour, the forwarder, threads, our two programs; they make real devices, so
  a GPU job), 10 tests:
  `tools/graphics-rig/gpu-job --build "git submodule update --init -q && cargo test -q -j 3 -p wcontainers-directx --no-run" -- "cargo test -q -j 3 -p wcontainers-directx -- --test-threads 1"`
  (add `--nocapture` to see what the programs print).
- Lint: `tools/graphics-rig/remote "git submodule update --init -q && cargo clippy -q -j 3 -p wcontainers-directx -p wcontainers-direct3d-11 --all-targets -- -D warnings"`.
- The scenes through our library copied in as d3d11.dll:
  `tools/graphics-rig/scenes --build "git submodule update --init -q && cargo build -q -j 3 -p wcontainers-directx" --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' --api d3d11 triangle quads topology`.
- Through the forwarder: the same with
  `--dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\d3d11.dll' --dll 'wcontainers_directx.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'`.
  The pictures are checked as usual, but the rig also prints one "did not load the replacement
  wcontainers_directx.dll" FAIL per scene: its programs report only where d3d11.dll and dxgi.dll came
  from, so it cannot see the library the forwarder reaches. Read the picture lines.
- The rig's own forwarding library (the control): `--build "cargo build -q -j 3 -p wcontainers-graphics-test-forward"`
  and `--dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll'`.
- Logs on the desktop in C:\wc\out\phase-2-graphics-direct3d-11\scenes\d3d11\<scene>.log and .stdout.
- Scratch (pyro-parked/wcontainers-scratch/graphics-direct3d-11): gen_com.py writes
  src/com/generated.rs from the windows crate's trait definitions (d3d11-traits.rs, dxgi-traits.rs,
  extracted there): `python3 gen_com.py d3d11-traits.rs dxgi-traits.rs <checkout>/crates/wcontainers-directx/src/com/generated.rs`,
  then `cargo fmt`. pngdiff.py says which cells of two PNGs differ. logs/ holds this run's outputs.

## How it is built

- `Device<B: Backend>` holds the back end, the immediate context's state and caches of programs (per
  vertex and pixel shader pair), vertex layout blocks (per input layout and strides) and vertex
  shaders' input signatures (by blake3 of the bytecode, for `CreateInputLayout`). Front objects hold
  back-end handles in `Kept`, which on drop hands the handle to a graveyard emptied at the next call.
- Shaders (src/shader.rs): the stage is read from the DXBC container's `SHDR`/`SHEX` version token,
  so bytecode of the wrong stage is `E_INVALIDARG` as Direct3D answers; then `convert` with the
  default `VariantKey`. Bytecode the converter rejects is `E_INVALIDARG`; what it cannot convert yet,
  and a shader reading the helper constants, is `E_NOTIMPL`, warned with the converter's reason.
- Binding sets: set 0 vertex, set 1 pixel, made from the reflection when a slot or shader changed.
- Creation checks run in the front before any back-end call and answer `E_INVALIDARG` as Direct3D
  does; what is valid but not served answers `E_NOTIMPL` with a warning. A draw that cannot be served
  (an empty slot read, storage views, indexed adjacency, SV_VertexID/SV_InstanceID with a start or
  base, lines under a multisampling or antialiased-line state) removes the device; COM methods not
  served answer `E_NOTIMPL`, and those without a result (on the device and context) remove the
  device, each warned once.
- The Windows library: `wcontainers_directx.dll` exports `D3D11CreateDevice`; COM objects are made
  with the `windows` crate's `implement`; src/com/generated.rs is generated (above), each method
  taking the library lock (`com::lock`) and calling the hand-written method. State the objects share
  (`Rc`s of front objects and of the device) sits in `Locked<T>`, which drops it under the lock.
- The back end is wgpu on Direct3D 12 (`com::backend`): what browsers' WebGPU uses on Windows, and it
  rasterises aliased lines by Direct3D's rule. On wgpu's default choice (Vulkan on the desktop) the
  topology scene differed on 43 pixels, all on line draws.
- build.rs makes d3d11.dll with the MSVC linker (found by the `cc` crate) from a generated `.def`:
  no code, each export forwarding to `wcontainers_directx`. It is written beside the library in the
  profile's output directory (target\debug).

## Tests

- crates/wcontainers-direct3d-11/tests/front.rs, 11 tests: the three scenes' calls on the recording
  back end (the scenes' own DXBC from the rig's corpus, converted), shader stages read from every
  corpus container, feature level choice, destruction after release, creation refusals (wrong stage,
  unconvertible shader, blend targets past the first), dynamic constant upload, and draws that remove
  the device (an empty slot, a vertex id with a start location, lines under the other line rules).
- crates/wcontainers-directx/tests/com.rs, 8 tests: QueryInterface identity and parents, children
  holding one reference on their device, the immediate context, views holding their resource,
  private data, unserved shaders refused (bad bytes `E_INVALIDARG`, unconvertible `E_NOTIMPL`), the
  forwarder resolving `D3D11CreateDevice` to the library's own, and 8 threads making buffers at once.
- crates/wcontainers-directx/tests/programs.rs, 2 tests: our two programs, system and ours.
- Last full run: 2026-11-17, 11 + 8 + 2 passed, on the desktop.

## Departures from the plan

- No call buffer or device thread yet: COM calls run the front on the calling thread under one
  re-entrant lock (section 4.8 puts the channel in the second batch).
- The recording back end is in the front crate (`record.rs`), not G1's wcontainers-gpu-test, which is
  on no branch yet. It stays for the front's tests until G1's lands.
- No pass tracker or combination tracker from G1 yet; the front opens a pass at the first draw after
  its targets change and ends it before copies, maps and clears.
- Buffers are padded to the back end's copy granule (4 bytes on WebGPU); the program sees its own
  `ByteWidth`.
- The feature level is 11_0 at most; 11_1's requirements have not been checked.
- Stencil fields are validated only with stencil on, the depth function only with depth on.
- `D3D11CreateDevice` with an adapter uses ours whatever is passed; the debug flag answers
  `DXGI_ERROR_SDK_COMPONENT_MISSING`; software, WARP, reference and null drivers `DXGI_ERROR_UNSUPPORTED`.
- Only d3d11.dll is a forwarder (section 10 names d3d12.dll and dxgi.dll too): the library exports
  no Direct3D 12 or DXGI entry points yet, and a forwarder must not name exports that do not exist.
- The library draws through wgpu on Direct3D 12 only (section 10 allows Direct3D 12 or Vulkan): a
  machine without Direct3D 12 gets `DXGI_ERROR_UNSUPPORTED` from `D3D11CreateDevice`.
- Lines under `MultisampleEnable` or `AntialiasedLineEnable` are refused at the draw on any target.
  Microsoft's pages disagree on whether these apply to single-sample targets; this follows the
  reading that they always pick the quadrilateral and alpha rules (as DXVK does). Not yet measured
  against real Direct3D.

## Interface changes

None.

## Measurements

All on the desktop (Windows 10, i9-9900K, RTX 3080) through the queue, not alone; desktop CPU load
26 % when sampled after the last runs; Mac load average 18 to 25.
- 2026-11-17, run 2: triangle, quads, topology through our library (both placements) 0 of 65536
  pixels differ each, largest difference 0, wgpu on Direct3D 12.
- 2026-11-17, run 2, wgpu's default adapter (Vulkan), before choosing Direct3D 12: topology 43 of
  65536 differ (largest difference 229), all in the six line cells; triangle and quads 0. Run 1 had
  measured the same 43 and wrongly put it on Direct3D 12.
- Rig corpus through the shaders crate (its corpus test, run here for the verdicts): 56 of 79 shaders
  convert to WGSL (3 of them not to GLSL ES); the rest name their reason (geometry, hull and domain shaders, storage images and
  counters, atomics, `SV_PrimitiveID`, a vertex shader without a position).

## Known issues

- Threads: one lock serialises all calls; no parallelism between a program's threads.
- Program and vertex layout caches are pruned when a shader or layout dies; binding sets are made per
  change and destroyed when replaced. The input-signature map is never pruned (small, one entry per
  distinct vertex shader).
- The rig's replacement check cannot see wcontainers_directx.dll behind the forwarder (above).

## Licences

Added this run: `cc` 1.11.2 (MIT OR Apache-2.0, build dependency), `parking_lot` 0.12.5 (MIT OR
Apache-2.0), both already in the lock file; our `wcontainers-shaders`, which builds dxbc-spirv (MIT)
in. `naga` is no longer a direct dependency of the front. Earlier: `windows` and `windows-core` 0.62.2,
`wgpu` 30.0.2, `naga` 30.0.2 (MIT OR Apache-2.0), `pollster` 1.0.1 (Apache-2.0/MIT), `blake3` 1.8.7
(CC0-1.0 OR Apache-2.0). Nothing installed on the desktop beyond build output in
C:\wc\target\phase-2-graphics-direct3d-11 (including own-programs\ copies) and pictures in C:\wc\out.
