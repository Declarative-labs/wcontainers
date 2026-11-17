# Graphics: the WebGPU back end (G2)

Branch phase-2-graphics-webgpu. The plan is docs/phase-2-graphics.md (sections 4, 5.7 and 12, G2).
Crate: crates/wcontainers-gpu-webgpu, on wgpu 30.0.2.

## Where things stand

G2a (the first slice) is done, run 1, 2026-11-17:
- The back end implements the whole `Backend` trait. Blocks are made once per value; a draw's
  combination (program, four blocks, pass target formats, topology and strip index format, the
  layout key of each bound set) names a cached render pipeline, built at the first draw (counted
  `PipelineMiss`), and `setPipeline` is called only when the pipeline changes. Bind group layouts
  are derived from the program's naga modules and the sets' layout keys (4.5), shared by content;
  uniform bindings take dynamic offsets up to the device's limit; binding sets are shared by
  content and each makes one bind group per layout, made again only when a buffer it names was
  renamed (generation). Compute dispatches use the same layouts.
- Writes follow 4.6: straight to the queue when no unsubmitted command uses the resource;
  otherwise `Discard` renames the buffer (old storage pooled until its work is done), `NoOverwrite`
  writes in place, `Ordered` goes through a staging buffer and a copy, splitting an open pass
  (`WriteSplitPass`). Read backs copy into a mapped buffer, mapped after the submit; rows arrive
  packed. Occlusion queries, `resolve_queries` by `resolveQuerySet` and a copy, resolves, surfaces
  (windows registered by the host with `WebGpu::add_window`) are in.
- The rig's three Direct3D 11 reference scenes (triangle; textured, depth-tested, blended quads;
  instanced draw with a constant buffer), drawn through the interface with WGSL programs, equal real
  Direct3D 11's pictures in every pixel on the RTX 3080: natively through wgpu on Direct3D 12 and
  on Vulkan, and built for WebAssembly in Chrome 157's WebGPU. Tolerance 0 (see Measurements).
- The quads with the colour source factor changed to one, or the depth test changed to greater,
  fail the comparison both natively and in Chrome.
- Writes keep command order exactly (test `writes_take_effect_in_command_order`: copies between an
  ordered write that stages, a second one, and a renaming discard each read the bytes written just
  before them), and an ordered write of a vertex buffer the open pass drew with splits the pass
  without changing the picture (`ordered_write_in_a_pass_splits_it`, equal to Direct3D's
  triangle, one `WriteSplitPass`).
- Reviewed for places doing less than the interface promises: extra viewports or scissors and
  differing per-face stencil references now fail loudly instead of being ignored, textures take
  only the copy usages their format allows (`depth24plus` has none), and anisotropy applies only to
  fully linear samplers. What is still missing is listed below.
- Not yet run: the shared back-end suite. origin/phase-2-graphics had no new commit since
  6efefe6 when this run ended (G1 has not published it).

## How another track builds and uses the back end

