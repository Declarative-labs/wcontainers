# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

G5a (first slice) is in progress, run 2.
- Done: `wcontainers-directx-graphics-infrastructure`, the portable core: the adapter description
  from the back end's `Caps`, output descriptions and their modes, DXGI format numbers to interface
  formats, swap chains (flip sequential, flip discard, blit discard, blit sequential), presenting by
  one copy into the host surface (sRGB and float back buffers encoded on the way), resizing, and frame
  pacing by the maximum frame latency. 7 tests on a recording back end. Pushed for the Direct3D 11
  track (see below).
- Done in `wcontainers-direct3d-12`: serialized root signatures (`root.rs`), the answers of
  `D3D12CreateDevice` and `CheckFeatureSupport` from `Caps` per plan section 9 (`features.rs`), the
  map from GPU virtual addresses and descriptor handles to their owners (`address.rs`), and now the
  device itself (`device.rs`, plain Rust, generic over the back end): committed resources (default
  heap buffers and textures; upload and readback buffers whose bytes live in front-owned memory
  handed out by `map`), `GetCopyableFootprints` (`resource.rs`), root signatures, graphics pipeline
  states (blocks from Direct3D 12's numbers in `state.rs`, the program, and `prepare` with streams
  packed as the input layout lays them out), command lists recorded front-side without the back end
  (`list.rs`; an unsupported call fails `Close`), `ExecuteCommandLists` replaying them (upload
  memory the lists read written first, counted `MappedUploadWhole`; whole-view clears become the
  next pass's `Clear` load or a `clear_texture`; a pass per target change; copies both ways and
  between textures; read backs into readback memory recorded after each copy into it), fences
  (`queue_signal` submits, `completed` and `wait` finish read backs whose work is done so the
  program sees the bytes with the value; `wait` answers false when only unsubmitted work could
  reach the value). A replay error removes the device with the reason.
- Shaders: no DXIL converter exists yet (origin/phase-2-graphics-shaders has none), so
  `stand_in.rs` holds hand-written WGSL for the rig's triangle shaders, chosen by the BLAKE3 hash of
  the exact DXIL bytes of the stored corpus; each use is counted (`stand_ins_used`) and warned
  (`take_warnings`); any other DXIL is refused at pipeline creation with `NotImplemented`.
- Drawn on the GPU: the rig's Direct3D 12 triangle scene, recorded call for call as gt-d3d12 makes
  it (tests/support/triangle.rs), through `Device<WebGpu>` on the RTX 3080, equals real Direct3D
  12's reference picture with a largest channel difference of 0 (rig tolerance 1), through wgpu's
  Vulkan and Direct3D 12 paths (2026-11-17, one gpu-job each, desktop CPU load 14% at the time).
- Not done: the Windows shells (`dxgi.dll`, `d3d12.dll`) and gt-d3d12.exe running unchanged
  through them; root parameters (constants, root descriptors, tables) and shader-visible
  descriptor heaps; depth-stencil targets; placed resources; queue `Wait`; bundles; queries;
  indirect execution; swap chains on a Direct3D 12 queue.

## Next, in this order

1. The Windows shells. Plan section 10 puts them in G4's crate `wcontainers-directx` (one library,
   forwarders for d3d11/d3d12/dxgi). Until it exists: build the library as a cdylib example of
   `wcontainers-direct3d-12` (examples may use dev-dependencies, so the core never depends on a
   back end), and `d3d12.dll` and `dxgi.dll` as pure forwarders (cdylib examples named `d3d12` and
   `dxgi` whose only content is a `#[used] #[link_section = ".drectve"]` string of
   `/EXPORT:Name=wcontainers_directx.Name` directives, so the linker makes forwarders with no code).
   gt-d3d12.exe imports from d3d12.dll exactly `D3D12CreateDevice`,
   `D3D12SerializeVersionedRootSignature` and `D3D12GetDebugInterface` (the last answers
   `E_NOINTERFACE`: there is no debug layer), and from dxgi.dll `CreateDXGIFactory2`; the scene
   uses IDXGIFactory4 (EnumAdapters1, EnumWarpAdapter), IDXGIAdapter1::GetDesc1, and on the device,
   queue, allocator, list, fence, resource, root signature, pipeline and descriptor heap only the
   methods in crates/wcontainers-graphics-test/src/d3d12/mod.rs and scenes.rs `triangle`. Generate
   the stubs of every other method from the `windows` crate's `*_Impl` traits with a script (in
   scratch), each answering `E_NOTIMPL`, or for a void method of a command list failing its `Close`.
   One device thread owns the back end (WebGpu handles are `Rc`); COM methods send it closures and
   wait. B = `WebGpu` on wgpu's Vulkan path: inside a process whose `d3d12.dll` is ours, wgpu's
   Direct3D 12 path would load our library as the system's.
2. Run the scene: `tools/graphics-rig/scenes --build "<build>" --dll d3d12.dll=... --dll
   dxgi.dll=... --dll wcontainers_directx.dll=... --api d3d12 triangle` against the stored reference.
3. The independent check: a small Direct3D 12 program of our own (an indexed draw with two vertex
   streams and a rectangle clear, say), CheckFeatureSupport answers compared with real Direct3D's on
   the desktop, and reading the code for anything faked.
