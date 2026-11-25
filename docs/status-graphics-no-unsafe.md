# Status: no unsafe code in five graphics crates (milestone GU1)

Crates: wcontainers-direct3d-12, wcontainers-gpu-webgl, wcontainers-gpu-webgpu,
wcontainers-graphics-test, wcontainers-graphics-test-forward. Rule: the graphics brief's section of
2026-11-22 evening (hard ban; a use that cannot go is reported with its count, not hidden).

## For the owner (rulings needed)

Every use left in these crates sits at one of five boundaries. For each: what it is, why no vetted
safe library does it, what removing it would cost.

1. Calling real Direct3D 11 and 12, DXGI and Win32 (graphics-test all 342; gpu-webgpu
   tests/direct3d.rs 19; direct3d-12 examples/check 169). The `windows` crate (Microsoft, 0.62.2) declares every function
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
   Measured in run 4: the browser half is safe as claimed. Under the workspace's forbid, a probe
   crate with `#[wasm_bindgen]` imports (`suspending` too), exports (`jspi` too), a struct with a
   constructor, and web-sys WebGL 2 calls (buffers, `get_buffer_sub_data`, fences, `read_pixels`,
   extensions) compiles for wasm32 and natively; `unsafe {}` added to it fails. So the back end's
   browser-only uses (the raw `unsafe extern "C"` import in platform.rs, `#[unsafe(no_mangle)]`
   in examples/browser.rs) can become wasm-bindgen imports and exports with no unsafe code.
5. Test windows and screen reading (gpu-webgpu tests/host 11): the present test makes a real
   window (Win32 or AppKit) and reads what the compositor shows (PrintWindow, DwmFlush;
   ScreenCaptureKit). winit (0.30.13, Apache-2.0, rust-windowing, about 68.6 million downloads)
   could make the window and hand wgpu a safe surface; reading the shown pixels would need a
   capture crate such as xcap (0.9.10, Apache-2.0, one maintainer, about 3.1 million downloads),
   not tried: whether it captures in exact sRGB on macOS and waits for the compositor (DwmFlush)
   is unknown. Both contain unsafe code for the owner to vet. Asked together with 1: allow the
   reference crates' Win32 calls, or these two imports?

## The five questions in short (to bring one at a time)

1. May the reference code call real Direct3D through the `windows` crate, whose every call is
   `unsafe fn`? 530 uses: graphics-test 342, direct3d-12 examples/check 169, gpu-webgpu
   tests/direct3d.rs 19.
2. May the Direct3D 12 Windows library (a C ABI DLL, direct3d-12 examples/directx, 326) be its own
   package that denies unsafe with each use marked, like the executors?
3. May the test-only forwarder that hooks a program's Direct3D (graphics-test-forward, 197) be a
   marked exception?
4. The WebGL 2 back end (gpu-webgl, 167): web-sys in the browser only (the native ANGLE route
   ends), glow kept natively as an exception, or a new remote back end that draws natively
   through headless Chrome?
5. May gpu-webgpu's present test (tests/host, 11) import winit and a screen-capture crate (xcap),
   or count as reference code under 1?

## Run 10 (2026-11-22 late)

- Still no ruling: the graphics brief and the orchestrator's open-with-owner notes were read again
  and neither rules on these five. No commit on `agents` since run 9 (a1872aa7). Counts unchanged
  (grep on the Mac): direct3d-12 495, graphics-test 342, graphics-test-forward 197, gpu-webgl 167,
  gpu-webgpu 30. glow 0.18.0, windows 0.62.2, windows-implement 0.100.0 and retour 0.3.1 are still
  the newest releases.
- No build or test: nothing these crates depend on changed. The shaders track's ps_position_centroid*
  containers are still uncommitted, so declared.expected is not recorded again yet (Next, 5).

## Run 9 (2026-11-22 late)

