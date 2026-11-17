# Graphics: the WebGPU back end (G2)

Branch phase-2-graphics-webgpu. The plan is docs/phase-2-graphics.md (sections 4, 5.7 and 12, G2).
Crate: crates/wcontainers-gpu-webgpu, on wgpu 30.0.2.

## Where things stand

G2 (the whole milestone, section 12) is in progress: G2 runs 1 to 4 are below as "Run 5" to
"Run 8". The pass bar items still open are listed under "Next (G2)". Earlier runs (G2a, the first
slice, runs 1 to 4) follow them.

Run 8 = G2 run 4, 2026-11-17:
- Merged origin/phase-2-graphics (at 41816d5) and origin/phase-2-graphics-direct3d-11. The
  interface changes they brought (doc sentences, recording back end checks) needed nothing here.
  The first desktop build after them failed to link a stale `wcontainers-gpu-test` library
  (unresolved symbols); `cargo clean -p wcontainers-gpu-test` fixed it. Recorded runs now clean
  both crates.
- Presenting a format the window lacks: a Metal layer takes no `rgba8unorm`, which every back end
  presents (plan 4.9), so the device lost it there ("Validation Error" from wgpu). Now
  `configure_surface` asks the window's formats (the back end keeps its adapter for this); a
  window lacking the format is configured in the other byte order (`bgra8unorm`), frames are drawn
  into a stand-in texture of the asked format, and `present` draws it into the window's texture
  (`PresentConverted`, interface change below; src/fill.rs `present_by_draw`). Not yet run on a
  shown window (see the present test below); on Windows and in Chrome the window takes the format
  and nothing changes.
- The suite's present test on macOS: AppKit makes windows only on the main thread and the standard
  test harness runs every test on a thread of its own, so tests/present.rs runs
  `present_shows_the_frame` with libtest-mimic on the main thread (on Windows it has no tests:
  tests/suite.rs runs present there). The host (tests/host/mod.rs, shared by both files) opens a
  borderless 4 by 4 pixel floating window whose content view's own layer is the surface, and reads
  it through ScreenCaptureKit (one window, `SCScreenshotManager`) in sRGB, which undoes the
  display's colour matching exactly: a Swift probe of the same steps read 256 of 256 colours
  exactly through an external monitor's ICC profile (`CGWindowListCreateImage` gave the display's
  values, and converting them back to sRGB missed 193 of 260). The screen recording permission is
  held by the session the tests run from here (`CGPreflightScreenCaptureAccess` true).
- That test still fails here, as a known failure with its reason: macOS reports every new window
  of this session as not visible (`occlusionState` 0x2000, also for a plain Swift window, at any
  place and level), and wgpu's Metal surface hands out no texture to an occluded window, so the
  frame is drawn unshown, as the back end does for an occluded window. Why this session's windows
  are occluded is not known (display not asleep, session not locked). The test now says so instead
  of comparing the window's grey.
- The suite's list for hosts that run present apart (the browser page and macOS) is one macro,
  examples/scenes/names.rs; web/check runs the page against the whole native list on Windows, so a
  test the shared crate adds and the list lacks fails there.

Run 7 = G2 run 3, 2026-11-17:
- Merged origin/phase-2-graphics (at f107eb5), then origin/phase-2-graphics-direct3d-12 and
  -shaders (each a few commits ahead of it). The shared line brought the interface change c8a7be9:
  `copy_texture` copies a combined depth-stencil format whole, both aspects, with `stencil` false.
- Combined depth-stencil copies now follow it: equal formats and whole levels are one WebGPU copy
  of every aspect (before, the depth aspect only, which WebGPU refuses); part of a level is drawn,
  depth by `DepthWrittenByDraw` and stencil by the new `StencilWrittenByDraw` (interface change
  below): per layer one draw clears the rectangle's stencil, then one draw per bit with that bit's
  write mask and reference 0xff keeps the fragments whose source has the bit (src/fill.rs). Copies
  between a combined format and another format mark the device lost.
- Part of a stencil from a buffer (WebGPU copies depth and stencil only whole) goes into an
  `r8uint` texture and is drawn the same way. Copies into a buffer and read backs that WebGPU
  cannot make, 24-bit depth at all and part of any depth or stencil level, draw the aspect into a
  colour texture first (`DepthReadByDraw`): 24-bit depth as `round(d * 16777215)` into `r32uint`
  (exact: a unorm value stored as the nearest float is within half a step of its integer after one
  correctly rounded multiply), 32-bit float depth's bits into `r32uint`, 16-bit depth into
  `r16uint`, stencil into `r8uint`. A depth or stencil texture made with `COPY_SRC` (which read
  backs need) gets binding usage, and one made with `COPY_DST` render usage, where its format
  allows, so these draws can run; textures without copy usages get neither. Part of a 16-bit depth level from a
  buffer marks the device lost (no Direct3D call makes one).
- A buffer copy or read back of a combined format's depth aspect was sized as both aspects (8
  bytes a texel); it is now the depth format's size.
- BC1 to BC5 against Direct3D: tests/direct3d.rs loads every BC format's seeded blocks with
  Direct3D 11 on the RTX 3080 and with WARP (a pixel shader compiled at test time by
  `D3DCompile`), and through the back end natively and decoded. Natively every texel equals
  Direct3D's. Decoded, BC1 to BC5 are within 2 / 255 of WARP (BC4 and BC5 within 2.6e-4, the
  16-bit float they are stored in), while the RTX 3080's own decoder differs from WARP by up to
  13 / 255: hardware decoders interpolate BC1 to BC5 within a tolerance. So the bar is: decoded
  texels no further from the GPU's than WARP's are, plus one step of the format they are stored in.
  It passes. The old one-step comparison with the GPU's decoder in tests/blocks.rs stays ignored
  as a known failure that WARP fails too.
- `prepare` in Chrome: examples/scenes/prepare.rs (shared by tests/prepare.rs and the page, timed
  by `performance.now()` there) draws 16 new programs unprepared, then 16 others after `prepare`;
  web/check prints both lines and fails if the prepared draws built anything. In Chrome the
  `prepare` calls return at once (0.6 ms for 16) and the compiles finish in the GPU process before
  the next submit completes, so `BACKGROUND_PREPARE` is confirmed there (measurements below).
- The scene tests (scenes.rs, raster.rs, the two writes.rs tests drawing the triangle) compare with
  the rig's allowance for each picture (references/d3d11/tolerances.txt, what WARP earns: 1 for the
  triangle, quads and viewports, 0 for the instanced scene), so they hold on any GPU, and print
  how many pixels differ at all. On the RTX 3080 every pixel still equals Direct3D's (printed, and
  web/check compares the twelve pictures at 0).
- Found: incremental builds on the desktop sometimes kept a stale library (a changed string in
  src/memory.rs did not reach the test binary, twice). Runs whose results are recorded here now
  start with `cargo clean -p wcontainers-gpu-webgpu` in the job's build command. Not reported to
  the rig track yet.

Run 6 = G2 run 2, 2026-11-17:
- Merged origin/phase-2-graphics at d31a49e (every other graphics branch was already in it); the
  lock file then lacked one dependency of the Direct3D 12 crate (`naga`), added in 28a01f6.
- Waiting in the browser is built (src/wait.rs, plan 4.8): `wait_read`, `wait_fence` and the
  24-bit depth probe suspend the WebAssembly stack through JSPI (wasm-bindgen 0.2.130's
  `#[wasm_bindgen(suspending)]` import of `Promise.resolve`) until a callback of the browser's
  WebGPU (map, work done, error, loss) wakes it; `block_on` does the same for futures (adapter and
  device requests). The call that waits must run inside an export marked `#[wasm_bindgen(jspi)]`.
  The probe now runs in the browser too, so `depth24()` always answers.
