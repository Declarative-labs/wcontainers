# Status: no unsafe code in five graphics crates (milestone GU1)

Crates: wcontainers-direct3d-12, wcontainers-gpu-webgl, wcontainers-gpu-webgpu,
wcontainers-graphics-test, wcontainers-graphics-test-forward. Rule: the graphics brief's section of
2026-11-22 evening (hard ban; a use that cannot go is reported with its count, not hidden).

## For the owner (rulings needed)

Every use left in these crates sits at one of five boundaries. For each: what it is, why no vetted
safe library does it, what removing it would cost.

1. Calling real Direct3D 11 and 12, DXGI and Win32 (graphics-test all 342; gpu-webgpu
   tests/direct3d.rs 19; direct3d-12 examples/check 169 and
   tests/gpu.rs 1, `D3DCompile`). The `windows` crate (Microsoft, 0.62.2) declares every function
   and every COM method `unsafe fn`. No maintained safe wrapper of Direct3D 11 or 12 exists (gfx-rs
   `d3d12` was retired into wgpu-hal, whose API is unsafe; nothing for Direct3D 11). These crates
   exist to draw the reference pictures with real Direct3D, so wgpu cannot stand in. Removing it
   means a safe Direct3D wrapper crate, which the rules forbid us to write as a helper. Asked: may
   the reference crates call real Direct3D (the comparison's yardstick, never shipped)?
2. Being a Direct3D 12 library for C programs (direct3d-12 examples/directx, 326): DLL
   exports need `#[unsafe(no_mangle)]`, which the lint counts as unsafe code; COM objects through
   `windows-core`'s `implement` macro do not compile under `forbid(unsafe_code)` at all (measured,
   below); and every method receives raw pointers from the program (descriptions, out-pointers,
   mapped memory), which only unsafe code may read. This is inherent to a C ABI library.
   Asked: may the Windows library be its own package that denies (like the executors) instead of
   forbidding, with each use marked? The front library itself (src/) is now free of it.
3. Hooking a program's Direct3D (graphics-test-forward, 197): it patches the method tables of
   objects the system's d3d11.dll and d3d12.dll made, wraps contexts, and detours the process's
   clock functions (retour, 0.3.1) so captured frames repeat. Writing into another library's
   tables and code is unsafe by nature; no library can make it safe. It is test-only and never
   shipped. Asked: keep it as a marked exception, like the executors?
4. The WebGL 2 back end's GL calls (gpu-webgl, 167). glow (0.18.0) declares every `HasContext`
   method `unsafe fn`, natively and in the browser; the native path also loads EGL and ANGLE at run
   time (khronos-egl `dynamic`) and casts extension entry points (`transmute`). In the browser,
   web-sys's `WebGl2RenderingContext` (wasm-bindgen project) is a safe API for every call the back
   end makes. Natively no vetted safe GLES binding exists (glium is desktop GL and a different,
   higher-level model). Removing it: rewrite the back end's calls on web-sys (browser only, about
   the size of the crate's src/, several runs) and run its tests in Chrome instead of on ANGLE;
   or keep glow natively as an exception. A third way keeps a native route with no unsafe code
   (run 3, not built): a remote back end, a new implementation of the graphics interface that
   sends each call to a page in headless Chrome and waits there for the answers that need one
   (read backs, queries, fences). Half of it exists (src/trace.rs: `Call` is plain serialized
   data, `Replay<B>` runs a stream on any back end, examples/replay.rs runs it in the browser);
   `Answer` holds only success, a bool and a read state, so the bytes of read backs and query
   results must be added, with a live channel (a WebSocket, through a safe crate such as
   tungstenite) and the page's end. Native programs and the Direct3D 11 track's Wine tests would
   then draw on Chrome's own WebGL 2, the shipped target, instead of on our patched native ANGLE
   (angle/). Cost: every answer is a round trip to the browser, so native timings would not stand
   for browser timings (the browser pages measure those already). Asked: which of the three?
5. Test windows and screen reading (gpu-webgpu tests/host 11): the present test makes a real
   window (Win32 or AppKit) and reads what the compositor shows (PrintWindow, DwmFlush;
   ScreenCaptureKit). winit (0.30.13, Apache-2.0, rust-windowing, about 68.6 million downloads)
   could make the window and hand wgpu a safe surface; reading the shown pixels would need a
   capture crate such as xcap (0.9.10, Apache-2.0, one maintainer, about 3.1 million downloads),
   not tried: whether it captures in exact sRGB on macOS and waits for the compositor (DwmFlush)
   is unknown. Both contain unsafe code for the owner to vet. Asked together with 1: allow the
   reference crates' Win32 calls, or these two imports?

