# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

G5a (first slice) is done (run 4 finished what run 3 had left out: swap chains, outputs and
presenting in the Windows library, and the independent check's findings). Next is G5b (Next, below).

- `wcontainers-directx-graphics-infrastructure`, the portable core: the adapter description from the
  back end's `Caps`; output descriptions, their modes, and the colour spaces swap chains present in
  (`presents_in`, `sdr`); DXGI format numbers to interface formats; swap chains (flip sequential,
  flip discard, blit discard, blit sequential) with presenting by one copy into the host surface (sRGB
  and float back buffers encoded on the way), resizing, and frame pacing by the maximum frame latency;
  what each DXGI format can be used for (`format_support`, `takes_samples`, `plane_count`,
  `dxgi_vertex_format`). 9 tests on a recorder whose GPU finishes work only when the test says so
  or a wait reaches it, so the pacing test sees `present` wait for exactly the frame past the
  latency.
- `wcontainers-direct3d-12`, the front (plain Rust, generic over the back end): serialized root
  signatures, the answers of `D3D12CreateDevice` and `CheckFeatureSupport` (plan section 9), GPU
  virtual addresses and descriptor handles, committed resources (default heap buffers and textures;
  upload and readback buffers in front-owned memory), `GetCopyableFootprints`, the size and alignment
  of resources (`allocation`), graphics pipeline states, command lists recorded front-side and
  replayed at `ExecuteCommandLists`, fences, and (run 4) swap chains with a resource standing for
  each back buffer (`create_swap_chain`, `chain_buffer`, `present`, `resize_swap_chain`).
- Shaders (run 4): DXIL goes through `wcontainers_shaders::convert` (origin/phase-2-graphics-shaders,
  merged); the hand-written stand-in and its hash table are gone. The vertex shader's reflected
  inputs give the locations; input elements the shader does not read fetch nothing. Shaders that read
  resources are refused with `NotImplemented` until root parameters bind them; a draw with a first
  vertex or instance whose vertex shader reads `SV_VertexID` or `SV_InstanceID` fails the replay
  (`draw_base: None` reads the bases as zero). The converter's message is kept as a warning.
- Fences (run 4): `SetEventOnCompletion` with an event registers it on the device and returns;
  `Queue::Signal` and `Fence::Signal` return at once. The device thread sets each event when the
  fence reaches its value, looking without waiting (`reached_events`) every 250 µs on a
  high-resolution timer while events wait, and at once after every call. Nothing blocks the program
  or the device thread for it. With no event the call waits, as the API defines.
  `GetCompletedValue` finishes read backs whose submission is done first, so readback memory holds
  the bytes whenever the program sees the value.
- The Windows library (`examples/directx`, built as `d3d12.dll`, with `dxgi.dll` a forwarder to it):
  factory, adapter, outputs (the host's monitors from GDI, primary first, as `IDXGIOutput6`), swap
  chains on a Direct3D 12 queue (`IDXGISwapChain4`: `CreateSwapChainForHwnd`, `CreateSwapChain`,
  `Present`, `Present1`, `GetBuffer`, `ResizeBuffers`, frame latency, colour spaces), and the device
  objects of run 3. The surface is made from the program's window on the device thread. Two things
  the real driver does needed handling: while a swap chain call waits for the device thread, the
  calling thread answers messages other threads send to its windows (the driver asks the window's
  thread while it makes the swap chain); and the NVIDIA driver loads DXGI by name on the device
  thread to present, which in this process finds our library, so `CreateDXGIFactory*` and
  `D3D12CreateDevice` called on the device thread go to the system's own libraries (loaded from the
  system directory). Interface methods not done are generated stubs (`com.py`, which now holds its
  object list and refuses to generate a stub for a method that returns a plain value, so every such
  method is answered for real).
- Drawn on the RTX 3080 through our libraries, unchanged rig programs (2026-11-17, run 4, desktop
  CPU load 13%): the triangle scene "0 of 65536 pixels beyond 1, largest difference 0", and the
  swapchain scene, four frames each "0 of 12288 (5120 after the resize) pixels beyond 1, largest
  difference 0", its results text the same as real Direct3D 12's (three presents succeed, the resize
  reports 64 by 80 with 3 buffers). Every other Direct3D 12 scene stops at its first call the front
  does not do (roots, bundles and blend at shaders reading resources; heaps at CreateHeap; compute
  and queues at CreateComputePipelineState; queries at CreateQueryHeap; copies, depth, stencil,
  formats and samplers at a command list method not done).