- The shared suite runs in Chrome (examples/scenes/suite.rs, web/suite.html, web/suite.mjs, one
  page per test, run by web/check against the native suite's `--list`): 24 of 24 pass on the RTX
  3080. Two things had to change for it:
  - wgpu 30.0.2's browser back end dropped a render pass's `occlusionQuerySet` (Chrome: "The
    occlusionQuerySet in RenderPassDescriptor is not set"); wgpu is vendored in third_party/wgpu
    with that one fix (d65f0be, a `[patch.crates-io]` entry in the workspace manifest, as for naga).
    wgpu's trunk has the same bug. Not reported upstream.
  - In the rig's headless Chrome 157, `drawImage` and `createImageBitmap` of a WebGPU canvas give
    transparent black; `toDataURL` gives the frame. The page reads the canvas with `toDataURL` and
    the host decodes the PNG.
- Real Direct3D 11 on the same GPU (tests/direct3d.rs, Windows only, through the rig crate's device
  and shaders): `generate_mips` of `r32float` on a device without `float32-filterable` and of
  `rgba16uint` holding `rgba16unorm` values, each level made from Direct3D's level above, against
  GenerateMips of a 64 by 48 texture (7 levels, odd sizes further down), and a four-sample
  `rgba8unorm` texture made with `reinterpret` resolved as `rgba8unorm-srgb` against
  ResolveSubresource with `R8G8B8A8_UNORM_SRGB`. Tolerances: `r32float` 2^-16 relative to the value
  (at least 1), `rgba16unorm` one step, the resolve exact. The same resolve as `rgba8unorm` differs.
- BC textures are decoded where the device has no `texture-compression-bc` (src/blocks.rs,
  `BlockDecoded`, plan 4.12): BC1 to BC3 and BC7 as `rgba8unorm` (sRGB kept), BC4 and BC5 as
  `r16float` and `rg16float`, BC6H as `rgba16float`, by bcdec_rs at each write; the blocks are kept
  on the host per level and layer and answer read backs at once, copies into buffers (written in
  command order), copies between BC textures and copies into uncompressed textures of the same
  block size. Copies into such a texture from a buffer or an uncompressed texture read the source
  back and wait (`HostCopyReadBack`). Declared `SAMPLE | FILTER | COPY_OUT`, one sample.
- Natively, a copy between a BC texture and an uncompressed one of the same block size (Direct3D's
  CopySubresourceRegion) failed as "different block sizes"; it now goes through a buffer with the
  destination's extent in its own texels (`CopyThroughBuffer`).
- `FRACTIONAL_VIEWPORT` is declared in the browser too: the eight-viewport scene (now
  examples/scenes/viewports.rs, shared by tests/raster.rs and the browser page) drawn in Chrome
  equals Direct3D 11's `raster-viewports.png` in every pixel, and moved to whole pixels differs
  (web/check). The depth probe in Chrome (Dawn on Direct3D 12) answers `Float(0.10000001)`:
  `depth24plus` is 32-bit float there, so `DEPTH24_UNORM` stays clear in the browser.
- New tests: tests/direct3d.rs (above); tests/blocks.rs: every BC format's seeded blocks
  (`WEBGPU_SEED`, default 188) on a device with BC and one made without, three levels of 12 by 8
  (the last a partial block), each level read back as written on both; BC6H and BC7 sample exactly
  alike on both; copies texture to texture, into a buffer at pitch 256 and back, from and into
  `rgba32uint` texels keep the blocks on both, `HostCopyReadBack` only where decoded. Known failure
  (`#[ignore]`, `bc1_to_bc5_sample_alike_natively_and_decoded`): see "Not carried out yet".

Run 5 = G2 run 1, 2026-11-17 (fixes from the independent check of run 4, then pass bar items):
- Merged origin/phase-2-graphics at 528db9c (a fast-forward). It brought three shared suite tests,
  `multisampled_targets_cover_their_samples_and_resolve`,
  `occlusion_queries_answer_whether_samples_passed` (which also covers `resolve_queries` at the
  unaligned destination offset 8) and `programs_run_per_sample_and_read_samples`; all 24 suite tests
  pass on Vulkan and on Direct3D 12.
- `clears_fill_exactly` passes on Direct3D 12 now: an integer clear wgpu's Direct3D 12 would round
  through a float is loaded and then drawn by a full-target draw writing the value from a uniform
  (src/fill.rs, new slow path `IntegerClearByDraw`, an interface change below).
- Copies into depth are built (`DepthWrittenByDraw`, src/fill.rs): from a buffer or a write, the
  bytes go into an `r32float` texture (32-bit float depth) or an `r32uint` one (24-bit depth, drawn
  as `(v & 0xffffff) / 16777215`) and a draw writes them as depth; from a texture of the typeless
  group (`r32float`, `r32uint`, `r32sint` into 32-bit depth; any depth into the same family) by a
  draw reading it, also into part of a texture, which WebGPU copies never do for depth. Depth
  textures made with copy usages get render and binding usage for it. Stencil copies WebGPU refuses
  (part of a stencil, or between formats) and multisampled depth copies mark the device lost with
  the reason (`StencilCopiedByDraw` is not built); so do copies of 24-bit depth into a buffer.
- web/check twists the quads on wgpu's Vulkan too, and a comparison gt-compare could not make
  (exit 2) now fails the check instead of counting as "differs". It builds with `--locked`.
- The lock file: the shared line's daa1630 brought it in step; every build and test command here
  now passes `--locked`, so a stale lock file fails the build instead of being rewritten.
- A program without a fragment shader draws into colour targets through an empty fragment stage
  of the back end's own with every write mask off (was "not built yet").
- The 24-bit depth probe of 4.11 runs natively when the back end is made (src/probe.rs,
  `WebGpu::depth24()`): on the RTX 3080 wgpu's Direct3D 12 answers one 24-bit step
  (`Unorm(0.10000009)`, `DEPTH24_UNORM` set) and wgpu's Vulkan two steps (`Other(0.10000015)`:
  NVIDIA's Vulkan bias unit for D24 is 2^-23), so there the bit stays clear and fronts bias in the
  program. In the browser it does not run yet (waiting needs JSPI, 4.8), so the bit is clear there.
- The rig's depth-bias scene (constant, slope and clamped bias on `D24_UNORM_S8_UINT`) drawn
  through the back end on `depth24plus-stencil8` equals Direct3D 11's `depth-bias.png` in every
  pixel on Direct3D 12 (tests/depth.rs; on Vulkan the bit is clear, so the test says the front's
  route applies and draws nothing).
- Samplers: a linear sampler bound where the view cannot be filtered marks the device lost with
  "fronts filter in the program (FilterInShader)" instead of wgpu's validation text.
- Formats: blending is declared only where a format renders (wgpu marks `r8snorm` blendable on a
  device that cannot render it); 16-bit normalised formats are asked for natively
  (`TEXTURE_FORMAT_16BIT_NORM`).
- Programs are validated when made with naga's validator for WebGPU's core language and the
  device's features (`wgpu_naga_bridge::create_validator` with `DownlevelFlags::compliant()`), and
  the back end keeps that validation's info, not the front's.
- `prepare` builds the combination's pipeline into the cache, keyed as the draw will key it, so
  the first draw builds nothing (counted). `BACKGROUND_PREPARE` is declared only in the browser.
- `FRACTIONAL_VIEWPORT` is declared natively: the rig's eight-viewport scene (four viewports start
  half a pixel down; drawn here once per viewport with the first instance as the viewport number,
  in place of its geometry shader) equals Direct3D 11's `raster-viewports.png` in every pixel on
  both APIs, and the same viewports moved to whole pixels differ (tests/raster.rs). In the browser
  the bit stays clear until the same check runs in Chrome.
- A pass fitted by the shared pass tracker with four-sample colour targets of 2 by 2 and 3 by 3
  draws right on the GPU, sample by sample (tests/fitted.rs, ported from the shared helpers test
  that runs on the recording back end): `AttachmentsFitted` once, no `MultisampleCopyResolved`.
- New tests: tests/copies.rs (buffers at a 36-byte pitch and writes into all four depth formats;
  `r32float`, `r32uint` and `depth32float` into part of 32-bit depth and 24-bit into part of
  24-bit depth; seven typeless group copies of colour formats, through a buffer or native for sRGB
  twins, each exact and with its slow path counted); tests/depth.rs (the probe, the bias picture);
  tests/prepare.rs (prepared combinations build nothing at their first draw, timed);
  tests/bindings.rs `programs_without_a_fragment_stage_write_only_depth`,
  `helper_blocks_of_each_stage_read_their_own_values` (set 3, binding 0 read by the vertex stage
  and 12 by the fragment stage), `layouts_follow_each_views_filterability` (an `rgba8unorm` view with
  a point then a linear sampler is one pipeline; `r32float` and `depth32float` views at points on a
  device made without `float32-filterable`, where a linear sampler is refused; `r32float` filtered
  with it), `tier1_formats_are_declared_only_with_it` (on a device made without wgpu's 16-bit-norm
  and adapter-specific format features: no 16-bit normalised format and none can be made, 8-bit
  snorm without render or blend bits; with them, all six 16-bit formats sample and the unsigned
  and 8-bit snorm ones render), `programs_are_validated_with_webgpus_core_language`.

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
  (`WriteSplitPass`). Writes that take effect at once but are not whole words (and any later
  write of that buffer before the submit) are copies in a lead encoder submitted ahead of the
  frame's encoder (run 3, below). Read backs copy into a mapped buffer, mapped after the submit; rows arrive
  packed. Occlusion queries, `resolve_queries` by `resolveQuerySet` and a copy, resolves, surfaces
  (windows registered by the host with `WebGpu::add_window`) are in.
- The rig's three Direct3D 11 reference scenes (triangle; textured, depth-tested, blended quads;
  instanced draw with a constant buffer), drawn through the interface with WGSL programs, equal real
  Direct3D 11's pictures in every pixel on the RTX 3080: natively through wgpu on Direct3D 12 and
  on Vulkan, and built for WebAssembly in Chrome 157's WebGPU. Tolerance 0 (see Measurements).