## Run 3 (2026-11-22 night)

- No ruling on the five boundaries was in the graphics brief at the start of the run; the counts
  are unchanged (summed again per file: direct3d-12 496, graphics-test 342, graphics-test-forward
  197, gpu-webgl 167, gpu-webgpu 30).
- Looked again for uses outside the boundaries: none. The small Win32 calls in graphics-test
  (process launching, job objects, the debugger loop of gt-launch) could go through safe crates,
  but each is one more import to vet and the crate stays at boundary 1 for its Direct3D calls, so
  they were left. `D3DCompile` in direct3d-12 tests/gpu.rs could become a run of the SDK's fxc.exe
  (present on the desktop under Windows Kits 10.0.26100.0, not on PATH); left too, for the same
  reason (the package stays on the list for examples/).
- Added the third way for gpu-webgl to question 4.
- Not split: examples/directx into its own package. Its header says it moves into
  wcontainers-directx (plan section 10), and the rig's build commands (tools/graphics-rig, not
  this track's) name it as an example of this package.

## Done

- wcontainers-direct3d-12 src/: free of unsafe (3 uses gone) and held to it with
  `#![forbid(unsafe_code)]` at the library's root. Mapped memory of upload, readback and shared
  heaps is `Rc<Box<[Cell<u8>]>>` zeroed by zerocopy (`FromZeros::new_box_zeroed_with_elems`, so
  untouched pages stay untouched as with `vec![0; n]`); `Map` returns the cells and the Windows
  library hands the program `cells.as_ptr()`, a pointer that stays valid while the device holds
  the cells (interior mutability, no `&mut` ever made over it). Descriptor heap slots are
  `Box<[Cell<Descriptor>]>`, read with `Cell::get` instead of `read_volatile`.
  Cost: an upload copies the program's bytes out of the cells into one reused staging vector
  before `write_buffer` (one more pass over each uploaded range), and a read back lands in the
  staging vector before the cells. Measured: see Measurements.
- tests: 4 of 5 uses gone (writes through `Map` use the cells); `D3DCompile` (tests/gpu.rs) stays,
  boundary 1.
- Package total 504 -> 496 (lines with the word, `grep -rw unsafe --include="*.rs"`), all in
  examples/ and the one test; root Cargo.toml and the manifest say 496.
- The other four crates: nothing removed yet; every use is at a boundary below (For the owner).
  Counts as listed: gpu-webgpu 30, gpu-webgl 167, graphics-test 342, graphics-test-forward 197.
- `#[unsafe(no_mangle)]` was measured in the probe too: "usage of the unsafe `no_mangle`
  attribute" under forbid.

## Results (2026-11-22, RTX 3080, desktop shared with other tracks)

- Lint: clippy `--all-targets -p wcontainers-direct3d-12 -D warnings` clean, the library and both
  forwarders included (logs/clippy-d3d12-2.log).
