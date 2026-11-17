# Direct3D 11 front status

Branch: phase-2-graphics-direct3d-11. Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
Runs so far: five on G4a, the first slice.

## Where things stand

G4a (the rig's three Direct3D 11 scenes drawn through our own d3d11.dll) is done:
- Run 5 merged origin/phase-2-graphics (G1's recording back end with usage checks and seeded call
  sequences; the interface's usage notes and no-overwrite rule), -webgpu (through the shared line) and
  -direct3d-12 (swap chains and outputs in its own example library; the infrastructure crate's
  output and swap chain changes). All merged cleanly. Then it fixed what the independent check of run 4
  found:
  - Reference counts are Direct3D's. Device children are COM objects of our own making
    (src/com/counted.rs, a `counted!` macro in place of the windows crate's `implement`) with two
    counts: the program's (`AddRef`, `Release` and what they answer) and the library's own (a view's
    resource, what the context has bound, the device's state objects), which the program never sees.
    An object lives while either is held and holds its device only while the program holds it, so a
    texture released by the program lives on in its view and comes back from `GetResource` at the same
    address (as DXVK keeps private counts). Checked against real Direct3D by the seeded random program
    `lifetimes` (below), which matches line by line.
  - Binding a subresource as a target unbinds its shader resource views in every stage, and binding a
    view of a subresource bound as a target binds none, as Direct3D does (by mip and layer ranges, so
    one level can be drawn while another is sampled).
  - Rebinding the same objects no longer marks the stage's binding set stale: no binding set per draw.
  - Back-end refusals are named: every back-end call goes through `Device::gpu`, which removes the
    device with the back end's reason ("the back end refused a call: ...") so `GetDeviceRemovedReason`
    answers, and out of memory is `E_OUTOFMEMORY`. Instance step rates other than one are refused by
    `CreateInputLayout` with `E_NOTIMPL` (the back end does not fetch them in the program yet).
  - `CheckFormatSupport`, `CheckMultisampleQualityLevels` and `CheckFeatureSupport` are served:
    the uses this front serves on its back end (no multisampling, so one quality level at one sample and
    none above), and zeros for absent features; a wrong structure size is `E_INVALIDARG`. A test creates
    a texture for every use each claimed format reports.
  - A use of a format the back end lacks (an `R8G8B8A8_SNORM` target on WebGPU) is now `E_NOTIMPL`;
    Direct3D's own refusals (depth binding of a colour format, a depth or block format as a target)
    stay `E_INVALIDARG`.
  - `D3D11CreateDevice` with an adapter and a named driver type is `E_INVALIDARG`, as Direct3D answers.
  - The library and the forwarding d3d11.dll export `D3D11CreateDeviceAndSwapChain` (served without a
    swap chain; with one, `E_NOTIMPL`) and `D3D11On12CreateDevice` (`E_NOTIMPL`), so programs importing
    them load.
  - Staging read backs start when the copy into the resource is recorded (plan 4.6); `Map` finishes
    them, waiting only if still pending (`ReadBackWait`), and with `D3D11_MAP_FLAG_DO_NOT_WAIT` answers
    `DXGI_ERROR_WAS_STILL_DRAWING` after submitting. A write-only map reads back (`StagingRefreshed`)
    only if the GPU wrote the subresource since its shadow was filled. A map of a staging resource no
    copy wrote reads nothing back.
  - The COM generator is in the repository (crates/wcontainers-directx/generate.py), reading the
    windows crate's own sources; it reproduces generated.rs exactly.
- Run 4 merged origin/phase-2-graphics (the pass tracker, occlusion counter and combination tracker
  in the interface; G1's recording back end crate), -webgl (its conflict is settled on its side
  now), -shaders and -rig, and checked everything again on the RTX 3080. The shaders track's
  explicit level-zero sampling makes the rig's dynamic scene match exactly; its exact sizes of
  dynamically indexed constant buffers meant the front test of a grown constant buffer now binds a
  buffer of one register (the instanced scene's own buffer is no longer smaller than declared).
- The three scene programs, unchanged, run with our library beside them and match real Direct3D 11
  on the RTX 3080 with a largest difference of 0 (the rig compares 8-bit pictures with an allowance of
  1 per channel and floating-point ones with 0, no pixel allowed beyond): triangle, quads and topology;
  so do the rig's instanced, dynamic and targets scenes. Their
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
- Run 3 fixed what the independent check of run 2 found:
  - The back end's losses reach the program. The front polls `Backend::status` in `Map`, `Flush`,
    `GetDeviceRemovedReason` and before every creation (plan 4.10): `Map` and the creations answer
    `DXGI_ERROR_DEVICE_REMOVED`, and the reason is `DXGI_ERROR_DEVICE_RESET`, `_DEVICE_REMOVED` or
    `_DRIVER_INTERNAL_ERROR` from the loss reason. A rig scene the back end cannot draw now stops with
    0x887A0005 instead of finishing with a wrong picture.
  - Slow paths are warned: each device and context method ends by printing the slow paths taken for
    the first time ("wcontainers direct3d 11: slow path taken: ...").
  - A copy, update or binding call the front cannot serve removes the device; an invalid one is
    dropped with a warning, as Direct3D drops it.
  - The context's `Get` methods are served from the objects it holds (with a library reference each
    since run 5); geometry, hull, domain and compute stages, stream output and
    predication take calls that bind nothing and report nothing bound.
  - Equal state descriptions give the same object (blend, depth-stencil, rasterizer, sampler); since
    run 5 the device keeps each with a library reference for its life.
  - A constant buffer smaller than its shader declares (dxbc-spirv declares a dynamically indexed one
    whole, 65536 bytes) grows into a buffer of that size whose bytes past its own are zero
    (`ConstantBufferGrown`, a new interface slow path). The rig's instanced scene now matches.
  - Draws blending with the second pixel output are refused (the converter writes no dual-source
    program yet); valid DXGI formats not served (typeless ones) are `E_NOTIMPL`, not `E_INVALIDARG`;
    a format the back end lacks altogether is `E_NOTIMPL`.
  - `CreateTexture2D` checks the description before reading the program's initial data array.
  - The adapter's description comes from the infrastructure crate (its LUID; memory sizes zero, its
    rule, since no browser API reports them).
  - The library tests copy the library from `deps` (where `cargo test` builds it) and the forwarder
    from the build script's output into their own directory, so they test this tree's build.
  - Unaligned buffer writes and copies are carried out by the merged WebGPU back end
    (`UnalignedBufferEdge`); `own_handled` checks them byte by byte.

What the front serves: device creation at 11_0 (10_0 without compute), buffers, 2D textures (arrays,
cubes, mips, immutable and staging), render target, depth-stencil (formats without stencil only, see
Known failures) and shader resource views of 2D textures, samplers, blend, depth-stencil and rasterizer states, input layouts, vertex and pixel
shaders converted from DXBC, constant buffers, draws (plain, indexed, instanced; every list and strip
topology; adjacency topologies without a geometry shader through `AdjacencyDropped`), clears, copies,
`UpdateSubresource`, maps of staging resources and of dynamic buffers (`MappedUploadWhole`),
`ClearState`, `Flush`, the context's `Get` methods, and the device's capability queries.

## Known failures

- A depth-stencil view of a format with stencil (D24_UNORM_S8_UINT, D32_FLOAT_S8X24_UINT) on the WebGPU
  back end: the back end makes a depth-only view in the combined format, which wgpu refuses ("this
  view format is not present in the texture's viewFormat array"), so the device is removed. The fix is
  in crates/wcontainers-gpu-webgpu (another track's): a combined-format view drawn into must hold both
  aspects, which the interface now says (`TextureViewDesc::stencil`), and a sampled one must be made in
  its aspect's format. Test `own_handled_draws_the_same_through_ours_with_depth_and_stencil`
  (tests/programs.rs) is marked ignored with this reason; run with `--include-ignored` it fails with
  0x887A0005. With D32_FLOAT the same program matches. The rig's depth and stencil scenes stop here.
  Still so in run 4 (origin/phase-2-graphics-webgpu had nothing new): the back end's `raw_view`
  takes the aspect from `stencil` for every view.

## What comes next (G4b and on)

0. Interfaces still answering `E_NOINTERFACE` (from the check of run 4, not done in run 5):
   `ID3D11Device1` and `ID3D11DeviceContext1` (Direct3D 11.1: whole interfaces to serve),
   `IDXGIDevice1`, `ID3D11Multithread` (its `Enter`/`Leave` would take the library lock), `IDXGISurface`
   and `IDXGIResource` on textures, and the adapter's `GetParent` (a factory) and outputs. The factory,
   outputs and swap chains should come from one DXGI shared with Direct3D 12: the Direct3D 12 track has
   them in its example library (crates/wcontainers-direct3d-12/examples/directx), which section 10
   puts in wcontainers-directx. Move them here and serve `D3D11CreateDeviceAndSwapChain`'s swap chain
   from them (item 4).
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
7. Move the context onto the interface's `PassTracker` (now on the branch) in place of its own
   pass opening and pending clears (context.rs `open_pass`, `add_clear`, `settle`). The tracker
   takes clears of bound targets only, so a clear of a view bound later must still become a pass
   load, not an extra `clear_texture`: keep that or add it to the tracker as an interface change.
   Then the `CombinationTracker` for building ahead (4.4).
8. Move the front's tests onto G1's recording back end (crates/wcontainers-gpu-test), which runs
   draws on the CPU and checks the interface's rules, and delete src/record.rs. Two things stand in
   the way: it declares no `COMPUTE`, so the front makes 10_0 devices on it, not 11_0; and a test
   cannot lose its device on purpose (the front test of a loss at the next wait sets `lost`).

## How to run

From the checkout root on the Mac (the desktop does all building and testing). The desktop's
checkout needs the shaders crate's submodules, so commands start with `git submodule update --init --recursive -q`.
- Front tests (recording back end, no GPU), 19 tests:
  `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo test -q -j 3 -p wcontainers-direct3d-11"`.
- Library tests (COM behaviour, the forwarder, threads, our programs; they make real devices, so
  a GPU job), 18 tests and one ignored known failure:
  `tools/graphics-rig/gpu-job --build "git submodule update --init --recursive -q && cargo test -q -j 3 -p wcontainers-directx --no-run" -- "cargo test -q -j 3 -p wcontainers-directx -- --test-threads 1"`
  (add `--nocapture` to see what the programs print, `--include-ignored` for the known failure).
  The random lifetimes comparison takes `WC_LIFETIMES=<first seed>,<seeds>,<steps>` (default 1,8,300);
  one seed alone: `set WC_LIFETIMES=17,1,300&& cargo test -q -j 3 -p wcontainers-directx --test
  programs object_lifetimes -- --nocapture`. A failure names the seed and the first line that differs.
  They work from a clean target directory: no `cargo build` is needed first.
- Lint: `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo clippy -q -j 3 -p wcontainers-directx -p wcontainers-direct3d-11 --all-targets -- -D warnings"`.
- The scenes through our library copied in as d3d11.dll:
  `tools/graphics-rig/scenes --build "git submodule update --init --recursive -q && cargo build -q -j 3 -p wcontainers-directx" --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' --api d3d11 triangle quads topology`.
- Through the forwarder: the same with
  `--dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\d3d11.dll' --dll 'wcontainers_directx.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'`.
  The pictures are checked as usual, but the rig also prints one "did not load the replacement
  wcontainers_directx.dll" FAIL per scene: its programs report only where d3d11.dll and dxgi.dll came
  from, so it cannot see the library the forwarder reaches. Read the picture lines.
- The rig's own forwarding library (the control): `--build "cargo build -q -j 3 -p wcontainers-graphics-test-forward"`
  and `--dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_graphics_test_forward.dll'`.
- Logs on the desktop in C:\wc\out\phase-2-graphics-direct3d-11\scenes\d3d11\<scene>.log and .stdout.
- src/com/generated.rs: `python3 crates/wcontainers-directx/generate.py <windows-0.62.2 crate dir>`
  (the directory cargo unpacked, e.g. ~/.cargo/registry/src/<index>/windows-0.62.2), then
  `cargo fmt -p wcontainers-directx`. A method named in its `hand` table calls the hand-written one.
- Scratch (pyro-parked/wcontainers-scratch/graphics-direct3d-11): pngdiff.py says which cells of two
  PNGs differ; logs/ holds the runs' outputs (run5-*.log for this run).

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
- The Windows library: `wcontainers_directx.dll` exports `D3D11CreateDevice`,
  `D3D11CreateDeviceAndSwapChain` and `D3D11On12CreateDevice`; the device, context and adapter are made
  with the `windows` crate's `implement`, the device children with our `counted!` (two counts, above);
  src/com/generated.rs is generated (above), each method
  taking the library lock (`com::lock`; on the device and context `com::Call`, which also prints the
  front's new slow-path warnings on leaving) and calling the hand-written method. The context keeps
  the program's bound objects (`Bound`, library references) for the `Get` methods; the device keeps
  its state objects by description. State the objects share
  (`Rc`s of front objects and of the device) sits in `Locked<T>`, which drops it under the lock.
- The back end is wgpu on Direct3D 12 (`com::backend`): what browsers' WebGPU uses on Windows, and it
  rasterises aliased lines by Direct3D's rule. On wgpu's default choice (Vulkan on the desktop) the
  topology scene differed on 43 pixels, all on line draws.
- build.rs makes d3d11.dll with the MSVC linker (found by the `cc` crate) from a generated `.def`:
  no code, each export forwarding to `wcontainers_directx`. It is written beside the library in the
  profile's output directory (target\debug).

## Tests

- crates/wcontainers-direct3d-11/tests/front.rs, 19 tests: the three scenes' calls on the recording
  back end (the scenes' own DXBC from the rig's corpus, converted), shader stages read from every
  corpus container, feature level choice, destruction after release, creation refusals (wrong stage,
  unconvertible shader, blend targets past the first, valid formats not served), dynamic constant
  upload, draws that remove the device (an empty slot, a vertex id with a start location, lines under
  the other line rules, dual-source blending), a back-end loss reported at the next wait, slow-path
  warnings handed out once, a constant buffer grown to its shader's size, staging read backs started
  at the copy with maps that may not wait (the recorder can hold read backs until a submit), no new
  binding set for objects bound again, and a view of a target unbound and refused.
- crates/wcontainers-directx/tests/com.rs, 14 tests: QueryInterface identity and parents, children
  holding one reference on their device, the immediate context, views holding their resource with a
  reference the program does not see (and handing it back after the program released it),
  private data, unserved shaders refused (bad bytes `E_INVALIDARG`, unconvertible `E_NOTIMPL`), the
  forwarder resolving `D3D11CreateDevice` to the library's own, 8 threads making buffers at once,
  equal state descriptions giving the live object, the context's getters (with reference counts and
  `ClearState`), unused stages taking unbinding calls, a target unbinding its shader views (by mip),
  device creation's answers (adapter with a driver type, the swap chain entry point), and capability
  queries claiming only what creation serves.
- crates/wcontainers-directx/tests/programs.rs, 4 tests and 1 ignored: our programs, system and
  ours; `lifetimes` (example), a seeded random sequence of makes, references, releases, bindings,
  getters, `GetResource`, `GetDevice` and `ClearState`, printing every count after each step, run beside
  the system's d3d11.dll and ours and compared line by line; `own_handled` with a D32_FLOAT depth-stencil view; the D24_UNORM_S8_UINT one (known failure).
  `own_handled` now also writes and copies a buffer at unaligned byte ranges and checks every byte.
- Last full run: 2026-11-17, run 5, on the desktop from this tree: front 19 passed; library 14 + 4
  passed (lifetimes at 8 seeds of 300 steps), and with `--include-ignored` the known failure fails as
  described (0x887A0005); lint clean. Before it: 2026-11-17, run 4, on the desktop from this tree after the merges: 16 + 11 + 3
  passed, and the known failure run with `--include-ignored` fails as described (0x887A0005); lint
  clean.

## Departures from the plan

- No call buffer or device thread yet: COM calls run the front on the calling thread under one
  re-entrant lock (section 4.8 puts the channel in the second batch).
- The recording back end is in the front crate (`record.rs`), not G1's wcontainers-gpu-test, which
  landed in run 4; moving over is item 8 of what comes next.
- The front does not use G1's pass tracker or combination tracker yet (they landed in run 4; item 7
  of what comes next); it opens a pass at the first draw after its targets change and ends it before
  copies, maps and clears.
- Buffers are padded to the back end's copy granule (4 bytes on WebGPU); the program sees its own
  `ByteWidth`.
- State objects are kept by the device for its life once made (DXVK does the same); Direct3D may free
  one whose references all went. The program sees the same answers (a count of one on the object it
  gets back); memory grows with the number of distinct descriptions (Direct3D caps it at 4096 each).
- `D3D11CreateDeviceAndSwapChain` with a swap chain asked for answers `E_NOTIMPL`: no swap chain yet.
- `CheckFeatureSupport` answers `D3D11_FEATURE_THREADING` with both FALSE (calls are serialised by
  the library's lock; deferred contexts are not served).
- The feature level is 11_0 at most; 11_1's requirements have not been checked.
- Stencil fields are validated only with stencil on, the depth function only with depth on.
- The adapter reports zero for its memory sizes (the infrastructure crate's rule; no browser API
  reports them). Programs that budget by `DedicatedVideoMemory` see none; whether to report a stated
  estimate instead is the owner's call.
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

- 7f73347 "interface: named the slow path of a constant buffer grown to its shader's declared size,
  and said a depth attachment holds both aspects": `SlowPath::ConstantBufferGrown`, and one sentence
  on `TextureViewDesc::stencil` (a combined-format view a pass draws into holds both aspects), with
  the plan's text in section 7 (Direct3D 11's constant buffers).

## Measurements

- 2026-11-17, run 5, after the merges and fixes, every Direct3D 11 rig scene through our library
  copied in as d3d11.dll (through the queue, not alone; desktop CPU load 44 % sampled after; Mac load
  average 53): the same 9 pictures match with largest difference 0 (triangle, quads, topology,
  instanced, dynamic, targets x4); the same 9 through our forwarder and through the rig's own
  forwarding library. The other scenes stop with an error code as before, except formats, which now
  passes `CheckFormatSupport` and stops with `E_NOTIMPL` at an `R8G8B8A8_SNORM` target.
- 2026-11-17, run 5, `lifetimes` beside the system's d3d11.dll and ours: seeds 1 to 8, 300 steps each
  (4,422 lines in all), identical; the device's last `Release` answers 0 both ways.

- 2026-11-17, run 4, after the merges, every Direct3D 11 rig scene through our library copied in as
  d3d11.dll (through the queue, not alone; desktop CPU load 23 % sampled after; Mac load average
  42): 9 pictures match exactly (largest difference 0): triangle, quads, topology, instanced,
  dynamic (new) and the four targets outputs. The other scenes stop with the same error codes as in
  run 3, except dynamic. The five matching scenes match the same through our forwarder and through
  the rig's own forwarding library (5 of 5).
- 2026-11-17, run 3, every Direct3D 11 rig scene through our library copied in as d3d11.dll (desktop
  CPU load 12 % sampled after; Mac load average 40): matching exactly (largest difference 0) are
  triangle, quads, topology, instanced (new: the grown constant buffer) and the four targets
  outputs. Every other scene now stops with an error code instead of a wrong picture: draws, raster,
  msaa, formats, textures, samplers, views, queries, mips, compute, geometry, tessellation,
  streamout, deferred `E_NOTIMPL`; constants and swapchain `E_NOINTERFACE` (Direct3D 11.1 and DXGI
  interfaces not offered); levels `DXGI_ERROR_UNSUPPORTED` (feature levels below 10_0); blend
  (dual-source refused), copies (typeless copies refused), depth and stencil (the known failure),
  dynamic (FXC) removed. The four matching scenes match the same through the forwarder, and through
  the rig's own forwarding library.

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

Added in run 5: `rand` 0.9.5 and `rand_chacha` 0.9.0 (MIT OR Apache-2.0, already in the lock file)
as dev-dependencies of the library, for the `lifetimes` program. Added in run 3: `naga` (the
workspace's, MIT OR Apache-2.0) to the front, for the declared sizes of constant buffers (still a direct
dependency); our `wcontainers-directx-graphics-infrastructure` to the library. Added in run 2:
`cc` 1.11.2 (MIT OR Apache-2.0, build dependency), `parking_lot` 0.12.5 (MIT OR
Apache-2.0), both already in the lock file; our `wcontainers-shaders`, which builds dxbc-spirv (MIT)
in. Earlier: `windows` and `windows-core` 0.62.2,
`wgpu` 30.0.2, `naga` 30.0.2 (MIT OR Apache-2.0), `pollster` 1.0.1 (Apache-2.0/MIT), `blake3` 1.8.7
(CC0-1.0 OR Apache-2.0). Nothing installed on the desktop beyond build output in
C:\wc\target\phase-2-graphics-direct3d-11 (including the tests' directx-tests\ copies) and pictures in C:\wc\out.