- The quads with the colour source factor changed to one, or the depth test changed to greater,
  fail the comparison both natively and in Chrome.
- Since run 4 the quads also equal Direct3D's picture drawn on `depth24plus-stencil8` (natively and
  in Chrome) and on every other depth format of 24 bits or more (natively), and the whole shared
  suite runs here, present included (Vulkan 21 of 21; Direct3D 12 20 of 21, the known clear).
- Writes keep command order (test `writes_take_effect_in_command_order`: copies between an
  ordered write that stages, a second one, and a renaming discard each read the bytes written just
  before them), and an ordered write of a vertex buffer the open pass drew with splits the pass
  without changing the picture (`ordered_write_in_a_pass_splits_it`, equal to Direct3D's
  triangle, one `WriteSplitPass`).
- Reviewed for places doing less than the interface promises: extra viewports or scissors and
  differing per-face stencil references now fail loudly instead of being ignored, textures take
  only the copy usages their format allows (`depth24plus` has none), and anisotropy applies only to
  fully linear samplers. What is still missing is listed below.

Run 2, 2026-11-17 (fixes from the independent check of run 1, then the shared suite):
- Read backs now count as uses of their source, so a later write in the same encoder is ordered
  after them and `destroy` waits for them (they returned the new bytes, and destroying the source
  lost the device). Resolve targets of a pass count as used too.
- The device is made with `float32-blendable` where the adapter has it (the RTX 3080 natively
  and Chrome do). A blend into any non-integer target the device cannot blend now marks the device
  lost with the reason (the front blends in the program, `BlendInShader`); before, the blend was
  silently dropped.
- The uniform arena of 4.5 and 4.6 is built: uniform-only buffers with `MapIntent::Often` live in
  slices of one large buffer; a `Discard` while recorded work uses one moves it to a free slice
  (counted in `Counters::slices`), its bind group stays, and the draw passes the slice's offset as
  a dynamic offset. Bind groups are shared by what they are made of, with dynamic offsets left out:
  sets differing only in a dynamic offset, or naming different arena buffers of one size, share one
  bind group. A draw whose sets did not change and whose bind epoch (moved only by a rename or arena
  move of a buffer some set names) is the same only reads each dynamic entry's offset, with no
  allocation; `setBindGroup` is called only when the group or an offset changed.
- Clears to a non-zero pattern copy from a block of at most 64 KiB of the pattern, counted as the
  new slow path `PatternClearCopied` (an interface change, below); before, they uploaded a host
  copy of the whole range uncounted.
- Buffer writes, copies and clears take any byte offset and length (`UnalignedBufferEdge`): the
  aligned middle is copied natively and the words a range only partly covers are merged by a small
  compute program of the back end's own (src/edge.rs); where source and destination differ in their
  place within a word, the program copies every word. So every buffer gets WebGPU's storage usage.
  Buffer and texture copies at row pitches WebGPU does not take go through a buffer at its pitch,
  one buffer copy per row (`RowsRepacked`).
- Under wgpu's Direct3D 12 (native only), which clears colour targets through 32-bit floats, an
  integer clear value past 2^24 now marks the device lost with the reason instead of clearing to
  a rounded value.
- Mirror-once samplers are refused as `Absent(MIRROR_ONCE)` (was `BORDER_COLOR`); dual-source
  blend factors without the feature as `Absent(DUAL_SOURCE_BLEND)` at `create_blend`.
- Handles compare equal when they name one object, and the caching test checks that equal blocks
  made twice are one block.
- New tests: tests/bindings.rs (twenty draws each after a `Discard` of their constant buffer make
  one bind group, no rename and nineteen slice moves, every pixel its own draw's colour; two sets at
  offsets 0 and 256 make one bind group; an `rgba32float` blend equals Direct3D's rule or fails
  loudly), tests/writes.rs (read backs keep command order for buffers and textures; destroying
  after a recorded read back; a pattern clear over 200000 bytes; arena buffers through discards,
  ordered writes, copies between two of them and a read back; a seeded random test of 400 byte-
  granular writes, copies and clears against a model, `WEBGPU_SEED` and `WEBGPU_CASES`), and
  tests/suite.rs (the shared suite of `wcontainers-gpu-test`, seven tests).
- The shared suite (origin/phase-2-graphics at a58f79a, merged): 7 of 7 pass on Vulkan, one of
  which, present, checked no texels (the host gave none; the shared line has since left it out); on
  Direct3D 12, 6 of 7: `clears_fill_exactly` fails, as a known failure, at its `R32Uint` clear of
  0xdeadbeef (wgpu-hal's dx12 command.rs passes clear values to `ClearRenderTargetView` as f32,
  which gave 0xdeadbf00; the back end now fails loudly there). Vulkan clears it exactly; Chrome's
  Dawn is expected to (it clears big integers with a draw on Direct3D 12), not yet run there.