- Tests (logs/test-d3d12-3.log, GPU queue): 55 pass in binaries of 20, 1, 18, 13, 3 and 0; 1 fails,
  declared.rs, not from this change: on Windows it named files with `\` (fixed, commit "the
  declared containers test names files with slashes ..."), and it then stops at line 454 because
  the shaders track added containers (cs_alpha.dxbc, 5e73ebc2, 2026-11-22) after
  declared.expected was recorded (06a2647d, 2026-11-21). Known failure, left failing; it needs the
  expected file recorded again by whoever owns its meaning (it records answers from before ranges).
- Own program (logs/own-1.log, `--desktop`): draw 0 of 4096 beyond 1; 35 of 35 cases equal;
  outputs 22 of 110 and answers 2 of 40 differ, none beyond real, as in run 52; features 157 of 872
  differ (run 52: 166; feature answers do not touch mapped memory, other commits since moved it);
  antialiased lines 0 of 4096 beyond 63, largest 38, as run 52. Fence round trip (not alone): ours
  first 152 us, mean 17 us, 99th percentile 19 us; real mean 1 us (run 52 alone: ours mean 17 us).

## Before and after the cells (run 2, 2026-11-22 evening, RTX 3080, desktop shared)

Two scratch clones of `agents` at fc549b36: gu1-after as it is, gu1-before with dde02576 (the
cells) reverted, nothing else different; each built on the desktop in its own folder; the rig's
comparison patch (`$worse` line of desktop/wine-run.ps1) applied in both, uncommitted. One command
per step: scratch bin/verify.sh `<before|after> <tag> <steps>` (programs, tests, files, own,
runner, scenes); bin/chain1.sh and chain2.sh ran them. Logs in scratch logs/<variant>-<tag>-*.
- Real programs, three rounds interleaved before, after (logs/*-r1..r3-programs.log, frames in
  *-frames/): every frame of ours after is byte-identical to ours before for Godot's 2D and 3D
  platformers and Microsoft's two samples; bgfx 01-cubes and 38-bloom identical outside the
  overlay's timing text (0,208,320,64, the rectangle the comparison ignores; scratch
  bin/pngdiff.py, 0 pixels differ). Against real Direct3D 12 the same as run 52: the 2D platformer
  and both samples equal, bgfx within its allowance, the 3D platformer the same one pixel (921,317,
  difference 48).
- Times, whole run of 120 frames in seconds, desktop CPU load 1 % to 53 % at the runs' starts
  and ends (Win32_Processor LoadPercentage),
  other tracks' jobs running beside: Godot 3D before 53.4 (cold shader caches), 9.7, 9.7; after
  9.8, 10.1, 9.8. Godot 2D before 5.4, 5.1, 4.4; after 4.9, 4.2, 4.0. bgfx 01-cubes 2.6 in all six.
  bgfx 38-bloom before 2.6, 2.5, 2.5; after 2.6, 2.6, 2.6. Hello triangle and constant buffers 2.5
  to 2.6 in all. The whole-run time includes start-up; the forwarder records no per-frame times.
  A difference of the size the extra copy predicts (about 0.1 ms a frame) is below this noise.
- Tests (after-c-tests.log): 55 pass in binaries of 20, 1, 18, 13, 3 and 0; declared.rs fails at
  line 454, and fails the same way at line 454 in the before clone (before-declared.log), so it is
  not this change.
- Own program (after-c-own.log): draw 0 of 4096 beyond 1; 0 of 35 cases differ; features 157 of
  872, outputs 22 of 110 and answers 2 of 40 differ, as run 1; antialiased lines 0 of 4096 beyond
  63, largest 38. Fence round trip ours mean 17 us, 99th percentile 21 us; real mean 1 us.
- Test files (after-c-files.log, logs in after-c-files-logs/): 189 of 198, the same 9 failures as
  the Direct3D 12 track's run 52 (its status file's "Test files" list).
- Shader runner (after-c-runner.log): 218 of 302 (run 52: 214; other tracks' converter work
  since). Scenes (after-c-scenes.log): 110 pass, 5 fail (msaa-alpha_to_coverage, raster-fill,
  samplers-filters, tessellation, topology), 1 absent (samplers-reduction). The before clone gives
  the same: runner 218 of 302 with the same failing files (before-c-runner.log), and every scene
  line the same, pixel counts included (before-c-scenes.log).
- So the change of run 1 changed no result and no picture; its time cost is not measurable here.
- The desktop's gu1-* folders (src, target, out; 16 GB) were deleted after the runs; the Mac's
  scratch clones (no build output) and logs stay as evidence.

## Doing COM and FFI safely: what compiles under `forbid(unsafe_code)` (for the other tracks)

Probe crate on the desktop (scratch probe/, Rust as the rig's env, windows 0.62.2):
- `#[derive(bytemuck::Pod, bytemuck::Zeroable)]` (1.25.2): compiles.
- zerocopy 0.8 derives (`FromBytes`, `IntoBytes`, `Immutable`, `KnownLayout`): compile.
- `#[windows_core::interface(..)]` declaring a COM interface trait: compiles.
- `#[windows_core::implement(..)]`: does NOT compile. It emits, per interface, an `unsafe` block
  (`as_interface_ref`) and an `unsafe fn as_impl_ptr` spanned on the interface's path, so the
  lint fires on the user's line ("usage of an `unsafe` block", "implementation of an `unsafe`
  method"). windows-implement 0.100.0 (2026-09) emits the same (source read, gen.rs).
