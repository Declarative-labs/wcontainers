# Direct3D 12 and DXGI infrastructure status (G5)

Branch: phase-2-graphics-direct3d-12. Plan: docs/phase-2-graphics.md, sections 4.9, 5.2, 9, 10 and
G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure and
crates/wcontainers-direct3d-12.

## Where things stand

G5a (first slice) is in progress, run 1.
- Done: `wcontainers-directx-graphics-infrastructure`, the portable core: the adapter description
  from the back end's `Caps`, output descriptions and their modes, DXGI format numbers to interface
  formats, swap chains (flip sequential, flip discard, blit discard, blit sequential), presenting by
  one copy into the host surface (sRGB and float back buffers encoded on the way), resizing, and frame
  pacing by the maximum frame latency. 7 tests on a recording back end. Pushed for the Direct3D 11
  track (see below).
- Done in `wcontainers-direct3d-12`: serialized root signatures (`root.rs`: a DXBC container with
  one `RTS0` part, versions 1.0 and 1.1, read and written), the answers of `D3D12CreateDevice` and
  `CheckFeatureSupport` from `Caps` per plan section 9 (`features.rs`: 11_0 with compute and the
  11_0 sizes, otherwise no device; 11_1 and binding tier 3 only with `RESOURCE_TABLES`; heap tier 2;
  every absent feature zero or FALSE; shader model 6.0; one quality level per sample count
  `FormatCaps` allows), and the map from GPU virtual addresses and descriptor handles to their
  owners (`address.rs`). 8 tests.
- Not done: the Direct3D 12 device itself, the Windows shells (`dxgi.dll`, `d3d12.dll`), and the
  rig's scene through them. Nothing has drawn yet: no back end is on any branch (checked
  2026-11-17: only phase-2-graphics, -rig, -shaders and this branch exist; G1's recording back end
  and G2's WebGPU back end are not pushed).

## Next, in this order

1. `Device<B>` in `wcontainers-direct3d-12` (plain Rust, generic over the back end), enough for
   the rig's `triangle` scene (crates/wcontainers-graphics-test/src/d3d12/scenes.rs and mod.rs):
   command queue, allocator, graphics command list recording into a front-side list of commands,
   root signature from `root::deserialize`, graphics pipeline state (blocks plus `prepare`),
   RTV descriptor heap (descriptors are front-side structs at `address.rs` handles), committed
   resources (default heap textures with a clear value; upload and readback heap buffers whose
   bytes live in front-owned memory handed out by `Map`), `ResourceBarrier` (nothing on either back
   end, 5.2), `ClearRenderTargetView` (a pass `Clear` load, or `clear_rects` for rectangles),
   `OMSetRenderTargets`, viewports, scissors, `IASetVertexBuffers` through the address map,
   `DrawInstanced`, `CopyTextureRegion` both ways with `GetCopyableFootprints` (rows of 256-byte
   pitch), `ExecuteCommandLists` replaying into the back end (upload bytes written first,
   `MappedUploadWhole`), `Signal`, `SetEventOnCompletion` and `GetCompletedValue` with read backs
   finished into readback memory when their fence completes (4.6). Test it on the recorder by
   replaying the triangle scene's calls and checking the recorded stream.
2. Shaders: DXIL needs G6's `convert` (origin/phase-2-graphics-shaders exists now; merge it and
   look). Until then, the triangle scene's two shaders are replaced by hand-written WGSL chosen by
   the exact DXIL bytes of the stored corpus (crates/wcontainers-graphics-test/shaders/dxil),
   counted and warned, and any other DXIL refused at pipeline creation with `E_INVALIDARG`.
3. The Windows shells. The rig's gt-d3d12.exe imports from d3d12.dll exactly
   `D3D12CreateDevice`, `D3D12SerializeVersionedRootSignature` and `D3D12GetDebugInterface` (the
   last answers `E_NOINTERFACE`: there is no debug layer), and from dxgi.dll `CreateDXGIFactory2`
   (read with llvm-objdump from C:\wc\out\phase-2-graphics-rig\scenes\bin\gt-d3d12.exe). Plan
   section 10 puts the one library in G4's crate `wcontainers-directx` with `.def` forwarders;
   until it exists, build the COM objects (the `windows` crate's `implement`, MIT OR Apache-2.0,
   already in the lock file through the rig) as a `windows` module of each of the two crates
   here, and a small cdylib inside crates/wcontainers-direct3d-12 that chooses `B`, so G4's
   library can take the modules over. One device thread owns the back end; COM methods send it
   closures and wait (the call buffer of 4.8 comes in the second batch).
4. Run the scene: `tools/graphics-rig/scenes --build "<build>" --dll d3d12.dll=... --dll
   dxgi.dll=... --api d3d12 triangle`, on the recorder first, then on G2's back end against the
   stored reference.
5. The independent check: a small Direct3D 12 program of our own, CheckFeatureSupport answers
   against real Direct3D's on the desktop, and reading the code for anything faked.

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

- Tests: `tools/graphics-rig/remote "cargo test -j 3 -p wcontainers-directx-graphics-infrastructure
  -p wcontainers-direct3d-12"` (15 tests: 7 infrastructure, 6 front, 2 unit; all pass on
  2026-11-17).
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 -- -D warnings"`.
- On the Mac only `nix develop -c cargo fmt` (the Mac is overloaded).

## Interface changes

- `Caps::adapter: AdapterInfo` (name, PCI vendor and device numbers, software): DXGI's adapter
  description is answered from `caps()` (plan 5.1), and `Caps` had nothing to answer it with. Back
  ends fill it from the host's adapter information (WebGPU's `GPUAdapterInfo`, WebGL's unmasked
  renderer); zeros where the host gives no numbers.

## Measurements

None yet.

## Departures from the plan

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

No new dependencies beyond the interface's own (`naga` with `wgsl-in`, `blake3`, both already
confirmed in plan section 3).

## Installed on the desktop

Nothing beyond the rig's setup.