Run 3, 2026-11-17 (fixes from the independent check of run 2):
- Ordered writes no longer land out of order. Before, a write not in whole words went through a
  copy in the frame's encoder without marking the buffer used, so a later whole-word write went
  straight to the queue and landed first. Now such a write, when it takes effect at once (no
  recorded command uses the buffer, or `NoOverwrite`, or after a `Discard` renamed it), is a copy
  in a second encoder, the lead encoder, submitted just before the frame's encoder; later writes of
  that buffer before the submit go there too (`BufferObj::lead_in`), so all of them keep their
  order, and queue writes made earlier still land before. It never splits a pass, also while an
  occlusion query runs. An `Ordered` write of a buffer the frame's encoder uses still stages in
  that encoder and marks the buffer used.
- `NoOverwrite` had three readings (the test model, the interface wording, the back end). The
  interface now says the caller promises that no unfinished command and no `Ordered` write since
  it last waited reads or writes the bytes (interface change below): Direct3D 11 allows
  no-overwrite only on dynamic resources, which copies and `UpdateSubresource` cannot write, and
  OpenGL leaves unsynchronised writes over pending operations undefined. The random test now keeps
  that promise: it tracks the bytes touched since its last wait and makes a write `Ordered` where a
  no-overwrite write would break it (it prints how many of each it made).
- Byte-granular copies are merged in pieces sized from the device's limits (workgroups per
  dimension less one, and the largest storage binding less one offset alignment and a word), not
  16 MiB; a 16 MiB copy between different places in a word lost the device before.
- Clears take a pattern of any length: the copied block is a multiple of the pattern and of 4
  bytes. A 12-byte pattern wrote wrong bytes past 64 KiB before.
