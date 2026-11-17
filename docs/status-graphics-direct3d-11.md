# Direct3D 11 front status

Branch: phase-2-graphics-direct3d-11. Plan: docs/phase-2-graphics.md, milestone G4 (section 12); this
run worked on G4a, the first slice.

## Where things stand

G4a (the rig's three Direct3D 11 scenes drawn through our own d3d11.dll) is partly done:
- The front (crates/wcontainers-direct3d-11) serves device creation at 11_0 (10_0 without compute),
  buffers, 2D textures (arrays, cubes, mips, immutable and staging), render target, depth-stencil and
  shader resource views of 2D textures, samplers, the blend, depth-stencil and rasterizer states, input
  layouts, vertex and pixel shaders, constant buffers, draws (plain, indexed, instanced; every list and
  strip topology; the adjacency topologies without a geometry shader through `AdjacencyDropped`),
  clears (taken as the next pass's loads, or `clear_texture`), passes opened at draws, copies,
  `UpdateSubresource`, maps of staging resources (read back) and of dynamic buffers (`WRITE_DISCARD`,
  `WRITE_NO_OVERWRITE`, uploaded whole on `Unmap`, `MappedUploadWhole`), `ClearState` and `Flush`.
  Released objects are destroyed at the device's next call.
- The Windows library (crates/wcontainers-directx, `wcontainers_directx.dll`) exports
  `D3D11CreateDevice` and wraps the front in COM objects made with the `windows` crate's `implement`:
  the device (`ID3D11Device` and `IDXGIDevice`), the immediate context, an adapter
  (`IDXGIAdapter1`, named after the GPU), and every object above. It draws through the WebGPU back end
  (merged from origin/phase-2-graphics-webgpu) on `wgpu`'s most capable adapter.
- The three scene programs, unchanged, with our library beside them as d3d11.dll, on the RTX 3080
  (2026-11-17, desktop otherwise in use by other tracks' queue jobs, load not measured):
  - triangle: pass, 0 of 65536 pixels differ, largest difference 0.
  - quads: pass, 0 of 65536 pixels differ, largest difference 0.
  - topology: FAIL, 43 of 65536 pixels differ, all in the six cells that draw lines (line list, line
    strip, both line adjacency kinds, both indexed line strips); points and every triangle cell are
    exact. Lines are rasterised by Direct3D 12 under `wgpu` with a different rule from Direct3D 11's
    aliased lines; the plan lists this as a loss (4.12, line rasterisation) and asks G4 to measure it:
    this is the first measurement. Left failing.
- The same three programs through the rig's forwarding library pass with largest difference 0, so
  the programs and references are unchanged.

What G4a still lacks, most important first:
1. Merge origin/phase-2-graphics-direct3d-12 (it now holds the infrastructure crate and an interface
   change: caps hold the adapter description) and origin/phase-2-graphics-shaders; take the adapter
   name from the infrastructure crate instead of `com::Gpu`.
2. Real DXBC conversion: the six shaders of the three scenes are hand-written WGSL with hand-written
   reflections (crates/wcontainers-direct3d-11/src/standin.rs), keyed by the blake3 of the DXBC like
   cache entries; every other shader is refused with `E_NOTIMPL`. Replace with
   `wcontainers_shaders::convert` when G6 has it.
3. Forwarders: d3d11.dll, d3d12.dll and dxgi.dll as export-forwarding libraries to
   wcontainers_directx.dll (section 10). Today the library itself is copied in as d3d11.dll.
4. A small Direct3D 11 program of our own that the front should handle and one it should refuse, run
   through the library (the COM tests cover the refusal of an unconverted shader and creation, but no
   separate program exists yet).

## How to run

From the checkout root on the Mac (the desktop does all building and testing):
- Front tests (recording back end, no GPU):
  `tools/graphics-rig/remote "cargo test -j 3 -p wcontainers-direct3d-11"` (9 tests).
- Library tests (COM behaviour; they make a real device, so they run as a GPU job):
  `tools/graphics-rig/gpu-job --build "cargo test -q -j 3 -p wcontainers-directx --no-run" -- "cargo test -q -j 3 -p wcontainers-directx"` (6 tests).
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 -p wcontainers-directx -p wcontainers-direct3d-11 --all-targets -- -D warnings"`.
- The scenes through our library:
  `tools/graphics-rig/scenes --build "cargo build -q -j 3 -p wcontainers-directx" --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' --api d3d11 triangle quads topology`.
  Logs on the desktop in C:\wc\out\phase-2-graphics-direct3d-11\scenes\d3d11\<scene>.log and .stdout.
- The same through the forwarding library: the same command with
  `--build "cargo build -q -j 3 -p wcontainers-graphics-test-forward"` and
  `--dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll'`.
- Scratch: pyro-parked/wcontainers-scratch/graphics-direct3d-11 holds gen_com.py (writes
  src/com/generated.rs from the windows crate's trait definitions, also extracted there) and pngdiff.py
  (which cells of two PNGs differ).

## How it is built

- `Device<B: Backend>` holds the back end, the immediate context's state and caches of programs (per
  vertex and pixel shader pair) and vertex layout blocks (per input layout and strides). Context
  methods are on the same type. Front objects (`object.rs`) hold back-end handles in `Kept`, which on
  drop hands the handle to a graveyard the device empties at its next call.
- Binding sets: set 0 for the vertex stage, set 1 for the pixel stage, made from the shader's
  reflection when a slot of that stage or its shader changed (plan 6.4).
- Creation checks run in the front before any back-end call and answer `E_INVALIDARG` as Direct3D
  does; what is valid but not served yet answers `E_NOTIMPL` with a warning. A draw that cannot be
  served (a shader reading an empty slot, storage views, indexed adjacency) removes the device; COM
  methods not served yet answer `E_NOTIMPL`, and those with no result (on the device and context)
  remove the device, each warned once.
- src/com/generated.rs is generated (scratch gen_com.py); hand-written methods live beside the objects.
- The recording back end (`record.rs`) logs every call and keeps host writes and copies, so the front
  tests check the calls each scene makes and the bytes of read backs.

## Tests

- crates/wcontainers-direct3d-11/tests/front.rs, 9 tests: the three scenes' calls on the recording
  back end (the scenes' own DXBC from the rig's corpus), the stand-in keys against the corpus, feature
  level choice, destruction after release, creation refusals (including blend targets past the first),
  dynamic constant upload, and an unservable draw removing the device.
- crates/wcontainers-directx/tests/com.rs, 6 tests on the RTX 3080: QueryInterface identity and
  parents, children holding one reference on their device, the immediate context holding one and being
  one object while it lives, views holding and returning their resource, private data (sizes, too
  small a buffer, not found), and an unconverted shader refused with `E_NOTIMPL`.
- Last run of all of them: 2026-11-17, 9 and 6 passed.

## Departures from the plan

- No call buffer or device thread yet: COM calls run the front on the calling thread (section 4.8
  puts the channel in the second batch; the Windows shell was to have it in-process). Calls from
  several threads are not guarded yet.
- The recording back end is in the front crate (`record.rs`), not G1's wcontainers-gpu-test, which
  was not on any branch when this began. It stays for the front's tests until G1's lands.
- The pass tracker and combination tracker of G1 are not there yet; the front opens a pass at the
  first draw after its targets change and ends it before copies, maps and clears, with clears
  pending until a pass on their view or the next work outside a pass.
- The front pads each buffer to the back end's copy granule (4 bytes on WebGPU) and writes initial
  data padded with zeros, so a 9-index 16-bit buffer needs no edge merge (the WebGPU back end's
  `UnalignedBufferEdge` is not built). The program sees its own `ByteWidth`.
- The feature level is 11_0 at most; 11_1's requirements have not been checked.
- Stencil fields of a depth-stencil state are validated only when stencil is enabled, and the depth
  function only when depth is enabled (real Direct3D accepted zeroed stencil fields with stencil off
  in the quads scene); whether Direct3D checks the depth function with depth off is not yet known.
- `D3D11CreateDevice` with an adapter uses our adapter whatever is passed; the debug flag answers
  `DXGI_ERROR_SDK_COMPONENT_MISSING`; software, WARP, reference and null drivers answer
  `DXGI_ERROR_UNSUPPORTED`.

## Interface changes

None.

## Measurements

- 2026-11-17, RTX 3080 desktop, through the queue (not alone): the three scenes' pictures as above.
  The topology line differences: line list 6 pixels, line strip 9, line list with adjacency 3, line
  strip with adjacency 7, indexed 16 and 32-bit line strips 9 each (pngdiff.py).

## Known issues

- topology fails on line rasterisation (above).
- Threads: the objects are not safe to call from several threads at once.
- Program and vertex layout caches are pruned when a shader or layout dies; binding sets are made per
  change and destroyed when replaced.

## Licences

Added: `windows` and `windows-core` 0.62.2 (MIT OR Apache-2.0), `wgpu` 30.0.2 (MIT OR Apache-2.0),
`pollster` 1.0.1 (Apache-2.0/MIT), `blake3` 1.8.7 (CC0-1.0 OR Apache-2.0), `naga` 30.0.2 (MIT OR
Apache-2.0); all already in the workspace's lock file. Nothing installed on the desktop beyond build
output in C:\wc\target\phase-2-graphics-direct3d-11 and pictures in C:\wc\out.