- Depend on it: `wcontainers-gpu-webgpu = { path = "../wcontainers-gpu-webgpu" }` (and `wgpu =
  "30.0.2"` to make the device). It builds natively (Windows: Direct3D 12 and Vulkan; Mac: Metal)
  and for `wasm32-unknown-unknown` (the browser's WebGPU).
- Make it from a wgpu adapter: ask for the device with `WebGpu::device_descriptor(&adapter)` (the
  features the back end uses where present, and the adapter's highest limits), then
  `WebGpu::new(&adapter, device, queue)`. `WebGpu` is the `B` of `Device<B: Backend>`. The whole
  pattern, with an adapter that must be a GPU, is `device()` in
  crates/wcontainers-gpu-webgpu/examples/scenes/draw.rs, which also shows a draw end to end
  (shaders from WGSL through naga, blocks, a binding set, a pass, a draw, a read back).
- Binding numbers follow 6.4: uniform buffers from 0, samplers from 16, textures from 32, storage
  from 160; group = binding set (vertex-only set 0, fragment-only set 1, both 2, helpers 3).
- Natively `wait_read` and `wait_fence` block on the device. In the browser they cannot block
  until the JSPI suspension of 4.8 exists; they panic with that reason. Poll `read_state` and
  yield to the event loop instead (as `draw_scene` in examples/scenes/main.rs does).
- `WebGpu::counters()` says how many pipelines, bind groups, layouts, renames and staged writes
  were made; `slow_paths()` the slow paths taken.
- Anything the back end does not carry out yet (list below) marks the device lost with
  `LossReason::Error` and prints `webgpu: <reason>` on standard error, so `status()` shows it.

## How to run

All from the checkout root on the Mac; nothing is built or run on the Mac but formatting and
`cargo check -j 2`.
- Tests on the RTX 3080 (5 tests in tests/scenes.rs and tests/writes.rs, need a GPU; `WEBGPU_API=dx12|vulkan|metal` picks wgpu's API,
  default the platform's, Vulkan on the desktop):
  `tools/graphics-rig/gpu-job --build "cargo test -j 3 -p wcontainers-gpu-webgpu --no-run" --
  "cargo test -j 3 -p wcontainers-gpu-webgpu -- --test-threads 1 --nocapture"`.
- Lint: `tools/graphics-rig/remote cargo clippy -j 3 -p wcontainers-gpu-webgpu --all-targets --
  -D warnings`; and `nix develop -c cargo check -j 2 -p wcontainers-gpu-webgpu --target
  wasm32-unknown-unknown --examples` for the WebAssembly build.
- Both ways against Direct3D, one command: `crates/wcontainers-gpu-webgpu/web/check [dir]`. One
  GPU job builds the scenes example natively and for WebAssembly, gt-compare and the rig's page
  driver packages, then (web/check.cmd) runs wasm-bindgen, draws the three scenes with wgpu on
  Direct3D 12 and on Vulkan, draws them in Chrome (web/draw.mjs, web/scenes.html), compares all
  nine with references/d3d11 at tolerance 0, and draws the quads with each twist both ways and
  requires them to differ. Prints Chrome's WebGPU adapter and fails unless it is a GPU, not a
  fallback. With a directory, the pictures are copied there. About 7 minutes cold, the job itself
  5 s.
- One renderer by hand, on the desktop: `scenes <dx12|vulkan|metal> <dir> [none|blend-factor|
  depth-function]` (the example's binary).

## Measurements

All on the desktop (Windows 10, i9-9900K, RTX 3080, driver 32.0.16.2101 / NVIDIA 621.01); desktop
CPU load 0 % when sampled after the runs; Mac load average 27 to 39 (other tracks).
- Every pixel equal to Direct3D 11's reference, all three scenes: wgpu on Vulkan (adapter "NVIDIA
  GeForce RTX 3080 (Vulkan, DiscreteGpu, NVIDIA 621.01)"), wgpu on Direct3D 12, and Chrome
  157.0.8103.78 WebGPU (GPUAdapterInfo: "NVIDIA GeForce RTX 3080", vendor nvidia, backend D3D12,
  type "discrete GPU", not a fallback). So the tolerance stated is 0: on the same GPU any allowance
  would hide a real difference. On another GPU expect the rig's WARP allowance of 1 (the rig
  measured wgpu on Metal differing by 1 on the triangle); not run here.
- The twists, natively and in Chrome alike: blend source factor one, 16384 pixels differ, by up to
  92; depth test greater, 25016 pixels differ, by up to 204.
- Caching (test `pipelines_and_bind_groups_are_cached`, Vulkan and Direct3D 12): 21 draws of two
  programs with equal blocks over two passes made 2 pipelines, 3 `setPipeline` calls, 2 bind groups
  (one per set), 1 bind group layout, 2 `PipelineMiss`.
- Last full run: 5 tests pass on Vulkan (scenes 0.95 s, writes 0.76 s) and on Direct3D 12 (1.10 s,
  0.69 s); `web/check` passes (job 4 s); clippy with warnings denied passes. First build of the
  crate's tests 2 min 44 s at `-j 3`.

## Not carried out yet (each marks the device lost with its reason, or is declared absent)

- Unaligned buffer writes, copies and clears (`UnalignedBufferEdge`).
- Buffer-texture copies at row pitches that are not multiples of 256.
- Copies into depth formats (`DepthWrittenByDraw`).
- Immediates: `immediate_bytes` is 0, so fronts spill them.
- A program without a fragment shader drawing into colour targets.
- An ordered write that splits a pass with a running occlusion query.
- Formats WebGPU lacks are declared without bits (no `FormatConverted`, `BlockDecoded`,
  packed-pair conversion yet), though 5.7 has the back end convert them.
- Declared absent until built or measured: `TIMESTAMPS`, `BACKGROUND_PREPARE` (`prepare` keeps the
  default), `FRACTIONAL_VIEWPORT` (needs the half-pixel test), `DEPTH24_UNORM` (needs the probe).
- Waiting in the browser (JSPI, 4.8).
- Not yet as 4.5 says for speed: the uniform ring for `MapIntent::Often` uniform buffers (so a
  per-draw `Discard` of a constant buffer renames it and makes a bind group per draw), sharing of
  bind groups between sets that differ only in dynamic offsets, a staging ring (each staged write
  makes a buffer), renaming textures on `Discard` (staged instead), and freeing cached layouts and
  pipelines of destroyed programs.

## Next (G2)

In order: run the shared suite once G1 publishes it; the uniform ring and the counter test of no
bind group per draw; `prepare` with createRenderPipelineAsync and its measurement; the rest of the
list above; then the remaining G2 pass bar of section 12 (layouts with depth and r32float views,
BC both ways, typeless copies, tier1 formats, generate_mips, fractional viewport, copy within one
buffer test, unaligned resolve_queries, the 24-bit depth probe, a fitted multisampled pass, helper
blocks, naga validation with WebGPU's core capabilities).

## Departures from the plan

- Every attachment is stored at the end of a pass, also when the interface says `Discard`: an
  ordered write may split the pass, and its first part must keep what it drew. Correct, but it
  gives up the bandwidth saving of a discard on tiled GPUs; recording a pass's draws before opening
  it would allow both.
- `DontCare` loads clear to zero (WebGPU has no don't-care load; a clear is its cheapest load).
- Depth writes happen only with the depth test on, as in Direct3D, OpenGL and Vulkan.
- Blend factors of min and max operations are set to one (WebGPU requires it; every API ignores
  them there), and blending is left off on formats without `BLEND` (integer targets), which every
  API ignores too.
- Anisotropy is set to one unless all three filters are linear, as WebGPU requires and Direct3D
  does.

## Interface changes

None.

## Licences

Confirmed in each crate's Cargo.toml on 2026-11-17: wgpu 30.0.2 and wgpu-naga-bridge 30.0.2 MIT OR
Apache-2.0 (dependencies). Test-only: png 0.18.1, anyhow 1.0.104, wasm-bindgen 0.2.130,
wasm-bindgen-futures 0.4.80, js-sys 0.3.107 MIT OR Apache-2.0; pollster 1.0.1 Apache-2.0/MIT;
bytemuck 1.25.2 Zlib OR Apache-2.0 OR MIT. Tool: wasm-bindgen-cli 0.2.130 MIT OR Apache-2.0.

## Installed on the desktop

- wasm-bindgen-cli 0.2.130 (`cargo install --locked`): wasm-bindgen.exe, wasm2es6js.exe and
  wasm-bindgen-test-runner.exe in C:\wc\cargo\bin, 18.9 MB; its 0.53 GB build directory was
  removed.
- The rig's page-driver packages in this branch's checkout
  (C:\wc\src\phase-2-graphics-webgpu\tools\graphics-rig\browser\node_modules), by the rig's
  install.ps1.
- Build output C:\wc\target\phase-2-graphics-webgpu, 3.28 GB; pictures in
  C:\wc\out\phase-2-graphics-webgpu-webgpu.