- Layouts follow 4.5 exactly: a texture the program only loads from is always
  `unfilterable-float`, and the pipeline and layout keys keep only the bits of textures the
  program samples through a sampler (`paired`, per shader, from naga's sampling set), so one
  program drawn with a filterable and then an unfilterable view of a load-only texture is one
  pipeline. Working out the keys per draw allocates nothing.
- Merged origin/phase-2-graphics at c15677a (a fast-forward): the suite host now leaves out
  `present_shows_the_frame`, which this host cannot check (a swap chain's texels cannot be read
  back once presented), so the suite here is 12 tests and present is not run.
- New tests: writes.rs `writes_of_any_length_keep_their_order` (every pair of modes, first or
  second write not whole words), `writes_ahead_of_a_pass_do_not_split_it` (an ordered write not
  in whole words of a buffer the pass does not use, a discard and a no-overwrite byte of the
  vertex buffer it draws with, inside a running occlusion query: no split, Direct3D's triangle),
  `large_copies_between_places_in_a_word_are_exact` (16 MiB from byte 1 to byte 2),
  `clears_take_patterns_of_any_length` (3, 12 and 16 bytes over 120000 bytes at byte 1);
  bindings.rs `load_only_textures_keep_one_pipeline_for_any_view` (an `rgba8unorm` view, then a
  `depth32float` view, whose key differs: one pipeline, both pixels right).

Run 4, 2026-11-17 (fixes from the independent check of run 3; merged origin/phase-2-graphics at
2684d97, a fast-forward):
- Views of textures with depth and stencil (`depth24plus-stencil8`, `depth32float-stencil8`) were
  invalid: a depth-only view kept the combined format, so any pass or binding with one lost the
  device. Each view now holds two wgpu views where they differ: every aspect in the texture's
  format, which passes draw into, and the aspect `stencil` names in that aspect's own format
  (`depth24plus`, `depth32float`, `stencil8`), which bindings read. A stencil view of a format
  without stencil is `Invalid`.
- The shared suite runs whole (`suite!(Native)`), so tests added to it run here at once.
  `present_shows_the_frame` runs too: the test window is a 4 by 4 popup, and `shown` reads what
  the compositor holds for it after two DWM compositions (`PrintWindow` with
  `PW_RENDERFULLCONTENT`, which includes swap chain contents also on the rig's hidden desktop,
  where reading the screen gave black; alpha set opaque, since a window holds none).
- `current_texture` no longer answers every surface state but success as `DeviceLost`. Outdated:
  configure again as last configured and try once more. Timeout, occluded, or still outdated: the
  frame is drawn into a texture of the configured size and `present` shows nothing, as a real
  swap chain does for an occluded window. Lost and validation failures mark the device lost with
  the reason, so `status()` agrees with the error.
- The device asks for `texture-adapter-specific-format-features` where the adapter has it
  (natively; browsers do not), so format caps carry the adapter's own sample counts, storage,
  filter and blend bits instead of WebGPU's guaranteed set. Copy usages still follow WebGPU's
  rules (wgpu's native table would offer copies of `depth24plus`, which WebGPU refuses).
- The binding-set table is pruned like the bind-group table: at each submit, once it has doubled
  since the last pruning, entries no set holds are dropped.
- A vertex format WebGPU lacks (three-component 8 and 16-bit, `Float16x3`, `Uint10_10_10_2`,
  `Float11_11_10`) now marks the device lost with "not built yet" instead of `Invalid` (a front
  bug): the interface and plan 5.7 give the conversion to the back end. Listed below.
- The shared test crate now brings proptest (getrandom, which needs a feature to build for
  WebAssembly), so it is a dev-dependency only off WebAssembly and tests/suite.rs is native only.
- `web/check` also draws the quads on `depth24plus-stencil8` (picture `quads-d24s8`) with wgpu on
  Direct3D 12 and Vulkan and in Chrome, compares it with Direct3D 11's quads at tolerance 0, and
  requires its twisted versions to differ.
- New tests: bindings.rs `combined_depth_stencil_views_draw_and_read_each_aspect` (both combined
  formats: cleared to depth 0.25 and stencil 7 through one view, then that view and a stencil
  view read by a program, [64, 7, 0, 255]); scenes.rs `quads_match_direct3d_on_every_depth_format`
  (`depth24plus`, `depth24plus-stencil8`, `depth32float`, `depth32float-stencil8`, stencil cleared
  where present, each equal to Direct3D's picture at tolerance 0).

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
- Views of `depth24plus-stencil8` and `depth32float-stencil8` work both as depth attachments and
  as bindings; `stencil: true` reads the stencil as unsigned integers (`texture_2d<u32>`), the
  depth as `texture_2d<f32>` or `texture_depth_2d`.
- Binding numbers follow 6.4: uniform buffers from 0, samplers from 16, textures from 32, storage
  from 160; group = binding set (vertex-only set 0, fragment-only set 1, both 2, helpers 3).
- Natively `wait_read` and `wait_fence` block on the device. In the browser they suspend through
  JSPI, so the export that leads to them (and to `WebGpu::new`, which waits for the depth probe)
  must be marked `#[wasm_bindgen(jspi)]`, as `draw_scene` and `suite_test` in examples/scenes are;
  `wcontainers_gpu_webgpu::block_on` runs a future (the adapter and device requests) the same way.
- Uniform-only buffers made with `MapIntent::Often` live in the uniform arena: give constant
  buffers rewritten per draw that intent and write them with `Discard`.
- `WebGpu::counters()` says how many pipelines, bind groups, layouts, renames and staged writes
  were made; `slow_paths()` the slow paths taken.
- Anything the back end does not carry out yet (list below) marks the device lost with
  `LossReason::Error` and prints `webgpu: <reason>` on standard error, so `status()` shows it.

## How to run

All from the checkout root on the Mac; nothing is built or run on the Mac but formatting and
`cargo check -j 2`.
- Tests on the RTX 3080 (tests/bindings.rs, blocks.rs, copies.rs, depth.rs, direct3d.rs (Windows
  only), fitted.rs, prepare.rs, raster.rs, scenes.rs, writes.rs and suite.rs, the shared suite; they need a GPU, and the suite's present test a desktop, so
  `--desktop`; `WEBGPU_API=dx12|vulkan|metal` picks wgpu's API, default the platform's, Vulkan on
  the desktop), both APIs in one job:
  `tools/graphics-rig/gpu-job --desktop --build "cargo test --locked -j 3 -p wcontainers-gpu-webgpu
  --no-run" -- "set WEBGPU_API=vulkan&& cargo test --locked -j 3 -p wcontainers-gpu-webgpu
  --no-fail-fast -- --test-threads 1 & set WEBGPU_API=dx12&& cargo test --locked -j 3 -p
  wcontainers-gpu-webgpu --no-fail-fast -- --test-threads 1"`. Keep `--no-fail-fast`, so one
  failing file does not hide the others, and `--locked`, so a stale lock file fails instead of
  being rewritten (which makes the tree differ and gpu-job refuse it).
- When a result is recorded, start the build command with `cargo clean -p wcontainers-gpu-webgpu &&`
  (incremental builds on the desktop once kept a stale library), and before `web/check` run
  `tools/graphics-rig/remote "cargo clean -p wcontainers-gpu-webgpu & cargo clean -p
  wcontainers-gpu-webgpu --target wasm32-unknown-unknown"`.
- The BC comparison with Direct3D and WARP with other blocks: `set WEBGPU_SEED=<n>&& set
  WEBGPU_CASES=<textures per format>&& cargo test --locked -j 3 -p wcontainers-gpu-webgpu --test
  direct3d bc_ -- --nocapture` as the job's command (prints each format's distances).
- The prepare timings: `tools/graphics-rig/gpu-job --alone --build "cargo test --locked -j 3 -p
  wcontainers-gpu-webgpu --no-run" -- "set WEBGPU_API=dx12&& cargo test --locked -j 3 -p
  wcontainers-gpu-webgpu --test prepare -- --nocapture"` (`WEBGPU_PROGRAMS=<n>` programs per half,
  default 16).
- The suite's present test on this Mac: `--test present` in the command below (the window shows
  for a moment in the middle of the main screen).
- On this Mac's GPU, one job only (never a build loop): copy the committed tree to scratch with
  `git archive HEAD | tar -x -C <scratch>/mac-src`, and there `nix develop <checkout> -c env
  CARGO_TARGET_DIR=<scratch>/mac-target WEBGPU_API=metal cargo test --locked -j 2 -p
  wcontainers-gpu-webgpu --no-fail-fast -- --test-threads 1`, detached with its own log.
- The random byte test with other seeds and lengths: `set WEBGPU_SEED=<n>&& set WEBGPU_CASES=<n>&&
  cargo test -j 3 -p wcontainers-gpu-webgpu --test writes byte_granular -- --nocapture` as the job's
  command (it prints the seed, the count, and how many no-overwrite writes it made).
- Lint: `tools/graphics-rig/remote cargo clippy --locked -j 3 -p wcontainers-gpu-webgpu
  --all-targets -- -D warnings`, and the same with `--target wasm32-unknown-unknown --examples` for
  the WebAssembly build.
- Both ways against Direct3D, one command: `crates/wcontainers-gpu-webgpu/web/check [dir]`. One
  GPU job builds the scenes example natively and for WebAssembly, gt-compare and the rig's page
  driver packages, then (web/check.cmd) runs wasm-bindgen, draws the three scenes and the quads on
  `depth24plus-stencil8` with wgpu on Direct3D 12 and on Vulkan, draws them in Chrome
  (web/draw.mjs, web/scenes.html), compares all twelve with references/d3d11 at tolerance 0, and
  draws both quads pictures with each twist natively on both APIs and in Chrome and requires them
  to differ, draws the eight-viewport scene in Chrome (equal to `raster-viewports.png`, and
  differing with whole-pixel viewports), prints first-draw timings with and without `prepare` in
  Chrome (failing if the prepared draws built anything), then runs the shared suite in Chrome, one
  page per test (suite-chrome.txt). Prints
  Chrome's WebGPU adapter and fails unless it is a GPU, not a fallback. With a directory, the
  pictures and the suite's results are copied there. About 7 minutes cold, the job itself 15 s.
- The old one-step BC comparison (ignored): add `--ignored` to the test command (`--test blocks --
  --ignored --nocapture`); it prints each format's largest difference and the texel.
- One renderer by hand, on the desktop: `scenes <dx12|vulkan|metal> <dir> [none|blend-factor|
  depth-function]` (the example's binary).

## Measurements

Run 7 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs, CPU
load not sampled; Mac load average 18 to 64):
- Last full run, after a clean build with the final usage rule (binding and render usage for depth
  and stencil only with copy usages): Vulkan 64 of 64 and Direct3D 12 64 of 64, one ignored on
  each; clippy with warnings denied passes natively.
- All tests after a clean build of the crate, at af2d885's code: Vulkan 64 of 64 and Direct3D 12
  64 of 64, one ignored on each (the old BC one-step comparison): bindings 10, blocks 2, copies 4,
  depth 2, direct3d 4, fitted 1, prepare 1, raster 1, scenes 4, suite 24, writes 11. Every scene
  picture equal to Direct3D's in every pixel on both APIs.
- tests/direct3d.rs BC comparison, seed 188 one texture per format (Direct3D 12) and seed 7 four
  per format (Vulkan), largest channel differences over the seed-7 run: WARP from the GPU / the
  decoded from the GPU / the decoded from WARP: BC1 0.0196 / 0.0196 / 0.0078; BC1 sRGB 0.0269 /
  0.0273 / 0.0092; BC2 0.0235 / 0.0196 / 0.0078; BC2 sRGB 0.0297 / 0.0293 / 0.0095; BC3 0.0235 /
  0.0235 / 0.0078; BC3 sRGB 0.0323 / 0.0273 / 0.0095; BC4 0.0235 / 0.0236 / 0.00025; BC4 signed
  0.0504 / 0.0504 / 0.00026; BC5 0.0247 / 0.0249 / 0.00025; BC5 signed 0.0508 / 0.0508 / 0.00026;
  BC6H both 0 / 0 / 0; BC7 0 / 0 / 0; BC7 sRGB 0.0037 / 0 / 0.0037 (WARP's sRGB conversion
  differs from the GPU's and the back end's). Natively 0 texels differ for every format.
- `web/check` (clean build): all twelve pictures equal Direct3D 11's at tolerance 0, every twist
  differs as before, the viewports as before, the suite in Chrome 24 of 24, Chrome's adapter the
  RTX 3080 (D3D12, discrete, not a fallback). `prepare` in Chrome 157 (16 programs per half,
  `performance.now()` steps of 0.1 ms): unprepared, the frame of 16 first draws (draws, submit,
  wait for the GPU) took 473.7 ms, each draw call under 0.1 ms; after `prepare`, the 16 `prepare`
  calls took 0.6 ms, waiting for a submit after them 441.3 ms (the compiles), and the frame
  4.4 ms with no miss.
- This Mac (Apple M5 Max, wgpu on Metal), two single jobs from the committed tree (af2d885, then
  65e7755 for copies and the suite; Mac load average 18 to 37; build 54 s at `-j 2`): bindings 10
  of 10, blocks 2 of 2, copies 4 of 4 (after the one-step allowance for 24-bit depth stored as
  float: the probe answers `Float(0.10000001)` here, and the read of one texel was one step off a
  near tie), depth 2 of 2, fitted 1, prepare 1, raster 1, scenes 4 of 4, writes 11 of 11, suite
  23 of 24 (present, the known failure below). Differences from the RTX 3080's Direct3D
  references, all within the rig's allowance: triangle 1952 pixels by 1, quads 2455 pixels by 1 (on
  every depth format), instanced 0, the eight viewports 662 pixels by 1. `prepare` there: 16
  unprepared first draws median 0.12 ms, longest 0.42 ms, frame 4.1 ms; after 2.9 ms of prepares,
  median 0.3 us, frame 0.89 ms.
- `prepare` natively, alone in the GPU queue: Direct3D 12, unprepared draw median 9.53 ms, longest
  13.56 ms, frame 158.5 ms; after 150.4 ms of prepares, draw median 0.9 us, longest 4.4 us, frame
  1.15 ms. Vulkan: unprepared median 0.43 ms, longest 0.83 ms, frame 7.9 ms; after 6.7 ms of
  prepares, median 0.5 us, frame 0.35 ms. Both far below run 5's (Vulkan median 6.5 ms then): the
  programs are the same each run, so the driver's own shader cache likely holds them; expected,
  not measured.

Run 6 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs, CPU
load not sampled; Mac load average 30 to 48):
- Before the BC work, all tests at e49d5f6: Vulkan 60 of 60 (bindings 10 in 1.71 s, copies 3, depth
  2, direct3d 3 in 1.29 s, fitted 1, prepare 1, raster 1, scenes 4, suite 24 in 3.92 s, writes 11
  in 1.53 s); Direct3D 12 60 of 60 (bindings 5.09 s, direct3d 2.61 s, suite 11.34 s, writes
  4.97 s). `web/check`: all twelve pictures equal Direct3D 11's at tolerance 0, the twists differ
  as before (16384 pixels by up to 92; 25016 by up to 204), the suite in Chrome 157.0.8103.78 24
  of 24; Chrome's adapter "NVIDIA GeForce RTX 3080", D3D12, discrete, not a fallback. The pictures
  in Chrome are now read back through JSPI waiting instead of polling.