- Still no ruling: the graphics brief's newest section and the orchestrator's open-with-owner
  notes were read again, and the notes still do not list these five. Nothing was committed to
  `agents` after run 8 (origin/agents is 2ed81071). Counts are unchanged: direct3d-12 495,
  graphics-test 342, graphics-test-forward 197, gpu-webgl 167, gpu-webgpu 30.
- No build or test was run: nothing they depend on has changed since run 8's clean Windows lint.
- The shaders track has new containers in tests/own that are not committed yet
  (ps_position_centroid*). Once they are committed, declared.expected must be recorded again
  (Next, 5).

## Run 8 (2026-11-22 late)

- Still no ruling: the graphics brief and the orchestrator's open-with-owner notes were read again,
  and neither names these five boundaries. No commit landed on `agents` after run 7. Counts are
  unchanged: direct3d-12 495, graphics-test 342, graphics-test-forward 197, gpu-webgl 167,
  gpu-webgpu 30.
- The boundary libraries are still the newest releases on crates.io (glow 0.18.0, windows 0.62.2,
  windows-core and windows-implement 0.100.0, retour 0.3.1), so no new release changes the answers.
- Windows lint of all five crates, all targets, `-D warnings` (How to run's command): clean, with
  0 warnings (logs/clippy-windows-run8.log). The desktop's CPU load was 64 % at the end. The job
  waited about 35 minutes behind another track's GPU job that may reset the driver. This lint
  covers run 7's rename and declared.rs on Windows.

## Run 7 (2026-11-23 early)

- Still no ruling (graphics brief; the orchestrator's open-with-owner notes do not list these five).
  Counts unchanged: direct3d-12 495, graphics-test 342, graphics-test-forward 197, gpu-webgl 167,
  gpu-webgpu 30.
- Literal check on a clean clone of `agents` (a git bundle sent to the rented ARM machine, both
  submodules checked out; scratch bin/literals-clone.sh, logs/lit-run7/lit.txt). The two
  "restated" findings of these crates were real and are gone:
  - gpu-webgl's buffer clear constant `BLOCK` (256) shared the word with the kernel's `ECHO_BLOCK`;
    renamed `PATTERN_SPAN` (a8db8bb9).
  - direct3d-12 tests/declared.rs wrote `> 300` containers, equal to the gpu-test suite's `ROWS`.
    The guard is now "every corpus folder holds containers"; the count is already checked against
    declared.expected's lines (576822bf).
  After them no finding of the check names these five crates. The check still fails on the clean
  clone with 8,725 new findings of other crates: 8,255 in the shaders crate's two submodules
  (dxil-spirv 4,247, dxbc-spirv 4,008, their scripts), then executor 157, webassembly-engine 122,
  processor-architecture 93 and smaller ones. Not this track's.
- declared.rs passes again. declared.expected recorded again (`WC_RECORD_DECLARED=1`, rented ARM
  machine): 171 lines added for the 57 containers the shaders track added since, none changed or
  removed (diff checked), and every added container is committed. So the front's answers for the
  old containers did not move. The DXIL lines give dense slots (vs_store_drawn's u2 is slot 1
  beside u1), as `Places` numbers them for the converter; not a fault.
- Checks (rented ARM machine, 96 cores, load 10.7 at the start, 17.1 at the end;
  logs/check-run7.log): clippy `--all-targets -D warnings` of direct3d-12 and gpu-webgl clean, and of
  gpu-webgl's library for wasm32; declared passes (logs/run7-verify.log). Not run on Windows: the
  changes are a renamed constant and a test of files, nothing that differs there.

## Run 6 (2026-11-22 night)

- Still no ruling (graphics brief, open-with-owner notes). The orchestrator's list of rulings the
  owner is to hear (its "no-unsafe sweep stopped" section) holds the socket sweep's items, not
  these five; the report of this run puts them first again.
- Removed without a ruling: tests/gpu.rs's `D3DCompile` (boundary 1's last use outside examples/).
  The depth clamp scene's eight shaders are now HLSL files in tests/clamp/ with compile.cmd (the
  SDK's fxc, as the shaders crate's tests/own/ does) and the committed .dxbc, read with
  `include_bytes!`. Checked byte for byte before the switch: a temporary test (not committed)
  compared each file with what `D3DCompile` gave for the old source strings, all eight equal
  (logs/clamp-blobs-equal-run6.log). The scene stays Windows-only (it uses the windows crate's
  patch-list topology value). direct3d-12: 496 -> 495, every use now in examples/ (directx 326,
  check 169). Commits df4afd11 (the test) and 71fd9125 (root list).