- The independent check (`examples/check_d3d12.rs`, run with real Direct3D 12 and through our
  libraries, How to run): draw "0 of 4096 pixels beyond 1, largest difference 0";
  features "493 of 872 answers differ, none beyond real Direct3D 12" (each difference read against
  section 9 in run 3: the intended absences, or abilities the WebGPU back end lacks natively; wave
  operations are absent because the WebGPU back end never declares `SUBGROUPS`, on any machine);
  outputs "22 of 110 answers differ, none beyond real": 21 colour spaces real Windows presents and
  ours does not (studio-range spaces 2, 3, 17, 20, 21 and HDR10 spaces 12 and 14, on 8- and 10-bit
  buffers), and the primaries (ours are sRGB's exactly; real are this monitor's, within 0.002).
  Read of the code for anything faked (run 4): barriers, `Unmap` and allocator `Reset` do nothing
  because nothing is needed (plan 5.2, 4.6); `SetGraphicsRootSignature` records nothing, exact for
  root signatures without parameters; `Present` with `DXGI_PRESENT_TEST` answers S_OK (the host
  says nothing of occlusion, so a present would always go ahead); `SetColorSpace1` accepts only the
  one space each format presents in. No method answers success without the work.
- Not done (later milestones): root parameters and shader-visible descriptor heaps; depth-stencil
  targets; placed resources; queue `Wait`; bundles; queries; compute; indirect execution;
  `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT` (refused with E_NOTIMPL at creation, so
  `GetFrameLatencyWaitableObject` answers null as DXGI does without the flag); scaling other than
  stretch; fullscreen (`SetFullscreenState(TRUE)` answers `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`).

## Next, in this order

1. Root parameters and shader-visible descriptor heaps (binding sets made at replay from the
   descriptors as they are then, plan 4.5 and 5.2), with `VariantKey::layout` from the shaders
   track for DXIL's binding numbers; then depth-stencil targets: the roots, heaps, blend, depth and
   stencil scenes.
2. The frame latency waitable object (a semaphore the device thread releases as frames finish, as
   fence events are set now).
3. Recording without allocating per command: recorded commands still own `Vec`s (vertex buffer
   views, viewports, scissors, targets, clear rectangles) and each draw looks its vertex layout up in
   a `HashMap`; a per-list arena and a last-layout cache would remove both.