- `web/check` with the viewports: Chrome's eight-viewport picture equals Direct3D 11's in all 32768
  pixels; moved to whole pixels, 2540 differ by up to 229. tests/raster.rs passes on both APIs
  through the shared scene.
- After the BC work, all tests: Vulkan 62 of 62 and Direct3D 12 62 of 62, one ignored on each (the
  known BC failure); blocks 2 in 0.53 s and 1.32 s; `web/check` passes again (suite in Chrome 24
  of 24, pictures equal, twists differ); clippy with warnings denied passes natively and for
  WebAssembly.
- tests/direct3d.rs, the same on both APIs: `generate_mips` of `r32float` filtered in the program
  (no `float32-filterable`) differs from GenerateMips by at most 7.8e-6, 1.5e-6, 1.2e-7, 0, 0, 0
  relative per level 1 to 6; filtered by the sampler (with the feature) it is equal at every level.
  `rgba16unorm` values in `rgba16uint` (program) differ by at most one step at each level; as
  `rgba16unorm` with the sampler, equal. So the sampler's filter on the RTX 3080 is what
  Direct3D's GenerateMips uses, and the program's exact average differs from it by those amounts.
  The sRGB resolve: 0 of 16384 bytes differ; resolved as `rgba8unorm`, 1269 differ by up to 56.
- tests/blocks.rs, seed 188, the same on both APIs: every level of every BC format reads back as
  written, natively and decoded; BC6H (both) and BC7 (both) decoded sample exactly as the GPU's
  decoder does; BC1 to BC5 differ by up to: BC1 2/255 (7.8e-3), BC1 sRGB 1.4e-2 linear, BC2 3/255,
  BC2 sRGB 2.0e-2, BC3 5/255, BC3 sRGB 1.8e-2, BC4 unorm 1.8e-2, BC4 snorm 4.0e-2, BC5 unorm 2.1e-2,
  BC5 snorm 3.9e-2. The GPU's BC4 values are not on an 8-bit grid (0.43237963 = 28336 / 65535).

Run 5 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs; Mac
load average 24 to 51):
- Last full run, all tests at 72cb445 on Vulkan: 57 of 57 (bindings 10 in 2.01 s, copies 3 in
  0.49 s, depth 2 in 0.26 s, fitted 1, prepare 1, raster 1, scenes 4 in 0.69 s, suite 24 in 3.48 s,
  writes 11 in 1.55 s); on Direct3D 12: 57 of 57 (5.12 s, 1.28 s, 0.83 s, scenes 1.74 s, suite
  10.33 s, writes 5.43 s).
- This Mac (Apple M5 Max, wgpu on Metal; one job at 6471d88's code, Mac load average 42 to 50, build
  1 min 54 s at `-j 2`): bindings 10 of 10, copies 3 of 3, depth 2 of 2 (the probe answers
  `Float(0.10000001)`: `depth24plus` is 32-bit float there, `DEPTH24_UNORM` clear, the bias
  picture not drawn), prepare 1 of 1 (16 first draws unprepared median 27.6 ms, longest 80.4 ms;
  after 427 ms of prepares median 0.54 µs, longest 6.5 µs), suite 23 of 24 (present: the host has
  no macOS window), scenes 2 of 4 and writes 9 of 11: `scenes_match_direct3d` (triangle and quads),
  `quads_match_direct3d_on_every_depth_format` (2455 pixels on `depth24plus`),
  `ordered_write_in_a_pass_splits_it` and `writes_ahead_of_a_pass_do_not_split_it` compare with
  the RTX 3080's references at tolerance 0 and differ; how far was not printed. fitted.rs and
  raster.rs were not yet written then.
- `web/check` (run twice, last at a561a61): all twelve pictures equal Direct3D 11's at tolerance
  0; every twist differs natively
  on Direct3D 12 and on Vulkan and in Chrome (blend factor 16384 pixels by up to 92, depth function
  25016 by up to 204, on `quads` and `quads-d24s8` alike); Chrome's WebGPU adapter "NVIDIA GeForce
  RTX 3080", D3D12, discrete GPU, not a fallback.
- The depth probe: Direct3D 12 `Unorm(0.10000009)`, Vulkan `Other(0.10000015)`.
- `prepare` (job alone in the GPU queue, CPU load not sampled; 16 programs per half, each a
  fragment loop of 24 rounds, distinct per program): first draws without prepare, Vulkan median
  6.52 ms, longest 13.6 ms, 115 ms for 16; Direct3D 12 median 26.9 ms, longest 39.3 ms, 460 ms for
  16. After prepare (Vulkan 107 ms, Direct3D 12 410 ms for the 16 prepares): every first draw
  median 0.8 to 1 µs, longest 4.3 µs, no miss and no pipeline built while drawing. Not measured in
  Chrome yet.
- Clippy with warnings denied passes, natively and for WebAssembly.