- Any `windows` function or COM method call: `unsafe fn`, needs an unsafe block.
- Memory a foreign program writes: `[Cell<u8>]`; `as_ptr()` is safe and the pointer stays valid;
  reading it into a `&[u8]` without a copy is not possible in safe code.

## Dependencies added

- zerocopy 0.8.63 (already in Cargo.lock through other crates) to wcontainers-direct3d-12, feature
  `alloc`. BSD-2-Clause OR Apache-2.0 OR MIT; Google; about 1.19 billion downloads. Contains
  unsafe (that is its job: layout proofs behind a safe API).

## Next

1. The owner's rulings above. Until then the five crates stay on the root list with their counts.
2. If ruling 2 allows a separate package: move examples/directx into it (the rig builds it with
   `cargo build -p wcontainers-direct3d-12 --examples`; tools/graphics-rig must follow), then the
   front package inherits the workspace lints once examples/check (boundary 1) is settled too.
3. gpu-webgl: if ruling 4 says web-sys, rewrite its calls on web-sys behind one module. Note the
   Direct3D 11 track builds wcontainers-directx with feature `webgl`, which draws through this
   back end natively on ANGLE (its Wine tests and programs), so web-sys alone ends that route;
   with the remote back end (the third way) that route moves to Chrome instead. The browser suite
   (browser/suite.mjs) already runs the crate's checks in Chrome.
4. If ruling 5 allows winit and a capture crate: tests/host of gpu-webgpu on them, checked against
   today's PrintWindow and ScreenCaptureKit reading.
5. Removing the extra upload pass needs a fast path in crates/wcontainers-gpu (the front filling
   the back end's staging bytes, wgpu's `Queue::write_buffer_with`); not this track's crate.

## How to run

- The rig on a shared branch: `remote` and `gpu-job` send the Mac's working tree to
  C:\wc\src\agents, which every agent on branch `agents` shares, and a job fails with 125 when
  another agent sent a newer tree while it waited ("holds tree X, not the sent Y"). Resubmit; a
  higher `--priority` (10) shortens the wait. Do not edit the tree while a job waits.

- Lint (Windows, all targets including the library): `tools/graphics-rig/remote "set
  CARGO_INCREMENTAL=0&& cargo clippy -j 3 --all-targets -p wcontainers-direct3d-12 -- -D warnings"`.
- Tests: `tools/graphics-rig/gpu-job --build "set CARGO_INCREMENTAL=0&& cargo test -j 3 -p
  wcontainers-direct3d-12 --no-run" -- "cargo test -j 3 --no-fail-fast -p wcontainers-direct3d-12"`.
- Counting: `grep -rcw unsafe crates/<crate> --include='*.rs'`.
- Before and after a change, away from the shared tree: clone `agents` into scratch twice
  (`git clone --shared`), branch gu1-before and gu1-after (the rig names the desktop folders after
  the branch), revert the change in one, build each with `tools/graphics-rig/remote "set
  CARGO_INCREMENTAL=0&& git submodule update -q --init --recursive && cargo build -j 3 -p
  wcontainers-direct3d-12 --examples && cargo build -j 3 -p wcontainers-graphics-test -p
  wcontainers-graphics-test-forward"` one after the other (3 jobs), then scratch bin/verify.sh.
- Logs in scratch graphics-no-unsafe/logs.

## Measurements

- The copy the change adds (cells into the reused staging vector, against `extend_from_slice` of
  a plain `[u8]`), release build, best of 5, on the GPU desktop (i9-9900K) with other tracks' GPU
  jobs running, CPU load 52 % before and 46 % after (Win32_Processor LoadPercentage), scratch
  probe/src/bin/cellcopy.rs: 1 MiB plain 30.1 us, cells 37.2 us; 4 MiB 143.1 us and 143.0 us;
  64 MiB 7455 us and 9383 us. So the cells read at memcpy speed; the cost is one more pass over
  each uploaded range (about 143 us a frame for 4 MiB of upload memory). Removing that pass needs
  the interface to let the front fill the back end's staging bytes directly (wgpu's
  `Queue::write_buffer_with`): a fast path with a default in crates/wcontainers-gpu, not made.

## Departures from the plan

- `#![forbid(unsafe_code)]` at the root of wcontainers-direct3d-12's library while the package
  cannot yet inherit the workspace lints (its Windows library and one test), so the front cannot
  regain unsafe code meanwhile.