4. The call buffer of plan 4.8 in the Windows library (below, Departures).
5. Move `examples/directx` into `crates/wcontainers-directx` (on the shared line now, the Direct3D 11
   track's) once the two tracks agree how its device thread serves both fronts.

## For the Direct3D 11 track: using the infrastructure crate

Depend on `wcontainers-directx-graphics-infrastructure` (path `../wcontainers-directx-graphics-infrastructure`).
A working example of all of it is this track's Windows library: `examples/directx/present.rs`
(outputs, swap chains, presenting) and `examples/directx/gpu.rs` (the window's surface).
- Adapter: `adapters(backend.caps())` gives the one `AdapterDesc` (`description_utf16()`,
  `vendor_id`, `device_id`, `luid` = `ADAPTER_LUID`, `software`). Memory sizes are zero: no browser
  reports them.
- Outputs: `OutputDesc` (name, desktop rectangle, refresh rate) comes from the host; natively,
  `present.rs`'s `outputs()` makes them from GDI. `modes(format)` and `closest_mode` answer the mode
  calls; `GetDesc1` takes `sdr::*` and `COLOR_SPACE_SRGB`.
- Colour spaces: `presents_in(DXGI_FORMAT, DXGI_COLOR_SPACE_TYPE)` is `CheckColorSpaceSupport`'s
  present bit and the check of `SetColorSpace1`.
- Formats: `dxgi_format(DXGI_FORMAT value) -> Option<(Format, typeless)>`.
- Swap chain: `SwapChain::new(&mut backend, window, &SwapChainDesc { width, height, format (DXGI
  value), buffers, model: Model::from_dxgi(SwapEffect)?, usage })`. `Err(SwapChainError::InvalidCall)`
  is `DXGI_ERROR_INVALID_CALL`. Fill a width or height of zero from the window's client area first.
  Natively, `window` is a number given to `WebGpu::add_window` with a `wgpu::Surface` made from the
  HWND (`gpu.rs`, `State::add_window`: the Win32 handle needs `hinstance`, falling back to the
  program's module, and a Windows display handle).
- Back buffers: `chain.buffer(i)` is a `B::Texture` (`chain.buffer_desc()` describes it); blit models
  give buffer 0 only. Draw into `chain.buffer(chain.current_index())`.
- `chain.present(&mut backend, sync_interval)`: end your open pass first. It records the copy,
  submits (your recorded work goes with it), shows the surface, rotates flip models, and waits while
  more than `max_latency()` frames are queued (`set_max_latency` for `SetMaximumFrameLatency`).
  `would_wait` answers `DXGI_PRESENT_DO_NOT_WAIT`.
- `chain.resize(&mut backend, buffers, width, height, format)` for `ResizeBuffers` (zeros keep the
  value); release your views of the old buffers first, and refuse while the program holds one.
  `chain.destroy(&mut backend)` at the end.
- On Windows, two things the real driver does (see Where things stand): answer sent messages while a
  swap chain call waits (`gpu::call_windowed`), and send the device thread's own `CreateDXGIFactory*`
  to the system's dxgi.dll. Without them making a swap chain hangs.
- Format queries: `format_support(caps, DXGI_FORMAT)` gives `support1` and `support2`; empty means
  the API's E_FAIL. `takes_samples(caps, format, count)`; `dxgi_vertex_format(DXGI_FORMAT)`.

## How to run

- Tests (one draws on the GPU): `tools/graphics-rig/gpu-job --build "cargo test -j 3 -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 --no-run" -- "cargo test
  -j 3 --no-fail-fast -p wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12"`
  (25 tests: 9 infrastructure, 6 front, 7 device on the recorder, 1 on the GPU, 2 unit).
  `WEBGPU_API=dx12|vulkan` picks wgpu's path for the GPU test.
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 -- -D warnings"`.
- The desktop needs the shaders crate's submodules once (the rig's `remote` sends none):
  `tools/graphics-rig/remote "git submodule update --init crates/wcontainers-shaders/dxbc-spirv
  crates/wcontainers-shaders/dxil-spirv && git -C crates/wcontainers-shaders/dxil-spirv submodule
  update --init third_party/spirv-headers"` (done in run 4; they stay checked out).
- Scenes: `E='%CARGO_TARGET_DIR%\debug\examples'; tools/graphics-rig/scenes --build "cargo build -j 3
  -p wcontainers-direct3d-12 --examples" --dll "d3d12.dll=$E\d3d12.dll" --dll "dxgi.dll=$E\dxgi.dll"
  --api d3d12 triangle swapchain` (drop the names for every scene).
- Own program: `tools/graphics-rig/gpu-job --desktop --build "cargo build -j 3 -p
  wcontainers-direct3d-12 --examples" -- "crates\wcontainers-direct3d-12\examples\check_d3d12.cmd"`.
  It runs `draw`, `features`, `outputs` and `fence-wait 2000` beside real Direct3D 12 and beside our
  libraries (in C:\wc\out\phase-2-graphics-direct3d-12\check), then compares. `--desktop`
  matters: in session 0 real DXGI refuses outputs and swap chains
  (`DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`).
- `com.rs`: `python3 examples/directx/com.py > examples/directx/com.rs`, then `cargo fmt`.
- On the Mac only `nix develop -c cargo fmt` (the Mac is overloaded).

## Interface changes

- `Caps::adapter: AdapterInfo` (run 1): DXGI's adapter description is answered from `caps()`.

## Measurements

- Run 4 (2026-11-17, RTX 3080, desktop CPU load 13%, shared, not `--alone`): fence events, 2000
  rounds of an empty queue's `Signal`, `SetEventOnCompletion` and `WaitForSingleObject`: real
  Direct3D 12 mean 1 µs, 99th percentile 4 µs, largest 39 µs; ours mean 33 µs, 99th percentile
  58 µs, the first round 30.5 ms (the device's first submission). A second run: real mean 1 µs,
  99th percentile 2 µs; ours mean 37 µs, 99th percentile 39 µs, first round 35.8 ms. Before the event and timer (a
  250 µs `recv_timeout`), ours was mean 23 µs, largest 26 ms (also the first round), 99th percentile
  not taken.
- Run 4, not timings: the triangle and swapchain scenes and the own program equal to real Direct3D 12
  (largest difference 0); features 493 of 872 differ, none beyond; outputs 22 of 110 differ, none
  beyond.
- Run 3: triangle scene through `Device<WebGpu>` on wgpu's Vulkan and Direct3D 12 paths, largest
  difference 0 (desktop CPU load 14%).
- Line lists (measured by the independent check, its own program in scratch
  `check/own`, mode `lines`): 8 aliased `LINELIST` segments on 64x64, real against ours "59 of 4096
  pixels beyond 1, largest difference 255". A listed loss, below.

## Departures from the plan

- The Windows library is built as cdylib examples of `wcontainers-direct3d-12` (examples may use
  dev-dependencies, so the front never depends on a back end), not in `wcontainers-directx`, which
  the Direct3D 11 track now has on the shared line (Next 5).
- Calls do not go through the call buffer of plan 4.8, although section 10 has the first batch build
  the Windows shell with the channel in-process. Every COM call, `ExecuteCommandLists` with its whole
  replay, every `Create*` and every `Release` included, crosses to the device thread as a boxed
  closure and waits for the answer, so object creation waits for the device thread, against 4.8.
  Command list recording runs on the program's thread. A cost of run speed, not of correctness; Next 4.
- The library draws on wgpu's Vulkan path: with our d3d12.dll loaded, wgpu's Direct3D 12 path would
  load it as the system's. A listed loss follows: Vulkan rasterises aliased lines differently from
  Direct3D 12 on this GPU (Measurements); the Direct3D 11 track moved to wgpu's Direct3D 12 path for
  this, which this library cannot. Points, strips and triangles were equal in the same check.
- `CheckColorSpaceSupport` answers as Windows does on this SDR desktop for the spaces we present
  (sRGB from 8- and 10-bit buffers, linear light only from 16-bit float buffers: real Windows refuses
  sRGB for those, which plan 4.9 had as present), and leaves out the studio-range and HDR10 spaces
  Windows also presents, since we convert none of them. `GetDesc1`'s luminance is Windows' answer
  for an SDR display here (0.5, 270, 270 cd/m²); the primaries are sRGB's.
- A 16-bit float swap chain makes the WebGPU back end configure an `rgba16float` surface, which its
  `Caps` lists as presentable, but wgpu's Vulkan surface on this desktop lacks it (it offers
  bgra8unorm, rgba8unorm, their sRGB forms and rgb10a2unorm): wgpu reports a validation error and
  `configure_surface` returns no error. For G2: `configure_surface` should fail for a format the
  surface lacks, and natively `Caps` should list the surface's formats; the swap chain would then
  fall back to `rgba8unorm`.
- Sync intervals 2 to 4 show each frame for at least one refresh, not exactly that many: the host
  surface has only "wait for the display" or not. A listed loss.
- DXGI's memory sizes in the adapter description are zero (no browser API reports memory).
- `GetResourceAllocationInfo` answers this front's layout (copyable footprints, 64 KiB pages, 4 MiB
  for multisampled textures); `GetCustomHeapProperties` answers a discrete adapter's heaps, as
  `ARCHITECTURE` does.
- Format support reports only what the back end does natively. The plan's routes for the rest
  (typed buffer views through storage, stream output, `NormAsInteger`, `FormatConverted`,
  `BlockDecoded`, `FilterInShader`, `ReinterpretCopy`) are not in the front yet, so on WebGPU those
  abilities are absent: `BUFFER`, `SO_BUFFER`, `CAST_WITHIN_BIT_LAYOUT` (without
  `FORMAT_REINTERPRET`), 3D block-compressed textures, filtering of depth views, and formats WebGPU
  lacks (16-bit normalised, `B5G6R5`, `B5G5R5A1`, `A8`, the packed pairs, the video formats).
- Feature level 11_1 is never reported (run 4): Direct3D 12's table asks logic operations and
  target-independent rasterisation of it (and binding tier 1, not 3), and no `Caps` capability
  promises the latter. The plan's sentence in section 9 is corrected to say so.
- `ARCHITECTURE` answers no UMA, no cache-coherent UMA, no tile-based renderer and no isolated MMU.
- check_d3d12 does not ask shader model 6.7 or root signature 1.2: this desktop's Direct3D 12
  runtime refuses both whatever the GPU (ours answers 6.0 and 1.1, tested in tests/front.rs).
- The checksum of a root signature container written by our `D3D12SerializeVersionedRootSignature`
  is zero: only our runtime reads it.
- Wave lane counts, where a back end has subgroups, are WebGPU's bounds (4 to 128).
- The infrastructure crate's tests keep their own recorder (tests/support/record.rs) rather than G1's
  (crates/wcontainers-gpu-test): G1's finishes every fence at once, so frame pacing could not be
  tested on it.

## Licences

- `slab` 0.4 (MIT, read in its Cargo.toml): the device's object tables.
- `wcontainers-shaders` (ours) and through it dxil-spirv (MIT) and dxbc-spirv (MIT), as plan
  section 3 and the shaders track record. The front no longer uses `naga` or `blake3` directly.
- Windows library only (dev-dependencies): `windows` and `windows-core` 0.62.2 (MIT OR
  Apache-2.0, as plan section 10 confirms).
- Test only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2 (MIT OR Apache-2.0), `pollster` (MIT OR
  Apache-2.0), `png` (MIT OR Apache-2.0), as the WebGPU track already uses them.

## Installed on the desktop

Nothing beyond the rig's setup. The shaders crate's submodules are checked out in this branch's
source directory there (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders: dxbc-spirv
and dxil-spirv, 18,052,177 bytes).