Run 4 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs,
CPU load 4 % sampled right after; Mac load average 27 to 50):
- All tests on Vulkan: 41 of 41 (bindings 5 in 0.58 s, scenes 4 in 0.57 s, suite 21 in 2.31 s,
  present among them, writes 11 in 1.08 s). On Direct3D 12: 40 of 41, the known
  `clears_fill_exactly` failure (bindings 2.08 s, scenes 1.80 s, suite 8.90 s, writes 4.41 s).
  The shared `stencil_operations_and_tests`, which lost the device before, passes on both.
- Before the fix of `shown`, reading the screen gave black on the rig's hidden desktop, and
  `present_shows_the_frame` failed with all texels [0, 0, 0, 255]; with `PrintWindow` it passes on
  both APIs.
- `web/check`: all twelve pictures (three scenes and `quads-d24s8`, each by wgpu on Direct3D 12,
  on Vulkan, and in Chrome 157.0.8103.78) equal Direct3D 11's at tolerance 0; the twists differ as
  before (blend factor 16384 pixels by up to 92, depth function 25016 by up to 204), on
  `quads-d24s8` too; Chrome's WebGPU adapter is "NVIDIA GeForce RTX 3080", D3D12, discrete GPU,
  not a fallback. Clippy with warnings denied passes; the WebAssembly build checks.
- The random byte test was not run again: nothing it exercises changed.

Run 3 (desktop: Windows 10, i9-9900K, RTX 3080, NVIDIA 621.01, shared with other tracks' jobs; Mac
load average 26 to 30):
- All tests on Vulkan: 30 of 30 (bindings 4 in 0.44 s, scenes 3 in 0.33 s, suite 12 in 1.12 s,
  writes 11 in 0.97 s). On Direct3D 12: 29 of 30, the suite's `clears_fill_exactly` failing loudly
  as before ("webgpu: an integer clear past 2^24, which wgpu's Direct3D 12 rounds through a float,
  is not built yet"; the suite then sees its read back never ready). Run with `--no-fail-fast`, so
  writes.rs ran on both.
- The random byte test, every run passing: on Vulkan seeds 1 to 20 at 3000 operations each and
  seed 987654321 at 6000; on Direct3D 12 seeds 1, 2, 3, 4 and 9 at 3000 and 987654321 at 6000
  (27 runs, 69000 operations). Of each seed's 3000, about 200 to 260 were no-overwrite writes and
  about 80 to 120 were turned into ordered writes to keep the promise (987654321: 457 and 188).
  Seeds 9 and 987654321 are those the independent check found failing.
- `web/check` passes again: all nine pictures equal Direct3D 11's at tolerance 0, both twists
  differ (blend factor 16384 pixels by up to 92, depth function 25016 by up to 204, natively and in
  Chrome), Chrome 157.0.8103.78's adapter is "NVIDIA GeForce RTX 3080", D3D12, discrete GPU, not a
  fallback. Clippy with warnings denied passes.

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
- Run 2 (desktop CPU load 27 % sampled right after, Mac load average 24 to 26): on Vulkan all
  20 tests pass (present among them without checking texels) (bindings 3 in 0.39 s, scenes 3 in 0.39 s, suite 7 in 0.96 s, writes 7 in 0.75 s);
  on Direct3D 12, 19 of 20 (suite 6 of 7, the known failure above; writes.rs ran in a separate
  command, since without `--no-fail-fast` the failure stops cargo first). The random byte test ran
  its default 400 operations, seed 0x5eed, on both; most other seeds failed then (run 3). Constants rewritten per draw: 20 draws made 1 bind
  group, 0 renames, 19 slice moves. `web/check` passes again after these changes: all nine pictures
  equal Direct3D 11's, both twists fail natively and in Chrome with the same counts as run 1, and
  Chrome's adapter is the RTX 3080 (D3D12, discrete, not a fallback); job 6 s. The arena, edge
  program and suite have not been run in Chrome yet (the browser page draws only the scenes).
  Clippy with warnings denied passes.
- Run 1, last full run: 5 tests pass on Vulkan (scenes 0.95 s, writes 0.76 s) and on Direct3D 12 (1.10 s,
  0.69 s); `web/check` passes (job 4 s); clippy with warnings denied passes. First build of the
  crate's tests 2 min 44 s at `-j 3`.

## Not carried out yet (each marks the device lost with its reason, or is declared absent)

- 3D BC textures natively need `texture-compression-bc-sliced-3d`, which the device asks for where
  the adapter has it; decoded ones need nothing.

- Multisampled depth or stencil copies that WebGPU cannot make (between formats); part of a
  16-bit depth level written from a buffer.
- Vertex formats WebGPU lacks (`VertexFormatConverted`): three-component 8 and 16-bit formats,
  `Float16x3`, `Uint10_10_10_2`, `Float11_11_10`. `create_vertex_layout` marks the device lost.
  The plan's route: widen the streams that use them into a shadow buffer by a compute pass at draw
  time, cached until the buffer is written.
- A surface wgpu reports lost (the window's surface must be made again from the instance, which
  the back end does not hold).
- Immediates: `immediate_bytes` is 0, so fronts spill them.
- An ordered write that splits a pass with a running occlusion query.
- Formats WebGPU lacks other than BC are declared without bits (no `FormatConverted` or
  packed-pair `BlockDecoded` yet), though 5.7 has the back end convert them.
- Declared absent until built or measured: `TIMESTAMPS`.
- On macOS the suite's `present_shows_the_frame` (tests/present.rs) fails as a known failure:
  macOS reports the test's window as occluded in the session the tests run from (Run 8), so no
  frame is shown. `PresentConverted` is built but has not drawn into a shown window yet.
- Not yet as 4.6 says for speed: a staging ring (each staged write makes a buffer), renaming
  textures on `Discard` (staged instead), and freeing cached layouts and pipelines of destroyed
  programs.
- Arena slices of buffers dropped without `destroy` are not reused until the arena is replaced.

## Next (G2)

Every pass bar item has been run; what is left:
- Present on this Mac (known failure above): run `--test present` while the Mac's display shows
  the session's windows (ask the owner when), which also runs `PresentConverted` for the first
  time.
- The rest of "Not carried out yet", vertex formats WebGPU lacks first. The plan's route (a compute
  pass widening the streams into a shadow buffer, cached until the buffer is written) has to run
  before the pass that draws, so a draw needing it inside an open pass splits the pass as an
  ordered write does; the cache needs a written-epoch on each buffer that every write, copy,
  clear and storage binding moves.

## Departures from the plan

- The shared suite's present test is not checked on macOS (see "Not carried out yet"); the suite's
  other 23 tests pass there. On macOS present runs in its own test file with a harness of its own
  (main thread), and tests/suite.rs there runs the list of examples/scenes/names.rs.
- The plan asks BC textures decoded to be compared with Direct3D "within the stated tolerance".
  The tolerance stated is WARP's distance from the GPU plus one step of the decoded storage
  format, since no fixed one-step tolerance is met by Direct3D's own WARP on these blocks.
- Scene pictures in this crate's tests are held to the rig's per-picture allowance (what WARP
  earns) so they hold on any GPU; the RTX 3080's exact equality is printed by them and asserted by
  web/check at tolerance 0.
- wgpu is vendored (third_party/wgpu, 30.0.2 plus one fix in src/backend/webgpu.rs) so its browser
  back end passes a render pass's occlusion query set to WebGPU; without it every occlusion query
  in the browser failed. This touches the workspace manifest, outside this crate.
- The `generate_mips` comparisons with Direct3D make each level from Direct3D's level above it, so
  one level's difference is not carried into the next and each tolerance is per step.

