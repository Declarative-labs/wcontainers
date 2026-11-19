# Graphics WebGL 2 back end status (G3, G3b)

Track: `crates/wcontainers-gpu-webgl`, the WebGL 2 back end on glow, branch `phase-2-graphics-webgl`.
Plan: docs/phase-2-graphics.md, sections 4, 5.8, 6.5, 10 and 12 (G3).

## Where things stand

G3b run 8, 2026-11-19 (about 01:20 to 03:00 Pacific). Merged origin/phase-2-graphics-webgpu, which held
the newest -direct3d-11 and -shaders (the shared line, -direct3d-12, -rig and phase-1-literals had
nothing new; no interface change among them). The run's own work:
- filter_draw's lost device (Next 0 of run 7): cause found and the loss removed. The recorded
  stream (scratch g3b7/filter.bin) replayed natively with changes (scratch g3b8/diag, a
  diagnostic kept out of the tree; Measurements, run G3b-8) showed that the test's 419 programs
  are four distinct ones (six distinct shaders) made again for every draw, that each live GL
  program of them held about 650 KB of the adapter's memory (354 MB to 629 MB over 419), and
  that ANGLE's device is lost at the 419th with every program alive, also with the shaders
  deleted after the link; it is not lost when each program is destroyed after its draw, nor when
  shaders of equal content are made once (the adapter's memory then does not rise). So shaders
  whose GLSL and bindings are equal are now one GL shader, counted by handles
  (`Gl::shaders`, `shader::ShaderText`), and their programs one GL program (the program cache
  by shader pair already did this for one shader made once). The module's `key` is not the key:
  tests make every module with key 0. A front that makes the same shader again (a game's
  re-created materials, the shaders crate's tests) no longer pays a compile, a link and the
  driver's memory each time. New check `programs_of_equal_shaders_draw_until_each_is_destroyed`.
  The 16 seeds of the shaders crate's filter_draw in one process (unchanged, it still destroys
  nothing), which lost the device in runs 3 to 7: no loss, all 792 comparisons drawn, every
  WebGL 2 one passing but the test's known anisotropic ones (its two failures are the WebGPU
  back end's own sampler; Measurements, run G3b-8). The recorded
  stream replayed in Chrome 157 (where run 7 lost the context at its first frame): all 419
  draws, 0 calls failed, the device Ok, `PipelineMiss` 2 (so the dxbc and dxil routes' shaders
  write equal GLSL, and the stream's 419 programs are two GL programs).
  Distinct large programs by the hundred stay unbounded (Known failures).
- Re-read the Direct3D 11, shaders and shared status files after the merge: nothing new asks
  anything of this back end. The front's WebGL 2 build's constants and queries scenes now match
  (its run 25); its 21 failing outputs are four scenes it refuses by name (raster: polygon mode,
  absent on mozangle; views; geometry and stream output). The shaders track's Chrome compile of
  its corpora lists GLSL ES files Chrome refuses for what GLSL ES 3.00 lacks; those are the
  converters' raw naga GLSL, not this back end's (bit functions and gathers are built here,
  multisampled reads and the primitive index absent).
- The literal check at the head: exit 0, 0 marks. This crate's baseline lines 630 to 629: the
  rename of `compile` to `write` moved the clip and cull distance limit's 8, now one named
  constant (`shader::CLIP_AND_CULL_DISTANCES`). In the LF worktree only the submodules the
  shaders crate builds from may be checked out (its "After cloning"): dxil-spirv's other
  submodules had been checked out there and gave 8,255 `+` lines; deinit'd, with
  third_party/spirv-headers checked out again.
- At the end the desktop's C:\wc\target\phase-2-graphics-webgl (16.9 GB) and this checkout's
  target (0.3 GB, from `cargo check`) were deleted. Kept: C:\wc\out\webgl\g3b8 (the diagnostic's
  memory samples, under 1 MB), the LF worktree. Nothing installed.

G3b run 7, 2026-11-18 22:30 to 2026-11-19 about 02:00 Pacific. Merged origin/phase-1-literals
(the seventh rules), -webgpu, -direct3d-11, -direct3d-12 and -shaders (the shared line and -rig
had nothing new; no interface change among them touches this back end). The run's own work:
- The ATG samples' wait inside texture writes (Next 1 of run 6) is found and removed. It was not a
  texture the front writes: every second frame this back end refreshes the sRGB twin of the
  swap chain's BGRA buffer (`fresh`, `copy_whole`), and that copy went through a pixel buffer
  (`copy_through_pbo`), whose unpack ANGLE does by mapping the buffer on the CPU, waiting for
  the GPU. Copies between textures whose texels have the same bytes now go by a draw where one
  carries both formats and neither is a float (`copy_same_bytes`), so they stay on the GPU; the
  copy draw (convert.rs) now also carries 8-bit sRGB and BGRA formats. SimpleLightingPC's time
  under the back end's functions went from 16.3 ms to 0.06 ms a frame (Measurements, run G3b-7).
  New check `srgb_copies_keep_every_byte` (every byte value through each 8-bit sRGB pair) passes
  on ANGLE and in Chrome.
- The 0xc0000005 seen in run 6: read. Every such crash on this route (five, in the ATG samples
  only) is at one place in the system's d3d11.dll (offset 0x96777, version 10.0.19041.5794), at
  start-up, before any frame, in about one run in four; Microsoft's public symbols name no
  function there. ANGLE's calls reach that library through the front's forwarding by caller.
  Cause not found (Known failures).
- filter_draw's lost device (Next 0): still neither built nor refused, but now reproduced in
  Chrome's own ANGLE, by replaying the test's WebGL 2 calls recorded natively (the recording
  back end gained `renderer` and `last_log`, which the shaders crate's tests call). Details
  under Known failures.
- Re-read the Direct3D 11, shaders and shared status files after the merges: nothing new asks
  anything of this back end (the front's primitive index and run-time indexed constants on
  WebGL 2 are its own routes).
- bgfx-01-cubes through the WebGL 2 build still draws nothing (no frame report in 10 s; the Direct3D
  11 track's ground), so a program with many draws a frame is still not measured.
- The literal check after the merges: one `init` for the seventh rules (as docs/literals.md step
  4 says), then the lines the merges left without their literals removed. Exit 0 at the head.
- At the end the desktop's C:\wc\target\phase-2-graphics-webgl (21.5 GB: the debug, WebAssembly,
  Direct3D 11 WebGL 2 debug and release builds and the frames example) and this checkout's
  target (0.7 GB, from `cargo check`) were deleted. Kept: C:\wc\out\webgl\frames (1.17 GB, the
  frame breakdowns' profiles and logs) and the LF worktree, whose shaders submodules are now
  checked out (Installed on the desktop).

G3b run 6, 2026-11-18 (about 19:30 to 22:20 Pacific). Merged origin/phase-2-graphics-webgpu,
-direct3d-11, -direct3d-12 and -shaders (the shared line and -rig had nothing new; no interface
change among them needed anything here), and, as the brief now asks, origin/phase-1-literals
(the literal check; see "Magic literals"). Fixed what the check of run 5 found:
- The frame-time measurement now has the front's share: the WebGPU track's breakdown (Windows'
  recorder, samply, its frames example) run on real programs drawing through the Direct3D 11
  front on this back end over ANGLE (new crates/wcontainers-gpu-webgl/frames/run). Results and
  method under "Measurements, run G3b-6"; the departures it needed are listed there.
- `SAMPLE_MASK` and `SAMPLE_SHADING`: the reason was wrong. `SAMPLE_MASK` stays absent (no
  `sampleMaski` in WebGL 2); `SAMPLE_SHADING` is now built in Chrome with OES_sample_variables and
  OES_shader_multisample_interpolation, so the front's `SampleMaskInShader` route works there.
- Gathers through a sampler with a minimum level of detail above 0: fixed (level-zero sampler
  twins), compared equal with WebGPU natively and in Chrome.
- Linear values take NV_shader_noperspective_interpolation where the context has it (Chrome).
- The back end's reports (device lost, naga's refusals, the new frame report) now reach the
  browser's console; before, they went nowhere in WebAssembly.
- filter_draw's lost device: still not settled (Known failures, Next 0).
- B5G6R5's widths: kept as this crate's one named constant (Departures, run 6).
- The pass bar's stale suite count: replaced by a pointer to the newest measurements.
- At the end the desktop's C:\wc\target\phase-2-graphics-webgl (17.2 GB: debug, WebAssembly,
  the front's WebGL 2 debug and release builds, the frames example) and this checkout's target
  (0.4 GB, from `cargo check`) were deleted. Kept: the LF worktree (Installed on the desktop).
- The check's leftovers on the desktop (C:\wc\target and C:\wc\src
  phase-2-graphics-webgl-check11, and other -checkN folders): not made by this track's runs, so
  not deleted (the task allows deleting only our own); they still need removing.

G3b run 5, 2026-11-18 (about 16:00 to 17:30 Pacific). Merged origin/phase-2-graphics (3 commits)
and origin/phase-2-graphics-shaders (6 commits, run 33; its interface commit 34981564 changes only
the doc comment of `TargetInShader::format`, which this back end does not read). Not merged:
phase-1-literals (Magic literals). Fixed what the check of run 4 found:
- `noperspective` (linear) values: built (`linear_by_w`), on ANGLE and in Chrome; before, the
  front got `Invalid` for them. Fine derivatives: refused by name through the new interface bit
  `Features::FINE_DERIVATIVES` (Interface changes). Details under "G3b: what the fronts ask".
- The "about 400 programs" claim: corrected; it is not a bound on the count (Known failures).
- Read backs that convert on the host (BGRA's swap, B5G6R5 packed again) now count
  `FormatConverted`, now and later and into buffers; `half_floats_read_back_as_stored` and
  `bgra_textures_keep_their_byte_order` assert it (Departures: reads that only narrow are not
  counted).
- The bare numbers of runs 3 and 4: B5G6R5's packing and BGRX's alpha byte derived; the
  per-format widths wait for the interface (Magic literals).
- The not-built list is current (19 `unbuilt` sites, three added).
- Special float values written as constants: ANGLE folded them and lost -0 (and natively made a
  vector all infinities). Fixed by reading such bits through a uniform zero (`opaque_float_bits`);
  new check `special_float_values_keep_their_bits` passes on ANGLE and in Chrome.
- The frame-time measurement: the front's share is still not measured, and a program with many
  draws still did not record (Measurements, run G3b-5).
- At the end the desktop's C:\wc\target\phase-2-graphics-webgl (12.8 GB: the debug,
  WebAssembly, Direct3D 11 WebGL 2 and recording builds) and this checkout's own target (0.3 GB,
  from `cargo check`) were deleted; no build folder was made in scratch. Nothing was installed on
  the desktop.

G3b run 4, 2026-11-18 (about 14:15 to 15:45 Pacific). Merged origin/phase-2-graphics (52 new
commits; it held every track branch's newest work: storage words, binding kinds, indirect draws
past a stream, none needing a change here). Done:
- The device the shaders track's 16-seed `filter_draw` run lost (check9's open finding): cause
  narrowed. The test never destroys its programs; with about 400 of them alive mozangle's ANGLE
  removes its Direct3D 11 device (not a bare count of programs: see Known failures). With them destroyed, all 1,152 WebGL 2 draws of the 16 seeds pass; the other
  devices of the process play no part (details under "G3b: what the fronts ask"). The fix is in
  the shaders track's test.
- A depth texture a program both compares against and reads plainly (the shaders track's run
  32: "on WebGL 2 such programs stay refused"): built, plain reads go through a float copy of the
  texture's variable (check `depth_textures_compared_and_read_plainly`, ANGLE and Chrome).
- Every Direct3D 11 scene through the front's WebGL 2 build: 88 pass, 27 fail, 30 absent; each
  failing scene's log read, and every one stops in the front (list under "G3b: what the fronts
  ask").
- The 21 GL constants glow defines and this crate retyped are now glow's (section "Magic
  literals", which also says what is left of the audit's list).
- The full run of the suite at the end: Measurements, run G3b-4.
- At the end the desktop's C:\wc\target\phase-2-graphics-webgl (16.5 GB: the debug, WebAssembly
  and Direct3D 11 WebGL 2 builds, and the shaders crate's tests for the diagnosis) was deleted;
  no build folder was made on the Mac or in scratch. Nothing was installed on the desktop.

G3b run 3, 2026-11-18 (about 12:00 to 14:00 Pacific). Merged origin/phase-2-graphics (76 new
commits; -webgpu, -direct3d-11, -direct3d-12 and -shaders were already in it, -rig had nothing
new). Fixed what the independent check of G3b (check9) found:
- BGRX (`Bgrx8Unorm`, `Bgrx8UnormSrgb`) had no use, so the Direct3D 11 front's `CheckFormatSupport`
  answered `E_FAIL` and its formats scene stopped: run 1 wrongly put that on the front. Now built:
  stored as BGRA (RGBA8 or SRGB8_ALPHA8, red and blue swapped on the host, `FormatConverted`) with
  alpha held at one: host writes set the X byte to 255, clears write alpha one, draws leave alpha
  (the colour mask of every BGRX target of the pass; a pass with BGRX and other targets uses
  OES_draw_buffers_indexed and loses the device as not built without it), and passes never
  discard a BGRX target. Sample, filter, render, blend and copy out; read back X is 255, as on
  the WebGPU back end. Check `bgrx_textures_hold_alpha_one`.
- With BGRX served, the formats scene stopped at the next two formats, which render here but did
  not copy out (`E_NOTIMPL` from the front's check of the staging copy): `B5G6R5` now reads back
  as RGBA bytes packed again into 16 bits on the host (exact: 8 bits keep every 5 and 6-bit value
  apart), and `R11G11B10` through its bits drawn into an `R32Uint` twin, as the 16-bit floats are
  (`CopyThroughBuffer`; infinities, NaN payloads and denormals kept). Check
  `half_floats_read_back_as_stored` now covers both. The front's formats scene through its WebGL 2
  build: every output passes but the two snorm targets, absent natively as declared (Measurements).
- The shaders track's seed 14 of `tests/filter_draw.rs` (a WebGL 2 draw that lost ANGLE's device
  in its run 15): alone it passes at today's code, but 16 seeds in one process lose ANGLE's device
  again (about case 4), with no driver reset. OPEN, cause not found (Known failures; details under
  "G3b: what the fronts ask").
- `submit` no longer calls `getError`, which in Chrome waits for the GPU process: GL errors are
  read after the back end has waited anyway (`wait_fence`, `read_texture_now`), and a lost context
  is seen at `submit` and `status` through `isContextLost` (no wait). A lost context's missing
  sync object no longer panics (the fence is done, the device lost), and natively a context that
  cannot be made current leaves no context current and loses the device (`Removed`). On
  SimpleLightingPC's frames the WebGL 2 frame went from 5.3 ms to 0.27 ms (Measurements, run G3b-3).
- A stage reading more uniform buffers than `Limits::uniform_buffers_per_stage` is refused at
  `create_shader` (`Invalid`), not lost at its first draw. Natively the limit is now 10, not 12
  (Departures). Check `stages_past_the_uniform_buffer_limit_are_refused`.
- The signed normalised minimum is judged per level: a level drawn into reads back through its
  twin while another level holds the minimum. Layers of one level are still judged together.
- The `record` feature records nothing without `WEBGL_TRACE` and draws as `Gl` does, so a build
  that gets the feature through another crate keeps working; the crate's examples and tests use
  `Gl` by name.
- The frame-time replay no longer hangs: each replay's device is dropped before the next is
  made, the page logs its progress, and the command stops itself after 300 s. The cause of the
  hang was not isolated (Departures).
- Cube gathers whose footprints cross face edges are compared with WebGPU's own gathers (new
  comparison `cube_gathers_across_face_edges_equal_webgpu`, natively and in Chrome), and the gather
  rewrite takes its first argument up to the comma outside brackets (unit test).
- New from the shared line: copies within one level of a 3D texture (one subresource) went
  through `copyTexSubImage3D` on the same texture, which ANGLE garbled; they now go through a
  buffer (`CopyWithinTexture`). Its two new suite tests run in Chrome too.
- At the end the desktop's C:\wc\target\phase-2-graphics-webgl (16.8 GB: the debug, release,
  WebAssembly, recording and Direct3D 11 WebGL 2 builds) and this checkout's own target (0.95 GB,
  from `cargo check`) were deleted. Nothing was installed on the desktop.

G3b run 2, 2026-11-18 (about 08:30 to 10:15 Pacific). Merged origin/phase-2-graphics (it held
-shaders and -direct3d-11 already; -webgpu and -rig had nothing new). The open findings of G3's
check were fixed in run 1 (list below). Done in this run:
- The shared line's three new suite tests (`a_copy_within_one_depth_or_stencil_subresource_moves_the_region`,
  `the_occlusion_counter_counts_every_sample_that_passes`, `every_vertex_format_reads_its_element`)
  pass on ANGLE and in Chrome; the browser host lacked them and now runs them. The full run after
  the merge: see "Measurements, run G3b-2".
- The frame-time measurement (the last item of G3b): a call stream of the interface
  (src/trace.rs, features `trace` and `record`), recorded natively while a real rig program draws
  through the Direct3D 11 front on this back end over ANGLE, and replayed in Chrome on the WebGL 2
  and the WebGPU back ends (examples/replay.rs, browser/replay.mjs). One interface change for it
  (serde on the descriptors). Measured on DirectX Tool Kit's SimpleLightingPC; where the time goes
  is under "Measurements, run G3b-2", and the slow part found is first under Next.
- Re-read the Direct3D 11, shaders and shared status files after the merge. New for this back end:
  the shared line's three suite tests (above). The Direct3D 11 front's new WebGL 2 items are its
  own: texture views at 10_0 (refused here by name, `TEXTURE_VIEWS`, the front's
  `ReinterpretCopy`), per-target blends where OES_draw_buffers_indexed is absent (this back end
  declares `INDEPENDENT_BLEND` where the extension is: Chrome and mozangle both have it), and
  `resinfo` through `TEXTURE_INFO` (served by the front since its run 19).
- At the end the desktop's C:\wc\target\phase-2-graphics-webgl (12.9 GB, the debug, release,
  WebAssembly and recording builds) and this checkout's own target (4.2 GB, from `cargo check`)
  were deleted.
- Not run in this run: the Direct3D 11 front's WebGL 2 scenes (run 1's count stands, 65 outputs
  passing; command under "Measurements, run G3b-1").

