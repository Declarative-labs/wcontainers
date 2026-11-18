# Graphics WebGL 2 back end status (G3, G3b)

Track: `crates/wcontainers-gpu-webgl`, the WebGL 2 back end on glow, branch `phase-2-graphics-webgl`.
Plan: docs/phase-2-graphics.md, sections 4, 5.8, 6.5, 10 and 12 (G3).

## Where things stand

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
- `noperspective` inputs (shaders track: not written for WebGL 2): refused, naga's GLSL writer
  answers `MissingFeatures(NOPERSPECTIVE_QUALIFIER)` when the program is made. Not built: linear
  interpolation needs the vertex stage to write the value times w and w beside it, which needs
  both stages at once (Next).
- `SAMPLE_MASK` and `SAMPLE_SHADING`: absent (WebGL has no OES_sample_variables directive for
  `gl_SampleMask`; see Capabilities).
- Fine derivatives: refused by naga's writer (`DERIVATIVE_CONTROL`), as the shaders track says.
- (Run 2) The shared line's three new suite tests (copies within one depth or stencil
  subresource, the occlusion counter, every vertex format, which the shared status says GL ES may
  lack): pass on ANGLE and in Chrome with every extension; on the routes without
  EXT_clip_control they stop at the suite's own raster block like the 25 known.
- (Run 2) The frame-time measurement through the Direct3D 11 front: built (Measurements, run G3b-2).

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
- The shared suite in Chrome's WebGL 2: 39 of 39 with every extension, and with stencil texturing,
  the format and the raster extensions off. Without EXT_clip_control: the 26 known failures (25
  natively) of the suite's own raster blocks.
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
    Not built today (every `unbuilt` call in src/lib.rs, as of run 18):
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
      vertex 2^31 - 1.
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
  of the process into `<name>.1`) while drawing on ANGLE; without `WEBGL_TRACE` it makes no device.
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
  Each back end replays the stream twice, on a new device each time: once with each call timed
  inside the back end, once timing whole frames only; frames before the tenth are left out. The round trip is tested natively:
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
Everything else is absent: `POLYGON_MODE_POINT` (WebGL draws polygon edges only), `OCCLUSION_COUNT`
and `MULTISAMPLE_READ` (WebGL 2 has neither), and `SAMPLE_SHADING`, which OES_sample_variables and
OES_shader_multisample_interpolation would give (both in Chrome 157 on the RTX 3080); not built:
naga's GLSL writer takes the `sample` qualifier only from GLSL ES 3.20
(third_party/naga/src/back/glsl/features.rs, `SAMPLE_QUALIFIER`), so the second extension needs a
change in naga first (the shaders track's); it already writes OES_sample_variables' directive. The adapter is WebGL's renderer string
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
EXT_color_buffer_float); not R11G11B10 or RGB9E5 from the GPU, depth or stencil (since G3b run 1
16-bit floats copy out too, through their integer twin, and the 10-10-10-2 formats in their own
packing, `UNSIGNED_INT_2_10_10_10_REV`, which GLES 3.0 reads back for them). Since run 3, samples:
1, 2, 4 and 8 for the colour formats that render and are not integers, and for depth formats (as
GL reports them on the RTX 3080); integer formats take one.
No bits yet: the swizzled formats other than BGRA8 (BGRX8, B5G5R5A1, BGRA4), A8, Stencil8. Since
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
  from the front's `CheckFormatSupport`, no back end message), geometry and streamout (no
  geometry stage at 10_0).
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

## Departures from the plan

- (G3b run 2) The frame-time measurement of a real program in Chrome runs the program natively
  (no WebAssembly build of the Direct3D 11 front exists) and replays the interface calls it made
  in Chrome. So it measures the back ends' share of a frame in Chrome, not the front's. The
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
  case of `gathers_equal_their_footprints` equal, ANGLE and Chrome on the RTX 3080); a sampler
  whose minimum level of detail is above 0 would move the samples off level 0 (not tested).

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

G3b is done apart from the independent check. In this order after it:
1. The WebGL 2 frame's time in Chrome (Measurements, run G3b-2): `submit`'s `getError` waits for
   Chrome's GPU process every frame. Measure it apart from the twin refresh (a replay with the
   check moved), then: find GL errors where the back end already waits (read backs, fence waits),
   with `isContextLost` at every submit for losses; and find which texture's twin SimpleLightingPC
   refreshes every second frame (likely an sRGB target view of its swap chain's buffer) and whether
   a draw or a blit can refresh it without the pixel buffer.
2. Record more real programs: one with many draws a frame (bgfx's examples drew nothing through
   the WebGL 2 build; Godot's compatibility renderer overflowed its stack: ask the Direct3D 11
   track), and agree with the WebGPU track that it replays the same files (src/trace.rs is generic
   over `Backend`; scratch g3b2/atg-simplelightingpc.bin is one).
3. `noperspective` inputs: the vertex stage writes the value times w and w beside it, the
   fragment stage divides; needs the pair of stages at link time (refused by name today).
4. Rerun the Direct3D 11 front's WebGL 2 scenes after its next runs (constants: the uniform block
   limit; formats; views at 10_0 through `ReinterpretCopy`).

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
4. `SAMPLE_SHADING` with OES_sample_variables and OES_shader_multisample_interpolation, where
   Chrome has them (absent today, and refused when a shader is made since run 14); needs naga to
   write the `sample` qualifier on GLSL ES 3.00 with the extension (see "Capabilities").
5. Copies between signed normalised textures drawn into (Chrome) go through the host, a wait;
   both render, so copyTexSubImage could copy them on the GPU.
6. The present-feature routes of `TIMESTAMPS` and `FRACTIONAL_DEPTH_BIAS` (`write_timestamp`,
   `timestamps_disjoint`, a fractional `polygonOffset`) have run on no GPU: the RTX 3080 gives
   zero timestamp bits and ANGLE rounds bias units. A GPU and browser that have them would test
   them.
