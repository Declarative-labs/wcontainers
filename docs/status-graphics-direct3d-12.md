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
  pacing by the maximum frame latency. 7 tests on a recording back end.
- Not done yet: the Windows `dxgi.dll` shell (COM objects over this core), the Direct3D 12 front,
  `d3d12.dll`, and running the rig's Direct3D 12 scene through them.

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

- Tests: `tools/graphics-rig/remote "cargo test -j 3 -p wcontainers-directx-graphics-infrastructure"`.
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p
  wcontainers-directx-graphics-infrastructure -- -D warnings"`.
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

## Licences

No new dependencies beyond the interface's own (`naga` with `wgsl-in`, `blake3`, both already
confirmed in plan section 3).

## Installed on the desktop

Nothing beyond the rig's setup.