- Checks of the change: clippy `--all-targets -D warnings` of direct3d-12 clean on Windows (desktop,
  logs/clippy-d3d12-run6.log) and on aarch64 Linux (rented ARM machine, 96 cores, load 3.3 at the
  start and 8.5 at the end; logs/check-d3d12-run6.log; the x86 machine did not answer). Tests on the
  RTX 3080 through the GPU queue (logs/test-d3d12-run6.log): 55 pass in binaries of 20, 1, 18, 13,
  3 and 0, both depth clamp tests among them; declared.rs fails at line 454 as in runs 1 and 2
  (known, above).
- Literal check: the rented copy is not a git checkout, so it ran on the desktop's tree
  (logs/literals-run6.log). It fails there for reasons outside this change: Windows line endings
  turn every multi-line string into a new finding (124 lines with `\r\n`), and other tracks'
  uncommitted work is in the shared tree. Nothing names tests/gpu.rs or tests/clamp/. Two
  "restated" findings land in these crates because other crates added equal constants:
  direct3d-12 tests/declared.rs:89 (300, now also `ROWS` of the gpu-test suite) and gpu-webgl
  src/lib.rs:4764 (256, now also the kernel's `ECHO_BLOCK`); to be judged on a Linux checkout
  once the tree is quiet.

## Run 5 (2026-11-23 morning)

- Still no ruling (graphics brief and the orchestrator's open-with-owner notes read again); counts
  unchanged, counted on the rented machine: direct3d-12 496 (examples/directx 326, examples/check
  cases.rs 129 and check_d3d12.rs 40, tests/gpu.rs 1), gpu-webgl 167, gpu-webgpu 30, graphics-test
  342, graphics-test-forward 197.
- Build check after the merge a00ab267 and the interface's aliased-line pixel variant (3a86f258),
  clippy `-D warnings`:
  - x86-64 Linux, rented x86 machine (96 cores, load 0.0 at the start, 4.1 at the end): all five
    crates, all targets, clean (logs/check-five-run5.log).
  - wasm32: gpu-webgpu lib and examples clean; gpu-webgl lib with examples browser and draw-cost,
    and with `trace` the replay example, clean; native gpu-webgl with `record`, all targets, clean
    (logs/check-webgl-wasm-run5.log). webgl-scenes is a native example (it opens ANGLE) and is not
    built for wasm32.
  - Windows, the desktop (CPU load 39 %, no other cargo running), the Mac's tree at ccef11b8 with
    the shaders track's uncommitted edits: all five crates, all targets, clean
    (logs/clippy-windows-run5.log).
  - Controls: `unsafe {}` added to the libraries of gpu-webgpu and direct3d-12 fails in both ("usage
    of an `unsafe` block").
- Looked again for uses that need no ruling: none. graphics-test's uses that are not Direct3D
  (gt-launch's debugger loop, gt-desktop's desktop and job object, gt-chrome's raw command line,
  lib.rs's loaded module path, debug.rs's info queue message) have no std equivalent. The lock
  file has no winit or capture crate (only raw-window-handle), so question 5 needs new imports.
- gpu-webgl: considered a safe seam module over its 151 distinct GL calls. Natively it would hold
  one unsafe per call (about 151, plus platform.rs's 41, against 167 today), and it pays off only
  once question 4 is ruled, so it was not started. All three answers share one part: the browser
  half calls WebGL through web-sys.

## Run 4 (2026-11-23 early)

- Still no ruling in the graphics brief or the orchestrator's notes; counts unchanged
  (direct3d-12 496, graphics-test 342, graphics-test-forward 197, gpu-webgl 167, gpu-webgpu 30).
- gpu-webgpu: its library and both examples (scenes, frames) now carry `#![forbid(unsafe_code)]`
  at their roots, as direct3d-12's library does, so the shipped back end cannot regain unsafe code
  while its tests (boundaries 1 and 5) keep the package on the list. Checked on the ARM machine
  (load 9 to 20 on 96 cores): clippy `-D warnings` of lib and examples clean natively (aarch64
  Linux) and for wasm32; `unsafe {}` added to the library fails ("usage of an `unsafe` block").
  Not rebuilt on Windows: the attribute only adds an error where unsafe code is, and none is in
  these files on any target (grep). Scripts: scratch bin/probe-wasm.sh, bin/check-webgpu.sh;
  logs/probe-wasm-run4.log, logs/check-webgpu-run4.log.
- Measured for question 4: wasm-bindgen and web-sys under forbid (see there).

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
- No code changed. A clippy run of the five crates on the desktop (logs/clippy-run3.log) waited
  30 minutes behind another track's GPU job that may reset the driver and was stopped before it
  began, so this run has no new build result; the last is run 2's, at fc549b36.
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
- tests: all 5 uses gone (writes through `Map` use the cells; run 6: `D3DCompile` replaced by
  committed fxc output, tests/clamp/).
- Package total 504 -> 495 (lines with the word, `grep -rw unsafe --include="*.rs"`), all in
  examples/; root Cargo.toml and the manifest say 495.
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
   front package inherits the workspace lints once examples/check (boundary 1) is settled too
   (its tests are free of it since run 6).
3. gpu-webgl: if ruling 4 says web-sys, rewrite its calls on web-sys behind one module. Note the
   Direct3D 11 track builds wcontainers-directx with feature `webgl`, which draws through this
   back end natively on ANGLE (its Wine tests and programs), so web-sys alone ends that route;
   with the remote back end (the third way) that route moves to Chrome instead. The browser suite
   (browser/suite.mjs) already runs the crate's checks in Chrome.
4. If ruling 5 allows winit and a capture crate: tests/host of gpu-webgpu on them, checked against
   today's PrintWindow and ScreenCaptureKit reading.
5. declared.expected must be recorded again each time the shaders track or the rig adds containers
   (the test fails at the first new one). Record it on any machine, then check with `diff` that
   lines were only added before committing.
6. Removing the extra upload pass needs a fast path in crates/gpu (the front filling
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
  `Queue::write_buffer_with`): a fast path with a default in crates/gpu, not made.

## Departures from the plan

- `#![forbid(unsafe_code)]` at the root of wcontainers-direct3d-12's library while the package
  cannot yet inherit the workspace lints (its Windows library and one test), so the front cannot
  regain unsafe code meanwhile. The same at the roots of wcontainers-gpu-webgpu's library and
  examples, whose package waits on its tests.

## How to run on the rented machines (no GPU)

- `cloud/arm-run <agents checkout> --name graphics-no-unsafe -- "$(cat <scratch>/bin/<script>)"`;
  the scripts edit only the machine's copy (probe crates, control `unsafe {}` lines).
- All five crates on Linux and wasm32, counts and controls: `cloud/x86-run <agents checkout> --name
  graphics-no-unsafe --jobs 32 -- "$(cat <scratch>/bin/check-five.sh)"`, then bin/check-webgl-wasm.sh
  for gpu-webgl's browser builds (check-five's wasm32 step for gpu-webgl builds every example and so
  fails on the native webgl-scenes; use check-webgl-wasm.sh for that crate).
- Windows lint of all five: `tools/graphics-rig/remote "set CARGO_INCREMENTAL=0&& git submodule
  update -q --init --recursive && cargo clippy -j 3 --all-targets -p wcontainers-direct3d-12 -p
  wcontainers-gpu-webgpu -p wcontainers-gpu-webgl -p wcontainers-graphics-test -p
  wcontainers-graphics-test-forward -- -D warnings"`.