4. Then the rig's other Direct3D 12 scenes need root parameters and shader-visible heaps (binding
   sets made at replay from the descriptors as they are then, plan 4.5 and 5.2).

## For the Direct3D 11 track: using the infrastructure crate

Depend on `wcontainers-directx-graphics-infrastructure` (path `../wcontainers-directx-graphics-infrastructure`).
- Adapter: `adapters(backend.caps())` gives the one `AdapterDesc` (`description_utf16()`,
  `vendor_id`, `device_id`, `luid` = `ADAPTER_LUID`, `software`). Memory sizes are zero: no browser
  reports them.
- Formats: `dxgi_format(DXGI_FORMAT value) -> Option<(Format, typeless)>`.
- Swap chain: `SwapChain::new(&mut backend, window, &SwapChainDesc { width, height, format (DXGI
  value), buffers, model: Model::from_dxgi(SwapEffect)?, usage })`. `Err(SwapChainError::InvalidCall)`
  is `DXGI_ERROR_INVALID_CALL`. Fill a width or height of zero from the window before calling.
- Back buffers: `chain.buffer(i)` is a `B::Texture` (2D, one level, one sample, `reinterpret` so sRGB
  views work); blit models give buffer 0 only. Draw into `chain.buffer(chain.current_index())`.
- `chain.present(&mut backend, sync_interval)` ends nothing of yours: end your open pass first. It
  records the copy, submits (your recorded work goes with it), shows the surface, rotates flip
  models, and waits while more than `max_latency()` frames are queued (`set_max_latency` for
  `SetMaximumFrameLatency`). `would_wait` answers `DXGI_PRESENT_DO_NOT_WAIT`.
- `chain.resize(&mut backend, buffers, width, height, format)` for `ResizeBuffers` (zeros keep the
  value); release your views of the old buffers first. `chain.destroy(&mut backend)` at the end.

## How to run

- Tests (one includes the GPU, so through the queue): `tools/graphics-rig/gpu-job --build "cargo
  test -j 3 -p wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 --no-run" --
  "cargo test -j 3 -p wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12"`
  (21 tests: 7 infrastructure, 6 front, 5 device on the recorder, 1 on the GPU, 2 unit).
  `WEBGPU_API=dx12|vulkan` picks wgpu's path for the GPU test.
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 -- -D warnings"`.
- On the Mac only `nix develop -c cargo fmt` (the Mac is overloaded).

## Interface changes

- `Caps::adapter: AdapterInfo` (name, PCI vendor and device numbers, software): DXGI's adapter
  description is answered from `caps()` (plan 5.1), and `Caps` had nothing to answer it with. Back
  ends fill it from the host's adapter information (WebGPU's `GPUAdapterInfo`, WebGL's unmasked
  renderer); zeros where the host gives no numbers.

## Measurements

- Triangle scene through `Device<WebGpu>` against real Direct3D 12's picture: largest channel
  difference 0, 0 pixels beyond 1 (RTX 3080; wgpu Vulkan and Direct3D 12; 2026-11-17; desktop CPU
  load 14%). The run took 0.47 s including device creation (not a timing; not run alone).

## Departures from the plan

- Hand-written WGSL stands in for the triangle scene's two DXIL shaders, chosen by the hash of
  their exact bytes, counted and warned, until the shaders crate converts DXIL. This is keyed on a
  shader's contents, which the rules forbid for anything lasting; it goes when G6's converter lands.
- `CreateGraphicsPipelineState` prepares the combination with tightly packed vertex streams:
  Direct3D 12's strides come with the vertex buffers, not the pipeline. A draw with other strides
  builds its combination at the first draw.
- The WebGPU back end does not override `prepare` yet, so every first draw counts `PipelineMiss`.
- Merging origin/phase-2-graphics-webgpu needed `Caps::adapter` filled in its `caps()` (this
  track's interface change, which G2 did not have): commit "filled the adapter description in the
  merged webgpu back end's caps". G2 should take the same lines when it merges this branch or the
  interface.
- The recording back end of G1 is not on any branch yet, so the infrastructure crate's tests use a
  small recorder of their own (tests/support/record.rs, test-only, never built into a library). It
  goes when G1's lands.
- Sync intervals 2 to 4 show each frame for at least one refresh, not exactly that many: the host
  surface has only "wait for the display" or not. A listed loss.
- DXGI's memory sizes in the adapter description are zero (no browser API reports memory).
- The checksum of a root signature container written by our `D3D12SerializeVersionedRootSignature`
  is zero: only our runtime reads it, and it does not check it. A blob saved by a program and later
  given to real Direct3D 12 would be refused there.
- Wave lane counts, where the back end has subgroups, are WebGPU's bounds (4 to 128), since `Limits`
  holds no subgroup sizes.

## Licences

- `naga` with `wgsl-in` and `blake3`: already confirmed in plan section 3.
- `slab` 0.4 (MIT, read in its Cargo.toml): the device's object tables.
- Test only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2 (MIT OR Apache-2.0), `pollster` (MIT OR
  Apache-2.0), `png` (MIT OR Apache-2.0), as the WebGPU track already uses them.

## Installed on the desktop

Nothing beyond the rig's setup.