- `prepare` calls `createRenderPipeline`, not `createRenderPipelineAsync`, and keeps no cap on
  compiles in flight: wgpu 30 has no asynchronous pipeline creation. In Chrome the pipeline still
  compiles in the GPU process while the page goes on, and a draw waits only if it is not ready
  (expected, not measured in Chrome yet). Natively the compile happens at the `prepare` call, so
  `BACKGROUND_PREPARE` is declared only in the browser.
- The 24-bit depth probe accepts a third answer instead of calling it a back end bug: wgpu's
  Vulkan on the RTX 3080 moves depth by two 24-bit steps per unit of bias. The bit stays clear and
  fronts take `DepthBiasInShader`, which is right for any storage.
- `texture-formats-tier1` cannot be asked for through wgpu 30 (it has no such feature), so in the
  browser the tier 1 formats are declared without their tier 1 bits even where Chrome has them
  (fronts take `NormAsInteger`). Natively the same formats come from wgpu's 16-bit-norm and
  adapter-specific format features, and the test makes a device without those.
- Reading 24-bit depth back in a program is exact only to one 24-bit step on the RTX 3080 (a clear
  to 0.5, stored as 2^23, reads as 0.50000006), so the depth copy tests allow one step for 24-bit
  depth and none for 32-bit float depth.

- Every attachment is stored at the end of a pass, also when the interface says `Discard`: an
  ordered write may split the pass, and its first part must keep what it drew. Correct, but it
  gives up the bandwidth saving of a discard on tiled GPUs; recording a pass's draws before opening
  it would allow both.
- `DontCare` loads of colour attachments clear to zero (WebGPU has no don't-care load; a clear is
  its cheapest load); of depth and stencil they load, which is as cheap there and keeps the bytes.
- Depth writes happen only with the depth test on, as in Direct3D, OpenGL and Vulkan.
- Blend factors of min and max operations are set to one (WebGPU requires it; every API ignores
  them there). Blending is left off on integer targets, which OpenGL and Direct3D ignore and Vulkan
  and WebGPU refuse; into any other target without `BLEND` the draw fails loudly.
- The uniform "ring" of 4.5 is an arena of slices reused by size, not a ring: a buffer's latest
  bytes stay in its slice until its next `Discard`, so slices cannot be overwritten in ring order.
  It is still one large GPU buffer, replaced by one twice as large when it fills. A slice given up
  is reused after the next submit, not after the GPU finishes, because queue writes land after all
  work already submitted.
- Copies between two arena buffers go through a temporary buffer (counted `CopyWithinBuffer`),
  since both live in one GPU buffer.
- Anisotropy is set to one unless all three filters are linear, as WebGPU requires and Direct3D
  does.
- A frame the window cannot take (occluded, timed out, outdated after configuring again) is drawn
  into a texture of its own and not shown, with no error: the interface has no word for an
  unshown frame, and DXGI answers it with a success code (`DXGI_STATUS_OCCLUDED`).

## Interface changes

- 8f7db1b `interface: named the slow path of a surface format the window lacks converted by a draw
  at present`: `SlowPath::PresentConverted`, and the plan's 4.9 sentence on surface formats.

- 3f5519a `interface: named the slow path of stencil written by a draw per bit`:
  `SlowPath::StencilWrittenByDraw`, and the plan's 4.2 sentences on depth and stencil copies WebGPU
  makes only whole (written by draws, read by `DepthReadByDraw`). `StencilCopiedByDraw` names a
  different job (an 8-bit texture filled from the stencil for sampling, WebGL 2's).
- b999c16 `interface: named the slow path of integer colour clears made by a draw`:
  `SlowPath::IntegerClearByDraw`, and the sentence in the plan's 5.7. wgpu's Direct3D 12 clears
  colour targets through f32, which cannot hold integers past 2^24.

Gaps found for the interface track (not changed here): no capability tells a front which vertex
formats a back end converts itself or that instance step rates other than one are refused
(`StepMode::Instance(n != 1)` is `Invalid` here, as the interface's doc has the program fetch
them, `InstanceRateInShader`).

- db9e0d8 `interface: made no-overwrite writes promise that no unfinished command or ordered write
  touches their bytes`: the doc of `WriteMode::NoOverwrite` and the plan's 4.6 entry. The promise
  was only about reads; a back end writing at once cannot order itself after a pending copy into
  the same bytes, and Direct3D 11 and OpenGL do not ask it to.
- 61a266e `interface: named the slow path of buffer clears to a non-zero pattern made by copies`:
  `SlowPath::PatternClearCopied`, and the plan's sentence on `clear_buffer` (4.2) saying WebGPU
  fills non-zero patterns by copies.

## Licences

Confirmed in each crate's Cargo.toml on 2026-11-17: wgpu 30.0.2 (vendored, LICENSE.MIT and
LICENSE.APACHE kept) and wgpu-naga-bridge 30.0.2 MIT OR Apache-2.0 (dependencies); bcdec_rs 0.2.1
MIT (its Cargo.toml; plan section 3 records its repository's licence); half 2.7.1 MIT OR Apache-2.0;
in the browser build js-sys 0.3.107 and wasm-bindgen 0.2.130 MIT OR Apache-2.0. Test-only, new in
run 6: web-sys 0.3.107 and getrandom 0.4.3 MIT OR Apache-2.0 (browser); windows 0.62.2 MIT OR
Apache-2.0 and wcontainers-graphics-test (ours) for tests/direct3d.rs; new in run 7, image 0.25.10
MIT OR Apache-2.0 (its Cargo.toml; already in the lock file through the rig) and the windows crate's
`Win32_Graphics_Direct3D_Fxc` feature, for tests/direct3d.rs. Test-only: windows-sys 0.61.2 MIT OR Apache-2.0 (the suite's window and reading the screen, already
in the lock file); wcontainers-gpu-test (ours); png 0.18.1, anyhow 1.0.104, wasm-bindgen 0.2.130,
js-sys 0.3.107 MIT OR Apache-2.0 (wasm-bindgen-futures is no longer used); pollster 1.0.1 Apache-2.0/MIT;
bytemuck 1.25.2 Zlib OR Apache-2.0 OR MIT; fastrand 2.5.0 Apache-2.0 OR MIT (the random test). Tool: wasm-bindgen-cli 0.2.130 MIT OR Apache-2.0.

New in run 8, test-only: libtest-mimic 0.8.2 MIT/Apache-2.0 (its Cargo.toml); on macOS only,
block2 0.6.2 MIT, objc2 0.6.5 MIT, objc2-foundation 0.3.2 MIT, and objc2-app-kit,
objc2-core-foundation, objc2-core-graphics, objc2-quartz-core and objc2-screen-capture-kit 0.3.2,
each Zlib OR Apache-2.0 OR MIT (their Cargo.toml files; most already in the lock file through wgpu).

## Installed on the desktop

- wasm-bindgen-cli 0.2.130 (`cargo install --locked`): wasm-bindgen.exe, wasm2es6js.exe and
  wasm-bindgen-test-runner.exe in C:\wc\cargo\bin, 18.9 MB; its 0.53 GB build directory was
  removed.
- The rig's page-driver packages in this branch's checkout
  (C:\wc\src\phase-2-graphics-webgpu\tools\graphics-rig\browser\node_modules), by the rig's
  install.ps1.
- Build output C:\wc\target\phase-2-graphics-webgpu, 3.28 GB; pictures in
  C:\wc\out\phase-2-graphics-webgpu-webgpu.