G3b (what the Direct3D 11 front and the shaders track need from this back end), run 1, 2026-11-18.
Merged origin/phase-2-graphics-rig, -webgpu, -shaders and -direct3d-11 cleanly (the shared line
had nothing new; -direct3d-12 not merged, nothing of it was needed). Done in this run:
- The open findings of G3's passing check (scratch orchestrator/graphics-webgl-round2-open-findings.json):
  - Signed normalised minimum judged wrongly (Chrome): a host write is now judged only on whole
    components (16-bit ones aligned, so bytes 00 80 across two components are not -32768) of the
    rows it writes (the padding between rows and slices is left out). The judgement is cleared
    when the texture is written whole again, when a later write leaves the host copy with no
    minimum while no draw has written the texture, and when a pass clears (or loads as don't
    care) a texture of one subresource. New check
    `signed_normalised_minimum_judged_by_whole_components` (the checker's two cases and three
    clearing cases; Chrome only, natively it reports "not run: Rgba8Snorm does not render here").
  - Checks whose route ran nothing are reported with the reason, not only the absent feature:
    `not_run_because`, and `signed_normalised_texels_drawn_then_copied` natively now reports
    "not run" (it returned silently before). The suite's own early returns are listed under
    "The pass bar" below.
  - The per-draw cost command now has `--desktop` (without it Chrome kept no WebGL 2 context in
    the checker's run), and the spread of medians between runs is recorded under Measurements.
- What the other tracks' status files ask of this back end, each settled (list under "G3b: what
  the fronts ask" below): half floats read back (built), two devices on one thread natively
  (built), GLSL ES 3.10's bit functions and gathers on GLSL ES 3.00 (built, plan 6.5), and the
  rest refused by name or already built.
- Not done yet: the frame-time measurement of a real program through the Direct3D 11 front on
  this back end in Chrome (see Next).

G3 is at its pass bar (item by item below) apart from named known failures that wait on other
tracks, on hardware, or on what WebGL 2 cannot do (see "Not built" under "Done"). Run 18 (the
sixth run of the G3 milestone task) merged origin/phase-2-graphics-webgpu,
origin/phase-2-graphics-shaders and origin/phase-2-graphics-rig (the shared line had nothing new)
and fixed what the independent check of run 17 found:
- Copies from and into signed normalised formats. A copy from `r8snorm` (or any snorm format) into
  another format of its group lost the device as not built. WebGL 2 cannot read their bits: a
  draw samples -128 as -127 does, and `readPixels` returns -128 as -127. Now every signed
  normalised texture keeps a host copy (`HostCopyKept`, src/host.rs), which host writes, buffer
  copies and copies into it fill, so copies out of it come from the host. Without EXT_render_snorm
  (mozangle) no draw writes them, so they also copy out (`COPY_OUT`). With it (Chrome), a pass
  drawing into one marks its host copy behind; a draw writes no -128, so its texels are drawn into
  a texture of the signed integer twin and read back into the host copy before it is read
  (`HostCopyReadBack`), or copied on the GPU by a draw into a destination that renders
  (`CopyThroughBuffer`). Texels written -128 by the host and then drawn over cannot be told from
  -127 afterwards; copying their bits loses the device as not built (named). Copies from 16-bit
  floats into a texture kept on the host now go through the float's integer twin. Checks:
  `copies_within_a_typeless_group` (13 signed normalised pairs, the smallest value in every fifth
  component) and the new `signed_normalised_texels_drawn_then_copied` (Chrome only; natively the
  formats do not render, and it says so).
- A host write covering only part of an index never updated where the largest index may be, so a
  one-byte write completing 0xffff was drawn as a restart. Now every index a write touches is
  re-judged: it may hold the largest value only if every byte the write put into it is 0xff
  (src/restart.rs, unit test `writes_of_part_of_an_index_may_complete_the_largest_value`, and a
  case in `lists_draw_the_largest_index_as_a_vertex`). The ranges are kept in the `rangemap`
  crate's `RangeSet` instead of a hand-written list.
- The per-draw cost scene also draws indexed lists without restart (see "Measurements, run 18").
- The status lists every route that is not built (under "Done"), the licences of image's `exr`
  dependencies, and what the seeded runs of run 17 actually covered.

Run 17 tracked where indices may hold the largest value (WebGL 2 restarts there in every indexed
draw) and rewrote draws that must not restart (`RestartRewrite`), reported the timestamp and
fractional bias checks as not run where their feature is absent, copied within the 10-10-10-2,
R8, RG8 and RG32 groups, and compared BC1 to BC7 with real Direct3D 11 (tests/bc_reference.rs).

Run 16 copied floats within one format by blits where a probe finds they keep every bit, stored
decoded BC4 and BC5 as half floats, and put NaN payloads, infinities and denormals into every
copied float region.

Earlier runs, briefly: run 15 found the fractional viewport comparison read backwards and ran the
comparisons in Chrome; run 14 refused shaders needing absent features when they are made, built
copies from cubes, of rgb32 textures, into half floats and within one subresource, and seeded the
checks; run 13 fixed depth writes and read backs (24-bit words, combined formats,
rounding; `depth-bias_depth` exactly equal to Direct3D 11); run 12 built the extension routes; run 11
fixed layer invalidation in Chrome and the base level patch of mozangle; runs 1 to 10 built the back
end. The git log has each run's commits.

## G3b: what the fronts ask

Read after merging (status-graphics-direct3d-11.md at 66e54fa6, status-graphics-shaders.md at
93fed7c1, status-graphics.md). Each request and where it stands:
- (Run 8) The shaders track's filter_draw losing ANGLE's device (runs 3 to 7, below): built.
  Its programs were copies of a few shaders made again for every draw, each a GL program holding
  about 650 KB of the adapter's memory; shaders of equal GLSL are now one GL shader and their
  programs one GL program, so its 16 seeds in one process draw every comparison natively and its
  recorded stream replays whole in Chrome (Where things stand, Measurements, run G3b-8). Distinct
  large programs by the hundred stay a known failure with no limit to declare.
- (Run 8) Pass bar of G3b, where it stands: every request above built or refused by name; the
  shared suite as before in Chrome and on ANGLE (newest Measurements); slow paths counted as
  listed under Done; the frame-time measurement under Measurements, runs G3b-2 to G3b-7
  (SimpleLightingPC, WebGL 2 28.1 us a draw against WebGPU 53.8 us on the same frames). Known
  failures left: the ATG samples' start-up crash in the system's d3d11.dll, and no real program
  with many draws a frame measured (bgfx draws nothing through the front's WebGL 2 build).
- Reading half floats back (Direct3D 11's mips and targets scenes: a staging texture of a format
  the back end cannot read back): built. `R16Float`, `Rg16Float` and `Rgba16Float` now have
  `COPY_OUT`: read back now, later and into buffers through their bits drawn into the unsigned
  integer twin (`CopyThroughBuffer`, `through_draw` in src/lib.rs), NaN payloads, infinities and
  denormals kept. Check `half_floats_read_back_as_stored`; `copies_within_a_typeless_group` now
  reads its half floats directly. Passes on ANGLE and in Chrome.
- Two devices on one thread natively (Direct3D 11, Wine's tests): built. Every back end call makes
  its own EGL context current first where another back end on the thread made its own current
  (one thread-local compare per call natively, nothing in the browser), and dropping a back end no
  longer terminates the default display the others share. Check
  `two_devices_on_one_thread_keep_their_own_objects`.
- Bit operations (`bfi`, `ubfe` and the like: `bitfieldExtract`, `bitfieldInsert`, `bitCount`,
  `bitfieldReverse`, `findLSB`, `findMSB`), which GLSL ES 3.00 lacks (shaders track; plan 6.5):
  built. The program defines them, for `int`, `uint` and their vectors, with shifts and masks
  (`bit_functions` in src/shader.rs). Check `bit_operations_equal_their_definition`: every
  operation, scalar and vector, signed and unsigned, against WGSL's definition computed on the
  host, offsets and counts past the width included.
- (Run 3) Cube gathers across face edges: the footprint's texels past an edge are sampled by
  direction on the next face; comparison `cube_gathers_across_face_edges_equal_webgpu` (every edge
  of every face, three places each, corners left out) equals WebGPU's own gathers natively (wgpu,
  Direct3D 12) and in Chrome.
- `textureGather` and `textureGatherOffset` (shaders track; plan 6.5): built. Each is rewritten to
  four samples at the texel centres of the footprint at level 0 (`gathers_by_samples`), given the
  bound view's size; cube footprints are sampled by direction, comparisons through the comparison
  sampler. Check `gathers_equal_their_footprints`: 2D with linear filtering and repeat (footprints
  past every edge), with an offset, an integer 2D array with nearest and clamp, a cube inside its
  faces, a depth comparison. Departures: a sampler's minimum level of detail above 0 moves the
  samples off level 0 where a gather stays on it.
- A program GL refuses to link answers device lost at its first draw, not an error (shaders
  track): kept. Links run in parallel and finish at the first draw, which has no error to return;
  the programs that met it were the bit functions and gathers above.
- Texture views (Direct3D 11's views scene, `E_NOTIMPL`): refused by name, `Absent(TEXTURE_VIEWS)`
  at `create_view` and in binding sets; WebGL 2 has no views (plan 4.12: the front keeps a second
  texture, `ReinterpretCopy`).
- Polygon mode (raster scene): `POLYGON_MODE` with WEBGL_polygon_mode, which Chrome has (check
  `polygons_filled_with_lines`) and the mozangle build lacks, so natively it is absent and refused
  by name; `POLYGON_MODE_POINT` absent everywhere.
- `TEXTURE_LEVEL_QUERY` (samplers scene, refused by the front by name): absent, GLSL ES 3.00 has no
  `textureQueryLevels`; fronts bind the `TEXTURE_INFO` variant (plan 4.11).
- The constants scene ("device lost in the back end, not read"): read in this run. The front's
  pixel program declares more uniform blocks than `Limits::uniform_buffers_per_stage` (12 here,
  Direct3D 11 has 14), and GL refuses the link ("FRAGMENT shader uniform block count exceeds
  GL_MAX_FRAGMENT_UNIFORM_BUFFERS (12)"). The limit is declared; past it the front binds the
  shaders track's `constants_as_texture` (`ConstantsAsTexture`), which the front does not use yet
  (for the Direct3D 11 track). The back end now prints why it lost the device on the host's
  error output once (`fail`), and two losses that had no message (a write past a buffer's end, an
  indexed draw without indices) now name the broken rule.
- The samplers scene: the front refuses it by name (`TEXTURE_LEVEL_QUERY`, below), as the log of
  this run's scene shows; nothing of this back end runs.
- An empty `viewports` (shaders track: the interface does not say what it means): the interface
  now says one viewport; this back end loses the device by the interface's rule (`Invalid`, "0
  viewports in one draw") for any other count.
- Read back later through `glGetBufferSubData`, which GLES lacks natively: built earlier (natively
  through `glMapBufferRange`, platform.rs `get_buffer_sub_data`).
- `noperspective` inputs (shaders track: not written for WebGL 2): built in run 5. Runs 1 to 4
  called them "refused by naga's writer", but the front saw only `Invalid` (a front's bug), not an
  absent feature (check of run 4). Now each linear value is written perspective-correct times the
  vertex's w into a varying of its own (`_wc_linear<location>`, the value itself kept for
  transform feedback), and the fragment stage multiplies by `gl_FragCoord.w` (1 / w), or at the
  centroid divides by w carried in a centroid varying beside it: a perspective-correct a·w over w
  is the window-space value exactly, clipped vertices included. Each stage knows its own linear
  locations (WGSL's interpolation is on both sides), so no pair is needed. `linear_by_w` in
  src/shader.rs; unit test `linear_values_are_carried_times_w`; check
  `linear_values_interpolate_in_window_space` (a triangle with w from 0.5 to 3 and one clipped at
  the far plane, centre and centroid, against the window-space value within 1e-4, and the
  perspective-correct value at least 0.05 away somewhere).
- `SAMPLE_MASK` and `SAMPLE_SHADING` (shaders track: "SAMPLE_MASK cannot serve WebGL 2"; run 1 to
  5 gave a wrong reason, check of run 5). Settled in run 6:
  - `SAMPLE_MASK` is the raster block's mask (`RasterDesc::sample_mask`), which GL sets with
    `glSampleMaski`: GLES 3.1, not in WebGL 2 or any WebGL extension (WebGL 2's
    `sampleCoverage` takes a fraction, not bits). So it stays absent everywhere, and a front with
    a mask clearing samples binds the `SampleMaskInShader` variant, which needs `SAMPLE_SHADING`.
  - `SAMPLE_SHADING` (programs run per sample, read and write the mask, values interpolated per
    sample): built where the context has OES_sample_variables and
    OES_shader_multisample_interpolation, which Chrome 157 on the RTX 3080 has and mozangle lacks.
    naga writes the sample number and mask under OES_sample_variables' directive itself; its
    `sample` qualifier it writes only from GLSL ES 3.20, so the module is written with those
    values at the centre and `sample` is put back on both stages' declarations under
    OES_shader_multisample_interpolation's directive (`qualifiers_written` in src/shader.rs). The
    web platform now enables these extensions with `getExtension` (glow does not). Check
    `programs_run_per_sample_and_write_the_sample_mask` (four samples: each run sees its own
    number, only its own coverage bit, its own interpolated place, and a written mask writes only
    the samples it names): passes in Chrome; natively not run (absent). Unit test
    `linear_and_per_sample_values_take_their_qualifiers`. So on Chrome the Direct3D 11 front's
    sample masks are served through `SampleMaskInShader`; natively (mozangle) the front reports
    single-sample targets for them, as the interface says. Still not run anywhere here: the suite's
    `programs_run_per_sample_and_read_samples`, which also needs `MULTISAMPLE_READ` (WebGL 2 has
    no multisampled textures).
  - With `SAMPLE_SHADING` present the suite's `a_fragment_covering_no_sample_writes_nothing` ran
    in Chrome for the first time and failed: GL ignores a program's sample mask on a
    single-sample target, where Direct3D and WebGPU drop a fragment whose mask clears sample 0.
    Now after the mask is written a fragment with `gl_NumSamples == 1` whose mask clears bit 0 is
    discarded (`single_sample_masks`); the test passes in Chrome.
- (Run 6, check of run 5) Linear values where the context has NV_shader_noperspective_interpolation
  (Chrome 157 on the RTX 3080): declared `noperspective` under its directive and read as they are,
  no extra varying, no multiply. The carried route (`linear_by_w`, below) stays for contexts
  without it (mozangle; any browser lacking it), and Chrome's fourth suite route switches the
  extension off so both routes run in Chrome. `linear_values_interpolate_in_window_space` passes
  on both routes in Chrome and natively.
- (Run 6, check of run 5) Gathers through a sampler whose minimum level of detail is above 0: GL
  clamped the gather rewrite's `textureLod(..., 0.0)` to the minimum, so it sampled level 1 with
  level 0's coordinates. Now a sampler with a minimum above 0 keeps a twin whose minimum is 0, and
  a texture unit that gathers read binds the twin (`gathers_at_level_zero`, `SamplerObj::level_zero`):
  the gathers' own unit where only gathers read the pair, else a second sampler uniform of its own
  that the gathers read, so other samples through the pair keep the minimum. New comparison
  `gathers_through_a_sampler_with_a_minimum_level_equal_webgpu` (8 by 8, four levels, red telling
  every texel of every level apart; minimum 0 and 1; gathers alone and beside a sample through the
  same pair): equal to WebGPU natively (wgpu, Direct3D 12) and in Chrome. Cost: one sampler object
  more per sampler with a minimum above 0, and a texture unit more for a pair both gathered and
  sampled; nothing per draw.
- Fine derivatives: refused by name in run 5 (before, the front saw `Invalid`, as for
  noperspective). Interface change `Features::FINE_DERIVATIVES`; this back end lacks it (GLSL ES
  3.00 has only `dFdx` and `dFdy`, which may be coarse), and `create_shader` answers
  `Absent(FINE_DERIVATIVES)` for a program using `dpdxFine`, `dpdyFine` or `fwidthFine`
  (`needs`). Coarse ones are made (GLSL's plain derivatives may be coarse). Checked in
  `absent_features_are_refused`. Direct3D's levels below 11_0 have no fine derivatives, so the
  front at 10_0 loses nothing; the WebGPU back end should declare the bit (WGSL always has them).
  A shader naga still cannot write as GLSL ES 3.00 is `Invalid` (a front's bug), and naga's
  reason is now printed on the host's error output.
- (Run 5, from the check of run 4) Special float values held as constants: ANGLE folds a float
  made from constant bits and writes it into HLSL as a decimal literal, so negative zero lost its
  sign in Chrome and natively, and natively (mozangle) a vector of infinity, -0, 2^-24 and 65504
  drew infinity in all four. Check `special_float_values_keep_their_bits` (+inf, -0, -inf, 1.5 into
  `rgba32float` by four routes) failed before the fix: natively the constant route gave
  [7f800000 x4], and the variable route (the DXBC converter's form for non-finite constants) and
  a `-0.0` literal gave +0 for -0, on ANGLE and in Chrome; the uniform route was exact. Now every
  `uintBitsToFloat`, `intBitsToFloat` and `-0.0` outside global constants reads its bits through
  an exclusive or with a uniform that is never set (0 after the link), which neither ANGLE nor
  the HLSL compiler can fold (`opaque_float_bits`, unit test `float_bits_are_kept_from_folding`).
  The check passes on ANGLE and in Chrome. Cost: one integer operation per such call (not
  measured). Left: global constants (Next 3).
- (Run 2) The shared line's three new suite tests (copies within one depth or stencil
  subresource, the occlusion counter, every vertex format, which the shared status says GL ES may
  lack): pass on ANGLE and in Chrome with every extension; on the routes without
  EXT_clip_control they stop at the suite's own raster block like the 25 known.
- (Run 2) The frame-time measurement through the Direct3D 11 front: built (Measurements, run G3b-2).
- (Run 3, from check9) The formats scene's `bgrx8_unorm` (`DXGI_FORMAT_B8G8R8X8_UNORM`): built
  (above, "Where things stand"). Run 1 listed the scene's stop as the front's; it was this back
  end's caps, which gave BGRX no use, so the front's `CheckFormatSupport` answered `E_FAIL`.
- (Run 3) Then `b5g6r5_unorm` and `rg11b10_float` of the same scene, which this back end could
  render but not copy out: built (above).
- (Run 3, from check9) The shaders track's `tests/filter_draw.rs` seed 14 (anisotropy 4), which
  in its run 15 (2026-11-17 18:34, `run15/filter12.log`, seeds 1 to 16 in one process) lost
  ANGLE's device in a WebGL 2 draw: "Internal D3D11 error: HRESULT: 0x887A0005 (removal reason:
  HRESULT: 0x887A0005): Failed to map D3D11 resource". The display resets log has no reset then
  (the day's resets were 14:40 to 17:22, 19:35 and 20:37), so it removed ANGLE's device without a
  driver reset. Rerun alone at this run's code (seed 14, one case, `gpu-job --alone`, scratch
  g3b3/seed14-1.log): 48 comparisons, 0 failed, 32 known anisotropic failures, no device lost;
  WebGL 2 included (dxbc and dxil routes, R32_FLOAT and RGBA8). Then seeds 1 to 16 in one
  process, as run 15 had them (`gpu-job --alone`, 14:10 to 14:14 desktop time, scratch
  g3b3/seed1-16.log): ANGLE's device was removed again, this time around case 4 (anisotropy 1),
  with the same message ("Failed to map D3D11 resource", removal reason 0x887A0005), and this back
  end reported it ("wcontainers webgl: device lost (Error): GL error 0x502"; the test then
  panicked on `DeviceLost`). No display reset and no Application or System event in that window,
  and the queue was not held. So it is not seed 14's sampler state: a process that draws many
  cases loses ANGLE's Direct3D 11 device at some point, without a driver reset. The 32
  anisotropic differences are the shaders track's known failure (its tap footprint), not this
  back end's.
  (Run 4) Cause narrowed: the test never calls `destroy`, so every case leaves its WebGL 2 programs
  alive (WebGPU's objects end when their handles drop, this back end's only at `destroy`, as the
  interface says), and with about 400 of its programs alive mozangle's ANGLE removes its Direct3D 11
  device (run 5: not a bound on the count; the check of run 4 kept 800 small programs alive
  without a loss, so it depends on the programs; see Known failures). A copy of the test's WebGL 2 draws (three per comparison, as at worst; seeds 1 to 16,
  1,152 draws; scratch g3b4/webgl_filter_diag.rs, put into the shaders crate's tests for the
  runs only and removed after; `gpu-job --alone`, desktop 14:56 to 15:07, nothing else on the
  GPU), logs g3b4/diag1.log and diag2.log:
  - WebGL 2 alone, nothing destroyed: device lost between draw 360 and 432 (case 6), "Failed to
    map D3D11 resource", GL error 0x502, as in run 3.
  - WebGL 2 alone, everything destroyed after each draw: 1,152 draws, no loss.
  - Every device of the original test (wgpu's Direct3D 12, real Direct3D 11 and WARP), nothing
    destroyed: lost at the same place, so the other devices play no part.
  - Everything destroyed but the textures and views: no loss; but the buffers and binding sets: no
    loss; but the programs and their shaders: lost at the same place.
  So the shaders track's test needs to destroy what it makes, as a front does (the shaders
  track's to change: `ours` in its tests/filter_draw.rs, and any test drawing the same way); with
  that, the 16 seeds' WebGL 2 draws pass here. ANGLE's Direct3D debug layer could not be used:
  ANGLE reports "Failed creating Debug D3D11 device" on the desktop (the Graphics Tools are not
  installed, and installing a Windows feature is not ours to do). How many live programs Chrome's
  own ANGLE holds was not measured (Next).
- (Run 3, from check9) A program past the uniform block limit (the constants scene): refused at
  `create_shader` with `Invalid` naming `Limits::uniform_buffers_per_stage`, so the front sees the
  broken rule when it makes the shader, not a lost device at the draw. The scene still needs the
  front to bind `constants_as_texture` past the limit (the Direct3D 11 track's).
- (Run 3) The shared line's two new suite tests (copies within one 3D or compressed subresource,
  an indexed draw past a stream's end): pass on ANGLE and in Chrome; the first needed the 3D copy
  fix above.
- (Run 4, shaders status run 32) A depth texture both compared against and read plainly in one
  program (a shadow map; the shaders track's `ps_shadow`; "on WebGL 2 such programs stay
  refused", naga's GLSL writer refusing a load from a depth texture): built. Each plain read
  (sample without a reference, load, gather) of a depth texture the program also compares against
  becomes a call to a function of its own reading a float copy of the texture's variable at a
  binding of its own (`plain_depth_reads` in src/shader.rs; GL gives a depth texture's depth in
  red with comparison off). Check `depth_textures_compared_and_read_plainly` (comparison, sample,
  load and gathers with and without an offset, in a function and in a branch, against the texels)
  passes on ANGLE and in Chrome; unit test `plain_depth_reads_write_through_a_float_copy`. Still
  refused, by name at `create_shader`: a depth texture read plainly through a function argument,
  and an offset that is not a vector of constants (neither is met by the converters' output).
- (Run 4) The Direct3D 11 front's scenes through its WebGL 2 build, all of them (scratch
  g3b4/d3d11-scenes1.log): 88 outputs pass, 27 fail, 30 absent (the front's run 21: 67, 52, 27).
  Every failing scene stops in the front, none in this back end (each scene's log read):
  raster ("wireframe without the back end's polygon mode": `POLYGON_MODE` absent on mozangle,
  present in Chrome), views (`TEXTURE_VIEWS` absent), constants (this back end's early refusal past
  `Limits::uniform_buffers_per_stage`; the front's constants through a texture are not built),
  queries ("counting the samples of a pixel shader variant": `OCCLUSION_COUNT` absent, WebGL 2's
  occlusion queries answer only any or none), geometry and streamout ("geometry shaders and stream
  output without the back end's compute programs": plan section 7 routes them through this back
  end's transform feedback, `TRANSFORM_FEEDBACK` declared and tested here, which the shaders
  track's stage helper does not use yet). Absent: the snorm targets (no EXT_render_snorm in
  mozangle), feature levels 10_1 and 11_0, and min/max filtering.

## The pass bar, item by item (plan section 12, G3)

The shared suite's own early returns (G1's crate): five of its tests return without drawing on
WebGL 2 and both hosts count them as passes: `fragments_write_storage_buffers`,
`vertices_pulled_from_storage_match_the_fetched_ones`,
`typed_buffers_through_storage_read_their_views_place_and_length` (no storage),
`programs_run_per_sample_and_read_samples` and `a_fragment_covering_no_sample_writes_nothing` (no
sample shading or multisample reads). `instances_start_at_the_first_instance` and
`indexed_strips_restart_and_add_the_base_vertex` use bases of 0 without `BASE_VERTEX_INSTANCE`;
non-zero bases are covered by this crate's `vertex_and_instance_numbers_include_the_bases`. So of
the suite's pass counts below, five ran nothing here.

Where each item is shown; "check" is a function of examples/common/checks.rs (or extensions.rs), run
by both hosts; "against" is a comparison of examples/common/against.rs, run natively by
tests/against_webgpu.rs and in Chrome by browser/against.mjs.
- The shared suite (with this crate's checks) in Chrome's WebGL 2: every test passes with every
  extension and on the reduced route, apart from those reported "not run" with their reason; the
  current counts are in the newest "Measurements, run G3b-n" section. Without EXT_clip_control:
  the known failures of the suite's own raster blocks (`Absent(CLIP_DEPTH_ZERO_ONE)` or
  `Absent(PROVOKING_VERTEX_FIRST)`).
- The absent list of 4.11 declared: check `absent_features_are_refused` (every absent feature
  left out of the caps and refused through `Absent`: blocks, samplers, bindings, query sets, and
  since run 14 shaders needing them, 14 shader cases). Every slow path counted: each route calls
  `slow.hit`; checks assert the counters where a route must or must not be taken (fans and loops,
  BC decoding, stencil copies, copies within one subresource). Interface rules a draw or clear
  breaks: check `broken_rules_lose_the_device`.
- Cube, array and 3D textures with the GL target of `own_view`: against `cube_array_and_3d_textures_equal_webgpu`.
- BC and RGB9E5 read back as written: check `block_compressed_and_shared_exponent_textures...`.
  Since run 17 also sampled against real Direct3D 11 (tests/bc_reference.rs): exactly equal
  through GL's compressed formats; the decoded route is a known failure (the GPU's own decode).
- Stencil sampled with and without WEBGL_stencil_texturing, equal answers: check
  `stencil_sampled_and_read_back`, run on both routes in Chrome (ANGLE natively lacks the extension).
- Copies within a typeless group exact: checks `copies_within_a_typeless_group` (16-bit and 32-bit
  floats with NaN payloads, denormals and infinities in every third component since run 16, rgb32
  since run 14, and copies within one format since run 16), `copies_within_one_float_format_take_the_probed_route`, `cube_faces_copied_between_typeless_formats`
  (run 14, depth included), `copies_between_depth_and_colour_formats` (run 13),
  `bgra_textures_keep_their_byte_order`, and within one subresource `copies_within_one_subresource`
  (run 14) and the suite's `a_copy_within_one_subresource_moves_the_region`. Since run 17 the
  typeless check also copies within the 10-10-10-2 group and the R8, RG8 and RG32 groups. Since
  run 18 also from and into the signed normalised formats (their smallest value included), and
  `signed_normalised_texels_drawn_then_copied` in Chrome.
- `generate_mips` exact against G2 for `r32float` without OES_texture_float_linear and an integer
  twin of a 16-bit normalised format: against, route against route byte-equal; against WebGPU's
  hardware filter a known failure (G1's helper). The integer twin where box averages are exact ties
  (run 16): equal natively, one value of 128 one step off in Chrome, a known failure (G1's helper).
- A fractional viewport against G2's native one: against
  `a_fractional_viewport_in_the_program_equals_webgpu_native` draws as a Direct3D front does
  without `FRACTIONAL_VIEWPORT` (whole-pixel viewport, adjusted positions, Direct3D's implicit
  scissor), clipped at the viewport's edges, written in WGSL here; equal on both routes natively
  and in Chrome since run 15; G6's crates/wcontainers-shaders/tests/variant_draw.rs runs
  the real `VIEWPORT_ADJUST` variant on this back end. `FRACTIONAL_VIEWPORT` absent here, check
  `fractional_viewports_are_refused`.
- A buffer used as indices and vertices, written by host, copy and transform feedback, drawn both
  ways: scene `quads-twin_copy`, check `captured_indices_drawn_both_ways`.
- 8-bit indices: check `vertex_and_instance_numbers_include_the_bases`.
- `capture_vertices` with and without rasterising: checks `vertices_captured_with_and_without...`
  and `strips_fans_loops_and_indexed_draws_captured_in_list_order`.
- `timestamps_disjoint`: check `timestamps_and_their_disjoint_flag` refuses it as absent here (0
  timestamp bits) and reports "not run" (since run 17; it passed silently before); the present
  route has not run on any GPU (known failure, hardware). The Linux
  laptop is the rig's CPU-only machine, so its Chrome has no GPU to try it on.
- The 24-bit depth probe: at creation; `DEPTH24_UNORM` set in Chrome and on ANGLE (Measurements).
- `vertex_index` and `instance_index` with bases under `BaseVertexRebind`, indexed and not:
  check `vertex_and_instance_numbers_include_the_bases`.
- A `Discard` write then `NoOverwrite` writes above it: check `discard_then_no_overwrite_writes`.
- Depth and stencil read back by `DepthReadByDraw`, with and without WEBGL_stencil_texturing: checks
  `depth_and_stencil_read_back` (exact since run 13) and
  `depth_and_stencil_written_read_back_as_written` (run 13, 2D arrays and cubes).
- A copy between two multisampled textures whose resolve equals the source's: check
  `multisampled_copies_resolve_as_their_source`.
- Both window-origin routes, each equal to G2's (culled triangle, `gl_FragCoord` gradient, mip
  chain, read back): against `both_window_origins_equal_webgpu`, on ANGLE with and without
  EXT_clip_control, natively and (since run 15) in Chrome against its WebGPU.
- A fractional `depth_bias` and `ConstantAlpha`: checks `fractional_depth_bias` (the probe finds
  ANGLE rounds the units, so `FRACTIONAL_DEPTH_BIAS` is absent and fronts bias in the program; the
  check tests the refusal and reports "not run", natively and in Chrome, so the biasing route has
  not run here) and `constant_alpha_blends`.
- The suite natively on ANGLE: every test and check with every extension (see "Measurements, run
  17"). A check whose feature is absent runs only its refusal and reports "not run": natively it
  passes and prints "not run:" (before G3b "not run, feature absent:"; all.cmd lists those lines), in Chrome it is counted
  as "not run". Since run 17 that includes `timestamps_and_their_disjoint_flag` and
  `fractional_depth_bias`, on both hosts.
- Primitive restart only where asked (found by run 16's check, not a pass bar item): check
  `lists_draw_the_largest_index_as_a_vertex`.
- Per-draw cost against G2 on one scene: Measurements (runs 3, 5, 11 and 14; since run 18 also
  indexed lists without restart).

## Done

- `WebGl` implements `wcontainers_gpu::Backend` on glow 0.18, the same code in Chrome (wasm32) and
  natively on ANGLE:
  - Buffers (one GL buffer per WebGL type, indices and other data apart, made at creation by
    usage), 2D, 2D array, cube and 3D textures made with the GL target of `own_view` by
    `texStorage`, texture writes by `texSubImage` with row length and image height, samplers,
    binding sets (uniform ranges, textures, samplers), passes on cached framebuffers with
    `clearBuffer*` loads and `invalidateFramebuffer` for `DontCare` and `Discard`, direct draws
    (arrays and elements, instanced, 8, 16 and 32-bit indices), `copy_buffer` between GL buffers of
    one type, read backs later (pixel pack buffer and a sync object) and now (`readPixels`,
    `getBufferSubData`), submit (with a `getError` check that turns a GL error into device loss) and
    fences.
  - State applied only where it changed (plan 4.4): each draw compares block handles with the last
    applied ones; for a changed block only the GL values that differ from what GL holds are set
    (state.rs, `GlValues`). Bindings are walked only when the program or a set
    changed, and each texture unit, sampler and uniform block point is rebound only when it differs.
    Vertex attributes are respecified only when the layout, a stream's buffer or its offset changed.
    Clears open the masks and scissor through the same record, and mark the blocks for reapplying.
    Creating a texture or writing one binds it on unit 0, so the next draw walks its bindings again
    (fixed in the second run: before, a draw with the same program and set after an upload sampled
    the uploaded texture). No allocation per draw or per pass.
  - Shaders: naga 30 writes GLSL ES 3.00 (WebGL flavour, `FORCE_POINT_SIZE`); each combined sampler
    gets a unit and each uniform block a binding point when the link is read; programs are linked
    once per (vertex, fragment) pair and counted per handle. `instance_index` reads
    `gl_InstanceID + naga_vs_first_instance`, set per draw only when it changes. Base vertex and
    first instance move the vertex streams (`BaseVertexRebind`, counted).
  - Window origin (plan 6.5): with EXT_clip_control the device draws upper-left and zero-to-one
    (minus-one-to-one per raster block that asks for it); without it the GLSL's last vertex step
    negates `gl_Position.y` and the front face is swapped, and only minus-one-to-one is taken. Both
    routes draw every picture exactly; on the second the test programs rewrite clip z as 2z - w, as
    a front's `ClipDepthRewrite` does.
  - Refused as absent at creation: logic ops, per-target blends, second-source factors, constant
    colour with constant alpha in one blend, depth bounds, per-face stencil masks, fill modes, first
    provoking vertex, depth clamp, bias clamp, wide lines, partial sample masks, storage bindings,
    border and mirror-once samplers, compute, texture views beyond the base ones.
  - Refused as `Absent(CLIP_DEPTH_ZERO_ONE)` without EXT_clip_control: a raster block with
    `depth_minus_one` false (the interface's rule; the front rewrites clip depth instead).
  - Second run: texture copies by `copyTexSubImage2D/3D` from a read framebuffer (colour formats a
    framebuffer can read); buffer-to-texture copies through the pixel unpack buffer; texture-to-
    buffer copies by `readPixels` into the pixel pack buffer, with `PACK_ROW_LENGTH` for the pitch;
    buffer clears as host writes of the pattern into every GL buffer of the buffer; read backs of
    every format `readPixels` reads exactly (format.rs `read_as`: 8-bit normalised read as RGBA
    bytes, integers as RGBA 32-bit integers, 32-bit floats as RGBA floats), kept narrow on the host.
  - Index twins (plan 4.6): a GPU write that reaches only one of a buffer's two GL buffers marks the
    other stale for that range; a draw reading it as indices (or other data), a copy from it and a
    read back first refresh it with a read back (`IndexTwinReadBack`, counted). Draws look only
    when some buffer is stale (a device count). Every GPU write refreshes its destination first, so
    at most one side is stale.
  - Programs: `linkProgram` is issued at creation and read at the first draw (plan 5.8); that draw
    counts `PipelineMiss` when it must wait (always without KHR_parallel_shader_compile, otherwise
    when `COMPLETION_STATUS_KHR` is still false). A failed link loses the device and leaves GL's log
    in `last_log`.
  - `vertex_index` with a base vertex: naga's `naga_vs_base_vertex` is set per draw, only when it
    changes, as `naga_vs_first_instance` is (check `vertex_and_instance_numbers_include_the_bases`).
  - Run 3: triangle fans and line loops drawn as `TRIANGLE_FAN` and `LINE_LOOP` (`draw_fan_or_loop`
    overridden, `FAN_AND_LOOP` declared); guest indices go once into the index ring. A vertex stream
    of stride 0, and one stepping per instance at rate 0, reads one element for every vertex and
    instance (instance divisor 2^31; GL's stride 0 means tightly packed). `PROVOKING_VERTEX_FIRST`
    with WEBGL_provoking_vertex (`GL_ANGLE_provoking_vertex` natively), set per raster block only
    when it changes. Buffer clears write one 256-byte block of the pattern from the host into every
    GL buffer of the buffer and double it by copies within each (`PatternClearCopied`, counted; no
    host copy of the whole range). A native buffer read whose map fails, a WebGL read that raises an
    error, and a read back whose sync object could not be made lose the device instead of answering
    with stale bytes. `max_point_size` is 1 (programs write `gl_PointSize = 1`; `POINT_SIZE` absent).
  - G3 run 1: each block is converted to the GL values it stands for when it is made (state.rs
    `GlBlend`, `GlDepthStencil`, `GlRaster`, `GlLayout`, plan 4.4), so a draw that applies a changed
    block only compares and sets values. A draw whose binding sets lack a resource its program reads
    (or hold the wrong kind) loses the device with the slot named in `last_log` instead of reading what
    the unit held before. Surfaces: one per back end (a WebGL context draws into its one canvas;
    natively the EGL surface); `configure_surface` sizes the canvas's drawing buffer (natively a new
    pbuffer) and makes an `rgba8unorm` texture that `current_texture` hands out; `present` copies it
    into the drawing buffer by `blitFramebuffer`, upside down (GL's row 0 is the bottom). `vsync` changes
    nothing: the browser shows the canvas at the display's rate and presenting never waits. Depth read
    backs (`DepthReadByDraw`, counted): a full-screen triangle fetches each depth texel of a 2D or 2D
    array texture and writes its stored value (16-bit unorm, 24-bit unorm in a 32-bit word, or a
    float's bits) into an `r32uint` texture, which reads back like any colour texture; it draws with
    GL's lower-left origin so rows match on both window-origin routes. The depth of depth-stencil
    formats reads the same way; their stencil with WEBGL_stencil_texturing (enabled by the back end;
    `GL_ANGLE_stencil_texturing` natively, which mozangle 0.7.1 lacks), fetched as unsigned integers
    with the texture's `DEPTH_STENCIL_TEXTURE_MODE` set to `STENCIL_INDEX` for the draw and put back.
    Views in the sRGB twin: the
    twin is a second GL texture of the same shape, made at the first such view; each side records
    when it was written (passes drawing into it, writes, copies) and is brought up to date from the
    other before it is read, sampled or written, every level and layer through a pixel pack buffer
    (`CopyThroughBuffer`, counted). Draws look at sampled textures only while a twin exists. Read backs
    waited for in the browser are ready at once (WebGL updates a sync object only between tasks).
  - What is not built loses the device when reached (`LossReason::Error`, `Err(DeviceLost)` where
    the call returns a result) with the missing piece named in `WebGl::last_log()`, never a panic.
    Not built today (every `unbuilt` call in src/lib.rs, 19 sites, as of G3b run 5):
    - copies of multisampled depth, stencil and sRGB textures (`copy_samples`);
    - sRGB twins of formats `readPixels` reads wider than stored (no format with an sRGB twin is
      read that way today);
    - depth written into a format no depth write route takes (`write_depth`);
    - copies into a texture kept on the host from a format that is neither read back nor a 16-bit
      float (no such pair in one typeless group today);
    - the bits of signed normalised texels written -128 by the host or a copy and drawn over after
      (Chrome only): a draw reads -128 as -127, and nothing tells which texels the pass covered;
    - copies within a format that neither renders nor reads back nor is kept on the host, and
      copies between depth and colour formats no draw carries (`R11G11B10` without
      EXT_color_buffer_float is one);
    - depth and stencil views of one texture fetched by one draw (Direct3D 11 allows it; GL fetches
      one of the two per texture at a time);
    - negative stream offsets after a base vertex;
    - draws, fans and loops without restart whose 32-bit indices hold both 0 and 2^32 - 1, or whose
      lowered indices need a base vertex past 2^31 - 1;
    - captures (`capture_vertices`) reaching below vertex 0 by a negative base vertex, or past
      vertex 2^31 - 1;
    - (G3b) reading back a format through its integer twin (`bits_by_draw`: 16-bit floats and
      R11G11B10) where the draw copying its bits into the twin is not built; every format with a
      twin has that draw today, so no call reaches it;
    - (G3b) reading a signed normalised texture's drawn texels into its host copy where the draw
      into its signed integer twin is not built ("draws from this signed normalised format");
    - (G3b) a pass drawing into BGRX and formats with alpha together without
      OES_draw_buffers_indexed (BGRX keeps alpha one by the colour mask of its targets; Chrome and
      mozangle both have the extension).
    `fetches of multisampled textures` also reads "not built", but needs `MULTISAMPLE_READ`, which
    is absent, so no valid call reaches it.
- platform.rs: the context. In the browser `WebGl::on_canvas(WebGl2RenderingContext, off)`;
  natively `WebGl::on_angle(dir, off)` loads `libEGL` from `dir` with khronos-egl (EGL 1.0 calls only)
  and makes an OpenGL ES 3.0 context current on a 1 by 1 pbuffer. `off` names WebGL extensions to
  treat as missing, so tests take the routes of browsers without them. `clipControlEXT` (which glow
  lacks) is called through the extension object in the browser and `glClipControlEXT` natively.
  WebGL extension names map to ANGLE's `GL_` names in one table. GLES has no `glGetBufferSubData`,
  so natively buffers are read by `glMapBufferRange` for reading (`platform::get_buffer_sub_data`);
  in the browser by WebGL's `getBufferSubData`.
- Test programs (examples/): `common/scenes.rs` draws each scene through the interface, generic over
  `Backend`, with the rig's own scene data (scene.rs included by path) and WGSL programs
  (triangle.wgsl from the rig, quads.wgsl and instanced.wgsl here, as the rig's HLSL);
  `webgl-scenes` draws natively on ANGLE and checks pictures; `browser` is the same for WebAssembly,
  driven by browser/run.mjs and page.html. The list of pictures (`Change::PICTURES`) is shared by
  both: the three scenes; two altered quads that must fail (blend factor One, depth test Greater);
  and four routes that must still equal the reference: `stale_unit` (the quads drawn into another
  target, then a new texture made and written magenta, then the same draw with the same program and
  set), `twin_copy` (the index buffer made INDEX|VERTEX and given its indices by `copy_buffer` from a
  data buffer), `base_vertex` (the quads after four other vertices, drawn with base vertex 4) and
  `clear_near` (the triangle, clip z 0, over depth cleared to 0.25 and tested Less). Run 3 added
  (`Expected` says what each must equal): `quads-fan` (two fans from guest 16-bit indices with a
  restart between) and `quads-fan_sequential`, equal to Direct3D's quads; `triangle-line_loop`, equal
  exactly to `triangle-line_strip` (indices 0, 1, 2, 0) drawn in the same run; `triangle-stride_zero`
  (colour from a second stream of stride 0 holding red) and `triangle-flat_red` (colour flat from
  the first vertex, red, where the first may provoke, else from the last with the vertices rotated),
  each equal to Direct3D's triangle with every covered pixel red; `triangle-clear_rects` (the
  triangle, then a 64-pixel square cleared red by `clear_rects`, which draws with the interface's
  built-in programs), equal to Direct3D's triangle with that square red. Every fan or loop picture
  also fails if `FanConverted` or `LoopConverted` was counted while `FAN_AND_LOOP` is present.
- tests/suite.rs: the shared suite (`wcontainers_gpu_test::suite!`) and the checks of
  examples/common/checks.rs on ANGLE, Windows only. examples/browser.rs is the suite's host in
  Chrome: browser/suite.mjs opens browser/suite.html once per test (a failing test panics and the
  module traps), each device on a new canvas, `Host::shown` reading the canvas through a 2D canvas.
  The list of tests comes from the native suite's `--list`, so a test the browser host lacks fails.
- examples/common/checks.rs: `vertex_and_instance_numbers_include_the_bases` (points writing
  `vertex_index`, `instance_index` and the elements a per-vertex and a per-instance stream read into
  an `rgba32uint` target; non-indexed and 8, 16 and 32-bit indexed instanced draws with first vertex,
  base vertex and first instance), `discard_then_no_overwrite_writes` (a read back recorded before a
  `Discard` write of part of a buffer sees the old bytes; `NoOverwrite` writes above the part land),
  `depth_and_stencil_read_back` (every depth format with `COPY_OUT` cleared to 0.25 and stencil 0x5a,
  the middle 2 by 2 set to 0.75 and 0xc3 by `clear_rects`, read back as stored: unorm depth within
  one step, float depth and stencil exactly).
- browser/chrome.mjs: what run.mjs and suite.mjs share. Chrome is launched again (up to three times)
  when no WebGL 2 context survives half a second in ten tries, which once made the whole check fail.
- G3 run 2:
  - Stencil read backs without WEBGL_stencil_texturing (`StencilCopiedByDraw`, plan 4.12): eight
    full-screen draws into an `r8unorm` texture, each passing the stencil test only where its bit is
    set and adding 2^k/255 by blending with the constant colour; the depth fetch program then reads
    it with a scale of 255 into the `r32uint` texture read back. Stencil views: with the extension
    the depth-stencil texture itself, its `DEPTH_STENCIL_TEXTURE_MODE` set when a draw binds the view
    (a draw fetching both aspects of one texture loses the device as not built); without it an
    `r8uint` copy of the whole texture, made at the first stencil view and filled by the bit draws
    whenever the stencil changed since (tracked like the sRGB twin). Depth-stencil formats now have
    `SAMPLE` and `COPY_OUT` on every route.
  - Copies (`copy_texture`): equal renderable formats by `copyTexSubImage` as before; depth and
    depth-stencil formats to themselves by `blitFramebuffer`; formats of one typeless group through a
    pixel pack buffer where the source reads back as stored (8-bit normalised), else by a draw that
    fetches each texel, recovers its bits and writes them as the destination's value (src/convert.rs:
    8, 16 and 32-bit integers, 16 and 32-bit floats, 8-bit unorm, 10-10-10-2), both counted
    `CopyThroughBuffer`. Half floats are taken apart by hand so NaN payloads survive. Signed
    normalised sources (whose -128 samples as -127 does), sRGB destinations by draw, packed floats and
    cube sources by draw, and depth to colour lose the device as not built.
  - The 24-bit depth probe (plan 4.11) when the back end is made: a triangle at depth 0.1 into a 1 by
    1 `depth24plus` texture without and with a constant bias of one unit, each fetched as float
    bits; one 24-bit step apart sets `DEPTH24_UNORM`, one float step leaves it clear, anything else
    loses the device with the depths in `last_log`.
  - Textures read only by loads (a program's `textureLoad` with no sampler) are bound with the back
    end's nearest sampler, whose minification filter is `NEAREST_MIPMAP_NEAREST`: before, integer and
    depth textures were incomplete under the texture's default filters and read zeros, and fetches
    reached only the base level.
  - examples/common/kit.rs: small drawing helpers generic over the back end, for the checks and the
    comparisons. New checks (examples/common/checks.rs, run in Chrome and on ANGLE):
    `stencil_sampled_and_read_back` (each pixel's own stencil value, sampled through a stencil view
    and read back, twice with the stencil changed between), `copies_within_a_typeless_group` (13
    pairs, every bit pattern from a fixed xorshift sequence, two layers, a region moved),
    `loads_count_levels_from_the_view` (loads and `textureDimensions` of every level of views starting
    at levels 0, 1 and 2), `fractional_viewports_are_refused` (without `FRACTIONAL_VIEWPORT` a draw
    with a viewport at a fraction of a pixel loses the device as a broken rule, naming the viewport,
    instead of being rounded as before).
  - tests/against_webgpu.rs (Windows): the same drawings on ANGLE, with and without
    EXT_clip_control, and through the WebGPU back end on wgpu's Direct3D 12, read back and compared
    byte for byte: `both_window_origins_equal_webgpu` (a culled one-sided pair of triangles, a
    `gl_FragCoord` gradient, the gradient drawn into each level of a mip chain),
    `cube_array_and_3d_textures_equal_webgpu` (a cube sampled towards 16 directions, a 2D array and a
    3D texture loaded), `generate_mips_of_r32float_without_float_filtering_equals_webgpu` (with and
    without OES_texture_float_linear) and `generate_mips_of_an_integer_twin_equals_webgpu`
    (`rg16unorm` values stored as `rg16uint`).
- Independent check, all done on 2026-11-17: drew the three scenes and compared (exactly equal, both
  on ANGLE and in Chrome, with and without EXT_clip_control); the quads with the colour blend's
  destination factor One fails (25016 pixels differ, largest 38) and with the depth test Greater
  fails (25016 pixels, largest 204), on both; Chrome's WebGL 2 renderer (unmasked, through
  WEBGL_debug_renderer_info) is "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 (0x00002206) Direct3D11
  vs_5_0 ps_5_0, D3D11)" and chrome://gpu lists the same GL_RENDERER; the capability list was read
  against the WebGL 2 specification, GLES 3.0's format table and the extensions Chrome reports (see
  "Capabilities").

- G3 run 3:
  - Occlusion queries (`QueryKind::OcclusionAny`, `ANY_SAMPLES_PASSED`): a set holds one GL query
    per index, made at its first begin; a read back holds the query objects it reads, so a query
    begun again before the read finished gets a new one. `resolve_queries` is the interface's
    default (`QueryResolveWait`). `Occlusion` (exact counts) is `Absent(OCCLUSION_COUNT)`: GL ES
    answers only whether any sample passed. WebGL hands out query results only between tasks, so
    `wait_read` of queries suspends the device thread until the browser's next task through JSPI:
    the back end imports `next_task` from the module `wcontainers-device`, which the host supplies
    as a `WebAssembly.Suspending` function (browser/device.js), and the host calls the code that
    waits through `WebAssembly.promising` (suite.html calls the plain export `suite_run`). Natively
    the wait spins on `QUERY_RESULT_AVAILABLE`.
  - Timestamps (`write_timestamp` by `queryCounterEXT`, `timestamps_disjoint` from
    `GPU_DISJOINT_EXT`), declared only where EXT_disjoint_timer_query_webgl2 is present and
    `QUERY_COUNTER_BITS_EXT` for `TIMESTAMP_EXT` is above zero. On the RTX 3080 both Chrome and
    mozangle give zero bits (ANGLE's Direct3D 11 back end writes no timestamps; it has elapsed-time
    queries only), so `TIMESTAMPS` is absent there and the route with it is built but not run.
  - Multisampled textures are renderbuffers (WebGL 2 has no multisampled textures), made by
    `renderbufferStorageMultisample` with the counts GL reports per format (`getInternalformatParameter`
    `SAMPLES`; in WebGL that answer is the whole list). `RESOLVE` on colour formats that are not
    integers. Pass resolve targets and `resolve` by `blitFramebuffer`; a resolve in a format other
    than the source's stored one counts `ResolveInStoredFormat`. Whole copies between multisampled
    textures resolve the source into a temporary texture and draw it into every sample of the
    destination (`MultisampleCopyResolved`). `MULTISAMPLE_READ` and `SAMPLE_SHADING` are absent: a
    draw reading a multisampled texture loses the device naming it; integer samples cannot be
    copied (a loss naming `MULTISAMPLE_READ`); multisampled depth, stencil and sRGB copies lose the
    device as not built. The texture's storage is an enum (`Storage`), so every GL texture use
    handles the renderbuffer case.
  - `FRACTIONAL_DEPTH_BIAS` by a probe when the back end is made (beside the 24-bit one): a
    triangle 0.45 of a step above step 1000 of `depth16unorm`, without and with a bias of 0.1
    units; declared only when it then stores 1001. Without it a raster block with a fractional bias
    is `Absent(FRACTIONAL_DEPTH_BIAS)` (the front adds the bias in the program).
  - New checks (examples/common/checks.rs, in Chrome and on ANGLE): `timestamps_and_their_disjoint_flag`,
    `multisampled_copies_resolve_as_their_source` (a partly covered diagonal, resolved at the pass's
    end and by `resolve`, and the copy's resolve equal to the source's, `MultisampleCopyResolved`
    counted once), `fractional_depth_bias` (the refusal where the probe found none),
    `constant_alpha_blends` (`ConstantAlpha` natively at alphas 0.5 and 0.25, within one step, and
    a blend of the constant colour with the constant alpha refused as absent). The browser host runs
    the suite's three new tests (multisampled targets, per-sample programs, occlusion queries).
  - tests/against_webgpu.rs: `a_fractional_viewport_in_the_program_equals_webgpu_native` (a
    triangle in a viewport of 16 by 16, drawn here over the whole target by a WGSL variant moving
    the positions as the viewport would, `ViewportInShader`). The viewport is at x 2.25, y 1.25, 11
    by 12.5 since run 14 (x 2.5 before, a pixel-centre edge Direct3D leaves undefined); it passes
    since run 15.
  - examples/draw-cost.rs, browser/cost.html and cost.mjs: the per-draw cost of both back ends in one
    Chrome (see "Measurements").

- G3 run 4:
  - Transform feedback (`capture_vertices`, `TRANSFORM_FEEDBACK`, `capture_buffers` from
    `MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS`). A capture links the program again from its GLSL
    with its location outputs (`_vs2fs_locationN`) as varyings, interleaved for one target and
    separate for more, with the empty fragment shader when it does not rasterise. There are four
    such programs per program at most, each made at its first capture. The draw runs between
    `beginTransformFeedback` and `endTransformFeedback`, with `RASTERIZER_DISCARD` and the default
    framebuffer outside a pass. A target that holds indices too gets its index twin marked stale, so
    the next index read refreshes it (`IndexTwinReadBack`). Generic buffer bindings are cleared
    first, because WebGL refuses a capture target bound anywhere else. GLES 3.0 captures only draws
    of lists without indices: strips, fans, loops and indexed captures lost the device as not built
    until run 6, which expands them (below).
    A target too small for the vertices, or the wrong number of targets, is `Invalid`.
  - Size queries: a program that calls `textureSize` gets a hidden uniform `<sampler>_size` per
    texture (the bound view's first level: width, height, and depth or layers). `textureSize(t, l)`
    becomes a shift of it, set when the bound view changes. ANGLE (Chrome's and mozangle alike)
    answers `textureSize` for the wrong level once a texture's base level has changed.
  - Host copies (`HostCopyKept`, src/host.rs): RGB9E5 and BC textures keep every level on the host.
    Writes go there and to GL; read backs, now and later, come from it. BC is GL's compressed
    format where its WebGL extension is present (`compressedTexSubImage`, not in 3D), else decoded
    by bcdec_rs (`BlockDecoded`): BC1 to BC3 and BC7 as RGBA8, BC4 and BC5 as R16F and RG16F
    (since run 16; R8 and RG8 before), BC6H as RGBA16F. `BLOCK_COMPRESSION` is declared when all four extensions are
    present. Their sRGB views lose the device as not built (copies were built in run 7).
  - New checks (in Chrome and on ANGLE): `vertices_captured_with_and_without_rasterising`
    (interleaved, separate at an offset, while drawing, whole primitives only, and the pass rule),
    `captured_indices_drawn_both_ways` (a buffer holding indices and vertices, written by the host
    and then captured, drawn as indices and as a vertex stream, the twin refreshed once),
    `block_compressed_and_shared_exponent_textures_read_back_as_written` (RGB9E5 and all 14 BC
    formats; two layers of three levels, the last smaller than a block; one block written again;
    read back now and later; a solid BC1 block and an RGB9E5 texel loaded), and
    `fragment_depth_follows_the_clip_convention`.
  - all.cmd's Chrome run without WEBGL_stencil_texturing, and its ANGLE run without both clip
    extensions, also switch off the four BC extensions, so the decoded route runs.

- G3 run 6:
  - Natively the surface is an EGL window surface on a host window (`WebGl::add_window(name,
    hwnd)`, unsafe: the window must outlive its surfaces). `create_surface` makes it current in place
    of the 1 by 1 pbuffer, `present` blits the frame into its back buffer (stretched to the window's
    size if that differs) and calls `eglSwapBuffers`, and `vsync` is `eglSwapInterval`. Destroying the
    surface puts the pbuffer back. A window not named by `add_window` is `Invalid`. Where ANGLE has
    EGL_ANGLE_direct_composition (mozangle does), the surface is made with
    `EGL_DIRECT_COMPOSITION_ANGLE`, a flip-model swap chain shown through DirectComposition. Without
    it, ANGLE's blit-model swap chain left the test window black as `PrintWindow` read it (measured
    once, then dropped). The pbuffer resizing and `read_presented` are gone. The EGL instance is now
    EGL 1.1 (for the swap interval).
  - The native suite's host opens a 4 by 4 popup window and reads it as the compositor shows it
    (`PrintWindow` with `PW_RENDERFULLCONTENT` after two `DwmFlush`), as G2's host does. The suite
    therefore needs `gpu-job --desktop` natively.
  - Captures of strips, fans, loops and indexed draws (`CaptureExpanded`, a new slow path, see
    "Interface changes"): src/capture.rs puts the draw's vertex numbers in list order (odd strip
    triangles and fan triangles ordered to keep the raster block's provoking vertex where it was,
    as the interface's converted fans do; loops closed; the largest index value restarts, as GL's
    draws always do). Indices are read back first (the draw waits for the GPU). The capture program
    then draws points, one draw per run of consecutive vertex numbers, instance after instance, with
    `RASTERIZER_DISCARD`; when rasterising, the original draw follows for the picture. Unit tests of
    the ordering in capture.rs, and the check
    `strips_fans_loops_and_indexed_draws_captured_in_list_order` (Chrome and ANGLE). Still not built:
    a capture whose base vertex takes a vertex number below 0 (device loss naming it).
  - `max_line_width` is 1: wide lines are refused as `Absent(WIDE_LINES)`, so the limit no longer
    reports GL's range.
  - Timestamps: Chrome 157 on the RTX 3080 gives 0 timestamp bits (64 elapsed-time bits) on every
    ANGLE back end it offers: Direct3D 11, Direct3D 11 on 12, Vulkan and OpenGL (`--use-angle=`,
    measured 2026-11-17 10:15). So `TIMESTAMPS` cannot run in Chrome on this machine either.

- G3 run 7:
  - Merged origin/phase-2-graphics (688aff2) and origin/phase-2-graphics-webgpu (one conflict in
    slow.rs: both branches had added a slow path at the end; both kept).
  - `INSTANCE_STEP_RATE` (new in the interface) is declared: `vertexAttribDivisor` takes any rate.
    Expanded captures (`CaptureExpanded`) draw one instance at a time, and before this run they moved
    per-instance streams by one element per instance whatever the rate. `draw_from` now takes how
    many instances past the first a draw starts, `instance_index` adds them and each per-instance
    stream steps by them at its own rate (never, for rate 0 and stride 0).
  - `TEXEL_BUFFERS` (new) is absent: a texel buffer binding is refused as `Absent(TEXEL_BUFFERS)`
    (before: `Invalid`).
  - A sampler with anisotropy past `max_anisotropy` is `Invalid`, as the recording back end has it;
    before, it was clamped silently, and without EXT_texture_filter_anisotropic ignored.
  - Copies into and out of textures kept on the host (RGB9E5, BC), which lost the device before:
    between two of them, and from one into an uncompressed texture whose texels are its blocks, from
    the host copy as a host write; into one from an uncompressed texture or a buffer, read back
    first (`HostCopyReadBack`); into a buffer from the host copy, row by row as buffer writes.
    `extent` is in the source's texels, as the WebGPU back end takes it.
  - sRGB views of BC textures (device loss before): the sRGB twin is a second host-kept texture;
    it is brought up to date by copying the host bytes over and uploading every level (counted
    `HostCopyKept`), in the same twin tracking as other sRGB views.
  - New checks (Chrome and ANGLE): `absent_features_are_refused` (every feature WebGL 2 lacks is
    left out of the caps and refused as `Absent` of its bit at the block, sampler, shader, binding,
    table or query set that asks for it; those an extension gives, only where it is off; anisotropy
    past the limit `Invalid`) and `copies_into_and_out_of_host_kept_textures` (BC1, BC3, BC7 sRGB,
    BC4 snorm and RGB9E5; a copied solid BC1 block loads red). `vertex_and_instance_numbers_include_the_bases`
    now steps its per-instance stream at rates 1, 2, 0 and 3, and the list-order capture check
    captures a strip with a per-instance stream at rates 1, 2 and 0. And
    `srgb_views_of_block_compressed_textures` (a grey BC1 block loads 131 to 133 through the
    texture's view and 58 to 60 through the sRGB one, then white after a later write).

- G3 run 8:
  - Merged origin/phase-2-graphics 164ad69 (one conflict in slow.rs, the same two slow paths in
    the other order; the shared line's order kept, so slow.rs equals it) and
    origin/phase-2-graphics-webgpu cf2f03c.
  - `textureSize` of a texture a program only sizes: the size rewrite (run 4) leaves no other use of
    the sampler uniform, so GL dropped it, the link gave the texture no unit, and its size uniform
    stayed 0 (sizes of 1). Now such a texture still takes a unit, so its size uniform follows the
    bound view. Found by the shaders track (`ps_size`, `GetDimensions` only). The check
    `loads_count_levels_from_the_view` now also draws with a program that only sizes the texture.
  - `bgra8unorm` and `bgra8unorm-srgb` (plan 5.8): stored as RGBA8 and SRGB8_ALPHA8; writes swap red
    and blue on the host (`FormatConverted`, counted), read backs now and later swap them back while
    repacking (`ReadAs::swap`). They sample, filter, render, blend, resolve, take GL's sample counts
    and copy out. Copies between the two go through a pixel pack buffer as stored. Copies from a
    buffer into them read the buffer back and write it as a host write; copies out of them into a
    buffer read the texels back and write them into the buffer (`HostCopyReadBack`). `PRESENT`
    stays on `rgba8unorm` only.
  - The same host route now builds three copies that lost the device as not built: texture to
    buffer of formats `readPixels` reads wider than stored (8 and 16-bit integers, and depth and
    stencil through `DepthReadByDraw`), texture into an index-only buffer, and index-only buffer
    into a texture. Copies out of formats without `COPY_OUT` (16-bit floats, packed formats) are
    `Invalid`, as the interface's rule makes them.
  - New check (Chrome and ANGLE): `bgra_textures_keep_their_byte_order` (both formats written, read
    back now and later, loaded through a view, copied into a vertex and an index-only buffer and
    back, cleared red as a target; an `r8uint` texture copied into a buffer). The typeless check
    copies `bgra8unorm` to `bgra8unorm-srgb` and back.

- G3 run 9:
  - Merged origin/phase-2-graphics 02ab37d. It brought the interface's rule that without
    `TEXTURE_VIEWS` a draw reads each texture through one mip range, and that bound views are of
    all layers in the texture's own dimension (the recording back end refuses both).
  - Refusals the back end lacked: a binding set holding a texture view that is not the whole
    texture in its own dimension (one layer of an array, a cube face, part of the layers, a cube as
    an array) is `Absent(TEXTURE_VIEWS)` (before, the set was made and the draw bound the GL texture
    whole, which GL then refused or sampled wrongly); a draw reading one texture through two mip
    ranges loses the device naming it (before, the last range set on the texture won for both); an
    indirect draw now names itself in `last_log` (it lost the device without a word); a draw with
    immediates past `immediate_bytes` (0) loses the device (before, they were ignored).
  - `generate_mips` overridden (plan 5.8): `generateMipmap` with the texture's base and max level
    set to the levels asked, where `values` is the texture's own format, the format renders and
    filters here, every layer is asked, the texture is not kept on the host, and it is a 2D or cube
    texture; otherwise the interface's draws through `generate_mips_by_draws` (see "Interface
    changes"). 2D arrays and 3D textures stay on the draws because the check below could not show
    them equal (see "Known failures").
  - `clear_rects` overridden (plan 5.8): one pass loading the view, then per rectangle the scissor
    and one `clearBuffer*` (float, unsigned, signed, depth, stencil or both), in place of the
    built-in programs' draws. `clear_texture` keeps the interface's default, which on this back end
    already clears by `clearBuffer*` in its passes (see "Departures").
  - EXT_texture_norm16 (enabled in the browser; `GL_EXT_texture_norm16` natively, both present on
    the RTX 3080): `r16unorm`, `rg16unorm` and `rgba16unorm` sample, filter, render, blend, resolve,
    take GL's sample counts and read back (`readPixels` as RGBA shorts, kept narrow on the host);
    the three snorm formats sample and filter. Copies within their typeless groups go through the
    existing routes (the draw route now knows 16-bit unorm). Without the extension they have no uses,
    as before. all.cmd's reduced routes also switch the extension off.
  - EXT_render_snorm (Chrome 157 on the RTX 3080 has it; mozangle 0.7.1 does not, so natively
    the caps now differ from Chrome's in this): the signed normalised formats render and blend,
    one sample only (Chrome's `getInternalformatParameter` refuses them with INVALID_ENUM, which
    lost every device in the first job of this run). They never copy out: Chrome's `readPixels`
    returned a written -128 as -127 (measured, first job), so read backs would not be exact.
  - `PRESENT` on `bgra8unorm`: its storage is RGBA8 with the channels in their places, so the frame
    blits into the RGBA drawing buffer as `rgba8unorm`'s does.
  - New checks (Chrome and ANGLE): `sampled_views_are_whole_textures_of_one_mip_range`,
    `generate_mipmap_equals_the_draws` (rgba8unorm, its sRGB form, bgra8unorm, r32float with
    OES_texture_float_linear, rgba16unorm and r16unorm; 2D, odd-sized 2D array, cube and 3D; every
    level and only the last two; against `generate_mips_by_draws` on the same texels),
    `sixteen_bit_normalised_formats_follow_their_extension`,
    `signed_normalised_formats_follow_their_extension` (a clear to -1, 0, 1 and 64 steps loads
    back as those values; no `COPY_OUT`) and `bgra_surfaces_present`;
    `absent_features_are_refused` also draws indirectly and with immediates; the typeless check
    copies four 16-bit normalised pairs, and skips a pair whose source does not read back.

- G3 run 5:
  - The test tool: angle/build.cmd unpacks mozangle 0.7.1 into C:\wc\tools\mozangle-0.7.1-patched
    and applies angle/d3d11-clip-control.patch. The patch holds three upstream ANGLE fixes to the
    Direct3D 11 renderer, written against mozangle's older code:
    - ba845fcf25d0: the scissor is not mirrored under the upper-left origin.
    - 6092e0b75c22: a change of the clip depth mode reaches the shaders. This change makes it do so
      by marking the viewport dirty.
    - 1dd3a4c4053a: `gl_FragCoord.z` is taken from the built-in position.
    The tool then builds ANGLE from nothing into `%CARGO_TARGET_DIR%\angle-patched`. Natively, every
    test that had failed under EXT_clip_control now passes, apart from `present_shows_the_frame`.
  - tests/against_webgpu.rs: `generate_mips_of_r32float_filtered_in_the_program_on_both_equals_webgpu`.
    It compares this back end without OES_texture_float_linear against the WebGPU back end on a
    device made without float32-filterable (and without adapter-specific format features, as G2's own
    Direct3D comparison makes it). Both must count `FilterInShader`, and every level must be
    byte-equal. The older comparison against WebGPU's hardware filter stays as it was, as a known
    failure.
  - examples/draw-cost.rs: `gpu_cost` is now a JSPI export (`#[wasm_bindgen(jspi)]`) that makes the
    adapter and device with the WebGPU back end's `block_on`. That back end now runs its depth
    probe in the browser and waits for it by suspending. Called from a plain async export, the
    page hung. wasm-bindgen-futures is no longer used. Lint the example for WebAssembly by adding
    `--example draw-cost` to the wasm32 clippy command.

- G3 run 10:
  - Merged origin/phase-2-graphics e9387a6 (it already held the WebGPU branch's newest). The browser
    host runs the suite's four new tests (`a_scissor_past_the_target_is_clipped_to_it`,
    `a_stream_of_stride_zero_reads_its_first_element`,
    `comparison_samplers_compare_with_the_stored_depth`, `resolves_average_in_the_format_named`).
  - Scissors (draws and `clear_rects`) are clipped to the pass's area before they reach GL. Before,
    a scissor of width `u32::MAX` became -1 (Chrome then drew nothing; on another route it drew
    everywhere).
  - A texture a program samples through two samplers (a comparison and a plain one, or two
    comparisons): naga's GLSL writer combines each texture with one sampler and refused the shader
    (`ImageMultipleSamplers`). src/shader.rs `split_samplers` now rewrites such a module before
    writing it: each further sampler reads the texture through a copy of its global variable at a
    binding past every front's (from 2^20), which the link maps back to the texture's slot, so
    each pair gets a texture unit of its own. A texture expression sampled through two samplers
    at once is still refused (`Invalid`, naming it). Found by the suite's comparison sampler test.
  - Passes without attachments (`PassDesc::empty`, device loss before): an `R8` renderbuffer of
    the pass's size and samples on a framebuffer with no draw buffer, cached by size and samples,
    invalidated at the start and end. New check `passes_without_attachments_rasterise` (occlusion
    queries at 1 and 4 samples).
  - `generate_mips` by `generateMipmap` now takes 2D arrays and 3D textures too. In Chrome it equals
    the box average of the four texels above on even sides, and equals the draws on odd 2D, odd
    cube, 3D and every layer the draws write (see "Known failures" for the layers they do not).
    `generate_mipmap_equals_the_draws` now also takes a 13 by 7 2D texture, a 16 by 8 2D array and
    a 7 by 7 cube (three levels: the full chain), and gives the first differing texel with the box
    average. New check `levels_stay_as_written_while_others_are_read_and_drawn`.

- G3 run 11:
  - Merged origin/phase-2-graphics f7e5dfe (G1's run 11: viewport bounds, `capture_components`, two
    new suite tests, which the browser host now runs), then origin/phase-2-graphics-shaders and
    origin/phase-2-graphics-rig. No conflicts.
  - Layers of 2D arrays are never invalidated (`DontCare` loads and `Discard` stores of an attachment
    whose texture is a 2D array do nothing). In Chrome, invalidating one layer lost the other layers
    of that level. That is why the mip draws left every layer but the last black, and why
    `levels_stay_as_written_...` failed. Run 10 guessed that sampling the array while drawing into it
    was the cause. This run first built that fix (draws sampled a copy of the levels they read,
    under a new slow path): the copy was made, and the layers were still black. The slow path was
    taken back out of the interface. With only the invalidation skipped, both checks pass in Chrome.
  - Viewports are checked against the interface's new bounds (WebGPU's, by `max_texture_2d`), and
    depths outside 0 to 1 or reversed are refused too; such a draw loses the device as a broken rule.
    Before, GL clamped them. New check `viewports_past_their_bounds_are_refused`.
  - Immediates (plan 5.8): `immediate_bytes` is 256. naga writes a program's immediates as one
    uniform struct per stage; each scalar, vector and matrix of it is set from the draw's bytes at
    its offset, only when the program's last bytes differ. Matrix columns are read 8 or 16 bytes
    apart, as the WGSL layout puts them. Bytes past what a draw gives read zero, as GL's uniforms
    start. New check `immediates_reach_both_stages`.
  - Vertex formats GL ES cannot read as stored (`VertexFormatConverted`, counted per draw): GL reads
    `Bgra8Unorm` as four normalised bytes and `Uint10_10_10_2` and `Float11_11_10` as one unsigned
    word. A program variant per layout key (two bits per location) declares those inputs as such
    and turns them into the program's values at the start of `main` (bytes swapped; ten-bit fields;
    11 and 10-bit floats decoded, denormals and infinity included). Variants are relinked from the
    program's GLSL at the first draw with each key, captures included (src/shader.rs `relink`, which
    now also makes capture programs). New check `packed_vertex_formats_are_converted`, and a unit
    test of the GLSL rewrite. An attribute location past the limit is now `Invalid`.
  - The test tool: angle/immutable-base-level.patch carries upstream ANGLE's fix for immutable
    textures whose base level changes (adapted: level 0's size comes from its image, since
    mozangle's texture storage has no size query). The first build of it crashed the native suite
    (access violation) on textures without images; it now keeps the old size there. Natively the
    two level checks pass.

- G3 run 12:
  - Merged origin/phase-2-graphics-shaders (its two interface commits: the variant key's linked
    values and numbered sampler state codes, neither used here) and origin/phase-2-graphics-rig.
    origin/phase-2-graphics had nothing new.
  - The extensions plan 4.11 lists for WebGL 2, each declared only where the context has it and
    refused as absent where not (before: all absent):
    - `INDEPENDENT_BLEND` (OES_draw_buffers_indexed): a blend block whose targets differ sets each
      target's blending, equation, factors and write mask with the indexed calls. GL's values are
      now kept per target (state.rs `GlTarget`); a block the same for every target sets them with
      the calls for all targets, only where some target differs.
    - `DUAL_SOURCE_BLEND` (WEBGL_blend_func_extended): the four second-source factors, and naga's
      `layout(location = 0, index = 1)` output. A draw blending a second source into a pass of more
      than one colour target loses the device naming it (GL blends a second source into one
      target only).
    - `POLYGON_MODE` (WEBGL_polygon_mode): `FillMode::Line` by `polygonModeWEBGL`; with a depth
      bias, the line offset is on too, as Direct3D biases wireframe. Point filling is
      `POLYGON_MODE_POINT`, new in the interface, never declared here (WebGL's polygon mode draws
      lines only).
    - `DEPTH_CLAMP` (EXT_depth_clamp), `DEPTH_BIAS_CLAMP` (EXT_polygon_offset_clamp,
      `polygonOffsetClampEXT` when the clamp is not zero), `MIRROR_ONCE`
      (EXT_texture_mirror_clamp_to_edge).
    - `CLIP_DISTANCE` and `CULL_DISTANCE` (WEBGL_clip_cull_distance) are built but stay absent: see
      "Known failures". naga writes `gl_ClipDistance` with the extension's directive (the shaders
      track's change); the back end enables clip distances 0 to n - 1 for a program writing n when
      the program changes, and none for its own draws.
  - `POINT_SIZE`: programs writing `gl_PointSize` draw points of that size, up to
    `ALIASED_POINT_SIZE_RANGE` (`max_point_size`); programs writing none still draw 1.
  - `create_shader` refuses a module whose outputs need a feature the back end lacks (clip or cull
    distances, a point size, a second fragment output) as `Absent` of it. Before, such a shader
    failed to compile and the device was lost at the first draw.
  - mozangle 0.7.1's Direct3D 11 renderer has OES_draw_buffers_indexed and EXT_blend_func_extended
    but none of the others (read from its renderer11_utils.cpp), so natively only those two run;
    Chrome 157 reports all of them on the RTX 3080.
  - New checks (examples/common/extensions.rs, Chrome and ANGLE, each where its feature is
    declared): `independent_blends_per_target`, `second_source_blends`,
    `depth_clamped_to_the_viewport_range`, `depth_bias_clamped`, `polygons_filled_with_lines`,
    `mirror_once_addressing`, `clip_and_cull_distances` (runs nowhere today),
    `points_sized_by_the_program`. A unit test checks the clip distance count and the GLSL.
    `absent_features_are_refused` now refuses each of them only where it is not declared, point
    filling always, and the shaders above. all.cmd's reduced Chrome route and third ANGLE route
    also switch the new extensions off.
  - The rig's own Direct3D 11 pictures of wireframe filling (`raster-fill`), depth bias with a
    clamp (`depth-bias`, `depth-bias_depth`) and depth clipping and clamping (`depth-clip`,
    `depth-clip_depth`) are drawn through the back end with the rig's vertices, cells and states
    (examples/common/rig.rs) by the scene runners in Chrome and on ANGLE, and compared with the
    stored references within their recorded allowances. A picture whose feature the back end lacks
    is skipped as absent.
  - Found by `depth-bias_depth`: `DepthReadByDraw` read a 24-bit depth of 1 as 0. The fetch computed
    `uint(d * 16777215.0 + 0.5)`, and a float cannot hold 16777215.5, so it rounded to 2^24, whose
    low 24 bits are 0. The value is now kept at most 16777215. `depth_and_stencil_read_back` now
    clears its middle to depth 1 (it used 0.75, which never reached the edge).

## How to run

All from the checkout root on the Mac.

- Natively the suite presents into a window, so every job that runs it needs `gpu-job --desktop`
  (the queue's own session refuses swap chains); the commands below have it.
- Stale builds: in run 6, cargo still took run 4's WebAssembly build of `wcontainers-gpu` (release
  and debug) as current, and the WebGPU back end then failed to build against it. Cleaning with
  `--target wasm32-unknown-unknown`, and again with `--release`, fixed it.
- The desktop's clock: on 2026-11-17 the target directory held files written at 17:18 desktop time,
  while the clock read 09:27. The clock had gone back by about eight hours. Cargo then took every
  freshly checked-out source as older than its build, and ran stale test programs without saying so.
  The first job of run 5 ran run 4's binaries. If a build finishes at once after a change, clean
  our crates first:
  `tools/graphics-rig/remote "cargo clean -p wcontainers-gpu-webgl -p wcontainers-gpu -p wcontainers-gpu-test -p wcontainers-gpu-webgpu -p wcontainers-graphics-test"`
  (and again with `--target wasm32-unknown-unknown`).

- all.cmd writes each native suite run to `<out>\suite-angle*.txt` (with every test's output) and
  shows its summary, failures and the "not run:" lines of checks that ran only
  their refusal. Its last native run and its fourth Chrome run also switch `float_blits` off.
- Everything at once (both scene routes in Chrome and on ANGLE; the suite with the checks in
  Chrome four ways (every extension, without EXT_clip_control, without it and WEBGL_provoking_vertex,
  without WEBGL_stencil_texturing and the BC extensions) and on ANGLE three ways (the third also
  without the BC extensions); the comparisons with the WebGPU back end natively and in Chrome),
  one GPU job after the build; exit 1 on any failure, so today it exits 1 for the known failures. The
  Chrome suite verdicts land in `<out>\suite-chrome*.txt`:
  `tools/graphics-rig/gpu-job --desktop --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\all.cmd C:\wc\out\webgl\<name>"`.
  About 60 s as a job once built. Lint as part of the build:
  `tools/graphics-rig/gpu-job --desktop --build "crates\wcontainers-gpu-webgl\browser\build.cmd && cargo clippy -j 3 -p wcontainers-gpu-webgl --all-targets -- -D warnings && cargo clippy -j 3 -p wcontainers-gpu-webgl --lib --example browser --example draw-cost --target wasm32-unknown-unknown -- -D warnings" -- "cargo test -j 3 -p wcontainers-gpu-webgl --lib && crates\wcontainers-gpu-webgl\browser\all.cmd C:\wc\out\webgl\<name>"`
  (`--lib` runs the unit tests of src/capture.rs and src/restart.rs).
- The per-draw cost of both back ends in Chrome, as an exclusive job (build.cmd builds the
  example, in release, into `%CARGO_TARGET_DIR%\webgl-cost-pkg`):
  `tools/graphics-rig/gpu-job --alone --desktop --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "node crates\wcontainers-gpu-webgl\browser\cost.mjs %CARGO_TARGET_DIR%\webgl-cost-pkg C:\wc\out\webgl\cost.txt 30 \"10000 50000\""`
  (frames timed per case, then the draw counts). About 15 s. Without `--desktop` Chrome kept no
  WebGL 2 context in G3's check (run 18's measurements had it from all.cmd's desktop session).
- A real program's frames, recorded and replayed (G3b run 2; src/trace.rs). The `trace` feature
  gives the call stream (`trace::Call`, postcard, shaders as naga modules; `trace::Replay` runs a
  stream on any back end frame by frame). The `record` feature makes `WebGl` the recording
  `trace::Traced<Gl>`, which writes every call into the file `WEBGL_TRACE` names (a second device
  of the process into `<name>.1`) while drawing on ANGLE; without `WEBGL_TRACE` it records nothing
  and draws as `Gl` does (since G3b run 3; before, it made no device), so a build that gets the
  feature through another crate still works. The crate's own examples and tests use `Gl` by name,
  so `--all-features` builds them too.
  The Direct3D 11 library is built on it with
  `cargo build --release -p wcontainers-directx --features webgl,wcontainers-gpu-webgl/record --target-dir %CARGO_TARGET_DIR%\record`.
  Through the rig's forwarding library (`programs run --dll`) the programs recurse until the
  stack overflows (0xc00000fd): ANGLE inside this back end loads d3d11.dll by name and gets the
  rig's forwarder, which hands it to our library again. So scratch g3b2/record-direct.ps1 (copied
  to C:\wc\out\webgl\record-direct.ps1) runs one program with the three libraries beside it
  and no forwarder, for `-Seconds`, then stops it:
  `tools/graphics-rig/gpu-job --desktop --build "cargo build -j 3 --release -p wcontainers-directx --features webgl,wcontainers-gpu-webgl/record --target-dir %CARGO_TARGET_DIR%\record" -- "powershell -NoProfile -ExecutionPolicy Bypass -File C:\wc\out\webgl\record-direct.ps1 -Name atg-simplelightingpc -Exe xbox-atg-samples\PCSamples\IntroGraphics\SimpleLightingPC\x64\Release\SimpleLightingPC.exe -Cwd xbox-atg-samples -Seconds 10 -Library %CARGO_TARGET_DIR%\record\release"`.
  The stream lands in C:\wc\out\webgl\trace\<name>.bin; the library's first device is a probe
  that draws nothing, so the program's frames are in `<name>.bin.1`. Replay in Chrome on both
  back ends (build.cmd builds the example `replay` into `%CARGO_TARGET_DIR%\webgl-replay-pkg`):
  `tools/graphics-rig/gpu-job --alone --desktop --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "node crates\wcontainers-gpu-webgl\browser\replay.mjs %CARGO_TARGET_DIR%\webgl-replay-pkg C:\wc\out\webgl\trace\<program>.bin C:\wc\out\webgl\trace\<program>-replay.txt"`.
  Each back end replays the stream twice, on a new device each time (the last one dropped first):
  once with each call timed inside the back end, once timing whole frames only; frames before the
  tenth are left out. The page logs each replay's start, every hundredth frame and its end; the
  command stops itself (and Chrome) after 300 s, so a page that stops answering does not hold the
  GPU job. The round trip is tested natively:
  `cargo test -p wcontainers-gpu-webgl --features record --test trace` (a scene recorded on
  ANGLE, replayed on a new WebGL 2 back end and on the WebGPU back end, equal texels). With
  `record`, only the library and that test build (other examples use `Gl`'s own methods).
  Pages are served cross-origin isolated since run 2, so `performance.now()` reads to 5 us.
- BC1 to BC7 against real Direct3D 11 (run 17; all.cmd runs it into `<out>\bc-reference.txt`):
  `tools/graphics-rig/gpu-job -- "cargo test -j 3 -p wcontainers-gpu-webgl --test bc_reference -- --test-threads=1 --nocapture"`.
  It draws the rig's `textures-bc` and `textures-bc67` from the same seeded blocks, saves them
  under `%CARGO_TARGET_DIR%\tmp\bc-reference`, checks them with the rig's `check_dir` against
  references/d3d11 and prints each format's largest difference.
- The comparisons with the WebGPU back end alone (build.cmd builds them), natively:
  `tools/graphics-rig/gpu-job -- "cargo test -j 3 -p wcontainers-gpu-webgl --test against_webgpu -- --test-threads=1"`;
  in Chrome (the output's directory must exist; all.cmd writes `<out>\against-chrome.txt`):
  `tools/graphics-rig/gpu-job --desktop -- "node crates\wcontainers-gpu-webgl\browser\against.mjs %CARGO_TARGET_DIR%\webgl-pkg <out-file>"`.
  A comparison of examples/common/against.rs names its routes and its WebGPU device; both hosts
  take the list from there (natively a test per name, checked by `every_comparison_is_a_test`).
- The suite in Chrome alone (after build.cmd and a `--list` into a file, as all.cmd does):
  `node crates\wcontainers-gpu-webgl\browser\suite.mjs "%CARGO_TARGET_DIR%\webgl-pkg" <out-file> <list-file> [<extensions off>]`.
- Once per desktop, and again whenever angle/ changes (done in run 5, 4 m 30 s of compiling; done
  again in run 11 for the second patch):
  `tools/graphics-rig/remote "crates\wcontainers-gpu-webgl\angle\build.cmd"` builds ANGLE's
  libEGL.dll and libGLESv2.dll from the patched mozangle crate into C:\wc\tools\angle-mozangle-0.7.1.
  It downloads the crate from static.crates.io with curl.exe, unpacks it with tar.exe and applies
  both patches in angle/ with `git apply`. Other tracks loading these DLLs get the rebuilt ones.
- The scenes, natively through ANGLE and in Chrome's WebGL 2, each checked against the rig's stored
  Direct3D 11 pictures (crates/wcontainers-graphics-test/references/d3d11, with their tolerances):
  `tools/graphics-rig/gpu-job --build "crates\wcontainers-gpu-webgl\browser\build.cmd" -- "crates\wcontainers-gpu-webgl\browser\run.cmd C:\wc\out\webgl\g3a"`
  Add a second argument to run.cmd to switch extensions off, for example
  `run.cmd C:\wc\out\webgl\g3a-noclip "EXT_clip_control WEBGL_provoking_vertex"`. Exit 1 on any unexpected verdict. About 4
  s as a job once built.
- The shared suite on ANGLE (built by build.cmd too):
  `tools/graphics-rig/gpu-job --desktop --build "cargo test -j 3 -p wcontainers-gpu-webgl --test suite --no-run" -- "cargo test -j 3 -p wcontainers-gpu-webgl --test suite -- --test-threads=1"`.
  `WEBGL_ANGLE_DIR` overrides the ANGLE directory; `WEBGL_OFF=EXT_clip_control` takes the route
  without clip control. About 1 to 4 s.
  - build.cmd: installs the rig's browser packages, builds the native example `webgl-scenes` and the
    WebAssembly example `browser`, and runs wasm-bindgen into `%CARGO_TARGET_DIR%\webgl-pkg`.
  - run.cmd: `webgl-scenes angle <dir> <out>\angle` draws on ANGLE and checks (pictures and
    caps-angle.txt there); `node crates\wcontainers-gpu-webgl\browser\run.mjs` draws in the rig's
    headless Chrome and writes `.rgba` texels and caps-chrome.txt; `webgl-scenes check` checks those.
- Seeds and case counts (run 14): every pattern of examples/common/checks.rs is mixed with a seed.
  `WEBGL_SEED` (default 0, the patterns as written) and `WEBGL_CASES` (default 1) set it and how many
  seeds from it each check runs, natively (tests/suite.rs) and in Chrome (suite.mjs passes both to the
  page). For example `set WEBGL_CASES=20 && crates\wcontainers-gpu-webgl\browser\some.cmd <out> copies_`.
  Natively each seed is printed before its run (shown with `--nocapture`).
- Checks whose feature the back end declares absent run nothing beyond the refusal that
  `absent_features_are_refused` checks and call `not_run`; checks whose format lacks a use call
  `not_run_because` (G3b run 1). Chrome's suite output counts them as "not run: <reason>";
  natively they pass and print "not run: <test>: <reason>" (all.cmd lists those lines).
- If every rig command exits 2 at once with no output, the rig's ssh helper (common.sh `connect`)
  has met a missing ~/.ssh/wc-rig-refused on the Mac (`awk` on it fails under `pipefail`); an empty
  file there fixes it (made so in G3b run 3; the rig track's to fix).
- The literal check (G3b run 6): commit, then run it at HEAD in the LF worktree on the desktop
  (the desktop's own checkout is CRLF, which changes the keys of multi-line strings):
  `tools/graphics-rig/remote "cd /d C:\wc\src\phase-2-graphics-webgl-lf && git -c core.autocrlf=false checkout -q -f --detach <commit> && set GIT_CONFIG_COUNT=1&& set GIT_CONFIG_KEY_0=core.autocrlf&& set GIT_CONFIG_VALUE_0=false&& set RAYON_NUM_THREADS=3&& cargo run -j 3 -q -p wcontainers-literals -- check"`
  (scratch g3b6/lit-check.sh does exactly this at HEAD). It must print no `+`, `-`, `~`, `>` or
  `!` line.
- The frame-time breakdown of a real program through the Direct3D 11 front on this back end (G3b
  run 6; crates/wcontainers-gpu-webgl/frames/run, from the Mac):
  `crates/wcontainers-gpu-webgl/frames/run 3 10 30 atg-simplelightingpc simplelightingpc 'xbox-atg-samples\PCSamples\IntroGraphics\SimpleLightingPC\x64\Release\SimpleLightingPC.exe' xbox-atg-samples`
  (cargo jobs, short and long seconds, a name, the process name to count, the program and its
  working directory under C:\wc\programs\src, then its arguments and `NAME=value;...`
  variables). It prints each part's CPU milliseconds per second of the run; divide by the frames
  a second, from the two last `webgl: frames=` lines of
  C:\wc\out\webgl\frames\<name>-<long>\output.log (each line starts with the seconds since the
  program started). Sample the load beside it with `typeperf` over SSH, as the WebGPU track does.
  About 8 minutes a program once built.
- Lint and format: `nix develop -c cargo fmt -p wcontainers-gpu-webgl` and
  `nix develop -c cargo check -j 2 -p wcontainers-gpu-webgl [--target wasm32-unknown-unknown]` on the
  Mac; clippy on the desktop:
  `tools/graphics-rig/remote "cargo clippy -j 3 -p wcontainers-gpu-webgl --all-targets -- -D warnings && cargo clippy -j 3 -p wcontainers-gpu-webgl --lib --example browser --target wasm32-unknown-unknown -- -D warnings"`.
  The crate's tests are the scene runs and the suite above.

How another track uses the back end natively (G4's Windows library on WebGL 2): depend on
`wcontainers-gpu-webgl`, and make the back end on the device thread with
`WebGl::on_angle(Path::new(r"C:\wc\tools\angle-mozangle-0.7.1"), &[])` after running
angle\build.cmd (run 11's version, with both patches). To present into a window, name it first with
`unsafe { b.add_window(n, hwnd) }` and then `create_surface(n)` (since run 6). With every extension, the caps equal Chrome's on
the same GPU, as section 10 asks. Before run 5 this paragraph said to switch EXT_clip_control off,
because the unpatched ANGLE mirrored scissors under it. That is no longer needed: rebuild ANGLE
instead. The `&["EXT_clip_control"]` route still works and is the route of browsers without the
extension. The context is current on the calling thread, which must be the device thread. Natively the surface is
an EGL window surface on the named window (DirectComposition where ANGLE has it); with no surface the
context draws on a 1 by 1 pbuffer.
In the browser: `WebGl::on_canvas(canvas.getContext("webgl2"), &[])`; the canvas is the surface.
Every page that loads the back end's WebAssembly must supply the module `wcontainers-device` (an
import map to browser/device.js does) and call code that may wait through `WebAssembly.promising`.
With every extension both reported the same `Caps` on the RTX 3080 (compared on 2026-11-17:
identical lists); since run 9 they differ in the signed normalised formats, which render only with
EXT_render_snorm, which Chrome has and mozangle lacks.

## Capabilities

Declared today (each one built): `READ_BACK_NOW`, `FAN_AND_LOOP` (WebGL 2 topologies),
`PROVOKING_VERTEX_LAST` (GL's rule), `PROVOKING_VERTEX_FIRST` (with WEBGL_provoking_vertex),
`CLIP_DEPTH_MINUS_ONE`, `CLIP_DEPTH_ZERO_ONE` (with EXT_clip_control), `INDEX_U8`,
`INSTANCE_STEP_RATE` (since run 7; `vertexAttribDivisor`), `DEPTH24_UNORM`
(by the probe: set in Chrome 157 and on mozangle on the RTX 3080), `FRACTIONAL_DEPTH_BIAS` (by its
probe: absent in Chrome 157 and on mozangle on the RTX 3080, where ANGLE's Direct3D 11 rounds the
units), `TIMESTAMPS` (with EXT_disjoint_timer_query_webgl2 and timestamp bits above zero: absent
on the RTX 3080, zero bits), `BLEND_CONSTANT_ALPHA` (GLES 3.0 core; mixed with the constant colour
refused, as WebGL requires), `TRANSFORM_FEEDBACK` (since run 4; GLES 3.0 core), `BLOCK_COMPRESSION`
(since run 4, with WEBGL_compressed_texture_s3tc and its sRGB twin, EXT_texture_compression_rgtc and
EXT_texture_compression_bptc: all four in Chrome 157 and mozangle on the RTX 3080). Since run 12:
`POINT_SIZE` (GLES 3.0 core), and by extension `INDEPENDENT_BLEND` (OES_draw_buffers_indexed),
`DUAL_SOURCE_BLEND` (WEBGL_blend_func_extended), `POLYGON_MODE` (WEBGL_polygon_mode, lines),
`DEPTH_CLAMP` (EXT_depth_clamp), `DEPTH_BIAS_CLAMP` (EXT_polygon_offset_clamp), `MIRROR_ONCE`
(EXT_texture_mirror_clamp_to_edge). `CLIP_DISTANCE` and `CULL_DISTANCE` are built on
WEBGL_clip_cull_distance but absent until they clip in Chrome (see "Known failures").
Since G3b run 6: `SAMPLE_SHADING` with OES_sample_variables and
OES_shader_multisample_interpolation (both in Chrome 157 on the RTX 3080, neither in mozangle; the
`sample` qualifier is put back on naga's output, see "G3b: what the fronts ask"). Linear values
take NV_shader_noperspective_interpolation's `noperspective` where the context has it (Chrome).
Everything else is absent: `POLYGON_MODE_POINT` (WebGL draws polygon edges only), `OCCLUSION_COUNT`
and `MULTISAMPLE_READ` (WebGL 2 has neither), `SAMPLE_MASK` (no `sampleMaski` in WebGL 2), and
`FINE_DERIVATIVES`. The adapter is WebGL's renderer string
(unmasked in Chrome), PCI numbers 0 (WebGL gives none), software when it names SwiftShader,
llvmpipe or Microsoft Basic Render Driver.

Limits on the RTX 3080, Chrome 157 and ANGLE alike: 16384 2D and cube, 2048 3D and layers, 12
uniform blocks per stage of 65536 bytes (the plan's 16384 was the Mac), 16 texture units per stage,
16 attributes, stride 255 (WebGL's rule), 8 targets, 30 varying vectors, uniform offsets aligned to
256, points up to `ALIASED_POINT_SIZE_RANGE` (1024 before run 12, when it was unused), lines 1 wide,
anisotropy 16, immediates 256 (plain uniforms, since run 11).

Formats, from GLES 3.0's table with EXT_color_buffer_float, OES_texture_float_linear and
EXT_float_blend (all present here): every sized colour format GLES 3.0 holds as stored samples;
normalised ones filter, and render and blend unless signed or RGB9E5; 16-bit float and
R11G11B10 filter, render and blend; 32-bit float filter, render and blend (RGB32F does not render);
integers render (not RGB32), never filter or blend; depth formats are depth targets, and those
without stencil also sample. Copy out (read back): R8, RG8 and RGBA8 normalised and RGBA8 sRGB,
every colour-renderable 8, 16 and 32-bit integer format, and R32F, RG32F and RGBA32F (with
EXT_color_buffer_float); not RGB9E5 from the GPU, depth or stencil (since G3b run 3 R11G11B10
through its 32-bit twin and B5G6R5 as bytes packed on the host; since G3b run 1
16-bit floats copy out too, through their integer twin, and the 10-10-10-2 formats in their own
packing, `UNSIGNED_INT_2_10_10_10_REV`, which GLES 3.0 reads back for them). Since run 3, samples:
1, 2, 4 and 8 for the colour formats that render and are not integers, and for depth formats (as
GL reports them on the RTX 3080); integer formats take one.
No bits yet: the swizzled formats other than BGRA8 and BGRX8 (B5G5R5A1, BGRA4), A8, Stencil8. Since
G3b run 3 `bgrx8unorm` and `bgrx8unorm-srgb` have BGRA8's uses but `PRESENT`, alpha held at one.
Since G3b run 3 natively `uniform_buffers_per_stage` is 10, GL's 12 less mozangle's two (Departures). Since
run 9, with EXT_texture_norm16 (Chrome 157 and mozangle on the RTX 3080): `r16unorm`, `rg16unorm`,
`rgba16unorm` sample, filter, render, blend, resolve and copy out; the snorm ones sample and filter;
without it none of the six has a use. With EXT_render_snorm (Chrome only) the signed normalised
formats render and blend too (one sample, no copy out); without it (mozangle) they copy out from
their host copy (run 18). `PRESENT` on `bgra8unorm` too. Since run 8: `bgra8unorm` and `bgra8unorm-srgb` as `rgba8unorm` and its sRGB form
(sample, filter, render, blend, resolve, samples 1 to 8, copy out), red and blue swapped on the host. Since run 4:
the 14 BC formats sample, filter and copy out (read back from the host copy) on every route, and
RGB9E5 copies out too.
Since G3 run 1: `PRESENT` on `rgba8unorm` (the canvas's drawing buffer), and `COPY_OUT` on every
depth format (by `DepthReadByDraw`). Since run 2: `SAMPLE` and `COPY_OUT` on `depth24plus-stencil8`
and `depth32float-stencil8` on every route (stencil natively with WEBGL_stencil_texturing, which
Chrome has here and mozangle lacks, else by `StencilCopiedByDraw`).

## Measurements

- 2026-11-17, desktop (RTX 3080, driver 32.0.16.2101; other tracks' jobs queued, not timed): the
  three scenes equal real Direct3D 11's pictures exactly (0 of 65536 pixels differ, largest
  difference 0) through Chrome 157's WebGL 2 and through ANGLE (mozangle 0.7.1, Direct3D 11), with
  and without EXT_clip_control. Tolerance needed: none; the rig's recorded allowance (1, 1 and 0
  steps, from WARP) is not used up. Exactness is expected here because ANGLE draws on the same GPU
  through Direct3D 11 and the programs do the same float operations; other GPUs and browsers may
  need the recorded allowance.
- 2026-11-17, second run, same desktop (queued jobs, not timed; jobs near 14:00 desktop time): all 9 pictures as
  expected in all four runs (ANGLE and Chrome, with and without EXT_clip_control): the 3 scenes and
  the 4 same-picture routes exactly equal (0 pixels), the 2 altered quads failing (25016 pixels,
  largest differences 38 and 204). Before the fixes the check found `stale_unit`, `twin_copy` and
  (without clip control) `clear_near` failing (25016, 25016 and 19738 pixels).
- Shared suite on ANGLE, second run: 4 of 7 pass, on both routes (buffer writes and read backs,
  texture writes and read backs, copies, clears). Once, with the suite's raster block changed
  locally (not committed) to take provoking vertex Last and clip depth by the caps, 6 of 7 passed
  on both routes; only `present_shows_the_frame` failed.
- 2026-11-17, third run, same desktop (queued jobs, not timed): Chrome 157 and ANGLE report the same
  features with every extension: READ_BACK_NOW, FAN_AND_LOOP, PROVOKING_VERTEX_LAST and FIRST,
  CLIP_DEPTH_MINUS_ONE and ZERO_ONE, INDEX_U8, FRACTIONAL_DEPTH_BIAS, BLEND_CONSTANT_ALPHA. All 16
  pictures as expected in Chrome with every extension and with EXT_clip_control and
  WEBGL_provoking_vertex off, and on ANGLE with both off; on ANGLE with every extension 15 of 16
  (`triangle-clear_rects` fails, 4096 pixels: the known ANGLE failure). The three scenes and every
  same-picture route exactly equal (0 pixels); the two altered quads fail (25016 pixels each, largest
  38 and 204). Renderer in Chrome "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 (0x00002206) Direct3D11
  vs_5_0 ps_5_0, D3D11)". Lint clean on both targets.
- Shared suite on ANGLE, third run (13 tests, origin/phase-2-graphics c15677a): 11 pass with every
  extension (all but `generate_mips_averages_in_the_values_format` and `present_shows_the_frame`); 5
  without EXT_clip_control, and 5 without it and WEBGL_provoking_vertex (the others are refused by
  the suite's own raster block; see "Known failures").
- 2026-11-17, G3 run 1, same desktop (RTX 3080, driver 32.0.16.2101; jobs from the queue, not timed,
  about 40 s each between 15:02 and 15:19 desktop time; CPU load 17% just after): the shared suite at
  origin/phase-2-graphics 975c55c, 20 tests, plus the 2 checks:
  - Chrome 157.0.8103.78 with every extension: 22 of 22 pass. Without EXT_clip_control: 9 of 22;
    without it and WEBGL_provoking_vertex: 9 of 22 (the rest refused by the suite's raster blocks).
  - ANGLE (mozangle 0.7.1) with every extension: 20 of 22 (`present_shows_the_frame` and
    `the_scissor_bounds_the_draw` fail). Without EXT_clip_control: 8 of 22; without both: 8 of 22.
  - Before the fixes of this run the suite in Chrome failed 18 of 20 (read backs never ready after
    `wait_read`), and `generate_mips_averages_in_the_values_format` failed everywhere (no sRGB twin).
  - The 16 scene pictures: as in run 3 (all as expected in Chrome on both routes and on ANGLE without
    the extensions; `triangle-clear_rects` fails on ANGLE with EXT_clip_control).
  - Lint clean on both targets.
- The final tree of run 1 (b6a0c43) was checked once more as a whole (job 15:25 to 15:26 desktop
  time): the same verdicts as the run before it, listed next.
- Later the same day (job 15:23 to 15:24 desktop time, queued, not timed), with stencil read backs
  and the third check: Chrome with every extension 23 of 23; without EXT_clip_control 10 of 23,
  without both 10 of 23 (the third check passes on all three, with all five depth formats). ANGLE
  with every extension 20 of 23 (`depth_and_stencil_read_back` fails too: `Depth16Unorm: depth at 5
  is 0x4000, expected 0xbfff`, the scissor bug below); without the extensions 9 of 23 (the check
  passes, depth-only formats only).
- 2026-11-17, G3 run 2, same desktop (RTX 3080, driver 32.0.16.2101; queued jobs, not timed, about
  55 s each between 15:31 and 16:08 desktop time), after merging origin/phase-2-graphics 2684d97,
  origin/phase-2-graphics-webgpu 2fafcb2 and origin/phase-2-graphics-direct3d-12; 21 suite tests and
  6 checks (27). Final tree (job 16:06 to 16:07):
  - Chrome 157.0.8103.78 with every extension: 26 of 27 (`loads_count_levels_from_the_view`
    fails). Without WEBGL_stencil_texturing: 26 of 27, the same one. Without EXT_clip_control: 13
    of 27; without it and WEBGL_provoking_vertex: 13 of 27 (the rest refused by the suite's raster
    blocks, and the same one).
  - ANGLE (mozangle 0.7.1) with every extension: 22 of 27 (`present_shows_the_frame`,
    `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`, `stencil_sampled_and_read_back`:
    the scissor and pbuffer bugs; and `loads_count_levels_from_the_view`). Without EXT_clip_control:
    12 of 27; without both: 12 of 27 (the stencil and copy checks pass there).
  - Against the WebGPU back end (wgpu 30.0.2, Direct3D 12, same GPU): 2 of 4 pass on both ANGLE
    routes (window origins: the target and 4 mip levels byte-equal; cube, array and 3D textures
    byte-equal); the 2 generate_mips comparisons fail as listed under "Known failures".
  - The 24-bit depth probe sets `DEPTH24_UNORM` in Chrome and on ANGLE: without the bias the depth
    read back 0.09999997, with a one-unit bias 0.10000003 (one 24-bit step).
  - Before the fixes of this run: the stencil check read zeros through stencil views on every route
    (integer textures incomplete without a sampler), and the first level check read level 0 for
    every level of a view (fetches reached only the base level).
  - Lint clean on both targets.
  - Then with `fractional_viewports_are_refused` (job 16:09 to 16:10): every count above one higher
    (Chrome 27 of 28, ANGLE 23 of 28 and 13 of 28); the new check passes on every route.
- 2026-11-17, G3 run 3, same desktop (RTX 3080, driver 32.0.16.2101; queued jobs, not timed,
  about 60 s each between 16:15 and 16:39 desktop time), after merging origin/phase-2-graphics
  528db9c and origin/phase-2-graphics-webgpu 33949c3; 24 suite tests and 11 checks (35). Final tree
  (job ending 16:39):
  - Chrome 157.0.8103.78 with every extension: 34 of 35 (`loads_count_levels_from_the_view`
    fails). Without WEBGL_stencil_texturing: 34 of 35, the same one. Without EXT_clip_control: 19
    of 35; without it and WEBGL_provoking_vertex: 19 of 35 (the rest refused by the suite's raster
    blocks, and the same one).
  - ANGLE (mozangle 0.7.1) with every extension: 30 of 35 (`present_shows_the_frame`,
    `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`, `stencil_sampled_and_read_back`,
    `loads_count_levels_from_the_view`, as before). Without EXT_clip_control: 18 of 35; without
    both: 18 of 35.
  - Against the WebGPU back end: 2 of 5 (window origins; cube, array and 3D textures); the two
    generate_mips comparisons fail as before, the fractional viewport one as listed below.
  - The occlusion query test passes in Chrome and on ANGLE (with the extensions it needs); both
    multisample tests pass in Chrome and on ANGLE with 4 samples.
  - The new fractional bias probe found the bias lost in Chrome and on ANGLE: `depth16unorm` stored
    step 1000 with and without a bias of 0.1 units (the check found it first, when the feature was
    still declared).
  - `ConstantAlpha` at 0.5 on white gave 127 where 127.5 was expected (this GPU's blend rounds it
    down), so the check allows one step.
  - Lint clean on both targets.
- Per-draw cost, 2026-11-17 16:39 desktop time, as `gpu-job --alone` (RTX 3080, driver
  32.0.16.2101, CPU load 10% just after; Chrome 157.0.8103.78 headless, whose chrome://gpu says
  WebGL and WebGPU are "hardware accelerated but at reduced performance"; both back ends built for
  WebAssembly in release, in one page). The scene: one pass into a 256 by 256 `rgba8unorm`
  target, N quads of 4 vertices from `vertex_index`, each draw naming one uniform binding set or
  each its own (1024 sets over one buffer), then a submit. Timed by the page around each frame's
  recording and submission (`performance.now()`, which Chrome rounds to 0.1 ms here), 5 frames
  untimed then 30 timed, 20 ms between frames. Median per frame, and per draw:
  - 10,000 draws, one set: WebGL 2 1.5 ms (0.150 us a draw), WebGPU 1.6 ms (0.160 us).
  - 10,000 draws, a set per draw: WebGL 2 3.9 ms (0.390 us), WebGPU 3.8 ms (0.380 us).
  - 50,000 draws, one set: WebGL 2 4.9 ms (0.098 us), WebGPU 7.4 ms (0.148 us).
  - 50,000 draws, a set per draw: WebGL 2 16.6 ms (0.332 us), WebGPU 18.1 ms (0.362 us).
  These are the device thread's costs only (both APIs hand the work to Chrome's GPU process, whose
  cost is not in them). An earlier run of the same scene was void: the WebGL device had been lost
  at creation by a bug in the new probe.
- 2026-11-17, G3 run 4, same desktop (RTX 3080, driver 32.0.16.2101; queued jobs, not timed,
  about 75 s each between 16:47 and 17:11 desktop time), after merging origin/phase-2-graphics
  d31a49e and origin/phase-2-graphics-webgpu 2fddc0e; 24 suite tests and 15 checks (39):
  - Chrome 157.0.8103.78 with every extension: 39 of 39. Without WEBGL_stencil_texturing and the
    four BC extensions (BC decoded): 39 of 39. Without EXT_clip_control: 24 of 39; without it and
    WEBGL_provoking_vertex: 24 of 39 (the rest refused by the suite's own raster blocks).
  - ANGLE (mozangle 0.7.1) with every extension: 34 of 39 (`present_shows_the_frame`,
    `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`, `stencil_sampled_and_read_back`,
    `fragment_depth_follows_the_clip_convention`: the test tool's bugs under EXT_clip_control and
    its pbuffer). Without EXT_clip_control: 23 of 39; without both and the BC extensions: 23 of 39.
  - Against the WebGPU back end (wgpu 30.0.2, Direct3D 12): 4 of 5. The fractional viewport now
    passes, because G2 declares `FRACTIONAL_VIEWPORT`. The integer twin's mips pass since the size
    fix. The r32float mips still fail.
  - At the start of the run, before any change: Chrome 34 of 35 and ANGLE 30 of 35, as in run 3.
  - The capture checks passed the first time they ran, on every route. The new BC check failed
    once, on my own expectation: RGB9E5's green of 0.5 written to `rgba8unorm` gave 127 for 127.5,
    so the check now uses values that round without a tie.
  - Both Chrome and mozangle report the four BC extensions, so `BLOCK_COMPRESSION` is declared on
    both with every extension.
  - Lint clean on both targets (final tree checked at 17:08).
- 2026-11-17, G3 run 5, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times, queued
  jobs, not timed, about 100 s each between 09:29 and 09:42). Merged origin/phase-2-graphics f107eb5
  and origin/phase-2-graphics-webgpu 5054e1a. The suite has 24 tests and there are 15 checks (39):
  - The first job of the run (before any change) ran stale binaries, because of the clock (see "How
    to run"). Its verdicts were run 4's, and they are not counted.
  - After cleaning, with ANGLE patched (job 09:40 to 09:42):
    - Chrome 157.0.8103.78: 39 of 39 with every extension, and 39 of 39 without
      WEBGL_stencil_texturing and the BC extensions. Without EXT_clip_control, 24 of 39; without
      it and WEBGL_provoking_vertex, 24 of 39.
    - ANGLE with every extension: 38 of 39 (`present_shows_the_frame`). Before the patch it was 34
      of 39. Without EXT_clip_control, 23 of 39; without both and the BC extensions, 23 of 39.
    - All 64 scene verdicts are as expected, so `triangle-clear_rects` now passes on ANGLE with
      EXT_clip_control.
    - Against the WebGPU back end: 5 of 6. The new route-against-route r32float comparison passes;
      the comparison against the hardware filter fails as before.
  - With only the scissor and depth-mode fixes (job ending 09:31), ANGLE with every extension was 37
    of 39: `fragment_depth_follows_the_clip_convention` still read 0.75. The third fix (1dd3a4c4053a)
    made it pass.
  - Lint is clean on both targets.
- 2026-11-17, G3 run 6, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times, queued
  job 10:43 to 10:50, not timed). Merged origin/phase-2-graphics 41816d5 and
  origin/phase-2-graphics-webgpu eb1e292; 24 suite tests and 16 checks (40), one job of all.cmd on
  the final tree with `--desktop`:
  - Chrome 157.0.8103.78: 40 of 40 with every extension; 40 of 40 without WEBGL_stencil_texturing
    and the BC extensions; 25 of 40 without EXT_clip_control; 25 of 40 without it and
    WEBGL_provoking_vertex.
  - ANGLE (patched mozangle 0.7.1) with every extension: 40 of 40. Without EXT_clip_control 25 of
    40; without both and the BC extensions 25 of 40 (`present_shows_the_frame` passes there too).
  - The 4 unit tests of src/capture.rs pass. All 64 scene verdicts as expected.
  - Against the WebGPU back end: 5 of 6, the r32float comparison against the hardware filter
    failing as before.
  - Lint clean on both targets.
  - Before the window surface used DirectComposition, the test window read 240 in every byte (the
    window's own background painted over ANGLE's blit-model frame) and, with the paint suppressed,
    0 (black). Both were found by `present_shows_the_frame`.
- 2026-11-17, G3 run 7, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times, queued
  jobs, not timed; other tracks' jobs queued beside them). Merged origin/phase-2-graphics 688aff2
  and origin/phase-2-graphics-webgpu; 24 suite tests and 19 checks (43). Final tree, one job of
  all.cmd with `--desktop` (11:14 to 11:23), lint clean on both targets in its build:
  - Chrome 157.0.8103.78: 43 of 43 with every extension; 43 of 43 without WEBGL_stencil_texturing
    and the BC extensions; 28 of 43 without EXT_clip_control; 28 of 43 without it and
    WEBGL_provoking_vertex.
  - ANGLE (patched mozangle 0.7.1): 43 of 43 with every extension; 28 of 43 without
    EXT_clip_control; 28 of 43 without both and the BC extensions.
  - The 4 unit tests of src/capture.rs pass. All 64 scene verdicts as expected.
  - Against the WebGPU back end: 5 of 6, the r32float comparison against the hardware filter
    failing as before.
  - The job before (11:01 to 11:10), with the step rates, `TEXEL_BUFFERS` and the absent check but
    without the host-kept copies and sRGB BC views: 41 of 41 with every extension in Chrome and on
    ANGLE, 26 of 41 on the routes without the extensions, the same 5 of 6 against WebGPU.
- 2026-11-17, G3 run 8, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times, queued
  jobs, not timed; other tracks' jobs queued beside them; the desktop refused SSH sessions for a
  while at the start of the run, then answered). 24 suite tests and 20 checks (44).
  - After the merges and the size fix, before BGRA (job 11:32 to 11:41): Chrome 43 of 43 with every
    extension and without stencil texturing and BC; 28 of 43 on both routes without the clip
    extensions; ANGLE the same; 5 of 6 against WebGPU; all scene verdicts as expected; lint clean.
  - Final tree (job 11:48 to 11:58), lint clean on both targets in its build: Chrome 157.0.8103.78
    44 of 44 with every extension and 44 of 44 without WEBGL_stencil_texturing and the BC
    extensions, 29 of 44 without EXT_clip_control and 29 of 44 without it and
    WEBGL_provoking_vertex; ANGLE (patched mozangle 0.7.1) 44 of 44 with every extension, 29 of 44
    on both routes without the extensions (the same 15 known failures). The 4 unit tests of
    src/capture.rs pass; all scene verdicts as expected; 5 of 6 against WebGPU (r32float against the
    hardware filter, as before).
  - The size fix was not first run without it here; the failure it fixes was seen by the shaders
    track (`ps_size`: 2591 of 4096 texels differ on WebGL 2, every covered one sized 1 by 1).
- 2026-11-17, G3 run 9, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times; queued
  jobs, not timed; SSH refused new connections from before 12:00 to about 13:45 Pacific, the jobs
  ran through the checkout's still open shared connection). Merged origin/phase-2-graphics
  02ab37d. 24 suite tests and 25 checks (49).
  - First job: every Chrome device was lost (each made INVALID_ENUM by asking the sample counts
    of signed normalised formats); ANGLE 47 of 49 (the two new checks' own mistakes).
    `readPixels` returned a written -128 of `r8snorm` as -127 in Chrome. With `generateMipmap`
    on r32float (OES_texture_float_linear), the comparison with WebGPU's hardware filter passed.
  - Final tree (job 14:18 to 14:26 desktop time), lint clean on both targets in its build, the 4
    unit tests of src/capture.rs pass, all scene verdicts as expected: Chrome 157.0.8103.78 49 of
    49 with every extension and 49 of 49 without WEBGL_stencil_texturing and the format
    extensions, 34 of 49 without EXT_clip_control and 34 of 49 without it and
    WEBGL_provoking_vertex; ANGLE (patched mozangle 0.7.1) 48 of 49 with every extension, 33 of
    49 on both reduced routes; against WebGPU 5 of 6.
- 2026-11-17, G3 run 10, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times; queued
  jobs, not timed, other tracks' jobs beside them). Merged origin/phase-2-graphics e9387a6. 28
  suite tests and 27 checks (55).
  - First job (14:37 to 14:45, after the merge and the browser host's new tests only): Chrome 51
    of 53, ANGLE 50 of 53 with every extension (`a_scissor_past_the_target_is_clipped_to_it`,
    `comparison_samplers_compare_with_the_stored_depth`, and natively
    `generate_mipmap_equals_the_draws`); lint clean.
  - Final tree (job 15:46 to 15:53), lint clean on both targets in its build, the 4 unit tests of
    src/capture.rs pass, all 64 scene verdicts as expected: Chrome 157.0.8103.78 53 of 55 with
    every extension and 53 of 55 without WEBGL_stencil_texturing and the format extensions, 34 of
    55 without EXT_clip_control and 34 of 55 without it and WEBGL_provoking_vertex; ANGLE (patched
    mozangle 0.7.1) 53 of 55 with every extension, 34 of 55 on both reduced routes; against WebGPU
    5 of 6. The two failing checks everywhere are `generate_mipmap_equals_the_draws` and
    `levels_stay_as_written_while_others_are_read_and_drawn` (Known failures).
  - Chrome, `generate_mipmap_equals_the_draws` with `generateMipmap` on every shape (job ending
    15:38): every 2D, cube and 3D case equal to the draws; 16 by 8 by 2, layer 0, level 1 texel 0:
    `generateMipmap` [84, 138, 99, 79], box average [84.25, 137.75, 98.75, 79.0], the draws
    [0, 0, 0, 0]; layer 1 equal.
- 2026-11-17, G3 run 11, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times; queued
  jobs, not timed, other tracks' jobs beside them). 30 suite tests and 30 checks (60).
  - First job (ending about 16:20), with the copy route of the wrong guess: Chrome 157.0.8103.78
    and ANGLE 56 of 58 with every extension; the two level checks still failed in Chrome with the
    copy made (counted once per level), every layer but the last black. The two new suite tests,
    the viewport check and the merge passed. Lint clean.
  - Second job (16:35 to 16:43), the copy route removed and 2D array layers no longer invalidated:
    Chrome 60 of 60 with every extension and without stencil texturing and the format extensions,
    40 of 60 on both routes without the clip extensions. Natively every test crashed (access
    violation): the first build of the new ANGLE patch read a missing image. With that fixed (job
    ending about 16:58) the native suite gave 60 of 60.
  - Final tree after the shaders and rig merges (job 17:04 to 17:12), lint clean on both targets in
    its build, the 5 unit tests of src (capture.rs and the vertex input rewrite) pass, all 64 scene
    verdicts as expected: Chrome 60 of 60 with every extension and 60 of 60 without
    WEBGL_stencil_texturing and the format extensions, 40 of 60 without EXT_clip_control and 40 of
    60 without it and WEBGL_provoking_vertex; ANGLE (mozangle 0.7.1 with both patches) 60 of 60 with
    every extension and 40 of 60 on both reduced routes; against WebGPU 5 of 6. Every one of the 20
    failures on the reduced routes is `create_raster: Absent(CLIP_DEPTH_ZERO_ONE)` or
    `Absent(PROVOKING_VERTEX_FIRST)` from the suite's own raster blocks.
- 2026-11-17, G3 run 12, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times; queued
  jobs, not timed; CPU load 2% just after the last, 17:54). 30 suite tests and 38 checks (68).
  - First job with the new routes (ending about 17:40): Chrome 64 of 68 with every extension:
    `depth_clamped_...` and `depth_bias_clamped` (my programs' immediates needed naga's capability),
    `mirror_once_addressing` (my sample points sat on texel edges, where mirroring the texel index
    and mirroring the coordinate differ) and `clip_and_cull_distances` (see "Known failures").
    `depth-bias_depth` failed in 5568 of 16384 pixels by 1: every cleared depth read 0 (the
    `DepthReadByDraw` bug above). ANGLE 68 of 68.
  - Final tree (job ending 17:53), lint clean on both targets in its build, the 6 unit tests of src
    pass: Chrome 157.0.8103.78 68 of 68 with every extension and 68 of 68 without
    WEBGL_stencil_texturing, the format extensions and the new extensions; 48 of 68 without
    EXT_clip_control and 48 of 68 without it and WEBGL_provoking_vertex (the 20 known); ANGLE
    (mozangle 0.7.1 with both patches) 68 of 68 with every extension and 48 of 68 on both reduced
    routes; against WebGPU 5 of 6 (r32float against the hardware filter, as before). All scene
    verdicts as expected on all four runs.
  - The rig pictures in Chrome, with and without EXT_clip_control and WEBGL_provoking_vertex:
    `raster-fill`, `depth-bias`, `depth-clip` and `depth-clip_depth` exactly equal to real Direct3D
    11 (0 pixels differ); `depth-bias_depth` within its allowance (0 beyond 1.8e-7; 5332 pixels
    differ, by at most 6.0e-8, one 24-bit step). On ANGLE all five are skipped (mozangle lacks the
    extensions).

- Per-draw cost, run 11 (Chrome 157.0.8103.78 headless, RTX 3080; `gpu-job --alone`; the same scene
  and method as run 3; each launch of Chrome lost one WebGL context while its GPU process started):
  - Run ending 17:17, CPU load 21% just after. WebGL 2: 10,000 draws, one set 4.9 ms (fastest
    1.5 ms); a set per draw 9.0 ms (fastest 4.0 ms); 50,000 draws, one set 6.8 ms (0.136 us a
    draw); a set per draw 16.5 ms (0.330 us). WebGPU: 2.9, 3.8, 7.2 and 17.8 ms.
  - Run 17:21, CPU load 14% just after. WebGL 2: 4.6 ms (fastest 1.6), 6.3 ms (fastest 3.8), 6.0 ms
    (0.120 us a draw) and 16.5 ms (0.330 us). WebGPU: 2.3, 3.8, 7.3 and 17.5 ms.
  The 50,000-draw medians agree with runs 3 and 5 within about 20%. The 10,000-draw medians of both
  back ends were noisy against their fastest frames, as in run 5's first run; the fastest frames
  match the earlier medians. The cause of the noise was not found.
- Per-draw cost, run 5 (Chrome 157.0.8103.78 headless, RTX 3080; `gpu-job --alone`; the same scene
  and method as run 3). Since the merge, the cost page has to make the WebGPU device inside a JSPI
  export. Before that fix the page hung twice, once for 10 minutes and once for 2.
  - First measured run, ending 10:01:35, CPU load 13% just after. The WebGL 2 medians were noisy
    against their fastest frames:
    - WebGL 2, 10,000 draws: one set 6.4 ms (fastest 1.4 ms); a set per draw 10.8 ms (fastest
      3.8 ms).
    - WebGL 2, 50,000 draws: one set 9.6 ms; a set per draw 16.5 ms.
    - WebGPU: 1.6, 3.8, 7.8 and 18.0 ms.
    - Chrome's first launch in this run kept no WebGL context.
  - Second run, alone from 10:06:04, CPU load 10% just after:
    - WebGL 2, 10,000 draws: one set 1.5 ms (0.150 us a draw); a set per draw 4.3 ms (0.430 us).
    - WebGL 2, 50,000 draws: one set 5.7 ms (0.114 us); a set per draw 17.9 ms (0.358 us).
    - WebGPU, 10,000 draws: 1.6 ms (0.160 us) and 4.0 ms (0.400 us).
    - WebGPU, 50,000 draws: 7.8 ms (0.156 us) and 19.1 ms (0.382 us).
  The second run agrees with run 3 to within about 15%. Why the first run's WebGL 2 frames stalled
  was not found. Only the first run followed a Chrome launch that had failed.
- In the first full check of G3 run 1, Chrome's first launch kept no WebGL 2 context in ten tries
  and was closed; the second launch worked. In the other launches one context was lost first.
- Chrome's GPU process restarted once just after Chrome started in most runs (the first WebGL
  context lost, or none made); run.mjs waits for a context to survive half a second first and says
  how many did not.
- The headless Chrome the rig drives reports in chrome://gpu "WebGL: Hardware accelerated but at
  reduced performance" (and the same for WebGPU). Per-draw timings taken there (G3's later work)
  must say so; whether a headed Chrome differs is not measured.

## Measurements, run G3b-8

- filter_draw's stream replayed natively on mozangle's ANGLE, changed by mode (scratch
  g3b8/diag/stream-diag.rs and .cmd, put into examples/ for the run only; desktop, RTX 3080,
  `gpu-job --alone`, 01:39 to 01:43 desktop time, nothing else on the GPU; logs
  g3b8/diag1-task.log, adapter memory each second in g3b8/*-0.csv). The stream holds 419 draws,
  each with a new program from new shaders; six distinct shaders (naga expression counts 51, 64,
  742, 742, 821, 821), so four distinct programs. Each mode spent about 39 to 41 s before its
  first 24 draws (the four links among it). At the code before this run:
  - as recorded: device lost at draw 419 (read back of that draw), the adapter's dedicated
    memory 354 MB before the draws, rising about 32 MB a second with the draws to 629 MB, then
    352 MB after the process ended;
  - shaders destroyed right after their program was made: lost at draw 419, memory to 616 MB;
  - each draw's program and shaders destroyed after it: no loss, memory to 492 MB (the textures
    and buffers each draw makes stay alive);
  - shaders of equal content made once (so programs too, by the back end's pair cache): no
    loss, memory stayed at 352 to 353 MB, and draws 25 to 419 took 0.5 s, against 8.6 s as
    recorded.
  The memory rose by about 255 MB over the last 395 programs: about 650 KB a live program.
- The shaders crate's `tests/filter_draw.rs`, seeds 1 to 16 in one process
  (`WCONTAINERS_FILTER_CASES=16`, `gpu-job --alone --desktop`, desktop 02:08 to 02:09, nothing
  else on the GPU; log g3b8/job2.log), at 7c26c4b8's tree: 792 comparisons, no device lost; 2
  failed, "9 Rgba8Unorm bias dxbc webgpu's own sampler" and the same through dxil (the WebGPU
  back end's native sampler, not this back end); 64 known anisotropic failures, the shaders
  track's tap footprint, on both back ends. Every other WebGL 2 comparison passes.
- filter.bin replayed in Chrome 157 on the RTX 3080 at 7c26c4b8 (`gpu-job --alone --desktop`,
  log g3b8/replay.log), twice per back end: WebGL 2 "1 frames, 0 calls failed; Ok ... slow
  paths: PipelineMiss 2" both times (run 7: context lost at frame 0); WebGPU "0 calls failed;
  Ok ... ReadBackWait 419, PipelineMiss 419" (it builds a pipeline for every program).
- The whole run at 7c26c4b8 (lint of every build first, clean; all.cmd, `gpu-job --desktop`,
  not alone, desktop 02:25 to 02:28; log g3b8/all.log, outputs C:\wc\out\webgl\g3b8-all):
  as run 7's, each suite count one higher for the new check. Unit tests 15 and the trace round
  trip pass. Natively the suite 105 of 105 with every extension, 77 passed and 28 failed on each
  route without EXT_clip_control (the known ones: the suite's own raster blocks). In Chrome 102
  passed, 3 not run, 0 failed; without EXT_clip_control, and without it and
  WEBGL_provoking_vertex, 72 passed, 3 not run, 30 failed (known, as natively); without stencil
  texturing, the BC extensions, noperspective and float blits 94 passed, 11 not run, 0 failed.
  Against WebGPU natively 9 of 10 and in Chrome 11 of 13 (the known r32float mip and exact-tie
  differences); BC through GL exactly equal to Direct3D 11, BC decoded on the host the known
  failure.
- Every Direct3D 11 scene through the front's WebGL 2 build on ANGLE at 7c26c4b8 (`scenes
  --api d3d11 --dll d3d11.dll=...webgl...`, desktop 02:42; log g3b8/d3d11-scenes.log): 92
  passed, 21 failed, 30 absent (run 7: 88, 27, 30), as the front's run 25 found. The 21 are all
  the outputs of raster, views, geometry and streamout, each of which the front refuses by name
  (`E_NOTIMPL`): polygon mode (absent on mozangle, built in Chrome), texture views, and the
  geometry stage and stream output (compute programs); none differs from real Direct3D 11.

## Measurements, run G3b-7

- The frame-time breakdown after the twin refresh by draw (84f45baf; method and machine as in run
  G3b-6: desktop i9-9900K, 16 threads, RTX 3080, driver 32.0.16.2101; frames/run 3 10 30; each
  program run alone in the queue's hazard group, other tracks' builds and remote commands beside;
  load is the whole machine's `% Processor Time` from `typeperf` every 2 s, over each recorded
  run). Logs: scratch g3b7/frames-atg2.log, frames-inst2.log; load g3b7/load-atg2.csv,
  load-inst.csv.
  - atg-simplelightingpc (59.92 frames a second: 512 frames from 8.91 s to 17.46 s), per frame:
    front 0.020 ms, back end 0.005 ms, ANGLE 0.083 ms, the driver 0.28 ms, the program 0.028 ms;
    under the back end's functions 0.060 ms (ANGLE 0.046). Before (run G3b-6): 16.3 ms under the
    back end's functions, 12.8 of them waiting in `Gl::upload`; ANGLE 2.34 ms. Under the back end's
    functions only ANGLE's part is above 0.01 ms a frame now. Load: short run 4.3 % (2.8 to
    8.9), long run 3.7 % (2.4 to 7.8). The run before the fix (scratch g3b7/frames-atg.log, its
    long run finished by g3b7/finish-frames.sh after the Mac's client was stopped) still had
    0.76 s a second under `Gl::upload` from `copy_through_pbo`, which a temporary print (not
    committed; g3b7/pbo-diag.log) showed to be `Bgra8UnormSrgb` to `Bgra8Unorm`, 1280 by 720,
    every second frame: the twin refresh.
  - atg-simpleinstancingpc (59.98 frames a second: 512 frames from 9.06 s to 17.60 s): its short
    recorded run crashed at start-up (the 0xc0000005 under Known failures), so the difference of
    the two runs counts the long run's start-up and over-states every part, by up to half. As
    upper bounds a frame: front 0.039 ms, back end 0.005 ms, ANGLE 0.21 ms, driver 0.65 ms; under
    the back end's functions 0.26 ms (run G3b-6: 16.1 ms). Load over the long run 5.2 % (2.2 to
    16.0).
  - Counts a frame, both samples: `CopyThroughBuffer` every second frame (the twin refresh, now a
    draw), 3 and 5 `MappedUploadWhole` (the front).
- Per draw in Chrome on the same frames (SimpleLightingPC's recorded stream replayed, run G3b-2's
  method; `gpu-job --alone --desktop`, Chrome 157.0.8103.78, scratch g3b7/replay.log, desktop
  00:54): WebGL 2 28.1 us a draw, whole frame median 0.360 ms (calls untimed), inside the back end
  0.330 ms; WebGPU 53.8 us a draw, whole frame 0.725 ms, inside the back end 0.700 ms. Run G3b-3
  had 23.3 against 53.9 us and 0.27 against 0.73 ms; the twin refresh is now a draw, counted
  `CopyThroughBuffer` 291 times in 580 frames. The front does not run in Chrome yet (no
  WebAssembly build of it), so its share is the native one above.
- bgfx-01-cubes through frames/run (`--d3d11`): no `webgl: frames=` line in its 10 s run, only
  ANGLE's two device warnings; it draws nothing through the WebGL 2 build, as in runs 2 and 5.
- The full run after the merges (all.cmd, scratch g3b7/all1.log, desktop 22:57 to 23:04, queued
  beside other tracks' work, not timed), at 4dd6c481: unit tests 15 of 15. Chrome 100 passed, 3
  not run, 0 failed with every extension; 92 passed and 11 not run on the reduced route; 70
  passed, 3 not run, 30 failed without EXT_clip_control, the same without it and
  WEBGL_provoking_vertex. ANGLE 103 of 103 with every extension, 75 of 103 on both reduced
  routes (every failure the suite's own raster blocks). Against WebGPU natively 9 of 10, in
  Chrome 11 of 13 routes (the known two). BC reference 1 of 2 (the known decoded route). Clippy
  with warnings denied, native and wasm32: clean.
- The copy and twin checks after each change (some.cmd, scratch g3b7/srgb2.log and twin1.log):
  16 of 16 and 18 of 18, on ANGLE and in Chrome.
- The full run at the last code commit (13e75e3d: all.cmd, with clippy as its build,
  scratch g3b7/all2.log, desktop 01:16 to 01:21, queued beside other tracks' work, not timed):
  unit tests 15 of 15, the trace round trip (`--features record --test trace`) 1 of 1. Chrome
  101 passed, 3 not run (clip and cull distances, timestamps, fractional bias), 0 failed with
  every extension; 93 passed and 11 not run on the reduced route; 71 passed, 3 not run and 30
  failed without EXT_clip_control, the same without it and WEBGL_provoking_vertex. ANGLE 104 of
  104 with every extension, 76 of 104 on both reduced routes (every failure
  `Absent(CLIP_DEPTH_ZERO_ONE)` or `Absent(PROVOKING_VERTEX_FIRST)` from the suite's own raster
  blocks). Against WebGPU natively 9 of 10, in Chrome 11 of 13 routes (the known two). BC
  reference 1 of 2 (the known decoded route). Clippy with warnings denied: native all targets,
  `record` with the trace test, wasm32 (browser, draw-cost; replay with `trace`): clean.
- The Direct3D 11 front's scenes through its WebGL 2 build, natively on ANGLE (rig `scenes --api
  d3d11 --dll d3d11.dll=...\webgl\debug\wcontainers_directx.dll`, desktop 01:08 to 01:16,
  scratch g3b7/d3d11-scenes1.log): 88 pass, 27 fail, 30 absent, as in runs 4 to 6; the same six
  scenes stop in the front (constants 3, geometry 8, queries 3, raster 5, streamout 4, views 4
  outputs); formats (bgra8_srgb among them, drawn through the twin), targets, mips and samplers
  pass with largest difference 0 where shown.
- The literal check at the head: exit 0, 0 marks (g3b7/check5.log).

## Measurements, run G3b-6

- The frame-time breakdown, 2026-11-18 20:33 to 21:42 desktop time: desktop (i9-9900K, 16
  threads, RTX 3080, driver 32.0.16.2101), each program run alone in the queue's hazard group
  with other tracks' builds and remote commands beside. Load: the whole machine's
  `\Processor(_Total)\% Processor Time` sampled every 2 s by `typeperf` over SSH, averaged over
  each recorded run's job. Method: crates/wcontainers-gpu-webgl/frames/run (How to run), 10 s
  and 30 s runs, per second from their difference, divided by the frames a second between the
  last two `webgl: frames=` reports of a long run. The front's libraries are the release
  Direct3D 11 build on this back end over mozangle's ANGLE (feature level 10_0), so "front"
  includes the interface helpers it calls, "back end" this crate and glow, "angle" ANGLE's
  libraries. Logs: scratch g3b6/frames-atg.log, frames-atg2.log, frames-doom.log,
  frames-inst.log; load g3b6/load-atg.csv, load-run6b.csv.
  - atg-simplelightingpc (4 draws, 2 passes a frame; paced by the display: 59.98 frames a
    second, 512 frames from 8.95 s to 17.48 s of the second set's long run): per frame front
    0.009 ms, back end 0.007 ms, the program 0.015 ms; ANGLE 2.34 ms; the driver 2.05 ms. Under
    the back end's functions 16.3 ms a frame, of which 12.8 ms in ntdll under
    `wcontainers_gpu_webgl::Gl::upload` (texture writes: `texSubImage` through ANGLE waiting for
    the GPU, spinning), 2.31 ms ANGLE and 0.47 ms the driver. The program's thread was busy all
    30 s. Load: short run 5.4 % (3.7 to 8.6), long run 24.1 % (3.3 to 56.8). Counts a frame
    (frame report): 3 `MappedUploadWhole` (the front), `CopyThroughBuffer` every second frame.
    From the first set (frames-atg.log; its report lines were not yet timed, the rate is the
    second set's). The second set's short recorded run ended with 0xc0000005 before its 10 s
    (its wall time over the difference came to 1.40 s a second), so its breakdown is not used;
    the crash was not read further (Next 1).
  - chocolate-doom (attract mode, 35 frames a second: 256 frames from 8.37 s to 15.69 s): per
    frame front 0.023 ms, back end 0.011 ms, ANGLE 0.116 ms, driver 0.79 ms (mostly its own
    threads); under the back end's functions 0.153 ms (ANGLE 0.071, the driver 0.056). Counts a
    frame: 1 `FormatConverted` (the BGRA frame swapped on the host), 3 `MappedUploadWhole`. Load:
    short 6.4 % (2.9 to 12.4), long 3.4 % (1.7 to 8.0).
  - atg-simpleinstancingpc (59.92 frames a second: 512 frames from 9.13 s to 17.67 s): per frame
    front 0.009 ms, back end 0.009 ms, ANGLE 2.39 ms, driver 2.06 ms; under the back end's
    functions 16.1 ms, of which 12.6 ms in ntdll under `Gl::upload`. Counts a frame: 5
    `MappedUploadWhole`, `CopyThroughBuffer` every second frame. Load: short 7.8 % (4.1 to
    18.9); the long run fell after the sampling window, not measured.
  - Against the WebGPU back end (its status, run 21, same desktop, same method but to frames
    through the rig's forwarder): chocolate-doom front 0.037 ms, back end 0.013, wgpu 0.027 a
    frame (uncapped `-timedemo` there, attract mode here); atg-simpleinstancingpc front, back end
    and wgpu under 0.03 ms each, 0.163 ms under the back end. So our own code costs about the
    same on both (front 0.01 to 0.02 ms, back end about 0.01 ms a frame); the WebGL 2 route's
    frame goes to ANGLE (0.1 to 2.4 ms a frame) and, in the ATG samples, to waiting inside
    texture writes (about 12.7 ms of a 16.7 ms frame, CPU spinning in the driver).
  - Per draw on the same frames in Chrome (the replayed stream of SimpleLightingPC, run G3b-3,
    not repeated: no change since touches those calls' routes): WebGL 2 23.3 us a draw, WebGPU
    53.9 us; whole frame 0.27 ms against 0.73 ms. The front's share above is native; in Chrome
    the front does not run yet (no WebAssembly build of it).
- The full run at the head before this status commit (all.cmd with wasm32 clippy as its build,
  out C:\wc\out\webgl\g3b6-all2, scratch g3b6/all2.log, desktop about 22:10, queued beside
  other tracks' jobs, not timed): unit tests 15 of 15. Chrome 100 passed, 3 not run (clip and
  cull distances, timestamps, fractional bias), 0 failed with every extension; 92 passed and 11
  not run on the reduced route (now also without NV_shader_noperspective_interpolation, so the
  carried linear route runs in Chrome too); 70 passed, 3 not run and 30 failed without
  EXT_clip_control, the same without it and WEBGL_provoking_vertex. ANGLE 103 of 103 with every
  extension, 75 of 103 on both reduced routes; every reduced-route failure is
  `Absent(CLIP_DEPTH_ZERO_ONE)` or `Absent(PROVOKING_VERTEX_FIRST)` from the suite's own raster
  blocks (the suite's raster blocks; one more in Chrome than natively: the per-sample test that
  now runs there). Against WebGPU natively 9 of 10, in Chrome 11 of 13 routes (the known two),
  the new level-zero gather comparison equal on both. BC reference 1 of 2 (the known decoded
  route). The run before it (all1.log) found the single-sample mask failure fixed since.
  Clippy with warnings denied: native all targets, `record` with the trace test, wasm32
  (browser, draw-cost; replay with `trace`), and the WebGPU crate's frames example: clean. The
  literal check at the head: exit 0, nothing to do, 0 marks.
- The new and touched checks (scratch g3b6/new3.log, mask1.log): `programs_run_per_sample_and_write_the_sample_mask`,
  `linear_values_interpolate_in_window_space`, `absent_features_are_refused` and the suite's
  per-sample tests pass in Chrome and on ANGLE (the per-sample check not run natively: absent).
- The Direct3D 11 front's scenes through its WebGL 2 build, natively on ANGLE (rig `scenes --api
  d3d11` with the build's three libraries, desktop 22:03, scratch g3b6/d3d11-scenes1.log): 88
  pass, 27 fail, 30 absent, as in runs 4 and 5; the same six scenes stop in the front
  (constants 3, geometry 8, queries 3, raster 5, streamout 4, views 4 outputs); formats,
  targets, mips and samplers pass.

## Measurements, run G3b-5

- 2026-11-18, desktop (i9-9900K, RTX 3080, Chrome 157.0.8103.78), queued jobs in the hazard
  group beside other tracks' builds, load not sampled. 102 suite tests and checks (the two new
  checks among them).
  - The new and touched checks (some.cmd "linear_values special_float absent_features half_floats
    bgra_textures bgrx_textures", out C:\wc\out\webgl\g3b5-new, scratch g3b5/new4.log): 6 of 6 on
    ANGLE and 6 of 6 in Chrome; unit tests 14 of 14. Before the fix, `special_float_...` failed
    on both (new3.log, values under "G3b: what the fronts ask").
  - Linear values (ANGLE, new4.log): at most 1.07e-4 from the window-space value, while the GPU's
    own perspective-correct values are at most 1.17e-4 from the host's; the perspective-correct
    value is up to 0.24 from the linear one. The first version of the check asked for 1e-4 and
    failed at 1.07e-4 (first.log), which is the GPU's own interpolation precision, so the bar is
    now 1e-3 with both numbers printed.
  - Full run (all.cmd, out C:\wc\out\webgl\g3b5-all1, scratch g3b5/all1.log, at 34c7dfd8's tree):
    Chrome 99 passed, 3 not run (clip and cull distances, timestamps, fractional bias), 0 failed
    with every extension; 91 passed and 11 not run on the reduced route; 70 passed, 3 not run and
    29 failed without EXT_clip_control, the same without it and WEBGL_provoking_vertex. ANGLE 102
    of 102 with every extension, 74 of 102 on both reduced routes; all 56 reduced-route failures
    natively are `Absent(CLIP_DEPTH_ZERO_ONE)` or `Absent(PROVOKING_VERTEX_FIRST)` from the
    suite's own raster blocks. Against WebGPU natively 8 of 9, in Chrome 10 of 12 routes (the
    known two). BC reference 1 of 2 (the known decoded route). Clippy with warnings denied:
    native all targets, wasm32 (browser, draw-cost, replay with `trace`): clean.
- The Direct3D 11 front's scenes through its WebGL 2 build, every scene, natively on ANGLE
  (scratch g3b5/d3d11-scenes1.log): 88 outputs pass, 27 fail, 30 absent, as in run 4, the same
  six scenes stopping in the front (raster and views `E_NOTIMPL`; constants "a stage reads more
  uniform buffers than Limits::uniform_buffers_per_stage"; queries "counting the samples of a
  pixel shader variant"; geometry and streamout `E_NOTIMPL`).
- Recording bgfx-01-cubes (125 draws a frame) through the recording build with `--d3d11` (scratch
  g3b5/record2.log): ANGLE made two devices, but no stream was written in 10 s (the trace files
  kept their 09:49 times, so no recording device was made); as in run 2, bgfx draws nothing
  through the WebGL 2 build. Not read further (the Direct3D 11 track's ground). So the per-draw
  comparison still rests on SimpleLightingPC's 4 draws and 2 passes a frame (run G3b-3), and the
  front's own share is not measured: it ran natively while recording, and the WebGPU track's
  profiler route (frames/run) runs programs through the rig's forwarder, which recurses with
  this back end's ANGLE.

## Measurements, run G3b-4

- 2026-11-18, desktop (i9-9900K, RTX 3080, driver 32.0.16.2101, Chrome 157.0.8103.78), queued
  jobs beside other tracks' work, not timed; the Mac at load 60 to 110 (only formatting ran on
  it). 100 suite tests and checks natively (the new `depth_textures_compared_and_read_plainly`
  among them).
  - Full run at the head before this status commit (all.cmd with clippy as its build, out
    C:\wc\out\webgl\g3b4-all1, scratch g3b4/all1.log, desktop 15:13 to 15:18): clippy with
    warnings denied clean on native all targets, wasm32 (browser, draw-cost, replay with `trace`)
    and `record`. Unit tests 12 of 12; trace round trip 1 of 1. Chrome 97 passed, 3 not run (clip
    and cull distances, timestamps, fractional bias), 0 failed with every extension; 89 passed
    and 11 not run on the reduced route; 68 passed, 3 not run and 29 failed without
    EXT_clip_control, the same without it and WEBGL_provoking_vertex. ANGLE 100 of 100 with every
    extension, 72 of 100 on both reduced routes. Every reduced-route failure is
    `Absent(CLIP_DEPTH_ZERO_ONE)` or `Absent(PROVOKING_VERTEX_FIRST)` from the suite's own raster
    blocks, as in run 3. Against WebGPU natively 8 of 9, in Chrome 10 of 12 routes (the known two),
    BC reference 1 of 2 (the known decoded route).
  - The depth, gather and stencil checks and suite tests (some.cmd "depth gather stencil", scratch
    g3b4/depth1.log): 17 of 17 on ANGLE, 16 passed and 1 not run (fractional bias) in Chrome.
- The Direct3D 11 front's scenes through its WebGL 2 build, all scenes, natively on ANGLE (the
  rig's `scenes --api d3d11 --dll d3d11.dll=...\webgl\debug\wcontainers_directx.dll`, at the
  merge, desktop 14:29 to 14:30, scratch g3b4/d3d11-scenes1.log): 88 outputs pass, 27 fail, 30
  absent; the failing scenes are listed under "G3b: what the fronts ask".
- The 16-seed filter run's diagnosis (`gpu-job --alone`, scratch g3b4/diag1.log and diag2.log):
  under "G3b: what the fronts ask". The first case of each run took 41 to 54 s (its programs'
  first compiles), every later case about 1.8 s.
- The frame-time measurement was not repeated in this run (run G3b-3's stands; no change in this
  run touches the replayed calls' routes).

## Measurements, run G3b-3

- 2026-11-18, desktop (i9-9900K, RTX 3080, driver 32.0.16.2101, Chrome 157.0.8103.78), queued
  jobs beside other tracks' work, not timed. 99 suite tests (the shared line's two new ones
  included) and 55 checks.
  - The new and touched checks (scratch g3b3/new2.log): 14 of 14 on ANGLE and 14 of 14 in Chrome
    (`bgrx_...`, `bgra_...`, `stages_past_the_uniform_buffer_limit_are_refused`,
    `broken_rules_...`, the snorm, typeless, within-one-subresource, gather, half float and two
    device checks); unit tests 11 of 11; `cube_gathers_across_face_edges_equal_webgpu` equal
    natively and in Chrome. The first native run of the limit check, with the limit at 12,
    aborted ANGLE (scratch g3b3/bgrx2.log; Departures).
  - Full run (all.cmd, out C:\wc\out\webgl\g3b3-all2, scratch g3b3/all2.log): Chrome 96 passed,
    3 not run (clip and cull distances, timestamps, fractional bias), 0 failed with every
    extension; 88 passed and 11 not run on the reduced route; 67 passed, 3 not run and 29 failed
    without EXT_clip_control, and the same without it and WEBGL_provoking_vertex. ANGLE 99 of 99
    with every extension, 71 of 99 on both reduced routes. Every reduced-route failure (28
    natively, 29 in Chrome; one more each than run 2: the new `an_indexed_draw_past_...`) is
    `Absent(CLIP_DEPTH_ZERO_ONE)` or `Absent(PROVOKING_VERTEX_FIRST)` from the suite's own raster
    blocks (counted in suite-angle-noclip.txt and -neither.txt: 28 panics, 0 other). Against
    WebGPU natively 8 of 9, in Chrome 10 of 12 routes (the known two). BC reference 1 of 2 (the
    known decoded route). Unit tests 11 of 11. Clippy with warnings denied: native all targets,
    `record`, wasm32 (browser, draw-cost, replay with `trace`): clean.
  - The first full run (scratch g3b3/all1.log) found the 3D copy (native, the new suite test) and
    the two tests the browser host lacked.
  - After the B5G6R5 and R11G11B10 read backs, at the pushed head (all.cmd, out
    C:\wc\out\webgl\g3b3-all3, scratch g3b3/all3.log): the same counts on every route as all2
    (Chrome 96 + 3 not run; 88 + 11; 67 + 3 + 29 known twice; ANGLE 99 of 99, 71 of 99 twice, 28
    panics on each reduced route, all from the suite's raster blocks), against WebGPU and BC as
    above, unit tests 11 of 11, the trace round trip (`--features record --test trace`) 1 of 1,
    clippy clean on every build above. The read back checks with the two formats (scratch
    g3b3/small2.log): 5 of 5 on ANGLE and in Chrome.
- The Direct3D 11 front's scenes through its WebGL 2 build (`cargo build -p wcontainers-directx
  --features webgl --target-dir %CARGO_TARGET_DIR%\webgl`, the rig's `scenes --api d3d11 --dll
  d3d11.dll=...`, natively on ANGLE, against the rig's real Direct3D 11 pictures):
  - formats, targets and mips (scratch g3b3/d3d11-scenes3.log): 43 passed, 0 failed, 7 absent.
    Every format's picture equals real Direct3D 11's within the rig's tolerance (largest
    difference 0), `bgrx8_unorm`, `b5g6r5_unorm` and `rg11b10_float` included; absent: the two
    snorm targets (no EXT_render_snorm in mozangle, so `CheckFormatSupport` reports no target and
    the caps line says less than the GPU), and the four targets outputs of feature level 10_1.
    Before this run's format work (d3d11-scenes1.log) formats stopped with `E_NOTIMPL`.
  - constants (d3d11-scenes2.log): the front's device is removed with "the back end refused a
    call: a stage reads more uniform buffers than Limits::uniform_buffers_per_stage", which is
    this run's early refusal reaching the front; the scene needs the front to bind its constants
    through a texture past the limit (the Direct3D 11 track's).
- The frame-time measurement again, 2026-11-18 13:00 desktop time, `gpu-job --alone --desktop`,
  CPU load 4% just before and 4% just after, Chrome 157.0.8103.78 headless on the RTX 3080, the
  same stream as run 2 (C:\wc\out\webgl\trace\atg-simplelightingpc.bin.1, SimpleLightingPC
  through the Direct3D 11 library on this back end, 580 frames). Result scratch
  g3b3/atg-simplelightingpc-replay3.txt, log g3b3/replay1.log. All four replays finished (0 calls
  failed). Frames 10 to 579:
  - WebGL 2: whole frame median 0.270 ms (fastest 0.050, slowest 1.985) with calls untimed (run 2:
    5.34 ms). Inside the back end, per frame: draws 0.093 ms (23.3 us a draw), begin_pass 0.035
    ms (17.6 us each), buffer writes 0.035 ms (11.6 us each), submit 0.029 ms (run 2: 5.574 ms),
    present 0.016 ms, fence_done 0.011 ms, the rest under 0.005 ms. Slow paths: `CopyThroughBuffer`
    291 (every second frame, now queued without a wait), `PipelineMiss` 1.
  - WebGPU: whole frame median 0.725 ms (fastest 0.125, slowest 3.070); run 2 0.78, check9 could
    not reproduce it because the page hung. Per frame: draws 0.216 ms (53.9 us a draw), begin_pass
    0.127 ms (63.5 us each), current_texture 0.089 ms, submit 0.084 ms, buffer writes 0.054 ms
    (18.1 us each), present 0.030 ms, the rest under 0.011 ms. Slow paths: `PipelineMiss` 3.
  - So on these frames our WebGL 2 side costs about a third of our WebGPU side per frame (0.27
    against 0.73 ms), and a draw 23 against 54 us. Measured: our back ends' share only (the
    Direct3D 11 front ran natively while recording; Departures). Not measured: the GPU's own time
    and the browser's GPU process.

## Measurements, run G3b-2

- 2026-11-18, desktop (i9-9900K, RTX 3080, driver 32.0.16.2101, Chrome 157.0.8103.78), queued
  jobs beside other tracks' work, not timed. 39 + 3 suite tests and 53 checks (95).
  - Full run after the merge (all.cmd, out C:\wc\out\webgl\g3b2-all1, scratch g3b2/all1.log):
    ANGLE 95 of 95 with every extension; 68 of 95 on both reduced routes (the 27 known: run 1's 25
    and the two new suite tests that need a raster block, all `Absent(CLIP_DEPTH_ZERO_ONE)` from the
    suite's own blocks). Chrome 89 passed, 3 not run, and 3 failed only because the browser host
    lacked the three new tests; after adding them (some.cmd, out C:\wc\out\webgl\g3b2-new,
    scratch g3b2/new3.log) they pass in Chrome and on ANGLE (3 of 3 each). Against WebGPU natively
    7 of 8, in Chrome 9 of 11 (the known two). BC reference 1 of 2 (the known decoded route).
    Unit tests 10 of 10. Clippy with warnings denied: native all targets, `record` (library and
    trace test), wasm32 (browser, draw-cost, replay with `trace`): clean.
  - The last full run, at the pushed head (all.cmd, out C:\wc\out\webgl\g3b2-all2, scratch
    g3b2/all2.log): Chrome 92 passed, 3 not run, 0 failed with every extension; 84 passed and 11
    not run on the reduced route; 64 passed, 3 not run and 28 known failures (26 and the two new
    tests, all from the suite's raster blocks) without EXT_clip_control, the same without it and
    WEBGL_provoking_vertex. ANGLE 95 of 95, and 68 of 95 on both reduced routes (the 27 known).
    Against WebGPU and the BC reference as above; unit tests 10 of 10; trace test 1 of 1.
  - The trace round trip (tests/trace.rs, seed none: one fixed scene): 1 of 1, equal texels on a
    new WebGL 2 back end and on the WebGPU back end (wgpu, Direct3D 12).
- The frame-time measurement, 2026-11-18 09:58 desktop time, `gpu-job --alone --desktop`, CPU load
  2% just before and 4% just after (Win32_Processor LoadPercentage), Chrome 157.0.8103.78 headless
  on the RTX 3080 (WebGL 2 through ANGLE's Direct3D 11, WebGPU through Dawn's Direct3D 12).
  Program: DirectX Tool Kit's SimpleLightingPC (the rig's atg-simplelightingpc), recorded for 10 s
  through the Direct3D 11 library built on this back end with `record` over ANGLE (feature level
  10_0): 580 frames, 713,736 bytes (scratch g3b2/atg-simplelightingpc.bin; result
  g3b2/atg-simplelightingpc-replay2.txt, log g3b2/replay2.log). Each back end replayed it twice;
  0 calls failed on either. Frames 10 to 579 (570 frames), each frame 2 passes, 4 draws, 3 buffer
  writes, 2 binding sets made, 1 submit, 1 present:
  - WebGL 2: whole frame median 5.34 ms (fastest 0.32, slowest 18.79) with calls untimed. Inside
    the back end, per frame: submit 5.574 ms (one call), draws 0.092 ms (23.1 us a draw),
    buffer writes 0.037 ms (12.3 us each), begin_pass 0.036 ms (18.1 us each), present 0.020 ms,
    fence_done 0.011 ms, the rest under 0.004 ms. Slow paths over the replay: `CopyThroughBuffer`
    291 (every second frame), `PipelineMiss` 1.
  - WebGPU: whole frame median 0.78 ms (fastest 0.13, slowest 3.14). Inside the back end, per
    frame: draws 0.212 ms (53.0 us a draw), submit 0.128 ms, begin_pass 0.126 ms (62.9 us each),
    current_texture 0.092 ms, buffer writes 0.053 ms (17.8 us each), present 0.031 ms, the rest
    under 0.011 ms. Slow paths: `PipelineMiss` 3.
  - So on these frames a WebGL 2 draw costs our side less than half a WebGPU one (23 against 53
    us), but the WebGL 2 frame is about seven times longer, nearly all of it in `submit`.
    `submit` calls `getError`, which in Chrome waits for the GPU process to run everything sent
    before it; every second frame that includes refreshing a texture's twin through a pixel buffer
    (`copy_whole` in `fresh`, the `CopyThroughBuffer` count), which a draw only queues (its own 23
    us). Which twin the program's frames refresh, and how the wait divides between the two, is not
    measured yet (Next 1).
  - A first replay (09:52, same job conditions, frames alternately timed per call and whole, scratch
    g3b2/replay1.log) gave WebGL 2 whole frames 10.8 ms and WebGPU 0.83 ms; the alternation put
    every frame with the twin copy among the untimed ones, so the page now replays twice instead.
  - Our front's own time is not in these numbers: the front runs natively while recording, and has
    no WebAssembly build.
- Programs that did not record: through the rig's forwarder every program recursed (above, How to
  run); with the libraries beside them, bgfx-01-cubes made 5 devices that drew nothing in 10 s and
  Godot's 2D platformer (Godot's ANGLE on our library on our ANGLE) still overflowed its stack.
  Neither was read further (the Direct3D 11 track's ground).

## Measurements, run G3b-1

- 2026-11-18, G3b run 1, desktop (RTX 3080, Chrome 157.0.8103.78), queued jobs beside other
  tracks' work, not timed. 39 suite tests and 53 checks (92), the 5 new checks among them.
  - The new checks and those they touch (some.cmd, out C:\wc\out\webgl\g3b-new, scratch
    g3b/new2.log): 11 of 11 on ANGLE and 11 of 11 in Chrome (`bit_operations_...`, `gathers_...`,
    `half_floats_...`, `two_devices_...`, `signed_normalised_minimum_...`, the snorm, typeless and
    within-one-subresource copy checks). Natively both snorm checks report "not run: Rgba8Snorm
    does not render here". Seed 0, one case each.
  - Full run (all.cmd, out C:\wc\out\webgl\g3b-all1, scratch g3b/all1.log): Chrome 89 passed, 3 not
    run (clip and cull distances, timestamps, fractional bias), 0 failed with every extension; 81
    passed and 11 not run on the reduced route; 63 passed, 3 not run and the 26 known failures
    without EXT_clip_control, and the same without it and WEBGL_provoking_vertex. ANGLE 92 of 92
    with every extension (lines "not run:" for the absent features and the two snorm checks), 67
    of 92 on both reduced routes (the 25 known, every one `Absent(CLIP_DEPTH_ZERO_ONE)` or
    `Absent(PROVOKING_VERTEX_FIRST)` from the suite's own raster blocks). Against WebGPU natively
    7 of 8, in Chrome 9 of 11 routes (the known two). BC reference 1 of 2 (the known decoded
    route). Unit tests 10 of 10. Scene and rig verdicts as expected. Clippy with warnings denied,
    native and wasm32: clean.
  - The same full run again after the 10-10-10-2 read back and the loss messages (out
    C:\wc\out\webgl\g3b-all2, scratch g3b/all2.log, the commit before this status's last):
    identical counts on every route, unit tests 10 of 10. The copy checks touching 10-10-10-2
    (8, some.cmd, scratch g3b/packed1.log) passed on ANGLE and in Chrome before it.
- The Direct3D 11 front's scenes through its WebGL 2 build (the library built with
  `--features webgl` over this back end on ANGLE, the rig's references; scenes command as in the
  Direct3D 11 status, with `git submodule update --init --recursive -q` first in the build;
  scratch g3b/d3d11-webgl5.log, at ddcb5223 plus the 10-10-10-2 read back): 65 outputs pass, 56
  fail, 25 absent (feature levels 10_1 and 11_0). The front's run 18 had 43 passing. Now passing
  that stopped before: mips (every rgba16f, rgba8 and srgb level, largest difference 0) and
  targets (rgba16f, rgb10a2, r32ui, rgba8). Still stopping, none in this back end: constants (the
  front past the uniform block limit, above), samplers (`TEXTURE_LEVEL_QUERY` refused by the
  front), views (`TEXTURE_VIEWS`), raster (polygon mode, absent on mozangle), formats (`E_FAIL`
  from the front's `CheckFormatSupport`, no back end message; wrongly put on the front here: the
  `E_FAIL` was this back end's caps giving BGRX no use, built in run 3), geometry and streamout
  (no geometry stage at 10_0).
- Spread of the per-draw cost (G3's check, recorded here): one of the checker's two runs had
  WebGL 2 medians of 10,000 draws three to four times run 18's (6.2 ms against 1.7 to 1.8) while
  its fastest frames matched (1.6 ms); the CPU load just after was 24% against 5% in the run that
  matched. So single medians on this shared desktop vary that much between runs; compare
  fastest frames, or repeat runs, before reading a difference.

## Measurements, run 13

- 2026-11-17, G3 run 13, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times; queued
  jobs, not timed). 30 suite tests and 41 checks (71).
  - Start of the run, after the merges (job 19:54 to 19:56): as run 12, 68 of 68 in Chrome and on
    ANGLE with every extension, 48 of 68 on the reduced routes, 5 of 6 against WebGPU.
  - The new checks before the fixes: `Depth24Plus` written by the host read back 0x00ffff for
    0xffffff (Chrome and ANGLE); natively a `depth16unorm` region copied from a buffer kept its old
    values; `r32float` into `depth32float` lost the device. After the write fix, unorm 24-bit depth
    read back one step high for most values from 2^23 up, on both.
  - A cube face's 16 and 24-bit depth blitted into a 2D texture on mozangle read 0x3333 (0.2) for
    the first face blitted, whatever was written; Chrome was right. Read backs of cubes now sample
    the cube instead. With WEBGL_stencil_texturing, Chrome read a cube's stencil through
    `usamplerCube` as 0, so cube stencil is counted by bit draws on every route.
  - Final tree (job 20:19 to 20:22), lint clean on both targets in its build, the 6 unit tests of
    src pass: Chrome 157.0.8103.78 71 of 71 with every extension and 71 of 71 without
    WEBGL_stencil_texturing, the format and the raster extensions; 51 of 71 without EXT_clip_control
    and 51 of 71 without it and WEBGL_provoking_vertex (the 20 known); ANGLE (mozangle 0.7.1 with
    both patches) 71 of 71 with every extension and 51 of 71 on both reduced routes; against WebGPU
    5 of 6 (r32float against the hardware filter). All 74 scene and rig verdicts as expected, 10
    rig pictures skipped where their extensions are off; `depth-bias_depth` in Chrome now exactly
    equal to real Direct3D 11 (0 of 16384 pixels differ).

## Measurements, run 14

- 2026-11-17, G3 run 14, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times; queued
  jobs, not timed). 37 suite tests and 44 checks (81).
  - Start of the run, after the merges (job 20:53 to 20:55), before any change: Chrome 71 of 78 (the
    7 failures the 7 new suite tests the browser host did not list yet), ANGLE 77 of 78 (the new
    `a_copy_within_one_subresource_moves_the_region` lost the device: copyTexSubImage within one
    level is a feedback loop GL refuses).
  - Final tree (job 21:17:50 to 21:20:33), lint clean on both targets in its build, the 6 unit tests
    of src pass, every check with seed 0 and one case: Chrome 157.0.8103.78 80 passed and 1 not run
    (`clip_and_cull_distances`, both features absent) of 81 with every extension; 74 passed and 7 not
    run (their extensions off) without WEBGL_stencil_texturing, the format and the raster extensions;
    55 passed, 1 not run and the 25 known failures without EXT_clip_control, and the same without it
    and WEBGL_provoking_vertex. ANGLE (mozangle 0.7.1 with both patches) 81 of 81 with every
    extension, 57 of 81 on both reduced routes (the 24 known). Against WebGPU 4 of 6 (r32float
    against the hardware filter; the fractional viewport's row 13). All scene and rig verdicts as
    expected.
- Per-draw cost, both back ends in Chrome 157 on the RTX 3080 (job --alone 21:22:56 to 21:23:07;
  the desktop's CPU load read 17% at 21:23:26, after the job), median of 30 frames, the frame's CPU
  time (performance.now, 0.1 ms steps). WebGPU's adapter confirmed as "NVIDIA GeForce RTX 3080
  (discrete GPU, D3D12)".
  - WebGL 2: 10,000 draws 1.7 ms with one set, 4.0 ms with a set per draw; 50,000 draws 5.8 ms and
    16.6 ms (0.17, 0.40, 0.12 and 0.33 us a draw).
  - WebGPU: 10,000 draws 1.6 ms and 3.8 ms; 50,000 draws 7.3 ms and 17.7 ms.
  - The 50,000-draw figures agree with run 11's (WebGL 6.8 and 16.5, WebGPU 7.2 and 17.8); the
    10,000-draw ones are less than half run 11's (WebGL 4.9 and 9.0, WebGPU 2.9 and 3.8 there).
    The one change to the page is Chrome's WebGPU developer features flag; not found why.

## Measurements, run 15

- 2026-11-17, G3 run 15, same desktop (RTX 3080, driver 32.0.16.2101; desktop clock times; queued
  jobs, not timed). 39 suite tests and 44 checks (83), 6 comparisons with the WebGPU back end (10
  routes).
  - Start of the run, after the merges (job 21:27 to 21:29), before any change: Chrome 80 passed, 1
    not run, 2 failed (the two new suite tests the browser host did not list); ANGLE 83 of 83; against
    WebGPU natively 4 of 6 (r32float against the hardware filter; the fractional viewport).
  - Final tree (job 21:41:35 to 21:43:58), lint clean on both targets in its build, the 6 unit tests
    of src pass, every check with seed 0 and one case: Chrome 157.0.8103.78 82 passed and 1 not run
    (`clip_and_cull_distances`) of 83 with every extension; 76 passed and 7 not run without
    WEBGL_stencil_texturing, the format and the raster extensions; 56 passed, 1 not run and the 26
    known failures without EXT_clip_control, and the same without it and WEBGL_provoking_vertex.
    ANGLE (mozangle 0.7.1 with both patches) 83 of 83 with every extension, 58 of 83 on both reduced
    routes (the 25 known: run 14's 24 and the new
    `typed_buffers_through_a_texture_lie_2048_elements_to_a_row`, refused by the suite's raster
    block). Against WebGPU: natively 5 of 6 tests (and `every_comparison_is_a_test`), in Chrome 9 of
    10 routes, WebGPU's adapter "NVIDIA GeForce RTX 3080 (discrete GPU, D3D12)"; on both the one
    failure is r32float against the hardware filter. All 74 scene and rig verdicts as expected, 10
    rig pictures skipped where their features are absent.
- Per-draw cost, both back ends in Chrome 157 on the RTX 3080 (job --alone 21:44:04 to 21:44:27; the
  desktop's CPU load read 16% at 21:44:29, after the job), median of 30 frames, the frame's CPU time
  (performance.now, 0.1 ms steps):
  - WebGL 2: 10,000 draws 1.6 ms with one set, 3.9 ms with a set per draw; 50,000 draws 5.8 ms and
    16.6 ms (0.16, 0.39, 0.12 and 0.33 us a draw).
  - WebGPU: 10,000 draws 1.6 ms and 3.8 ms; 50,000 draws 7.6 ms and 18.2 ms (0.16, 0.38, 0.15 and
    0.36 us a draw).
  - Within 0.3 ms of run 14's on every case except WebGPU's 50,000 draws with a set per draw
    (17.7 ms there).

## Measurements, run 18

- 2026-11-18, G3 run 18, same desktop (RTX 3080, Chrome 157.0.8103.78). 39 suite tests and 48
  checks (87), 7 comparisons with the WebGPU back end (11 routes), 2 BC reference tests.
  - Full run (all.cmd, queued job, out C:\wc\out\webgl\r18a, log in scratch run18/all1.log):
    Chrome 84 passed, 3 not run (clip and cull distances, timestamps, fractional bias), 0 failed
    with every extension; 78 passed and 9 not run without WEBGL_stencil_texturing, float_blits,
    the format and the raster extensions; 58 passed, 3 not run and the 26 known failures without
    EXT_clip_control, and the same without it and WEBGL_provoking_vertex. ANGLE 87 of 87 with every
    extension (7 only refused), 62 of 87 on both reduced routes (the 25 known). Against WebGPU
    natively 7 of 8 tests, in Chrome 9 of 11 routes (the known two). BC reference 1 of 2 (the
    known decoded route). Unit tests 10 of 10. Scene and rig verdicts all as expected. Clippy with
    warnings denied, native and wasm32 (browser and draw-cost examples): clean.
  - `copies_within_a_typeless_group` (now with 13 signed normalised pairs) and
    `signed_normalised_texels_drawn_then_copied` pass in Chrome and on ANGLE (natively the second
    prints "not drawn": snorm formats do not render on mozangle).
  - Seeded, `WEBGL_SEED=777`, `WEBGL_CASES=20` (seeds 777 to 796): on ANGLE the 11 checks that
    read the seed (`block_compressed_and_shared_exponent_...`,
    `copies_between_depth_and_colour_formats`, `copies_between_index_and_data_buffers`,
    `copies_into_and_out_of_host_kept_textures`, `copies_within_a_typeless_group`,
    `copies_within_one_subresource`, `cube_faces_copied_between_typeless_formats`,
    `depth_and_stencil_written_read_back_as_written`, `generate_mipmap_equals_the_draws`,
    `levels_stay_as_written_while_others_are_read_and_drawn`,
    `sixteen_bit_normalised_formats_follow_their_extension`): 11 of 11 (scratch run18/seeded.log).
    On ANGLE and in Chrome, `copies_within_a_typeless_group` (seeded) with
    `signed_normalised_texels_drawn_then_copied` and `lists_draw_the_largest_index_as_a_vertex`
    (not seeded): 3 of 3 on both (run18/seeded-chrome.log).
  - Per-draw cost, `gpu-job --alone`, Chrome 157 on the RTX 3080, 30 frames a case, medians in ms a
    frame (10k one set / 10k set per draw / 50k one set / 50k set per draw), two runs (CPU load
    14% and 11% just after each):
    - WebGL 2 strips: 1.8 / 4.0 / 6.8 / 16.7, then 1.7 / 3.9 / 5.8 / 16.6.
    - WebGL 2 indexed lists without restart (one lookup of where the largest index may be per
      draw): 1.9 / 5.6 / 7.0 / 17.1, then 1.8 / 4.1 / 7.3 / 17.1. The 5.6 is one noisy run (its
      fastest frame 3.9).
    - WebGPU strips: 1.6 / 3.9 / 7.4 / 17.8, then 1.6 / 3.9 / 7.4 / 17.9.
    - WebGPU indexed lists: 1.7 / 3.8 / 8.0 / 18.7, then 1.7 / 4.3 / 8.0 / 20.2.
    So indexed draws cost about 0.01 to 0.03 us more than strips on both back ends, and WebGL 2
    stays at or below WebGPU at 50k draws. What the lookup alone costs is not separated from the
    rest of the indexed path.

## Measurements, run 17

- 2026-11-18, G3 run 17, same desktop (RTX 3080; queued jobs, not timed; its CPU load read 13%
  after the run). 39 suite tests and 47 checks (86), 7 comparisons with the WebGPU back end (11
  routes), 2 BC reference tests. No timings: the draw path gained one lookup per indexed draw
  without restart (an empty list for buffers no write put the largest value into), not measured.
  - Full run (all.cmd, out C:\wc\out\webgl\r17a, log in scratch run17/all3.log): Chrome 83 passed,
    3 not run (clip and cull distances, timestamps, fractional bias), 0 failed with every
    extension; 77 passed and 9 not run without WEBGL_stencil_texturing, float_blits, the format and
    the raster extensions; 57 passed, 3 not run and the 26 known failures without EXT_clip_control,
    and the same without it and WEBGL_provoking_vertex. ANGLE 86 of 86 with every extension (7 only
    refused, listed by all.cmd: clip and cull distances, depth bias clamp, depth clamp, fractional
    bias, mirror once, polygon mode, timestamps), 61 of 86 on both reduced routes (the 25 known).
    Against WebGPU natively 7 of 8 tests, in Chrome 9 of 11 routes (the r32float one on both, the
    exact ties in Chrome only). Unit tests 9 of 9. Scene and rig verdicts as expected. Clippy with
    warnings denied, native and wasm32: clean.
  - `lists_draw_the_largest_index_as_a_vertex` passes in Chrome and on ANGLE (8, 16 and 32-bit;
    host-written and GPU-copied indices; fans).
  - `copies_within_a_typeless_group` passes in Chrome and on ANGLE, the 10-10-10-2, R8, RG8 and RG32
    pairs included.
  - Seeded on ANGLE: `WEBGL_SEED=777`, `WEBGL_CASES=20` (seeds 777 to 796), the 9 `copies_`
    tests and `lists_draw_...`: 10 of 10 pass (scratch run17/seeded.log). Corrected in run 18:
    only 5 of those 10 read the seed (`copies_within_a_typeless_group`,
    `copies_within_one_subresource`, `copies_between_depth_and_colour_formats`,
    `copies_between_index_and_data_buffers` and `copies_into_and_out_of_host_kept_textures`); the
    other 5 ran the same data 20 times.
  - BC against real Direct3D 11 (tests/bc_reference.rs, the rig's `textures-bc` and
    `textures-bc67` from the same seed): through GL's compressed formats (every extension) both
    pictures are exactly equal to Direct3D 11's, every format 0 difference. Decoded on the host
    (BC extensions off): BC6H and BC7 exactly equal; BC1 to BC5 differ by up to 5 to 7 8-bit steps
    (unorm) and 12 to 13 (snorm, 0.048 and 0.052), 7457 of 16384 pixels differ, and the picture
    fails the rig's rule by 44 pixels (2354 beyond the tolerance, 2310 allowed, 12 tiles over).
    WARP (Direct3D's software rasteriser) draws the same picture 2310 pixels beyond, largest
    0.0517; our decoded picture is within 0.0095 (2.4 steps) of WARP's (gt-compare, 8499 pixels
    differ at all). So the RTX 3080 decodes BC1 to BC5 its own way; bcdec_rs and WARP follow the
    formula closely. Known failure, below.

## Measurements, run 16

- 2026-11-17, G3 run 16, same desktop (RTX 3080; queued jobs, not timed; its CPU load read 14% at
  the start of the run). 39 suite tests and 46 checks (85), 7 comparisons with the WebGPU back end
  (11 routes). No timings: nothing on the draw path changed, so run 15's per-draw cost stands.
  - The float blit probe: exact on ANGLE (mozangle 0.7.1) and in Chrome 157 (caps-angle.txt and
    caps-chrome.txt say `exact float blits: true`).
  - Full run (all.cmd, job before the browser host listed the new routing check, so Chrome counts
    it failed with "the browser host has no suite test"; listed and run in the next job, passing on
    both routes): Chrome 83 passed, 1 not run, that 1 failed with every extension; 77 passed, 7 not
    run, that 1 failed without WEBGL_stencil_texturing, float_blits, the format and the raster
    extensions; 57 passed, 1 not run, the 26 known and that 1 failed without EXT_clip_control (and
    the same without it and WEBGL_provoking_vertex). ANGLE 85 of 85 with every extension (5 only
    refused), 60 of 85 without EXT_clip_control and 60 of 85 without it, WEBGL_provoking_vertex,
    float_blits and the format extensions (the 25 known failures). Against WebGPU: natively 6 of 7
    and `every_comparison_is_a_test`, in Chrome 9 of 11 routes; the r32float one on both, the exact
    ties in Chrome only. The 6 unit tests of src pass; all scene and rig verdicts as expected.
  - Final tree (all.cmd once more, out C:\wc\out\webgl\r16e): Chrome 84 passed and 1 not run of 85
    with every extension; 78 passed and 7 not run without WEBGL_stencil_texturing, float_blits, the
    format and the raster extensions; 58 passed, 1 not run and the 26 known failures without
    EXT_clip_control, and the same without it and WEBGL_provoking_vertex. ANGLE 85 of 85 (5 only
    refused), and 60 of 85 on both reduced routes (the 25 known). Against WebGPU natively 7 of 8
    tests, in Chrome 9 of 11 routes, as above. Unit tests 6 of 6. Scene and rig verdicts as
    expected. all.cmd exits 1 for the known failures only.
  - Seeded: `WEBGL_SEED=777`, `WEBGL_CASES=20`, the copy checks, `cube_faces_...` and
    `interpolated_bc...` (11 tests, seeds 777 to 796): ANGLE 11 of 11; Chrome 10 of 10 (the routing
    check not yet listed there). Run 15's failing seed 778 passes.
  - BC4 and BC5 (endpoints 250 and 3, -125 and 101; 17 and 200, -111 and 127 for BC5's green; both
    modes), largest difference from the formula: stored decoded 2.2e-4 unorm (0.06 of a step),
    1.3e-4 snorm BC4, 2.0e-4 snorm BC5 (0.02 of a 1/127 step); decoded by the GPU (native route, ANGLE
    on Direct3D 11) 2.6e-2 unorm (6.5 steps), 5.0e-2 and 5.2e-2 snorm (6.3 and 6.7 steps). Real
    Direct3D 11's sampling of the same blocks is not compared yet (see "Next").

## Known failures

- (G3b run 7) Real programs through the Direct3D 11 front's WebGL 2 build sometimes end with
  0xc0000005 at start-up, before any frame: five times in about twenty runs of SimpleLightingPC
  and SimpleInstancingPC (2026-11-18 21:06, 21:33, 2026-11-19 00:18, 00:40, and run 6's), never
  in chocolate-doom's. The Application log puts every one in C:\Windows\System32\d3d11.dll
  (10.0.19041.5794) at offset 0x96777, after ANGLE has made its devices. Microsoft's public
  symbols for that build (d3d11.pdb 7D7427C8F5CAFC4FA22F64E4FA1C5C26 age 1) name no function
  there; the nearest public symbol is a short thunk 0x14a27 bytes before, so the place is in a
  private function. ANGLE's calls reach that library through the front's forwarding by caller.
  Cause not found; next would be a crash dump (no dump tool is installed on the desktop).

- (G3b run 8) Hundreds of distinct large programs alive at once are not bounded by any limit
  this back end declares. On the RTX 3080 each live GL program of the filter test's pixel
  shader (about 800 naga expressions, its tap loops compiled by ANGLE into one large Direct3D
  shader) holds about 650 KB of the adapter's memory, and ANGLE's device (Chrome's too) was
  removed with about 420 of them alive, near 275 MB above the idle adapter, far from its 10 GB.
  Copies of one shader are now one GL program, so the filter test no longer meets it; distinct
  programs of that size were not tried by the hundred (a replay of the stream spends about 40 s
  before its first 24 draws, which make its four). GL reports no size of a compiled program, so no limit can be declared from it.
  Closed by run 8 for the filter test (copies of one shader), kept below as the record:
- (G3b run 4, corrected in run 5) Natively, the shaders track's `tests/filter_draw.rs` loses
  mozangle's ANGLE device (Direct3D 11 "Failed to map D3D11 resource", reported as a lost device
  with GL error 0x502) after about 400 of its programs are alive at once; with its programs
  destroyed its 16 seeds pass. It is not a bound on the number of programs: the independent check
  of run 4 kept 800 small distinct programs alive and drew each once, natively and in Chrome,
  with no loss. So it depends on what the programs are (the filter test's are large tap loops,
  which ANGLE compiles into large Direct3D shaders) or on what is made per program. The cause
  inside ANGLE is not found; a real program keeping hundreds of large shaders alive may meet it.
  (G3b run 7) Chrome's own ANGLE meets it too: the test's WebGL 2 draws, recorded natively with
  the `record` feature (the diagnostic copy of the test, scratch g3b7/diag, every program kept
  alive; stream scratch g3b7/filter.bin, 3.8 MB), replayed in Chrome 157 on the RTX 3080
  (`gpu-job --alone --desktop`, desktop 01:07, scratch g3b7/replay2.log) lose the context the
  same way: "GL_INVALID_OPERATION ... Renderer11::mapResource ... HRESULT 0x887A0005 (removal
  reason 0x887A0005): Failed to map D3D11 resource", then CONTEXT_LOST_WEBGL. No display reset
  was logged and the queue was not held. While the native run drew, the adapter's dedicated
  memory (whole machine, `typeperf` each second, scratch g3b7/diag-keep.csv) rose from 354 MB to
  616 MB in about 12 s, and fell back when the process ended. Neither built nor refused: no
  count or size is known that bounds it, so no limit can be declared honestly yet.

- (run 17) `decoded_on_the_host_equals_direct3d_11` (tests/bc_reference.rs): BC1 to BC5 decoded by
  bcdec_rs, the route of browsers without the BC extensions, are up to 13 8-bit steps from what the
  RTX 3080 samples under Direct3D 11, and fail the rig's rule by 44 pixels; WARP is as far from the
  GPU and within 2.4 steps of us (see "Measurements, run 17"). The GPU's decode is its own; matching
  it would shape the decoder around one GPU. Through GL's compressed formats the pictures are
  exactly equal.
- (run 17) Natively 7 checks and in Chrome 3 run only their refusal and report "not run", among
  them `timestamps_and_their_disjoint_flag` (no timestamp bits on this GPU) and
  `fractional_depth_bias` (the probe finds ANGLE rounds the bias units, natively and in Chrome).
  Their drawing routes have run on no GPU here.

- (run 16) `generate_mips_of_an_integer_twin_at_exact_ties_equals_webgpu`, in Chrome only: texel
  (3, 3) channel 1 of level 1, whose four source values (8374, 25225, 55843, 24780) average to
  exactly 28555.5, is 28555 here and 28556 on Chrome's WebGPU; the other 11 ties agree, and
  natively all agree. G1's helper averages the decoded values in 32-bit floats and rounds; at an
  exact tie the result turns on the rounding of each compiler's arithmetic. For G1: averaging
  integer twins of normalised formats in integers would make it exact on every back end.

- Fixed in run 15: `a_fractional_viewport_in_the_program_equals_webgpu_native` (run 14). Run 14
  read its assertion backwards: WebGPU, not this back end, left row 13 undrawn, as Direct3D's
  implicit scissor does (see "Where things stand").
- (run 15) Since the merge of run 15 also `typed_buffers_through_a_texture_lie_2048_elements_to_a_row`
  on those routes, for the same reason: 25 natively, 26 in Chrome.
- (run 14) Suite without EXT_clip_control or WEBGL_provoking_vertex: 24 tests natively (the earlier
  20 and `captures_write_location_outputs_packed_in_draw_order`,
  `constants_read_through_a_texture_lie_256_rows_to_a_texture_row`,
  `missing_components_read_their_defaults` and `values_pass_between_programs_by_location` of run 14's
  merge), 25 in Chrome (also `fill_modes_draw_edges_or_corners`, which mozangle skips for want of
  polygon mode), every one refused by the suite's own raster blocks as below.

- `generate_mips_of_r32float_without_float_filtering_equals_webgpu` without
  OES_texture_float_linear: 5 of 16 values of level 1 differ from WebGPU's by one float step. The
  built-in mip program filters in the shader (`FilterInShader`, G1's helper) while WebGPU's sampler
  filters in hardware; the order of the arithmetic differs. With the extension (hardware filtering
  on both) they are equal. This comes from the helper, so it is for G1 (helpers.rs is G1's alone).
  Matching it exactly would mean doing the helper's arithmetic in the order this GPU's filter
  does, and other GPUs need not share that order.
  Since run 5 the pass bar's own wording (exact against G2, for a format generateMipmap refuses) is
  also checked route against route. The new
  `generate_mips_of_r32float_filtered_in_the_program_on_both_equals_webgpu` passes: every level is
  byte-equal with the WebGPU back end on a device without float32-filterable. The comparison
  against the hardware filter is left failing, unchanged.
- Fixed in run 5, by patching the test tool: on native ANGLE with EXT_clip_control,
  - `fragment_depth_follows_the_clip_convention` read 0.75 for clip z 0.5. mozangle's pixel shaders
    computed `gl_FragCoord.z` from clip z by the minus-one-to-one formula.
  - `triangle-clear_rects`, the suite's `the_scissor_bounds_the_draw`, `depth_and_stencil_read_back`
    and `stencil_sampled_and_read_back` failed because mozangle mirrored scissors under the
    upper-left origin.
  Upstream ANGLE had fixed both; see run 5 under "Done".
- Fixed in run 6 by the EGL window surface: suite `present_shows_the_frame` on native ANGLE. Before,
  reading this ANGLE's pbuffer default framebuffer
  gives 72 in every byte whatever was drawn into it, even right after a `glClear` of it, with no GL
  error, a complete framebuffer, an RGBA8 configuration with no samples, and reads in RGBA and in
  the implementation's BGRA format alike. In Chrome the same test passes (the canvas read through a
  2D canvas). The pbuffer is no longer presented into.
- Suite, without EXT_clip_control or WEBGL_provoking_vertex, in Chrome and on ANGLE: 15 tests
  (`triangle_covers_...`, `textured_draw_...`, `edges_through_pixel_centres_...`,
  `strips_take_flat_values_...`, `depth_writes_off_...`, `srgb_targets_...`,
  `values_are_interpolated_...`, `the_scissor_bounds_...`, `stencil_operations_...`,
  `near_clipping_...`, `linear_filtering_...`, `instances_start_...`, `indexed_strips_...`,
  `multisampled_targets_...`, `occlusion_queries_...`; run 7; since run 10 also
  `a_scissor_past_the_target_...`, `a_stream_of_stride_zero_...`, `comparison_samplers_...` and
  `resolves_average_...`, 19): the suite's own raster blocks ask for clip depth from zero
  (`depth_minus_one` false) and provoking vertex First, which the back end lacks on those routes and
  refuses as the interface says (`Absent(CLIP_DEPTH_ZERO_ONE)`, `Absent(PROVOKING_VERTEX_FIRST)`).
  The suite (G1's crate) should take both from the caps, as the built-in programs do.

- `CLIP_DISTANCE` and `CULL_DISTANCE` (run 12): Chrome 157 on the RTX 3080 has
  WEBGL_clip_cull_distance, but `clip_and_cull_distances` drew the whole triangle with two clip
  distances written (x and y) and `CLIP_DISTANCE0` and `1` enabled; the device stayed whole. naga
  writes `gl_ClipDistance = <array>;`; assigning each element instead changed nothing. Not found
  why. Both features stay absent (fronts take `ClipDistanceInShader` and `CullDistanceAsGeometry`),
  and the check runs only where the back end declares them, so its drawing route runs nowhere today:
  since run 14 Chrome's suite output says "not run" for it rather than "ok". Next: read ANGLE's
  translated HLSL in Chrome (`WEBGL_debug_shaders`) for the vertex program.

- Fixed in run 11, both found in run 10:
  - In Chrome (ANGLE on Direct3D 11), invalidating one layer of a 2D array (a pass's `DontCare` load
    or `Discard` store) lost the other layers of that level, so the mip draws left every layer but
    the last black. Layers of 2D arrays are no longer invalidated. Run 10's guess at the cause
    (sampling the array while drawing into it) was wrong: a fix built on it did not help.
  - mozangle 0.7.1 (the test tool) lost the last row and column of level 0 of an odd-sized texture
    once its base level was raised. It is fixed by angle/immutable-base-level.patch.
  - Before them (run 9), `generate_mipmap_equals_the_draws` found both, and 3D textures differed
    between `generateMipmap` and the draws on ANGLE; with both fixes every shape is equal on both.

## Magic literals

The owner's rule (brief, 2026-11-18) and the audit's list for this crate
(orchestrator/magic-literals/final/lists/graphics-webgl/wcontainers-gpu-webgl.txt: 657 code, 98
const, 499 unit-test and 4,736 test findings, 2 redefinitions, 15 "restated").
- Done (G3b run 4): the 21 GL constants this crate retyped although glow 0.18 defines them
  (platform.rs: clip control, provoking vertex, polygon mode, depth clamp, mirror clamp, clip
  distance, the second-source blend factors, timestamp and its counter bits; depth.rs: stencil
  texturing) are now glow's own. Left as one named constant each, because glow lacks them:
  `platform::GPU_DISJOINT` (GL_GPU_DISJOINT_EXT) and EGL's `DIRECT_COMPOSITION` (platform.rs).
- Left, in the audit's order: the 657 code findings (lib.rs 153, shader.rs 116, platform.rs 82,
  host.rs 64, trace.rs 61, convert.rs 61, state.rs 40, depth.rs 27, caps.rs 23, format.rs 18, the
  rest fewer), then the tests'. The two redefinitions (`SIZE` in examples/draw-cost.rs, `WINDOW` in
  tests/suite.rs) and the 15 "restated" lines were read: the values only coincide with the named
  facts (256 bytes of a row against `Features::PROVOKING_VERTEX_LAST`, and so on), so nothing is
  imported for them.
- (G3b run 5) New bare numbers of runs 3 and 4 (check of run 4): B5G6R5's packing on read now
  takes its shifts and steps from one named `B5G6R5_BITS` (format.rs) and its rounding from
  `u8::MAX`; BGRX's alpha byte is `u8::MAX`. Left as they are: the per-format component widths
  in convert.rs `bits()` (R11G11B10's `[11, 11, 10, 0]` among them) and `B5G6R5_BITS`, which the
  audit (fix-graphics.md item 16) gives to `wcontainers-gpu` beside `Format::block()`; until the
  interface has them, this crate keeps its one table and moves to the interface's when it lands.
  The new code of this run (linear values, special float values, read counting) names its GLSL
  names once (`VARYING`, `MAIN`, `RETURN`, `CARRIED`, `CENTROID_W` in shader.rs) and derives
  sizes from types and formats.
- (G3b run 5) The automatic check was not merged then.
- (G3b run 6) Adopted the check as the brief of 19:20 asks: merged origin/phase-1-literals (tip
  a295f55c, version 6 of the rules) in c33381de and ran its `init` once (1e337706). The
  desktop's checkout has CRLF line ends (core.autocrlf true), which changes the keys of multi-line
  strings (112 lines differed, 18 of them already in the literal branch's baseline), so the
  baseline was made, and the check is run, in an LF worktree of the same repository,
  C:\wc\src\phase-2-graphics-webgl-lf (How to run). Every graphics track running the check in its
  desktop checkout will meet the same.
  - This crate's baseline lines: 631 at adoption; 627 after run 6 (four removed: the run's new
    code names its GLSL words, separators and messages once, and two old literals went with the
    rewritten functions). The check exits 0 at the head, with 0 marks.
  - The changes that could add lines to the baseline, as the check lists them at the head:
    c33381def450 the start; 1e3377067526 the init after the baseline arrived by a merge;
    68214b2eec10 the start; 89f882810517, 7119e3f3a527, c3bf56d4ed9f, 31ec396f9a4a and
    0f1875633508 new rules, for literals already in the tree (the last six are the literal
    branch's own).
  - Naming the value "sample" as a constant made another crate's test restate it
    (wcontainers-directx examples/own_min_lod.rs); the qualifiers are now a table instead, as
    docs/literals.md step 8 asks.

- (G3b run 7) Merged origin/phase-1-literals at its seventh rules (through -direct3d-11, which
  carried them) and ran `init` once in the commit after the merge (949667f2), as step 4 of
  docs/literals.md says. The LF worktree needed the shaders crate's submodules checked out
  first: without them `init` dropped their 12,547 script lines and wrote two `unparsed` lines.
  The merge of -webgpu after it conflicted in the baseline; resolved by keeping every line of
  both sides (each as often as the side holding it most), then deleting the 470 `-` lines the
  check printed (4dd6c481). This crate's baseline lines: 627 after run 6, 631 after the init
  (the seventh rules found four), 630 at the head; exit 0, 0 marks. Changes that could add
  lines, as the check lists them: run 6's list, plus bd53fbb448e2 and 949667f2 (new rules,
  for literals already in the tree), and the starts and inits of the Direct3D 11 and WebGPU
  branches' own baselines, merged in (81489a2f8fab and 5657abb6ba15; 4bcacbf5c5b9 and
  2cc8ad761bf6).

## Departures from the plan

- (G3b run 5, run 6) Linear (`noperspective`) values, which GLSL ES 3.00 lacks, take
  NV_shader_noperspective_interpolation where the context has it (run 6) and are otherwise
  carried times w in the GLSL writing (plan 6.5's place for such fixes): a multiply per value in
  each stage, a varying per value, and at the centroid or per sample one more varying (w). Not a
  counted slow path: it is the only route such a context has, chosen when the shader is made. A
  program near the 30 varying vectors may fail its link on that route (a lost device), which no
  limit tells the front.
- (G3b run 6) The frame-time breakdown runs the WebGPU track's analysis (its examples/frames.rs),
  which this run taught the WebGL 2 back end's crates (as "back end") and ANGLE's libraries (as a
  part "angle"): five lines in crates/wcontainers-gpu-webgpu, which this track does not own,
  committed alone (071cbbfc) so that track can see it; its WebGPU numbers are unchanged by it.
  The programs run with the front's libraries beside them, not through the rig's forwarder (the
  front hands ANGLE's own Direct3D 11 calls to the system's library by their caller's module,
  which the forwarder hides, so they come back to the front), so they run for a time rather than
  to a frame: per-second CPU from the two runs' difference, frames a second from the back end's
  new frame reports (`FRAME_REPORT`, as the WebGPU back end prints), timed by direct.ps1.
- (G3b run 6) B5G6R5's widths (`B5G6R5_BITS`, format.rs) stay a named constant of this crate
  although the audit gives per-format widths to `wcontainers-gpu`, whose table is on no branch
  yet: turning it back into bare numbers would add lines to the literal baseline, which the check
  forbids. It moves to the interface's table when that lands.
- (G3b run 5) Read backs count `FormatConverted` where the host converts the values read (BGRA's
  swap, B5G6R5 packed again from bytes). Reads that only keep part of each texel `readPixels`
  wrote wider (R8 read as RGBA bytes, and the like) are not counted: they are WebGL 2's only read
  of those formats, a copy like dropping a row's padding, not a slower route beside a fast one.
- (G3b run 4) Plan 6.5 lists GLSL ES 3.00's fixes; one more is made here, by the same rule (it
  needs nothing beyond the module): plain reads of a depth texture the program also compares
  against go through a float copy of its variable (`plain_depth_reads`). Like the sampler split
  beside it, it is a rewrite when the shader is made, with no run-time slow path to count: the
  copy is one more texture unit bound to the same texture.
- (G3b run 3) BGRX is held in BGRA's storage with alpha kept at one by every writer (host writes,
  clears, a colour mask on draws), not in GL's RGB8, which reads alpha as one by itself: RGB8's
  sRGB twin SRGB8 does not render in GLES 3.0, and BGRX's two formats are one typeless group, so
  RGBA8 and SRGB8_ALPHA8 keep their copies on the routes BGRA already has. The mask costs nothing
  per draw: it is applied with the blend block, which is applied again only when the pass's set of
  BGRX targets changes.
- (G3b run 3) GL errors are read where the back end already waits for the GPU (after fences and
  `readPixels`), not at every `submit`: in Chrome `getError` is a wait for the GPU process, and it
  was most of a WebGL 2 frame. An error of ours is so reported at the next wait instead of the next
  submit; a lost context is still seen at `submit` and `status` (`isContextLost`, no wait).
- (G3b run 3) Natively `uniform_buffers_per_stage` is GL's count less 2 (10 on the RTX 3080):
  mozangle 0.7.1 (the test tool) places a stage's blocks from register b2 (b0 and b1 hold its own
  constants) and asserts each register below the block count, so a stage using the last two
  aborts the process (seen: `ProgramD3D.cpp:2414 ... registerIndex < caps.maxShaderUniformBlocks`).
  Chrome's ANGLE draws with all 12, so the browser keeps GL's count; the native caps differ from
  Chrome's here.
- (G3b run 3) The signed normalised minimum is judged per level, not per layer: a pass into one
  layer of a layered snorm texture whose other layer of that level holds -128 still loses the
  device as not built when copied out ("the bits of signed normalised texels drawn into after a
  write of their smallest value").
- (G3b run 3) The replay hang of check9 (the second WebGPU replay; the page stopped answering)
  was not reproduced and its cause not isolated: since each replay's device is dropped before the
  next is made, all four replays finished in the one run made (the earlier page held two WebGPU
  devices and their canvases at once while the second was made).
- (G3b run 2) The frame-time measurement of a real program in Chrome runs the program natively
  (no WebAssembly build of the Direct3D 11 front exists) and replays the interface calls it made
  in Chrome. So it measures the back ends' share of a frame in Chrome, not the front's. (Run 3:
  still so. A native timing of the front's calls belongs in the front's crate, which this track
  does not own; the recording wrapper sees only the time between interface calls, which mixes the
  front's work with the program's own.) The
  recording sits in this crate (the `record` feature makes `WebGl` a type alias of the recording
  wrapper) because the front's library names `wcontainers_gpu_webgl::WebGl` and this track owns
  only this crate; the stream form and the replay are generic over `Backend`, so the WebGPU track
  can replay the same files. The stream records the WebGL 2 caps the front saw (on ANGLE, equal
  to Chrome's with every extension but the signed normalised targets), so the WebGPU replay runs
  the routes the front chose for WebGL 2 (for example `TEXTURE_INFO` helpers), not those it would
  choose for WebGPU.

- (G3b run 1) Plan 6.5's lowering of bit functions and `textureGather` is done on the GLSL text
  naga writes, not in naga's module: the program gets definitions of GLSL ES 3.10's bit functions
  under their own names (free in GLSL ES 3.00), and each gather call is rewritten to a helper.
  naga's arenas need operands before their users, so lowering inside the module would mean
  rebuilding each function's expressions; the text route is the one `textureSize` already takes.
  Gathers are four samples at texel centres through the bound sampler rather than four
  `texelFetch` reads (the plan's words): fetches ignore the address mode, which places footprints
  past an edge. At a texel's centre a linear filter gives that texel exactly (measured: every
  case of `gathers_equal_their_footprints` equal, ANGLE and Chrome on the RTX 3080). A sampler
  whose minimum level of detail is above 0 moved the samples off level 0 until run 6, which binds
  such samplers' level-zero twins for gathers.

- (run 14) `MapIntent::Often` buffers take `DYNAMIC_DRAW`, as plan 4.6 says, and `Discard` writes
  orphan them (`bufferData` at their size, GL's renaming); plan 5.8's "kept small and renamed" is not
  built further: a buffer's size is the front's, and renaming on an `Ordered` write would copy the
  rest of the buffer, the cost it would avoid.
- (run 14) Copies within one subresource go through a buffer as `CopyWithinTexture` names it, except
  for 16-bit floats, which GL does not read back: those are held in a 2D texture and copied twice.
- (run 14) `rgb32float`, `rgb32uint` and `rgb32sint` textures keep a host copy (`HostCopyKept`), the
  route of RGB9E5: GL neither renders nor reads them back.
- (run 14) Copies into 16-bit floats from another format of their group go through the host
  (`HostCopyReadBack`, a wait for the GPU): a draw writes half floats from 32-bit ones, which make
  NaN payloads canonical.
- (run 16) Copies within one 16-bit or 32-bit float format are blits only where a probe at creation
  finds blits keep every bit: GL promises bit-exact copies of floats by no call, and ANGLE's
  copyTexSubImage changes NaNs and 32-bit denormals. Where the probe fails, the copies take the
  counted routes above (16-bit floats through their integer twin and the host). The probe costs
  two small read backs at creation. `off` takes the name `float_blits` beside extension names, so
  the tests run those routes on a GPU where the probe passes.
- (run 16) Copies within R11F_G11F_B10F stay on copyTexSubImage: GL cannot read the format back, so
  no route here can prove its NaN payloads kept, and its denormals are normal 32-bit floats, which
  that arithmetic keeps.
- (run 17) WebGL 2 cannot turn primitive restart off, and restarts lists too. The interface's
  `RESTART_SWITCH` (absent here, as plan 4.11 says) covers strips, which a front rewrites; lists
  and fans, loops and strips drawn with `restart` false are rewritten by this back end instead
  (`RestartRewrite`, the slow path the plan names for the front's rewrite). Only ranges where a
  write may have put the largest value are read back, so ordinary list draws cost one lookup in
  an empty list. A draw from GPU-written indices that do hold the value waits for the GPU at every
  draw (the indices are read back each time); nothing keeps a widened copy yet. A draw whose 32-bit
  indices hold both 0 and 2^32 - 1 loses the device as not built: no shift brings both into GL's
  range.
- (run 14) A fragment program that only reads storage is refused as `Absent(FRAGMENT_STORAGE_WRITE)`,
  as binding sets refuse storage: the interface has no bit for fragment storage reads.
- (run 15) The fractional viewport comparison cuts the scissor as Direct3D's implicit one (each edge
  rounded down), by interface change 4a72bce0, where the plan said the pixels whose centres the
  viewport covers.

- The suite crate `wcontainers-gpu-test` (G1's) takes proptest without its default features
  (cdcfbc4, a one-line Cargo.toml change): their process forking and timeouts do not build for
  WebAssembly, the suite's host in Chrome needs the crate there, and the crate uses neither. G1
  should keep it.
- (run 18) Signed normalised textures keep a host copy (plan 4.12 has their typeless copies go
  through a buffer, which `readPixels` cannot fill: it returns -128 as -127). So a buffer copy into
  one reads the buffer back first (`HostCopyReadBack`, a wait), as for the other formats kept on
  the host, and each holds its size again in host memory. With EXT_render_snorm, draws into one
  are read back into the host copy only when it is read. The one case left is not built: texels
  -128 from the host then drawn over (see "Not built").

- ANGLE natively is Mozilla's ANGLE (mozangle 0.7.1: Firefox 153 ESR, ANGLE commit 7b0bc3d196d4 of
  2026-06-09), built from source, not Chrome's: Chrome 157's libEGL.dll and libGLESv2.dll are 453 KB
  stubs with no exports (ANGLE is inside chrome.dll), and Electron 45 ships none. It is the same
  ANGLE on Direct3D 11, but a different version from Chrome's, and its extension list differs (no
  EXT_polygon_offset_clamp or EXT_depth_clamp on an ES 3.0 context; provoking vertex as
  GL_ANGLE_provoking_vertex); the table in platform.rs must map those names when they are built.
- GL buffers for indices and other data are made at creation from the usage, not at first binding
  (plan 4.6), so a buffer with both usages used one way pays a second upload. Simpler and never
  stale; revisit with `IndexTwinReadBack`.
- `wait_fence` in the browser calls `finish()`, because WebGL's longest wait on a sync object is zero;
  the plan's JSPI suspension belongs to the device thread's event loop, not built yet.
- Programs are linked at creation, not at the first draw, even without KHR_parallel_shader_compile;
  only the reading of the link waits for the first draw. Issuing the link early can only help, and
  the first draw still counts `PipelineMiss` as the plan says. A link failure is device loss at that
  draw rather than an error from `create_program`: the modules are validated, so a failure is a bug
  of ours.
- Natively the read of a buffer maps it (GLES 3.0 has no `glGetBufferSubData`); WebGL reads it with
  `getBufferSubData`. Same bytes, same wait.
- Buffer clears: WebGL 2 has no buffer clear, so every clear (zero included) takes
  `PatternClearCopied`, which the interface names for non-zero patterns only: a 256-byte host block,
  doubled by copies inside each GL buffer, so both twins stay whole and nothing large crosses from
  the host.
- The interface change ab4b7e9 edits `helpers.rs`, which section 12 gives to G1 alone. The fault was
  in a helper's block (the built-ins' raster asked for a provoking vertex WebGL 2 lacks without an
  extension), the fix is two lines in the style of the clip depth one beside it, and no other route
  made the fast-path defaults usable here; G1 should take it over.
- The last merge of origin/phase-2-graphics in run 1 was undone before it was pushed (see "Next"):
  its suite crate no longer builds for WebAssembly, and the Chrome host of the suite needs it.
- The native test tool is mozangle 0.7.1's ANGLE with three of upstream ANGLE's fixes applied by a
  patch in angle/ (run 5). It is still not Chrome's ANGLE. The patch is ANGLE's own code under its
  BSD-3-Clause licence, applied to a test tool that is never shipped. Before run 5, the native back
  end was to run with EXT_clip_control off; that departure is gone.
- Presenting copies the frame once more than WebGPU does: WebGL's drawing buffer is not a texture,
  so `current_texture` is a texture of the back end's and `present` blits it into the drawing buffer
  (one full-size copy per frame, expected cheap, not measured). Not counted as a slow path: it is
  the only route WebGL 2 has.
- A back end has one surface (`create_surface` a second time is `Invalid`): a WebGL context draws
  into one canvas only. `vsync` is not honoured (the browser decides).
- Views in the sRGB twin are a second texture kept in step inside the back end, since the interface
  allows such views on every texture; plan 4.12 has fronts copy only for other formats
  (`ReinterpretCopy`). Counted as `CopyThroughBuffer`, the route plan 4.12 names for WebGL 2's copies
  within a typeless group.
- `clear_texture` keeps the interface's default (run 9): plan 5.8 lists it among the overrides, but
  the default opens one pass per level and layer whose load is a clear, and this back end's passes
  clear by `clearBuffer*` on a cached framebuffer already; an override would repeat the pass's
  bookkeeping (twins, discards) for no fewer GL calls.
- Draws with a provoking vertex the raster block does not name: `draw_fan_or_loop` follows the raster
  block, as the default's converted list also assumes; its `provoking` argument is not used.

- A texture a program samples through several samplers is read through copies of its global
  variable (run 10, `split_samplers`): naga's GLSL writer takes one sampler per texture, and the
  shaders track owns naga. A change to naga's writer (one combined sampler per pair) would make the
  rewrite unneeded.
- Passes without attachments draw into an `R8` renderbuffer of their own (run 10), which the
  interface's `EmptyPass` allows; with no draw buffer, nothing is written to it.
- Attachments that are layers of 2D arrays are never invalidated (run 11): `DontCare` and `Discard`
  are hints, and in Chrome invalidating a layer loses the level's other layers. Not counted as a
  slow path, since nothing extra runs; the hint is only not given.
- Immediates (run 11) are plain uniforms (plan 5.8 names `uniform4uiv`; each part is set by its
  own type instead, as naga's GLSL writer declares them). Bytes a draw does not give read zero.
- Vertex formats GL ES cannot read as stored are converted in a program variant per layout
  (run 11), not fetched from a storage buffer or converted on upload as the interface's
  `VertexFormat` text puts it: WebGL 2 has no storage buffers, and a variant reads the guest's bytes
  in place. The first draw with a new layout key links a program (a stall the first time).

- (run 12) A draw blending a second source into a pass of more than one colour target loses the
  device naming it: the interface has no rule for it, GL refuses it, and Direct3D 11 leaves it
  undefined.
- (run 13) Every write and buffer copy into a depth format is drawn (`DepthWrittenByDraw`, and
  `StencilWrittenByDraw` for stencil), `depth16unorm` and `depth32float` included, which GL could
  take from bytes: one route for every depth format, and the native ANGLE dropped `depth16unorm`
  bytes from a pixel unpack buffer. Direct3D 11 does not write depth from the host at all, so the
  route serves Direct3D 12 and copies.
- (run 13) Unorm depth is written as `v * 2^-b + v * 2^-2b` (b the bits) from a uniform, not
  `v / (2^b - 1)`: the two products are exact and their sum rounds once to the nearest float of
  v / (2^b - 1), where a GPU's divide may round twice.
- (run 13) The stencil of cube textures is read back by bit draws (`StencilCopiedByDraw`) even with
  WEBGL_stencil_texturing, which read 0 through `usamplerCube` in Chrome 157.
- (run 12) mozangle lacks every new extension but OES_draw_buffers_indexed and
  EXT_blend_func_extended, so the others (and the rig pictures needing them) run only in Chrome.

- The back end's WebAssembly imports `next_task` from a module `wcontainers-device` (JSPI, plan
  4.8): the plan has the device thread suspend through JSPI but names no import; the call channel
  (second batch) should supply the same one. Only query waits use it today; fence and pixel read
  waits still call `finish()`.
- `FRACTIONAL_DEPTH_BIAS` and `TIMESTAMPS` come from probes of the context rather than from the
  extension list alone: GL takes float bias units and EXT_disjoint_timer_query advertises
  timestamps, but ANGLE on Direct3D 11 rounds the one and gives zero bits for the other.
- Copies of multisampled integer, depth, stencil and sRGB textures lose the device (integers as
  needing `MULTISAMPLE_READ`, the others as not built); plan 5.8 names `MultisampleCopyResolved`
  for copies in general.

- Size queries are answered from a hidden uniform holding the bound view's size (run 4), not by
  GL's `textureSize`, because ANGLE answers for the wrong level after a base level change. It costs
  one uniform per texture a program sizes, set only when the bound view changes.
- `capture_vertices` of strips, fans, loops and indexed draws (run 6) draws the capture program
  as points, one draw per run of consecutive vertex numbers, so the vertex stage runs once per
  captured vertex rather than once per vertex. Indexed captures read their indices back (a wait for
  the GPU). With rasterising the original draw runs again for the picture, so the vertex stage runs
  twice; Direct3D runs it once, but a vertex program gives the same results each time. A capture
  past MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS fails
  its link and loses the device with GL's log; the interface has no limit for it.
- Copies between BGRA textures and buffers go through the host (`HostCopyReadBack`): the bytes must
  be swapped, and WebGL 2 has no BGRA upload or read format. A draw could swap them on the GPU;
  not built, since buffer copies of render targets are rare next to texture writes and read backs.
- The sRGB twin of a BC texture is refreshed from the host copy and counted `HostCopyKept` (run 7):
  none of the named slow paths is closer, and the twin route for other formats counts
  `CopyThroughBuffer`, which this is not.
- BC4 and BC5 decoded without their extension are decoded as floats (bcdec_rs's `bc4_float` and
  `bc5_float`) and stored as the nearest half floats (R16F, RG16F), within half a half-float step of
  the formula (check `interpolated_bc4_and_bc5_values`). Until run 16 they were stored at 8 bits,
  up to 0.43 of a step off. The GPU's own decode, which the native route and real Direct3D 11 on
  the same GPU use, is further from the formula (see "Measurements, run 16"), so the decoded
  route differs from the reference by about that much. BC1 to BC3 and BC7 stay at 8 bits: bcdec_rs
  decodes them to bytes only. Read backs are unaffected (host copy). BC in 3D textures is always
  decoded, since GLES compresses only 2D, array and cube textures.

## Interface changes

- 8beef003 (G3b run 5) "interface: fine derivatives in programs are a capability":
  `Features::FINE_DERIVATIVES` (bit 57), and the plan's lists in 10 (WebGPU has it, WebGL 2 lacks
  it). Before, a shader using `dpdxFine` reached this back end's GLSL writing and was answered
  `Invalid`, so a front could not read the gap from the caps (check of G3b run 4). The WebGPU
  back end lists its features by hand and does not declare the bit yet (its track's to add:
  WGSL always has fine derivatives).
- 62e28dfe (G3b run 2) "interface: derived serde for the resource, pass and clear descriptors, so a
  call stream can be saved and replayed": `Serialize` and `Deserialize` on the descriptors that
  had none (buffer, texture, view and sampler descriptions and their usages, origins, extents,
  load and store operations, viewports, scissors, empty passes, query kinds, rings, fans and
  loops, clear values, binding layout keys, stage sets), as the state blocks already had for the
  cache. Used by src/trace.rs.
- 4a72bce0 (run 15) "interface: cut the scissor of a fractional viewport drawn in the program to the
  pixels its api draws, as direct3d rounds each edge down": the doc comment of
  `Features::FRACTIONAL_VIEWPORT` and plan 4.12's "Fractional viewports". Direct3D scissors to the
  viewport with each edge rounded towards negative infinity (D3D11.3 functional specification,
  15.6.1 and 15.7), so a Direct3D front cuts its scissor that way; Vulkan clips to the viewport only,
  so its front keeps the pixel-centre rule. Shown by the fractional viewport comparison: wgpu's and
  Chrome's WebGPU on the RTX 3080's Direct3D 12 leave the last row of a viewport ending at y 13.75
  undrawn. No code changes; fronts that cut the scissor read the text.

- d37c5d9 "interface: said which vertex outputs capture_vertices writes to which target and how
  they are packed": the vertex stage's location outputs in location order as packed 32-bit
  components, no built-ins. With one target they are interleaved; with more, output n goes to
  target n, as OpenGL's two modes. Strips are written as lists, and the targets hold every vertex.
  A doc comment on `capture_vertices` and `Limits::capture_buffers`, and the plan's 4.3 text.

- db2c406 "interface: said that a webgl back end refuses a blend block using the constant colour and
  constant alpha together": a doc comment on `Features::BLEND_CONSTANT_ALPHA` and the plan's WebGL 2
  list. WebGL forbids those two factors together in one colour blend (WebGL specification, "Blending
  With Constant Color"); the back end answers `Absent(BLEND_CONSTANT_ALPHA)` for such a block and the
  front blends it in the program.

- 4b71023 "interface: drew the built-in programs in the clip depth convention the back end has":
  the built-in raster block takes `depth_minus_one` from `CLIP_DEPTH_ZERO_ONE`. The built-ins write
  clip z 0, which both conventions hold. On its own it was not enough: the block still asked for
  provoking vertex First, which this back end then always refused (found by the second check).
- ab4b7e9 "interface: drew the built-in programs with the provoking vertex the back end has": the
  built-in raster block asks for First only where `PROVOKING_VERTEX_FIRST` is present, else Last; the
  built-ins pass no flat values, so either gives the same picture. Shown by `triangle-clear_rects`
  in Chrome with and without WEBGL_provoking_vertex and EXT_clip_control, and on ANGLE without them.

- 4e98cf5 (run 9) "interface: made the default body of generate_mips a public function, so an
  override can keep it for the cases it does not take": `wcontainers_gpu::generate_mips_by_draws`,
  the default's body moved unchanged; the trait's default calls it. Plan 4.3's `generate_mips`
  text names it. The back end's override needs it for formats `generateMipmap` refuses.

- 39b57ad4 and 9ba95585 (run 11): `SlowPath::SampledTargetCopied` added for a fix built on run 10's
  wrong guess, then taken back out in the next commit. The interface is as it was.

- 72dbc184 (run 12) "interface: gave point filling a capability of its own, since webgl's polygon
  mode draws lines only": `Features::POLYGON_MODE_POINT` (bit 56) for `FillMode::Point`;
  `POLYGON_MODE` now covers `FillMode::Line` only. Plan 4.11's lists and 5.3's `glPolygonMode` text.
  The recording back end and the WebGPU back end refuse both fill modes as `POLYGON_MODE`; they may
  name `POLYGON_MODE_POINT` for points.

- (run 6) "interface: named the slow path of captures expanded into points in list order":
  `SlowPath::CaptureExpanded`, for draws an API's transform feedback cannot capture as they are
  (GLES 3.0 takes only lists without indices), and the plan's 5.8 text for it.

## Installed on the desktop

- (G3b run 7) In the LF worktree C:\wc\src\phase-2-graphics-webgl-lf: the shaders crate's
  submodules (dxbc-spirv 0.01 GB, dxil-spirv 0.06 GB), which the literal check scans.

- (G3b run 6) C:\wc\src\phase-2-graphics-webgl-lf: a git worktree of this branch's desktop
  repository with LF line ends, for the literal check (How to run); source only, its size not
  measured. C:\wc\out\webgl\frames: the breakdown's profiles and logs. Nothing installed.

- (G3b run 2) C:\wc\out\webgl\record-direct.ps1 (2 KB, the recording script of How to run) and
  C:\wc\out\webgl\trace (the recorded streams and replay results, under 1 MB).
- C:\wc\tools\wasm-bindgen-0.2.130-x86_64-pc-windows-msvc (20 MB): the wasm-bindgen command, from its release
  archive on github.com/wasm-bindgen/wasm-bindgen (MIT OR Apache-2.0).
- C:\wc\tools\libclang-18.1.1: libclang.dll (84 MB) from the PyPI `libclang` 18.1.1 wheel (Apache-2.0
  with LLVM exception), which mozangle's bindgen needs to build ANGLE.
- C:\wc\tools\angle-mozangle-0.7.1: libEGL.dll and libGLESv2.dll (7.1 MB), built by angle\build.cmd.
- The mozangle build's objects in C:\wc\target\phase-2-graphics-webgl\release (the unpatched build of
  runs 1 to 4, about 200 MB) and C:\wc\target\phase-2-graphics-webgl\angle-patched (run 5, 403 MB)
  were deleted in run 13; angle\build.cmd makes the second again (about 4 m 30 s). The track's
  build directory C:\wc\target\phase-2-graphics-webgl (19.6 GB) was deleted at the end of run 15,
  as the runs are now asked to; the next build.cmd makes it again from nothing. C: had 407 GB free
  after. Run 16 built it again from nothing and deleted it at its end. Run 17 did the same (C: had 441 GB free after). Run 18 did the same (C: had 443 GB free after). G3b run 1 built it again (with the
  Direct3D 11 library's WebGL 2 build in its `webgl` folder) and deleted it at its end (11.4 GB;
  C: had 432 GB free after).
- C:\wc\tools\mozangle-0.7.1-patched (run 5): the mozangle crate's sources, unpacked and patched by
  angle\build.cmd. Its size was not measured.
- Left by the independent check of the first run (not ours to remove without asking): the pictures
  in C:\wc\out\webgl-check and an empty lock file C:\wc\locks\phase-2-graphics-webgl-check.

## Licences

- Only with the `trace` feature (G3b run 2, the call stream file): postcard 1.1.3, MIT OR
  Apache-2.0; its cobs 0.3.0 (with thiserror 2), embedded-io 0.4.0 and 0.6.1: MIT OR Apache-2.0
  (their Cargo.toml). serde 1 (already in the lock file). naga's own `serialize` and
  `deserialize` features.
- glow 0.18.0: MIT OR Apache-2.0 OR Zlib (its Cargo.toml); its slotmap 1.1.1: Zlib.
- half 2.7.1 (run 16, half floats of decoded BC4 and BC5): MIT OR Apache-2.0 (its Cargo.toml;
  already in the lock file through naga). Its dependencies: cfg-if 1.0: MIT OR Apache-2.0;
  zerocopy 0.8 and zerocopy-derive: BSD-2-Clause OR Apache-2.0 OR MIT.
- Tests only, Windows (run 17, tests/bc_reference.rs): image 0.25.10 (`exr`, `png`), rand 0.9.5
  and rand_chacha 0.9.0: MIT OR Apache-2.0 (their Cargo.toml; already in the lock file through
  the rig's crate, whose seeded blocks they reproduce). image's `exr` brings exr 1.74.2 and
  lebe 0.5.3: BSD-3-Clause (their Cargo.toml; recorded in run 18).
- rangemap 1.8.0 (run 18, the index ranges of src/restart.rs): MIT/Apache-2.0 (its Cargo.toml);
  no dependencies by default.
- bcdec_rs 0.2.1 (run 4, for `BlockDecoded`): MIT (its Cargo.toml; the plan's section 3 records its
  repository's LICENSE). No dependencies.
- khronos-egl 6.0.0 (native only, feature `dynamic`): MIT/Apache-2.0 (its Cargo.toml); its
  libloading 0.8.9: ISC; libc 0.2.191 and pkg-config 0.3.34: MIT OR Apache-2.0.
- naga 30.0.2 (`glsl-out`; `wgsl-in` for the test programs): MIT OR Apache-2.0.
- wasm-bindgen 0.2.130, js-sys and web-sys 0.3.107 (wasm32 only): MIT OR Apache-2.0.
- anyhow (test programs): MIT OR Apache-2.0.
- Tests only, Windows (run 6, the suite's window): windows-sys 0.61.2: MIT OR Apache-2.0 (its
  Cargo.toml; already in the lock file through the WebGPU back end's tests).
- Tests only, WebAssembly (the per-draw cost example): wasm-bindgen-futures 0.4.80, wgpu 30.0.2
  (`webgpu`) and the WebGPU back end of this workspace: MIT OR Apache-2.0 (their Cargo.toml).
- Tests only: getrandom 0.4.3 with `wasm_js` (wasm32, for proptest in the suite crate): MIT OR
  Apache-2.0; on Windows wgpu 30.0.2 (`dx12`) and pollster 1.0.1: MIT OR Apache-2.0 (their
  Cargo.toml), and the WebGPU back end of this workspace.
- mozangle 0.7.1 (the ANGLE build tool in angle/, outside the workspace, never a dependency of the
  back end): BSD-3-Clause (its Cargo.toml and LICENSE, ANGLE's). angle/d3d11-clip-control.patch
  carries upstream ANGLE's changes ba845fcf25d0, 6092e0b75c22 and 1dd3a4c4053a (BSD-3-Clause,
  github.com/google/angle), adapted to mozangle's older code. angle/immutable-base-level.patch
  (run 11) carries upstream ANGLE's handling of immutable textures in
  `TextureD3D::getLevelZeroWidth` and `getLevelZeroHeight` (read from its main branch at
  chromium.googlesource.com/angle/angle), adapted: level 0's size is read from its image.

## Next

G3b: every finding of the check of run 5 is fixed or recorded (run 6); run 7 removed the ATG
samples' wait; run 8 removed filter_draw's lost device (copies of one shader are one GL
program). In this order:
0. Distinct large programs by the hundred (Known failures, run 8): if a real program meets it,
   measure the adapter's memory per live program there; a bound on bytes of compiled code is
   what would need declaring, and GL reports none.
1. (Run 7: the ATG samples' wait was the twin refresh through a pixel buffer; now a draw.) The
   start-up crash in the system's d3d11.dll (Known failures): take a crash dump. A program with
   many draws a frame: bgfx draws nothing through the WebGL 2 build (the Direct3D 11 track's to
   read); measure the next real program that does.
2. The twin is refreshed every second frame (SimpleLightingPC's BGRA swap chain buffer and its
   sRGB twin, 1280 by 720): a draw now, but tracking which of the pair the front writes could
   avoid the copy altogether.
3. Global GLSL constants holding negative zero or bits made floats are still folded by ANGLE
   (`opaque_float_bits` cannot reach a constant initialiser); naga writes them only for named
   module constants, which the converters do not make today.
4. Rerun the Direct3D 11 front's WebGL 2 scenes after its next runs (constants through a texture
   past the uniform block limit; views at 10_0 through `ReinterpretCopy`).
5. Per-format component widths: move `B5G6R5_BITS` and convert.rs `bits()` to the interface's
   table once `wcontainers-gpu` has it (fix-graphics.md item 16), and work the literal list down.

After G3b, from G3:
What is left of G3's pass bar cannot be done in this crate:
- G1's suite should take clip depth and provoking vertex from the caps. Until it does, the routes
  without the extensions fail 26 suite tests in Chrome, 25 natively.
- The r32float comparison against WebGPU's hardware filter stays one float step off (G1's helper),
  natively and in Chrome.
- `TIMESTAMPS` cannot run on the RTX 3080, which gives zero timestamp bits, so the route used
  where it is present has never run on any GPU; only its refusal is tested.
- G1's helper should average integer twins in integers, so exact ties round the same on every
  back end (see "Known failures").

After that, in about this order:
1. A draw without restart from GPU-written indices holding the largest value reads them back at
   every draw; keep the widened indices until the buffer is written again.
2. Draws and captures whose base vertex reaches below vertex 0.
3. `CLIP_DISTANCE` and `CULL_DISTANCE` in Chrome (see "Known failures"): built but kept absent,
   so `clip_and_cull_distances` runs nowhere.
4. (Done in G3b run 6: `SAMPLE_SHADING` in Chrome.)
5. Copies between signed normalised textures drawn into (Chrome) go through the host, a wait;
   both render, so copyTexSubImage could copy them on the GPU.
6. The present-feature routes of `TIMESTAMPS` and `FRACTIONAL_DEPTH_BIAS` (`write_timestamp`,
   `timestamps_disjoint`, a fractional `polygonOffset`) have run on no GPU: the RTX 3080 gives
   zero timestamp bits and ANGLE rounds bias units. A GPU and browser that have them would test
   them.
7. The snorm minimum per layer (Departures, run 3), if a program meets it.
8. Cube gathers at a cube's corners (three faces) are compared with nothing: APIs leave them to the
   implementation, and the run 3 comparison leaves them out.
