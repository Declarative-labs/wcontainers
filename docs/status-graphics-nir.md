# Status: graphics, NIR route (G8) and OpenGL natively (G9)

Branch agents, worktree wcontainers-agents (since 2026-11-21; the track branches are gone). Plan:
docs/phase-2-graphics.md, sections 15.2, 15.3, 15.4 and 15.8 (G8, G9).

## Where it stands

G9 run 31 (2026-11-23, round 4's ninth run), from run 30's gap list. Built, each linted with
warnings denied with and without Mesa and the nir crate's route test passed (25) on the x86-64
machine; groups checked on the desktop (RTX 3080, one GPU job, 8 glcts at once):
- Atomics of either signedness on one word (0986b2f4). Blocks of one layout share a NIR type
  whatever their words' signedness (Mesa gives `buffer { uint a[4]; }` and `buffer { int b[4]; }`
  one type): a block variable's signed words are now keyed by the variable, which gets a type of
  its own. Workgroup memory, which the bridge makes one array of words, can hold both: an atomic
  minimum or maximum of the other signedness than its word is a compare-exchange loop
  (`SlowPath::AtomicExchangeLoop`, which the interface already named; each try compares with the
  value the last gave; a fragment program's helper invocation leaves). The route's
  `Routed::slow_paths` go into the reflection, and the driver now counts a program's reflected
  slow paths at each use (`Built::slow_paths`, which it did not before for any). KHR-GLES31
  `compute_shader.atomic-case2` and `-case3` pass (group run a). Test
  `signed_and_unsigned_atomics_beside_each_other_keep_their_own_kinds`.
- A centroid input also read another way (661a9ad7): the host interpolates the slot that other
  way, and each centroid read is made from it and its slopes at the centroid's place: the centre
  where every sample is covered (or for one sample), otherwise the first covered sample of
  `gl_SampleMaskIn` (inside both pixel and primitive), a program run per sample its own sample.
  dEQP-GLES31 `multisample_interpolation.interpolate_at_offset.centroid_qualifier` 11 pass (group
  run b). Test `a_centroid_input_also_read_another_way_is_made_from_the_other_way`.
- Points that are not rasterized are not drawn as squares (458438c4).
- Fragment programs get their pixel extras from the route (1cbbae1d), every program forcing early
  tests goes through the rewrite (c4b8743c; the plain route was refused "Capability
  EARLY_DEPTH_TEST is not supported"), and the driver draws it where no test is made or its
  results do not depend on when the tests run, refusing the rest by name (gap list). Test
  `early_tests_a_fragment_program_forces_are_in_its_pixel_extras`.
- The screen reports as many passed values for vertex, geometry and evaluation programs (and
  fragment inputs, `max_varyings`) as the stage helper's pass-through vertex program takes beside
  its position and the squares' point coordinate (`screen::passed_values`, the host's vertex
  inputs less two: 28 on the RTX 3080's Direct3D 12, was 31); before, a geometry program passing
  all 31 lost the device (KHR `geometry_shader.limits.max_output_components`).
- Traced, not this track's: dEQP-GLES3 `fbo.color.tex3d` 2 device losses (WebGPU back end, asked).
  Traced, this track's, not built: `vertex_ordering`'s time (gap list). Read, not proven: the
  storage coherence family (gap list).
- Unsafe code: unchanged (wcontainers-nir 116, wcontainers-opengl 370). No dependency or licence
  was added.

Run 31's group runs (scratch `results/g9r31/{a,b}`, logs `logs/g9r31/{a,b}.log`): a, at 0986b2f4's
driver: dEQP-GLES3 `fbo.color`, `rasterization.fbo` 350 cases, 2 Crash (tex3d), 2 Fail
(`rbo_multisample_max` lines: 18 pixels filled past the line with 8 samples, 4 samples pass; not
yet traced); dEQP-GLES31 `early_fragment_tests` 6 Crash; KHR-GLES31 `compute_shader` 37 of 37. b,
at 458438c4's: dEQP-GLES31, KHR-GLES31, KHR-GLES32 groups `multisample_interpolation`,
`synchronization`, `geometry_shader.limits`, `vertex_ordering`, `image_load_store`,
`vertex_transform_feedback`, `point_mode`, `wide_points`: 11 Crash to Pass, the storage family's
fails and flakes moved among its cases, nothing else changed.

THE TABLE, run 31's whole run at c4b8743c's driver (every change above; the shaders crate as the
worktree held it, which built; desktop, RTX 3080, one GPU job in the hazard group, 8 glcts at once,
the job from 23:46 to 00:19 desktop time; scratch `results/g9r31/full/<list>/results.csv`, log
`logs/g9r31/full.log`), every list whole:
- dEQP-GLES2: 17,165 cases; Pass 17,009, Warn 6, Fail 22, Skip 128: 17,015 of 17,037, 99.87%.
- dEQP-GLES3: 44,768; Pass 44,236, Warn 5, Fail 26, Crash 31, Skip 470: 44,241 of 44,298, 99.87%.
- dEQP-GLES31: 37,802; Pass 36,563, Fail 26, Crash 96, Flake 5, Skip 1,112: 36,563 of 36,690,
  99.65%.
- KHR-GLES2: 473; Pass 438, Skip 35: 100%.
- KHR-GLES3: 6,498; Pass 6,264, Crash 6, Skip 228: 6,264 of 6,270, 99.90%.
- KHR-GLES31: 4,101; Pass 3,706, Fail 14, Crash 45, Timeout 1, Skip 335: 3,706 of 3,766, 98.41%.
- KHR-GLES32: 1,405; Pass 1,102, Fail 7, Crash 44, Timeout 1, Skip 251: 1,102 of 1,154, 95.49%.
- KHR-GL33: 9,886; Pass 9,084, Fail 12, Crash 268, Skip 522: 9,084 of 9,364, 97.01%.
- KHR-GL46 (19,714): no 4.6 context (G9's second part).
- The bar's measure, every list that starts: 118,413 of 119,017 run, 99.49%, above the 92.9%
  floor (run 30: 99.45%). With KHR-GLES32's 251 NotSupported counted as failed: 118,413 of
  119,268, 99.28%.
- Against run 30's whole run: 53 cases pass that did not: `sign` 30 (dEQP-GLES3
  `builtin_functions` 18, dEQP-GLES31 12: the shaders track's change, not this run's),
  `multisample_interpolation` 11, `early_fragment_tests` 2 (the forms whose tests match tests
  after the program), `compute_shader.atomic-case2` and `-case3`, `shader_storage_buffer_object
  .basic-atomic-case1-cs`, KHR `geometry_shader.limits.max_output_components` 2, and the storage
  coherence family's flakes 5. Passed in run 30 and not now, 6: KHR-GL33
  `transform_feedback.discard_geometry_test` (with `capture_geometry_interleaved_test` and
  `query_geometry_interleaved_test`, which crashed before and now fail the same way): "geometry
  shader uses too many output components (120 > 112)", the link refusing what the screen now
  honestly reports (gap list); the other 5 are the storage coherence family moving among its cases
  (`compute.basic.image_barrier_single`, `image_load_store` `coherent` 2,
  `synchronization.inter_invocation` 2).

G9 run 30 (2026-11-23, round 4's eighth run). Built this run, each linted with warnings denied
with and without Mesa and the nir crate's route test passed (22) on the x86-64 machine:
- Points of one pixel stay the back end's (51817d17): a vertex program whose every point size is
  a constant no larger than one (`Nir::point_size_at_most`) draws points natively, not as squares.
  dEQP-GLES3 `transform_feedback` writes `gl_PointSize = 1.0` for points; its two cases that
  failed in run 29's group run (`array.interleaved.points.highp_ivec4`,
  `array_element.separate.points.lowp_uvec2`, squares of one pixel overlapping in another order)
  pass.
- Wide points after tessellation (24fce708): an evaluation program in point mode
  (`Nir::makes_points`) gets the vertex program's treatment: its copy writes the size at a free
  location and the squares' geometry program follows it in the stage helper, its block in the
  geometry stage's slot 0.
- Wide points from a geometry program (24fce708, 1ae22774): a geometry program whose output is
  points is rewritten to make the squares itself (`wc_nir_geometry_squares`,
  `Nir::geometry_squares`): every output it stores is held in a local, and each `EmitVertex`
  becomes a square of four corners ended as its own strip (the corners' code shared with
  `wc_nir_point_square`), its size the point size it writes or the rasterizer's, its block a
  uniform block in words at a Gallium slot of its own. The screen now reports one uniform block
  fewer where the host has more than Gallium's 16 slots (`screen::uniform_blocks`), so that slot
  is free. Refused by name (not built): a geometry program emitting with counters, writing clip
  distances or to several streams, and squares after tessellation or a geometry program while
  streaming or counting primitives. No test of the route's own (its GLSL entry takes vertex,
  fragment and compute programs only); measured by the whole run below.
- The first build of the rewrite refused every geometry program making points (1,254 refusals,
  599 dEQP-GLES31 cases from Pass to Crash: the block's slot was checked before whether the points
  were wide, and the RTX 3080's back end has more than 16 uniform buffers a stage); that run was
  stopped and 1ae22774 fixed both. Its lists are kept in scratch `results/g9r30/broken`.
- `run-windows.ps1` removes earlier results through the long-path prefix (2483bada).
- Unsafe code: wcontainers-nir 116, wcontainers-opengl 370; the root list carries both. No
  dependency or licence was added.
- The suite of both crates at 0d2108df (x86-64 machine, load 63 to 72, the shaders crate as
  committed): fmt clean; clippy with warnings denied clean with and without Mesa; with Mesa the nir
  crate's route 22, glsl_reference 10 and 2 ignored, glsl_buffers 2 and narrow 1 pass;
  wcontainers-opengl has no tests of its own.

THE TABLE, run 30's whole run at 1ae22774's driver (every change above, the shaders crate as
committed at f8b65a56; desktop, RTX 3080, one GPU job in the hazard group, 8 glcts at once, about
21:42 to 22:20 Mac time; scratch `results/g9r30/full/<list>/results.csv`, log
`logs/g9r30/full-c.log`), every list whole:
- dEQP-GLES2: 17,165 cases; Pass 17,009, Warn 6, Fail 22, Skip 128: 17,015 of 17,037, 99.87%.
- dEQP-GLES3: 44,768; Pass 44,218, Warn 5, Fail 44, Crash 31, Skip 470: 44,223 of 44,298, 99.83%.
- dEQP-GLES31: 37,802; Pass 36,540, Fail 36, Crash 109, Flake 5, Skip 1,112: 36,540 of 36,690,
  99.59%.
- KHR-GLES2: 473; Pass 438, Skip 35: 100%.
- KHR-GLES3: 6,498; Pass 6,264, Crash 6, Skip 228: 6,264 of 6,270, 99.90%.
- KHR-GLES31: 4,101; Pass 3,701, Fail 15, Crash 49, Timeout 1, Skip 335: 3,701 of 3,766, 98.27%.
- KHR-GLES32: 1,405; Pass 1,100, Fail 8, Crash 45, Timeout 1, Skip 251: 1,100 of 1,154, 95.32%.
- KHR-GL33: 9,886; Pass 9,085, Fail 9, Crash 270, Skip 522: 9,085 of 9,364, 97.02%.
- KHR-GL46 (19,714): no 4.6 context (G9's second part).
- The bar's measure, every list that starts: 118,366 of 119,017 run, 99.45%, above the 92.9%
  floor (run 29: 99.43%). With KHR-GLES32's 251 NotSupported counted as failed: 118,366 of
  119,268, 99.24%.
- Against run 29's whole run: 108 cases pass that did not, among them NotSupported wide points
  40 (`primitive_bounding_box.wide_points`), 7 (`tessellation_geometry_interaction.point_size`),
  12 (`clipping.point`), KHR `geometry_shader` 8 and `rasterization.fbo` 4, run 29's fixes (pointcoord, `restart_mode`,
  `texture_lod_bias_all`, the patch-size crashes) and KHR `shader_integer_mix` 9 (Fail in run 29,
  not traced why they pass now). Failing now and not in run 29: KHR-GLES31 and -GLES32
  `geometry_shader.limits.max_output_components` (NotSupported before, now refused: no location
  is left for `gl_PointCoord`), KHR-GLES31 `tessellation_shader.vertex.vertex_ordering` (Fail to
  Timeout), KHR-GLES32 `texture_cube_map_array.image_op_compute_sh` (it flakes). The process
  logs' device losses: 566 (run 29: 562), the same groups (gap list).

G9 run 29 (2026-11-23, round 4's seventh run). Run 28's whole run never started (the GPU queue was
held by another track's job until it stopped); this run made it, then built:
- Wide points (50f63af3, 46233d47; `SlowPath::PointsAsQuads`, which the interface already named).
  Where the back end draws points of one size (WebGPU) and a draw's points are larger (the vertex
  program writes `gl_PointSize`, or the rasterizer's size is above one), each point is drawn as a
  square of two triangles: the bridge makes a geometry program after the vertex program
  (`wc_nir_point_square`, `Nir::point_square`) that the stage helper runs as for any geometry
  program, its corners at half the size in pixels each side (from uniform block 0: the bound size,
  half a pixel's clip-space length per viewport axis, `gl_PointCoord`'s direction), each corner
  carrying every output. The vertex program's `gl_PointSize`, which the stage helper does not pass
  between stages, is also written at the first free generic location (`Nir::point_size_also_at`),
  where the squares read it (clamped to 1..the largest size) and then pass `gl_PointCoord`. The
  screen now reports the largest 2D texture's side as the largest point where the back end has no
  point sizes (`screen::largest_point_size`). The squares are drawn filled, never culled, unbiased
  and as front faces (OpenGL's rules for points); their z and w are the point's, so the host clips
  at near and far as OpenGL clips points, and keeps the parts of a point whose centre is past the
  viewport's side, as dEQP's reference renderer and guard-band hardware draw them. A draw that
  streams is streamed by the vertex program first, then drawn as squares; counts of primitives
  count the API's points. Refused by name: a vertex program writing clip distances, or outputs at
  offsets known only when run (not built). Not built: wide points after a geometry program or
  tessellation (their outputs carry no size through the stage helper, asked of the shaders track).
  Test `points_are_drawn_as_squares_passing_their_coordinate_after_every_location` (the squares
  routed through `direct::geometry` and validated).
- `gl_PointCoord` (50f63af3): a fragment program's point coordinate (Mesa's `VARYING_SLOT_PNTC`
  input, or the system value) is routed at `direct::POINT_COORD_INPUT`, interpolated without
  perspective; the driver moves it to where the squares pass it, or replaces it with the centre's
  (0.5, 0.5), which a point of one pixel reads. Before, every program reading it was refused
  (dEQP-GLES2 and -GLES3 `builtin_variable.pointcoord` crashed).
- Run 28's level-of-detail bias (d30f8a32) was refused by the route's rewrite for vertex and
  fragment programs ("a variant rewrite reading the program's stage values": KHR-GL33
  `texture_lod_bias.texture_lod_bias_all` went from pass to crash in this run's whole run). The
  rewrite now takes `VariantKey::lod_bias`, which the shaders crate builds (6ca0adbb).
- Run 28's constants for `gl_PatchVerticesIn` became globals with initialisers, which the route
  refuses ("a global variable with an initialiser", the 3 KHR-GLES31 and 3 KHR-GLES32
  `tessellation_shader` cases still crash in the whole run below); global initialisers are now
  stored on entry too (6ca0adbb).
- Read, not changed: KHR-GLES31 `shader_image_load_store.basic-glsl-misc-fs` ("Uses operations
  forbidden at this stage") is `memoryBarrier()` in a fragment program: the route writes a
  memory barrier, and WGSL has barriers only in compute work. Absent on WebGPU (Direct3D has
  `DeviceMemoryBarrier` in every stage); no exact route, left refused.
- Unsafe code: wcontainers-nir 113, wcontainers-opengl 369 (the squares' entries and the bound
  programs they read); the root list carries both (120a9f3c). No dependency or licence was added.
- The first Windows build of the squares failed: `PIPE_SPRITE_COORD_UPPER_LEFT` is an enum constant,
  `i32` with MSVC and `u32` with Linux's compilers; compared as the bitfield's type in b8c44613. 46233d47
  and 6ca0adbb did not build on Windows (pushed before the fix was).

Run 29's group run (the fixed driver; desktop, RTX 3080, one GPU job, 8 glcts at once, about
20:15 to 20:25 Mac time; scratch `results/g9r29/b`, log `logs/g9r29/b.log`): every case of the
eight lists matching `point`, `rasterization`, `clipping`, `primitive_restart`,
`transform_feedback`, `primitive_bounding_box`, `multisample`, `lod_bias`, `tessellation_shader`,
`isnan` (7,501 cases; KHR-GLES2 has none and does not start). Against this run's whole run:
- Pass now: dEQP-GLES2 and -GLES3 `builtin_variable.pointcoord`, KHR-GL33
  `primitive_restart.restart_mode` and `texture_lod_bias_all`, KHR-GLES31 and -GLES32
  `tessellation_shader.single.max_patch_vertices` and `..._PatchVerticesIn` (6 crashes, with
  dEQP-GLES31's), dEQP-GLES31 6 crashes; NotSupported cases that run and pass: dEQP-GLES2 7,
  dEQP-GLES3 11 (`clipping.point.wide_point_*` among them), dEQP-GLES31 7.
- NotSupported cases that now run and fail, 47, all wide points after a geometry program or
  tessellation, which are still drawn as one pixel (the stage helper carries no point size;
  this track's, needing the shaders track's ask): dEQP-GLES31 `primitive_bounding_box.wide_points`
  through tessellation or geometry 34, `tessellation_geometry_interaction.point_size` 6,
  `geometry_shading.basic` 1; KHR-GLES31 and -GLES32 `geometry_shader.input`, `.limits`,
  `tessellation_shader_point_mode` 3 each (KHR-GLES31 `tessellation_shader.vertex` 1 more). The
  screen reports one range of point sizes for every stage, as OpenGL has one; reporting one pixel
  again would take wide points from every vertex program.
- Passed in the whole run and fail now: dEQP-GLES3 `transform_feedback.array.interleaved.points
  .highp_ivec4` and `array_element.separate.points.lowp_uvec2` (2 of the 329 points cases; no
  refusal): the picture drawn while streaming differs from the one drawn without (largest
  difference 223, 191, 128). Both draws are squares; not yet traced (overlapping squares of
  different colours drawn in another order would give it).

THE TABLE, run 29's whole run at 8815575c's driver (run 28's changes; desktop, RTX 3080, one GPU
job that built and ran, 8 glcts at once, about 19:30 to 19:41 Mac time; scratch
`results/g9r29/full/<list>/results.csv`, log `logs/g9r29/full.log`), every list whole:
- dEQP-GLES2: 17,165 cases; Pass 17,001, Warn 6, Fail 22, Crash 1, Skip 135: 17,007 of 17,030,
  99.86%.
- dEQP-GLES3: 44,768; Pass 44,206, Warn 5, Fail 44, Crash 32, Skip 481: 44,211 of 44,287, 99.83%.
- dEQP-GLES31: 37,802; Pass 36,485, Fail 36, Crash 116, Flake 5, Skip 1,160: 36,485 of 36,642,
  99.57%.
- KHR-GLES2: 473; Pass 438, Skip 35: 100%.
- KHR-GLES3: 6,498; Pass 6,261, Fail 3, Crash 6, Skip 228: 6,261 of 6,270, 99.86%.
- KHR-GLES31: 4,101; Pass 3,689, Fail 20, Crash 50, Flake 1, Skip 341: 3,689 of 3,760, 98.11%.
- KHR-GLES32: 1,405; Pass 1,090, Fail 11, Crash 46, Timeout 1, Skip 257: 1,090 of 1,148, 94.95%.
- KHR-GL33: 9,886; Pass 9,080, Fail 13, Crash 271, Skip 522: 9,080 of 9,364, 96.97%.
- KHR-GL46 (19,714): no 4.6 context (G9's second part).
- The bar's measure, every list that starts: 118,261 of 118,939 run, 99.43%, above the 92.9%
  floor (run 27: 99.36%). With KHR-GLES32's 257 NotSupported counted as failed: 118,261 of
  119,196, 99.22%.
- Against run 27's whole run: KHR-GLES3 `copy_tex_image_conversions.required` 63 crashes pass,
  dEQP-GLES3 13 fails and 1 crash pass (`isnan`, `max_texture_lod_bias`, front facing), dEQP-GLES31
  6 fails pass. Passed before and not now: KHR-GL33 `texture_lod_bias_all` (crash, cause and fix
  above), dEQP-GLES31 `image_load_store.3d.qualifiers.volatile_r32f` (fail) and
  `2d_array.qualifiers.coherent_r32ui`, KHR-GLES31 `texture_cube_map_array.image_op_compute_sh`
  (flake), all of the intermittent storage coherence family.

G9 runs 23 to 28 (their whole runs, readings and lists) are told in full in `git show
fcbbdf19:docs/status-graphics-nir.md`, as run 29 left this file.

G9's FIRST PART IS COMPLETE for its independent check, stated in run 27 and unchanged since: every
list that starts runs whole at one driver, the bar's measure is above the 92.9% floor, and every
failing group is in the gap list below with its cause and owner (those not yet traced named as
such). No independent check has run yet. The table to check against is the newest whole run at
the top of this file (run 31's). The check asked for: a sample of each list on the rig against that table,
the NotSupported groups case by case, and the RTX 3080's own OpenGL (`run-windows.ps1 -Driver
system`) on the failing groups. G9's second part (OpenGL 4.0, so that KHR-GL46 starts) follows that
check.

Next, in order (run 31):
1. The independent check of G9's first part against run 31's table (not this track's to run).
2. This track's, built nowhere yet: forced early tests before storage writes in the program
   (`VariantFlags::EARLY_TESTS`, 4 dEQP and 1 KHR case); programs of equal content sharing their
   conversions and the stage helper's pipelines (KHR `vertex_ordering`'s 2 timeouts); wide points
   from a geometry program emitting with counters, writing clip distances or to several streams,
   and squares while streaming or counting primitives (each refused by name).
3. Still not traced: the storage coherence family (read in run 31, not proven), `rasterization
   .fbo.rbo_multisample_max` lines (read in run 31, not proven), the run-time gather offsets at
   texture edges, `geometry_shading.vertex_transform_feedback` 2, and the single cases of
   `logs/g9r30/groups.txt`.
4. Waiting on other tracks: the 2D view of a 3D packed target and the 96-bit copies (WebGPU back
   end); the pass-through's vertex inputs, the staged point size, storage in evaluation programs,
   the multisampled array layer bindings and the stage helper's buffer sizing (shaders track);
   the stencil blit's slow-path name (interface track).
5. G9's second part (OpenGL 4.0, so that KHR-GL46 starts), after the check.

## Gap list (G9's first part, at run 30's whole run, with run 31's changes marked)

Every failing case of run 30's whole run (651 Fail, Crash, Timeout and Flake over the eight lists)
by group, with the driver's note printed while it ran (scratch `logs/g9r30/reasons.txt` has every
case and its note, `logs/g9r30/groups.txt` the counts below) and its cause and owner. "This track"
is work left in wcontainers-nir or wcontainers-opengl; "not yet traced" means no cause is named
yet, which is this track's work. The earlier gap list, with its readings group by group, is in
`git show fcbbdf19:docs/status-graphics-nir.md`.
- KHR-GL33 `packed_pixels` 246 device losses: BC textures whose first level is not whole blocks
  lose the device in wgpu (the WebGPU back end track; asked in G9 run 17).
- KHR-GLES31 and -GLES32 `tessellation_shader` 72: an evaluation program reading
  `gl_in[].gl_PointSize` (the shaders track: the stage helper has no staged point size; asked);
  `tc_barriers` 6: a control program reading outputs after `barrier()`, whose control points the
  stage helper runs one after another (the shaders track).
- dEQP-GLES31 `copy_image.non_compressed.viewclass_96_bits` 32: the 96-bit RGB32 formats, which
  the host has no render format of (the WebGPU back end track; asked in run 27).
- dEQP-GLES31 `texture.border_clamp` 24, KHR `gpu_shader5` gather depth 4: a border on a
  comparison sampler (`BorderInShaderOnComparison`, the shaders track's filter library).
- dEQP-GLES2 and -GLES3 `shaders.invariance` 40: `invariant` has no WGSL or naga form, and the
  host's compiler reorders arithmetic (the shaders track's naga writers; absent on WebGPU).
- dEQP-GLES3 `builtin_functions` 18 and dEQP-GLES31's 12 (`sign`): all 30 pass in run 31's whole
  run (the shaders track's change).
- KHR-GL33 `transform_feedback` 3 (`discard_geometry_test`, `capture_geometry_interleaved_test`,
  `query_geometry_interleaved_test`), since run 31: a geometry program passing 30 values is refused
  at link ("120 > 112"). The stage helper's pass-through vertex program takes a geometry or
  evaluation program's passed values as vertex inputs (30 on Direct3D 12 through wgpu, one for the
  position, one kept for points' `gl_PointCoord`), so the screen reports 28 (run 30 reported 31,
  which the link accepted and which lost the device wherever the output was drawn: the first case
  passed only because it discards). OpenGL 3.2's least is 128 geometry output components and 128
  fragment input components (32 values). Absent until the pass-through reads its records another
  way (from storage, as pulled vertex input does): the shaders track, asked.
- dEQP-GLES3 `texture_functions` 25: comparisons with a bias, gradients or a level other than 0,
  which the common form lacks (the shaders track's common form).
- dEQP-GLES31 `primitive_bounding_box.lines` and `.triangles` 28: the stage helper sizes a
  geometry program's output after tessellation for factor 64 on every patch, past
  `max_buffer_size` (the shaders track; asked).
- dEQP-GLES31 `multisample_interpolation` 11: an input read at the centroid and at an offset. Built
  in run 31 (661a9ad7): all 11 pass in its group run.
- dEQP-GLES31 `synchronization` 11, `image_load_store.3d` 3 (flakes), KHR-GLES31
  `shader_image_load_store` 3 (`advanced-copyImage-cs`, `advanced-allMips-cs`,
  `advanced-sync-imageAccess2`): storage written by other invocations or through other bindings
  read stale; which cases fail moves from run to run (run 31's group run: 12 fail, 4 flake,
  `image_load_store.2d_array` qualifiers among them). Read in run 31, not proven: the route drops
  GLSL's `coherent` and `volatile` (naga's `MemoryDecorations`, HLSL `globallycoherent`, which
  wgpu gives only with its native feature `MEMORY_DECORATION_COHERENT` and the interface names
  no capability for); the `alias` cases write through one binding and read through another of
  the same buffer, which a host compiler may reorder where it takes bindings not to alias. This
  track's to trace further, then the interface and WebGPU back end tracks' if the decoration is
  the cause.
- dEQP-GLES31 `builtin_functions` 6 (`texture_size.samples_*_2d_array`): the multisampled array
  split binds every layer whatever the program reads, `BindingsBeyondLimit` (the shaders track;
  asked).
- dEQP-GLES31 `image_load_store.early_fragment_tests` 6, KHR-GLES31
  `shader_image_load_store.basic-glsl-earlyFragTests`: since run 31 the route gives fragment
  programs their pixel extras (1cbbae1d) and every program forcing early tests goes through the
  rewrite (c4b8743c); the driver draws it where no depth or stencil test is made or its results do
  not depend on when the tests run, and refuses by name the rest ("early depth and stencil tests
  ... (EarlyTestsInShader, not built)"): the program's tests before its storage writes, which the
  interface's `VariantFlags::EARLY_TESTS` route makes (a read-only depth-stencil attachment at
  `EARLY_DEPTH_BINDING`, `HelperValue::EarlyTests`, per-primitive draws where depth is written).
  This track, not built.
- dEQP-GLES31 `tessellation.common_edge` 6 and KHR `gpu_shader5.precise_qualifier`: `precise`
  has no form in naga's writers (the shaders track).
- dEQP-GLES31 `geometry_shading.input` 5 and KHR `geometry_shader.adjacency` 4: strips with
  adjacency, `gl_in[]` in Direct3D's order where OpenGL swaps the first two (the shaders track;
  asked).
- KHR-GLES31 `shader_multisample_interpolation` 3: `interpolateAtOffset` compared bit for bit with
  the host's own interpolation; WGSL has no interpolation at an offset (absent on WebGPU).
- Device losses besides `packed_pixels`: dEQP-GLES3 `texture.specification` 3 (16-bit depth
  written from a buffer: the WebGPU back end track), KHR-GL33 and -GLES3 `packed_depth_stencil` 16
  (textures destroyed while a recorded pass uses them: the WebGPU back end track unless a trace
  shows this driver's misuse), dEQP-GLES3 `fbo.color.tex3d` 2 (traced in run 31: the WebGPU back
  end's `PackedTargetQuantised` makes a 2D view of a 3D texture, "Invalid texture view dimension
  `D2` with texture of dimension `D3`"; the WebGPU back end track, asked).
- KHR-GLES31 `compute_shader` 2: an atomic unsigned minimum on a signed word. Built in run 31
  (0986b2f4): both pass in its group run.
- KHR `geometry_shader.limits.max_output_components` 2: traced in run 31, a device loss ("Vertex
  attribute location 30 must be less than limit 30"): the stage helper's pass-through takes the
  position and each passed value as vertex inputs, 30 on the host, where the screen reported 31
  passed values (and the squares need one more). Fixed in c4b8743c: the screen reports at most the
  pass-through's inputs less two.
- KHR-GLES31 and -GLES32 `tessellation_shader.vertex.vertex_ordering` 2 time out: traced in run
  31, alone with a 400 s limit KHR-GLES31's passes in 61.8 s (deqp-runner stops a case at 60 s;
  desktop, RTX 3080, one GPU job). The test makes a new evaluation program for each of its 6,156
  iterations; each is a new shader state, so its conversion and the stage helper's pipelines are
  made anew (20,521 pipelines, every one a miss, `TessellationAsCompute` 12,312,
  `StreamOutAsCompute` 6,156). Programs of equal content sharing their conversions and pipelines
  (a cache keyed by the route's key and variant, held while any shader state uses it) would remove
  most of it: this track, not built. Run 31 also stopped drawing points as squares where the
  rasterizer discards (458438c4), which this test does in point mode.
- dEQP-GLES3 `rasterization.fbo.rbo_multisample_max` `lines` 2 (8 samples; 4 pass): 18 pixels
  filled beside the line in each of 3 iterations, none missing. Read in run 31, not proven: wgpu's
  Direct3D 12 back end sets `MultisampleEnable` for multisampled pipelines, under which Direct3D
  draws lines as quadrilaterals wider than a pixel; more samples catch more of the wider edge.
  This track to prove (the same scene on `-Driver system`), then the WebGPU back end track's.
- Not yet read (this track): dEQP-GLES31
  `geometry_shading.vertex_transform_feedback` 2, KHR `gpu_shader5` `texture_gather_offset_color`
  2 (gathers at run-time offsets at a texture's edges), and the 51 single cases in
  `logs/g9r30/groups.txt` (among them KHR-GL33 `clip_distance` 2, KHR-GLES32 `texture_buffer` 1,
  `texture_cube_map_array.image_op_compute_sh` 1, which flakes).
- NotSupported, not failures but in the check: KHR-GLES32's 251 (storage blocks in evaluation
  programs, `constant_expressions`, waiting on the shaders track's cull route for a domain
  program writing storage), and each list's Skip count in the table, read case by case in
  run 23's and 27's sections of the history above.
- KHR-GL46 19,714: OpenGL 4.0 to 4.6 (this track, G9's second part).

## For the owner

- Unsafe code (the 2026-11-22 rule): wcontainers-opengl (370 uses, counted on 2026-11-23 as the
  root list counts them: every `unsafe` word outside comments) and wcontainers-nir (116) stay on
  the root list. Nearly all of them are the C boundary itself: wcontainers-opengl is a Gallium
  driver, whose every entry point is a C function pointer Mesa calls with raw pointers to its own
  structures (`pipe_context`, `pipe_resource`, `pipe_draw_info`, ...), and wcontainers-nir walks
  Mesa's NIR through the C bridge. No vetted library gives a safe Gallium or NIR binding (bindgen
  gives the declarations, every call still unsafe). Removing them means moving that glue into C
  (the same work, no longer checked by Rust) or writing and vetting a safe binding of Gallium's
  and NIR's structures, a project of its own. The few that are not the boundary (the transmutes of
  the bridge's enum values, `slice::from_raw_parts` of its arrays) can go with zerocopy-style
  checked conversions; not done yet. Asked: whether the boundary is the damn good reason the rule
  allows, or which of the two the owner wants.

## How to run

- The corpus: `crates/wcontainers-nir/corpus/fetch` (into `$CARGO_TARGET_DIR/nir-corpus`), then with
  `WCONTAINERS_MESA` set: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus WC_NIR_RESULTS=<file> cargo
  test -p wcontainers-nir --test corpus`. From the Mac: `orchestrator/cloud/x86-run <checkout> --name
  graphics-nir --jobs 64 -- 'crates/wcontainers-nir/corpus/fetch && export
  WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build && WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus
  WC_NIR_RESULTS=$CARGO_TARGET_DIR/nir-results-a.jsonl cargo test -p wcontainers-nir --test corpus'`,
  then `x86-run --fetch graphics-nir target/nir-results-a.jsonl <scratch>`. 1005 shaders take about
  2 minutes with the comparison (192 at once). Run 3's results are in
  `pyro-parked/wcontainers-scratch/graphics-nir/results/run3/nir-results-a.jsonl` (run 2's, conversion
  only, one folder up); run 4's in `results/run4/` (`nir-results-r4b.jsonl` the last, with the
  patch; `nir-choice-r4.jsonl` the timed one); `scripts/reasons.py <results> [n]` counts the reasons.
- The choice between the candidates, one command: `crates/wcontainers-nir/corpus/choose [results]`
  (fetches the corpus, builds optimised, runs one shader and route at a time on ten cores; its last
  lines give each route's counts and its median and mean time on the shaders every route matches).
  From the Mac: `x86-run <checkout> --name graphics-nir -- 'WC_NIR_TIMING_CORES=0-9
  crates/wcontainers-nir/corpus/choose $CARGO_TARGET_DIR/nir-choice.jsonl'` (the whole corpus of 18,222 items took 4 h 38 min in run 11; most of it is Zink's compiler
  aborting on GLSL in route A, each abort a core dump the machine's `apport` writes).
  `WC_NIR_SHADERS=<n>` takes a seeded sample: for the independent check,
  `WC_NIR_TIMING_CORES=0-9 WC_NIR_SHADERS=2000 crates/wcontainers-nir/corpus/choose
  $CARGO_TARGET_DIR/nir-choice-check.jsonl` (run 15: about 1,850 s at load 54 to 98; B 1,288 match,
  A 119; run `corpus/cts-programs` first, or the sample is drawn from fewer items). A reference
  that does not end on the seeded inputs is held on up to four seeds, two minutes each, so a
  sample with such programs takes longer.
- The licence of every linked Mesa file: `crates/wcontainers-nir/mesa/linked-licences [file]` (after
  the Mesa build; the route test linked with a map, each kept object traced to its source).

- Stopping this track's own CTS run on the desktop (a run on a build known to be broken, which
  holds the GPU queue for everyone): scratch `scripts/stop-cts.ps1`, copied to
  `C:\wc\graphics-nir\stop-cts.ps1` and run with `powershell -NoProfile -ExecutionPolicy Bypass
  -File`; it stops only processes running this track's run-windows.ps1, deqp-runner and glcts.
- G9 run 31: `scripts/g9r31-run.sh` is run 30's under its name (run 31's runs did not set
  `SHADERS_AT_HEAD`: the shaders crate built as the worktree held it); lint is still
  `scripts/g9r30-lint.sh`. `scripts/x86-retry.sh <log> <command>` runs a command on the x86-64
  machine again every two minutes while it does not answer (it was away several times on
  2026-11-23 and 24). One case alone with a longer limit than deqp-runner's 60 s:
  `run-case-windows.ps1 ... -Seconds 400` (KHR `vertex_ordering` passes in 61.8 s that way).
- G9 run 30: `scripts/g9r30-run.sh <log> [<lists> [<regexes>]]` and `scripts/g9r30-lint.sh [<test
  args>]` are run 29's and run 25's, with `SHADERS_AT_HEAD=1` taking the shaders crate as committed
  (scratch `overlay/shaders-head.tar`, made with `git archive HEAD crates/wcontainers-shaders/Cargo.toml
  crates/wcontainers-shaders/src`, sent to the desktop's `C:\wc\graphics-nir\overlay` and the x86-64
  machine's `/work/probes/graphics-nir-shaders-head.tar`), for while another track's uncommitted
  edits break that crate in this shared worktree. `run-windows.ps1` now removes a list's earlier
  results through the long-path prefix: run 30's first whole run stopped after five lists on a
  case's `.qpa` named past 260 characters (KHR-GLES31 tessellation). Pass the log as an absolute path.
- G9 run 29: `scripts/g9r29-run.sh <log> [<lists> [<regexes>]]` is run 28's under its name; the
  group run's lists and regexes are in this file's run 29 section. Run 29's whole run took about
  11 minutes once the job started.
- G9 run 27: `scripts/g9r27-run.sh` is run 26's under its name. Never submit a second job with
  `--build` (a group run) while a whole run waits in the queue: its build sends another tree to
  the desktop's copy and the waiting job then fails at its start ("holds tree ..., not the sent
  ..."), as run 27's first whole run did. Run them one after the other.
- G9 run 26: `scripts/g9r26-run.sh <log> [<lists> [<regexes>]]` is run 24's one-command run under
  its name (no lists: every list, one GPU job of about 40 minutes after the queue's wait);
  `scripts/g9r25-lint.sh` and `scripts/g9r25-table.py <run> <base>` as below. A case's `.qpa` is
  overwritten by the next run of its list: copy the ones to read to scratch `results/<run>/qpa`
  first (scp from `C:/wc/graphics-nir/cts/results/wcontainers/<list>/<case>.qpa`).
- G9 run 25: the whole run is still `scripts/g9r24-run.sh <log>` (one GPU job, about 40
  minutes); `scripts/g9r25-lint.sh [<test args>]` formats, lints both crates with warnings denied
  with and without Mesa and runs the nir crate's route test on the x86-64 machine (its SSH
  sometimes times out at the banner: run it again); `scripts/g9r25-table.py <run> <base>`
  (folders under `results/`) prints each list's counts and the moves against the base. Copy
  `case-notes.ps1`'s output back before the next run overwrites the process logs.
- G9 run 24: `scripts/g9r24-run.sh` is run 23's one-command run under its name. Give the
  command that starts it a time limit longer than the queue's wait (an hour or more on
  2026-11-23): a stopped client loses the job's printed counts, though the job carries on and
  its `results.csv` files land on the desktop as usual (`mine.ps1` shows whether it still runs). Reading a whole
  run's failures by their causes: copy scratch `scripts/case-notes.ps1` to `C:\wc\graphics-nir`
  and run it there (`powershell -NoProfile -ExecutionPolicy Bypass -File
  C:\wc\graphics-nir\case-notes.ps1`; every refusal, panic and device error of every list's
  process logs with the case running when it was printed, into `C:\wc\graphics-nir\case-notes.txt`,
  about a minute), copy that file back, then `python3 -I scripts/g9r24-reasons.py
  results/<run>/full <case-notes.txt> [case regex]` (scratch) counts each failing group by its
  note. A failing case's `.qpa` (pictures and GL calls) is kept beside the process logs in the
  list's results folder on the desktop; `scripts/qpa_images.py` writes its pictures.
- G9 run 23's one-command run (scratch `scripts/g9r23-run.sh <log> [<lists> [<regexes>]]`, no
  lists for every list): the Mesa build and the CTS run are ONE GPU job (`gpu-job --build`), so
  both use one snapshot of the working tree. A `remote` build followed by a separate `gpu-job`
  fails ("holds tree ..., not the sent ...") when the working tree changed between them, which
  in this shared worktree it does. `first-error.ps1 -List <list> -Case <text>` prints in full the
  first device error of a process log that ran a matching case; `mine.ps1` lists this track's
  processes on the desktop (copy both to `C:\wc\graphics-nir` first).
- G9 run 21's scripts are run 20's under `g9r21-` names (`g9r21-group.sh <log> <lists> [<regexes>]`,
  `g9r21-full.sh <log>`); `lost-causes.ps1 -List <list>` (copy it to `C:\wc\graphics-nir` first)
  counts each lost process's first device error with the group it ran. For the rented machine,
  another track's unfinished `crates/executor/implementation/host-process` (a manifest naming a
  missing test) stopped cargo loading the workspace on 2026-11-23: leave it out of the copy as well.
- G9 run 20's scripts are run 19's under `g9r20-` names (`g9r20-group.sh`, `g9r20-full.sh`), with
  `g9r20-compare.py <lists>` (a whole run's results against run 19's); on the desktop
  `refusals.ps1 -List <list>` counts a list's refusals and its cases' groups while it runs, and
  `first-lost.ps1 -List <list> -Group <text>` prints the first device error of the processes
  that ran a group (copy both to `C:\wc\graphics-nir` with scp first). The literal check's
  `literal-check.sh` now clones branch agents from the bundle.
- G9 run 19's one-command scripts (scratch `scripts/`, run 18's with the run's names; give the
  log as an absolute path): `g9r19-group.sh <log> <lists> [<regexes>]` builds Mesa with the driver
  on the desktop and runs those lists (those groups of them) as one GPU job; `g9r19-full.sh <log>`
  the same for every list. Never run `tools/graphics-rig/queue-check` to see the queue: it
  submits jobs of its own. Both leave out of the desktop's copy any
  workspace member folder without a manifest (another track's unfinished work, which stops cargo
  loading the workspace); for the rented machine prefix the command with `for d in
  crates/*/implementation/* crates/*/conformance; do if [ -d "$d" ] && [ ! -f "$d/Cargo.toml" ];
  then rm -rf "$d"; fi; done;` (the copy only: the checkout is not touched).
- One group alone (G9 run 7): add `-Include <regex>[,<regex>]` to `run-windows.ps1`, e.g.
  `-Lists gles2-main -Include dEQP-GLES2.functional.vertex_arrays.*`. The refusals of a whole
  run, from the Mac: `ssh -o BatchMode=yes tom@192.168.2.183 'findstr /C:"refused:" /C:"not
  built:" C:\wc\graphics-nir\cts\results\wcontainers\<list>\*.log'`.
- Reading a run's failures (G9 run 6): copy the list's folder back (`scp -o BatchMode=yes -r
  tom@192.168.2.183:C:/wc/graphics-nir/cts/results/wcontainers/<list> <scratch>`), then
  `scripts/qpa_images.py <case>.qpa` (scratch) writes each picture of a case's log as a PNG, and
  `scripts/crash_reasons.py <folder> <runner output>` lists each failed case with the driver's
  refusals in its process's log (a process runs many cases: a refusal is the crash's own, a
  validation error may be an earlier case's).
- Build Mesa for Windows (G9) on the desktop, from a checkout on the Mac: `tools/graphics-rig/remote
  'powershell -NoProfile -ExecutionPolicy Bypass -File crates\wcontainers-nir\mesa\build-windows.ps1'`.
  Tools, source, build and install under `C:\wc\graphics-nir` (kept between runs; the tools'
  downloads with their SHA-256 printed); `opengl32.dll` and `libgallium_wgl.dll` in
  `C:\wc\graphics-nir\mesa\install\bin`. Uses half the desktop's threads. The first build
  after G9 run 5 also builds llvmpipe's LLVM from the 21.1.8 source into
  `tools\llvm-21.1.8-mt` (X86 only, static runtime, libraries, headers and llvm-config; the
  source and build folders deleted after). A remote command waits
  while another track's hazard job holds the GPU slot. Kept between runs on the desktop for G9:
  `C:\wc\graphics-nir`, 4.8 GB at the end of G9 run 1 (the unpacked tools, LLVM most of it; the
  Mesa source, build and install); the downloaded archives were deleted (fetched again only if a
  tool is missing).
- The conformance suite on the desktop (G9): build once with `tools/graphics-rig/remote
  'powershell -NoProfile -ExecutionPolicy Bypass -File crates\wcontainers-opengl\cts\build-windows.ps1'`
  (glcts and deqp-runner; about 40 minutes the first time, at 8 jobs beside other tracks'
  builds). Run lists as a GPU job: `tools/graphics-rig/gpu-job --desktop --build 'if not exist
  "%CARGO_TARGET_DIR%" mkdir "%CARGO_TARGET_DIR%"' -- 'powershell -NoProfile -ExecutionPolicy
  Bypass -File crates\wcontainers-opengl\cts\run-windows.ps1 -Driver <wcontainers|llvmpipe|system>
  [-Lists gles2-main,gl46-main] [-Fraction 100 -FractionStart 1]'` (a list's name is its file's
  name without `.txt`). It prints each list's counts and share; deqp-runner's `results.csv`,
  `failures.csv` and logs are in `C:\wc\graphics-nir\cts\results\<driver>\<list>`. One case
  alone, for a hang or a crash: `tools/graphics-rig/gpu-job --desktop --build 'echo sent' --
  'powershell -NoProfile -ExecutionPolicy Bypass -File
  crates\wcontainers-opengl\cts\run-case-windows.ps1 -Driver wcontainers -List gles2-khr-main
  -Case <case> [-Seconds 40]'` (glcts's output, exit code and log printed; killed and reported
  as hung after the limit). Add `-Debugger` to run it under the LLVM release's lldb (with Python
  3.10 from NuGet, fetched once): every thread's stack and the stopped thread's stack words as
  symbols, read from the PDB files the build keeps (`buildtype = 'debugoptimized'`, no
  assertions). The release's folder had to be opened to the local Users group once
  (`build-windows.ps1` now does it when it unpacks LLVM), as the hidden desktop's user could
  not run lldb. Linting the driver on the desktop: `tools/graphics-rig/remote 'set
  WCONTAINERS_MESA=C:\wc\graphics-nir\mesa\build&& set RUSTFLAGS=-C target-feature=+crt-static&&
  set LIBCLANG_PATH=C:\wc\graphics-nir\tools\clang+llvm-21.1.8-x86_64-pc-windows-msvc\bin&& cargo
  clippy -q -p wcontainers-opengl --release --target-dir C:\wc\graphics-nir\opengl-target -- -D
  warnings'`; on the x86 machine with `WCONTAINERS_MESA` set and `LIBCLANG_PATH=$(nix build
  --no-link --print-out-paths --inputs-from path:/work/flake/graphics-nir
  nixpkgs#llvmPackages.libclang.lib)/lib`. Logs of
  run 2 in scratch `logs/g9r2-*.log`, its `results.csv` files in `results/g9r2/cts/<driver>/<list>`. The Mesa build (above) must have run first for
  `wcontainers` and `llvmpipe`. Kept on the desktop after run 3 (reused by every build): `cts` 3.5 GB, `mesa` 0.7 GB, `opengl-target` 2.4 GB (now with wcontainers-nir and the shaders crate's C++), `tools` 3.9 GB; the run copy's submodules are checked out by the build. After run 2: `C:\wc\graphics-nir\cts`
  3.8 GB (the suite's source and build), `tools` 3.9 GB (with deqp-runner), `mesa` 0.8 GB,
  `opengl-target` 0.8 GB (the driver's cargo output, reused by each Mesa build).
- Build Mesa, from the Mac: `orchestrator/cloud/x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/mesa/build path:/work/flake/graphics-nir'`. On any Linux machine with Nix:
  `crates/wcontainers-nir/mesa/build` (its argument, the flake, defaults to the checkout). The tools
  come from Nixpkgs' own `mesa` package (`nix print-dev-env --inputs-from <flake> nixpkgs#mesa`), so the
  repository's flake is unchanged and the tools are those of its locked nixpkgs. Output in
  `$CARGO_TARGET_DIR/mesa/{src,build,install}` (target/mesa without it). The script refuses a tag whose
  commit is not the pinned one, makes the build folder again when the commit or the options change
  (Meson reads a machine file only when a folder is made), and fails unless llvmpipe and lavapipe were
  built.
- GLSL tests: `cargo test -p wcontainers-nir --test route --test glsl_buffers --test
  glsl_reference` (with `WCONTAINERS_MESA`, on the x86 machine). The corpus run takes piglit's
  `.shader_test` files beside the `.spv` files, from the same `WC_NIR_CORPUS`; all 3546 take about
  4 minutes at 192 at once. `scripts/glsl_reasons.py <results> [route] [n]` (scratch) counts
  piglit's reasons; `scripts/piglit_outcomes.py <results> [route] [n] [prefix]` counts every
  outcome of the files under a prefix (`piglit/`, `bgfx/`). bgfx's shaders come with the same
  `corpus/fetch` and run in the same corpus run (any `.bin` in a folder named `glsl`).
- Test with Mesa (on the x86 machine, after the build):
  `export WCONTAINERS_MESA=$CARGO_TARGET_DIR/mesa/build; cargo test -p wcontainers-nir` and
  `cargo clippy -p wcontainers-nir --all-targets -- -D warnings`; and both again without the variable.
- dEQP's cases come with the same `corpus/fetch` (VK-GL-CTS's shader library data, sparse) and run
  in the same corpus run: 13,630 items take about 35 s alone (`WC_NIR_CORPUS` at
  `nir-corpus/vk-gl-cts`). `scripts/deqp_differences.py <results>` (scratch) groups route B's
  differences by file and message.
- The Vulkan CTS's programs: `crates/wcontainers-nir/corpus/cts-programs [flake]` (from the Mac:
  `x86-run <checkout> --name graphics-nir --jobs 96 -- 'crates/wcontainers-nir/corpus/cts-programs
  path:/work/flake/graphics-nir'`); the corpus run takes them from `nir-corpus` like the rest.
- The literal check needs the git history, which the rented machine's copy lacks. Run 6 ran it on
  the x86 machine from a git bundle of the branch (`git bundle create <scratch>/nir.bundle
  phase-2-graphics-nir`, sent with `x86-run --ssh 'cat > /work/probes/graphics-nir.bundle'`,
  cloned there, `cargo fetch`, then the check in `nix develop`; the clone and its build deleted
  after). Runs 1 to 5 ran it on the Mac (`RAYON_NUM_THREADS=3 cargo run -j 4 -q -p
  wcontainers-literals -- check`). With
  `crates/wcontainers-shaders/dxbc-spirv`'s own submodule checked out the check fails on its Python
  scripts (the baseline holds it not checked out): fetch only the shaders crate's two submodules and
  dxil-spirv's `third_party/spirv-headers`, which the shaders crate's build needs.

- A quick look at one shader: copy it alone into a folder and point `WC_NIR_CORPUS` there with
  `WC_NIR_DUMP` set; `nix shell nixpkgs#spirv-tools -c spirv-dis` reads the dumps.
- Run 8 fetched VK-GL-CTS's shader library data (sparse, at 5bb4c7e8) and a sparse bgfx on the
  x86 machine under `/work/probes` to read their formats; both deleted at the end of the run.
- Runs beside the timed choice: pin to `taskset -c 10-95,106-191` (cores 0-9 and their hyperthread
  siblings 96-105 are the choice's), and never build `--release` in `/work/target/graphics-nir`
  while it runs. Probe programs (a changed copy of one corpus program, compiled with Nixpkgs'
  glslang as `corpus/godot-forward-plus` does) go in a folder of their own under the build folder,
  pointed at by `WC_NIR_CORPUS`. `scripts/outcome_changes.py <old> <new> <route>` (scratch) lists
  the programs whose outcome changed between two results files.
- Kept between runs: a sparse clone of Dolphin's `Source/Core/VideoCommon` at 32a2b828 for reading
  how it makes its shaders (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/dolphin`,
  GPL-2.0-or-later: read only, nothing of it copied or paraphrased here), and bgfx's two ASSAO
  sources and their `uniforms.sh` (`downloads/bgfx-src`, BSD-2-Clause).
- Kept between runs: a sparse clone of Godot at 73ca6be2 for reading its shader formats and their
  C++ users (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/godot`, 12 MB, blobs fetched
  on demand: never `git grep` its history there, which fetches every blob; add paths with `git
  sparse-checkout add /<path>`).
- Godot's RenderingDevice shaders, one command after the fetch's sources: `crates/wcontainers-nir/corpus/godot-forward-plus [flake]`
  (from the Mac: `x86-run <checkout> --name graphics-nir --jobs 64 --
  'crates/wcontainers-nir/corpus/godot-forward-plus path:/work/flake/graphics-nir'`; 9 s). It
  fetches, writes the variants' sources (the corpus binary's `--godot-rd`, no Mesa needed) and
  compiles them with Nixpkgs' glslang into `nir-corpus/godot-forward-plus`, listing what glslang
  refuses in `glslang-refused`. Run it (and `corpus/cts-programs`) before `corpus/choose`, which
  fetches only `corpus/sources`. Alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot-forward-plus`.
- Godot's shaders come with the same `corpus/fetch` and run in the same corpus run (any `.glsl`
  under `drivers/gles3/shaders`); alone: `WC_NIR_CORPUS=$CARGO_TARGET_DIR/nir-corpus/godot`.
- Kept between runs: a sparse clone of VK-GL-CTS at 5bb4c7e8 for reading dEQP's formats
  (`pyro-parked/wcontainers-scratch/graphics-nir/downloads/vkglcts`, no build output); on the x86
  machine, the CTS source and build under `/work/target/graphics-nir/vk-gl-cts` (with Mesa and the
  rest, 18 GB in all after run 9).
- Kept between runs: a shallow clone of the pinned Mesa for reading on the Mac
  (`pyro-parked/wcontainers-scratch/graphics-nir/mesa-src`, 560 MB, no build output; logs beside it in
  `logs/`), and the Mesa build and cargo target on the x86 machine (`/work/target/graphics-nir`).
  No scratch copy of the checkout was made on the Mac, the laptop or the desktop. The literal check's
  own build in the checkout's `target` (282 MB) is deleted at the end of each run. On the x86
  machine `/work/target/graphics-nir` (11 GB after run 4: Mesa, the corpus, cargo's debug and release output, the last results) is kept.

## Measurements

- G9 run 12, the whole lists on our driver (desktop, RTX 3080, one GPU job, 8 glcts at once,
  other tracks' jobs queued beside): run A 20:15:12 to 20:39:31 (24 min for the six lists that
  start), run B of the two ES 3.1 lists 20:42:58 to 20:54:11 (11 min). Group runs: border_clamp
  alone (619 cases) 47 s; the texture groups of both ES 3.1 lists (2,078 cases) about 2 min.
  The Windows Mesa build after the patch (most of st/mesa again): about 6 minutes, 8 jobs.
- G9 run 12, the whole corpus run (18,738 items, 96 children, x86-64 machine, load 0.06 at the
  start and 2.1 at the end) at 35710fc3: 741 s. Route B: 12,090 match, 156 differ, 6 not
  compared, as run 11; route A unchanged. On the 1,072 shaders both routes match: A median
  426 us, mean 620 us; B median 338 us, mean 412 us. Results: scratch
  `results/g9r12/nir-results-g9r12.jsonl`. Again at 1c711c04 (the signed words' loads and
  stores; load 21 at the end, others' jobs beside): 741 s, route B 12,095 match (5 more: the
  signed atomic minimum and maximum), 156 differ, 6 not compared; B median 346 us, mean 416 us;
  `results/g9r12/nir-results-g9r12b.jsonl`.
- G9 run 12, the suite of wcontainers-nir and wcontainers-opengl at 1c711c04 (x86-64 machine,
  load 0.0 to 0.9): fmt clean; clippy with warnings denied clean with and without Mesa; with Mesa
  `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2, `route`
  11 passed; the Mesa build with the new patch made again first (32 s with the tests).

- G9 run 11, the whole lists on our driver (desktop, RTX 3080, one GPU job, 8 glcts at once,
  other tracks' jobs queued beside): run A 18:55:50 to 19:24:49 (29 minutes for the 110,807 cases of the six lists that start
  with the incremental build; dEQP-GLES31 about 16 of them), run B of the two ES 3.1 lists
  19:30:59 to 19:43:42. A 10% sample of dEQP-GLES3, -GLES31 and KHR-GLES31 (8,669 cases):
  18:38:58 to 18:44:54. The shadow group alone (432 cases): about a minute.
- G9 run 11, the whole corpus run (18,738 items, 96 children, x86-64 machine, load 0.7 at the
  start and 2.1 at the end): 742 s. Route B: 12,090 match, 156 differ, 6 not compared, as run 9;
  route A unchanged. On the 1,072 shaders both routes match: A median 424 us, mean 622 us; B
  median 341 us, mean 409 us. Results: scratch `results/g9r11/nir-results-g9r11.jsonl`, log
  `logs/g9r11-corpus.log`.
- G9 run 11, the suite of wcontainers-nir and wcontainers-opengl (x86-64 machine, load 0.0 to
  1.0): fmt clean; clippy with warnings denied clean with and without Mesa; with Mesa
  `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2, `route`
  11 passed.

- G9 run 10, the whole lists on our driver (desktop, RTX 3080, one GPU job each, 8 glcts at
  once, other tracks' jobs queued beside): run A 17:40:35 to 17:56:43 (about 16 minutes with the
  build; dEQP-GLES2 about 3, dEQP-GLES3 about 11), run B 18:02:36 to 18:15:19 (about 13). Group
  runs alone (`-Include`): ubo and texture, 8,252 cases, about 6 minutes; transform_feedback,
  ubo.random and fbo.blit and color, 2,588 cases, about 3.
- G9 run 10, wcontainers-nir's tests with Mesa and clippy (x86-64 machine, load 3 to 19): fmt
  clean; clippy with warnings denied clean for wcontainers-nir and wcontainers-opengl with and
  without Mesa; with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known
  failures), `narrow` 2, `route` 11 passed. The corpus run was not repeated (route B's changes:
  `fcanonicalize` read as a move, and the driver's own passes, which the corpus does not take).

- G9 run 9, the whole corpus run (12,748 items of route B's count, 96 children, x86-64 machine,
  load 15.7 at the end; Mesa's build kept from earlier runs): 742 s. Route B: 12,090 match, 156
  differ, 6 not compared (run 1 of G9: 12,068, 156, 6): the 22 more matches are programs the
  uniform block sizing and `b2b32` now take; no new difference. Time per shader to the validated
  common form at 96 at once on the 1,072 shaders both routes match: A median 422 us, mean 650 us;
  B median 343 us, mean 411 us. Results: scratch `results/g9r9/nir-results-g9r9.jsonl`, log
  `logs/g9r9-corpus.log`.
- G9 run 9, wcontainers-nir's tests with Mesa (x86-64 machine, load under 3): `glsl_buffers` 1,
  `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2, `route` 11 passed; clippy
  with warnings denied clean for wcontainers-nir and wcontainers-opengl with Mesa.
- G9 run 9, the four GLES lists on our driver (desktop, RTX 3080, one GPU job, 8 glcts at once,
  other tracks' jobs queued beside): 16:04:42 to 16:31, about 27 minutes for 68,904 cases.

- G9 run 7, wcontainers-nir's suite (x86-64 machine, load 1 to 5): fmt clean; clippy with
  warnings denied clean with and without Mesa (wcontainers-nir and wcontainers-opengl without it);
  with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2,
  `route` 11 passed. The driver's clippy with warnings denied clean on the desktop with Mesa. The
  corpus run was not repeated (route B's changes this run: the shared NIR step's texture
  lowerings, which the GLSL corpus front already made before it).
- G9 run 7, the whole dEQP-GLES2 and KHR-GLES2 lists on our driver (desktop, RTX 3080, one GPU
  job each, 8 glcts at once, other tracks' jobs queued beside): 14:39 to 14:45 (with the build),
  14:57:12 to 15:01:49 for the last (4 min 37 s with the incremental build). `vertex_arrays`
  alone (571 cases): 10 s.

- The whole suite of wcontainers-nir and wcontainers-opengl (G9 run 6, x86-64 machine, load 3
  to 7): fmt clean; clippy with warnings denied clean with and without Mesa; with Mesa
  `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known failures), `narrow` 2, `route`
  11 passed, and the driver's doc tests none (bindgen no longer copies Mesa's comments, whose C
  examples rustdoc ran as Rust and failed: 2 failures before); without Mesa every target builds,
  no tests run. The corpus run was not repeated this run (route B's one change, the sample level
  outside comparisons, is held by the conformance runs; the corpus run is the next run's).
- The whole dEQP-GLES2 and KHR-GLES2 lists on our driver (G9 run 6, desktop, RTX 3080, one GPU
  job, 8 glcts processes at once, other tracks' jobs queued beside; the desktop's processors at
  8% just after): 17,638 cases in 6 min 45 s (14:00:07 to 14:06:52, with the job's start). A 1%
  shard of both (177 cases): about 35 s; a 10% shard (1,765 cases): 71 s.
- The Windows Mesa build with the driver, incremental after a driver change (desktop, 8 jobs,
  shared): about 1 to 2 minutes, most of it cargo's release build of the driver.

- The whole corpus run (G9 run 1, 18,738 items, 96 children, x86-64 machine, load 10.9 at the
  start and 2.2 at the end; a new spot machine, Mesa and the corpus built again first): 740 s
  (12 min 20 s). Route B: 12,068 match, 156 differ, 518 failed by name, 6 not compared (run 15:
  12,069, 154, 519, 6); the three changes are piglit's `fs-shl-of-shr-int64` (was "not taken:
  umul_high", now matches) and two Slang vertex programs of Sascha Willems' samples
  (`occlusionquery/simple.vert`, `subpasses/gbuffer.vert`) whose outputs the route drops, now
  differences by the new vertex check: each stores a load of a local the program never writes
  (checked in the disassembly), an undefined value, listed with the undefined-behaviour
  differences. Route A unchanged (1,075 match, 7 differ, 147 not compared). Time per shader to
  the validated common form at 96 at once on the 1,072 shaders both routes match: A median
  448 us, mean 710 us; B median 358 us, mean 460 us. Results: scratch
  `results/g9r1/nir-results-g9r1.jsonl`.
- The whole suite of wcontainers-nir (G9 run 1, x86-64 machine, load 2 to 15): clippy with
  warnings denied clean with and without Mesa; without Mesa every target builds (no tests run);
  with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 2 ignored (the known llvmpipe failure and
  the negated NaN comparisons), `narrow` 2 (2,097,152 16-bit cases and 65,600 multiplications,
  0 differ), `route` 11 passed. `corpus/cts-programs` from nothing: about 400 s at `-j 96`.

- The whole corpus run (run 15, 18,738 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 41.1 at the start and 100.4 at the end, other tracks' jobs and the timed sample
  beside it): 735 s. Time per shader to the validated common form at 172 at once on the 1,074
  shaders both routes match: A median 536 us, mean 863 us; B median 428 us, mean 556 us (B's time
  no longer includes the cache key, which neither route's now does).
- The whole suite of wcontainers-nir (run 15, x86-64 machine, load 84 to 115): fmt clean; clippy
  with warnings denied clean with and without Mesa; without Mesa every test target builds and
  passes (no tests run without it); with Mesa `glsl_buffers` 1, `glsl_reference` 10 and 1
  ignored (the known failure, llvmpipe's), `narrow` 1, `route` 11 passed.

- The whole corpus run (run 14, 18,222 items, 180 children, x86-64 machine, load 11.4 at the
  start and 3.3 at the end): 731 s. Time per shader to the validated common form at 180 at once
  on the 1,012 shaders both routes match: A median 519 us, mean 790 us; B median 448 us, mean
  564 us.
- The whole corpus run (run 13, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 14.3 at the start and 3.7 at the end, the timed choice on cores 0-9 beside it):
  732 s, up from 251 s: the references that do not end are now run on up to four seeds, two
  minutes each, one after another. Time per shader to the validated common form at 172 at once on
  the 998 shaders both routes match: A median 542 us, mean 831 us; B median 429 us, mean 519 us.
- The whole suite of wcontainers-nir (run 14, x86-64 machine, load 10.3 to 11.2): without Mesa
  every test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1,
  `glsl_reference` 9 and 1 ignored (the known failure), `narrow` 1, `route` 11 passed. Clippy with
  warnings denied clean with and without Mesa; the literal check exits 0 (from a bundle of the
  branch on the x86 machine, as run 6).
- The whole suite of wcontainers-nir (run 13, x86-64 machine, load about 14): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1,
  `glsl_reference` 7, `narrow` 1, `route` 11 passed.

- The whole corpus run (run 12, 18,222 items, 172 children on cores 10-95 and 106-191, x86-64
  machine, load 6.0 at the start and 13.3 at the end, the timed choice running on cores 0-9 and
  other tracks' jobs beside): 251 s. Time per shader to the validated common form at 172 at once
  on the 991 shaders both routes match: A median 454 us, mean 714 us; B median 358 us, mean
  440 us. The same run before the subroutine change (load 87 at the start, 27 at the end): 253 s.
- The whole suite of wcontainers-nir (run 12, x86-64 machine, load about 6): without Mesa every
  test target builds and passes (no tests run without it); with Mesa `glsl_buffers` 1, `glsl_reference` 7,
  `narrow` 1, `route` 11 passed.
- The whole corpus run (run 11, 18,222 items, 192 children, x86-64 machine, load 10.0 at the
  start and 26.7 at the end, another run of ours just before): 250 s. Time per shader to the
  validated common form at 192 at once on the 991 shaders both routes match: A median 488 us, mean
  767 us; B median 381 us, mean 481 us (busy machine; the choice's times are the ten-core ones).
  `corpus/godot-forward-plus` (fetch, the sources, 572 glslang compilations at 192 at once): 9 s,
  load 1.0. Godot's RenderingDevice programs alone: 16 s, load 0.8 to 1.7.
- The whole corpus run (run 10, 17,650 items, 192 children, x86-64 machine, load 8.4 at the start
  and 10.8 at the end): 248 s. Time per shader to the validated common form at 192 at once on the
  748 shaders both routes match: A median 421 us, mean 607 us; B median 346 us, mean 406 us (busy
  machine; the choice's times are the ten-core ones). Godot's 241 items alone: 128 to 134 s (most
  of it the one program whose reference does not end, held to the 120 s limit).

- `corpus/cts-programs` from nothing (VK-GL-CTS clone, its `fetch_sources.py`, CMake and Ninja
  for `vk-build-programs`, the alpha_to_one programs), x86-64 machine, `-j 96`, load 5.2 at the
  start and 10.4 at the end: 320 s wall, 76 min user.
- dEQP's 13,630 items alone, 192 children, x86-64 machine, load 0.1 to 32.6: about 32 s.

- Mesa 26.3.1 clean build (configure, 1239 steps, install), x86-64 machine (c7i.metal-48xl, 192
  threads), `-j 64`: 45.9 s wall, 11 min 37 s user, load average 4.96 at the start and 10.51 at the
  end (other tracks' jobs). An earlier clean build of the same options with SPIRV-Tools: 45.9 s wall
  at load 0.43 at the start.
- Corpus run with the comparison, 1005 shaders, 192 children, x86-64 machine at load 0.3 rising to
  5.3: 124 s including an incremental build (run 3).
- The corpus run with bgfx (run 8, 3871 files, x86-64 machine, 192 children, load 4.5 at the start
  and 16.1 at the end): 250 s. At 192 at once, on the 735 shaders both routes match: A median
  441 us, B median 352 us (a busy machine; the choice's times are the ten-core ones).
- The whole suite with the SPIR-V corpus and piglit compared (run 7, x86-64 machine, 192 children,
  load 4.9 at the start and 13.2 at the end): the corpus run 245 s for 3546 files. Time per shader
  to the validated common form at 192 at once, on the 728 shaders both match: A median 431 us, B
  median 341 us (busy machine; the choice's times are the ten-core ones below).
- The whole suite with the SPIR-V corpus compared (run 6, after the GLSL work; x86-64 machine,
  192 children, load 1.0 at the start and 12.5 at the end, 245 s with the build;
  `results/run6/nir-results-r6.jsonl`): the same counts as runs 4 and 5, A 728 match, B 773
  match, 0 differ, 8 not compared on each.
- `tests/glsl_buffers.rs` and `tests/route.rs` (run 6): 0.08 s and 0.01 s, x86-64 machine, load
  0.5 to 4.2.
- `tests/narrow.rs` (2,097,152 invocations, both programs on lavapipe): 0.47 s, x86-64 machine,
  load 0.08 (run 5).
- The choice's timing (run 4, `corpus/choose`, x86-64 machine, ten cores, one child at a time, load
  5.5 at the start and 3.2 at the end with another track's tests running): 1781 s in all; time per
  shader to the validated common form on the 725 shaders both match: A median 219 us, mean 261 us;
  B median 107 us, mean 125 us. The same at 192 children at once (load 9 to 25): A median 383 us, B
  median 196 us.

## Routed decisions (15.9, owner not yet answered; followed as the orchestrator set them)

- Our own OpenGL driver on the interface, as 15.2 recommends. gex_wgpu and wvk are not adopted; nothing of
  theirs is copied or paraphrased (their repository has no licence; a related project is AGPL-3.0).
- Crate and module names are the plan's suggestions: `wcontainers-nir`, `wcontainers-opengl`.
- Mesa under its MIT licence with the permissive files 15.2 lists; dEQP, VK-GL-CTS and piglit for testing only.
- No change to the interface crate: what the second batch needs from it (15.7) is written below as requests.

## Licences brought in (for the owner to confirm)

- G9 run 15: nothing new; the driver now also uses Gallium's `util_blitter` render condition
  fields (`u_blitter.h`, MIT, already linked).

- G9 run 12: nothing new. The driver now also calls Gallium's `u_default_get_sample_position`
  (`src/gallium/auxiliary/util/u_sample_positions.c`, Alyssa Rosenzweig, SPDX MIT), in
  `libgallium` already linked; the new Mesa patch changes Mesa's own MIT files
  (`st_extensions.c`, `extensions_table.h`, `consts_exts.h`).

- G9 run 11: nothing new. The driver's new paths use only Gallium's headers and Mesa's MIT
  files already linked; no crate was added.

- G9 run 10: nothing new. The driver now also calls Mesa's `nir_fixup_deref_types` and the GLSL
  type constructors (`glsl_sampler_type`, `glsl_texture_type`, `glsl_type_wrap_in_arrays`), all
  in Mesa's MIT `src/compiler` files already linked.

- G9 run 9: nothing new from outside. Three more of Mesa's MIT files are called by the driver:
  `src/gallium/auxiliary/indices/u_indices.c` and the `u_indices_gen.c` Mesa's build makes from
  `u_indices_gen.py` (both VMware, MIT: the index translator and generator), and
  `src/gallium/auxiliary/util/u_prim_restart.c` (VMware, MIT: the restart splitter). All are in
  `libgallium`, which the Windows build already links.

- G9 run 8: nothing new from outside. Two more of Mesa's MIT files are called by the driver:
  `src/gallium/auxiliary/nir/tgsi_to_nir.c` (Broadcom and Rob Clark, MIT) and, through the
  bridge, `src/compiler/nir/nir_xfb_info.h`'s data (MIT). wcontainers-opengl now depends on
  wcontainers-shaders (ours) directly.

- G9 run 6: the DirectX Shader Compiler's Windows release v1.9.2609 (`dxcompiler.dll`,
  `dxil.dll`), fetched by `mesa/build-windows.ps1` onto the desktop only and placed beside the
  test driver; not in this repository. Its LICENSE.TXT is the University of Illinois/NCSA Open
  Source License (permissive) with notices for its LLVM-derived parts; to confirm from the
  archive's own files before anything ships with it. The `cc` crate (MIT or Apache-2.0, already
  a build dependency of wcontainers-nir) is now also a build dependency of wcontainers-opengl.

- Mesa 26.3.1 (fetched by the build scripts, not in this repository). Its `docs/license.rst` says
  each file's SPDX tag governs; the core is MIT. Linked into wcontainers-nir's tests: Mesa's static
  libraries `libzink`, `libglsl_standalone`, `libglsl`, `libglcpp`, `libglsl_util`,
  `libglcpp_standalone`, `libmesa`, `libvtn`, `libnir`, `libcompiler`, `libmesa_util`, `libblake3`
  and `libmesa_util_c11` (the linker keeps only the objects used). The files kept, listed again in
  G9 run 1 by `mesa/linked-licences` (scratch `results/g9r1/nir-linked-licences-g9r1.txt`): 242
  files. 196 carry the MIT permission text and 34 the SPDX tag MIT; 2 are generated by Mesa's
  build from MIT scripts (`nir_opcodes.c`, `nir_opt_algebraic.c`); 1 is BSL-1.0
  (`c11/impl/threads_posix.c`); 9 carry no licence text in the file: the 7 of `util/blake3`
  (BLAKE3 1.8.2 copied verbatim, its README says; upstream's licence is CC0-1.0 OR Apache-2.0 OR
  Apache-2.0 WITH LLVM-exception), `util/softfloat.c` (Berkeley SoftFloat 3e, BSD-3-Clause text
  in the file, which the script's MIT match does not count) and `util/format/u_format_etc.c` (MIT
  text in the header it includes, `texcompress_etc_tmp.h`). Against run 6's list (243 files) the
  one change is `nir_lower_fragcolor.c`, no longer linked (the bridge writes that step itself).
  Four of the MIT files are generated and carry a second notice: `glsl_parser.cpp` and
  `glcpp-parse.c` are Bison's output (Bison's skeleton is GPL-3.0-or-later with the Bison special
  exception, which lets the output be distributed under any terms), `glsl_lexer.cpp` and
  `glcpp-lex.c` flex's (flex's own BSD-style licence). All permissive. The Windows build
  (`mesa/build-windows.ps1`, G9) links nothing into anything of ours yet; G9 lists the driver's
  linked files when there is one.
- G9 run 2, wcontainers-opengl's dependencies: `pollster` 1 (Apache-2.0 OR MIT), `wgpu` 30.0.2
  (MIT OR Apache-2.0, the workspace's patched copy), `wcontainers-gpu` and `wcontainers-gpu-webgpu`
  (ours); build-dependencies `cc` (MIT OR Apache-2.0), `serde` and `serde_json` (MIT OR
  Apache-2.0). All already in the lock file.
- G9 run 3, wcontainers-opengl: `windows-sys` 0.61.2 (MIT OR Apache-2.0, already in the lock
  file, Windows only), `wcontainers-nir` (ours, without `standalone`: its bridge, route B and
  through it naga and wcontainers-shaders, as listed above), and the build-dependency `bindgen`
  0.71.1 (BSD-3-Clause, already in the lock file for unicorn-engine-sys) in place of `cc`.
  bindgen runs libclang at build time only (on the desktop, the LLVM 21.1.8 release the Mesa
  build already fetches, Apache-2.0 WITH LLVM-exception; on Linux Nixpkgs' clang 21.1.8);
  nothing of it is linked. The driver now links Gallium's `u_init_pipe_screen_caps`
  (`u_screen.c`, MIT) from Mesa's own library.
- G9 run 4, wcontainers-opengl: `naga` (the workspace's, MIT OR Apache-2.0, already a
  dependency of wcontainers-nir and the interface) as a direct dependency (the uniform blocks'
  declared sizes). The driver now also calls Mesa's `u_upload_mgr.c`, `u_blitter.c`,
  `u_framebuffer.c` and `util/format/u_format.c` (each MIT by its header) from Mesa's own
  library, which the WGL target links anyway; no new file of Mesa's is built.
- Testing only, on the Windows desktop (`crates/wcontainers-opengl/cts`, never linked into
  anything of ours): VK-GL-CTS `opengl-cts-4.6.8.1` (Apache-2.0, its `LICENSE`) with what its
  `fetch_sources.py` pins (glslang BSD-3-Clause and others, SPIRV-Tools and SPIRV-Headers
  Apache-2.0 and MIT, Amber Apache-2.0, jsoncpp MIT, zlib Zlib, libpng libpng-2.0, Vulkan-Docs and
  the video samples Apache-2.0 and MIT), and deqp-runner v0.23.3 (MIT, as 15.2 says; its own
  crates from crates.io, built by `cargo install --locked`).
- zlib (Zlib licence), linked as Mesa's build found it (Nixpkgs' static zlib).
- `serde_json` 1 and `serde` 1 (both MIT OR Apache-2.0), build- and dev-dependencies of
  wcontainers-nir; `cc` (MIT OR Apache-2.0), already in the workspace.
- `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception, already in the workspace), a
  dependency and build-dependency (the cache key and the converter's version).
- `wcontainers-gpu` (ours), a dependency (`SlotKind`, `ReflectedBinding`, `ShaderStage`).
- `glow` 0.18 (MIT OR Apache-2.0) and `khronos-egl` 6 with `dynamic` (MIT OR Apache-2.0),
  dev-dependencies since run 7 (llvmpipe in the corpus run); both already in the workspace through
  the WebGL back end.
- `ash` 0.38 (MIT OR Apache-2.0) and `libloading` 0.8 (ISC), dev-dependencies (lavapipe in the
  corpus run); both already in the lock file through wgpu.
- `spirv` 0.4 (Apache-2.0; the SPIR-V grammar's names, already in the workspace through naga), a
  dependency. `rand` 0.9 and `rand_chacha` 0.9 (MIT OR Apache-2.0, already in the workspace),
  dev-dependencies.
- `glam` 0.30.10 (MIT OR Apache-2.0), a dev-dependency since run 15 (new to the lock file): the
  GLSL comparison's seeded fixed-function matrices, their product and inverses.
- Corpus, test data only, fetched into the build folder and never in the repository or shipped:
  VK-GL-CTS at 5bb4c7e8 (since run 9; Apache-2.0, its `LICENSE`; only `data/gles2/shaders`,
  `data/gles3/shaders`, `data/gles31/shaders` and the licence are fetched by `corpus/fetch`;
  `corpus/cts-programs` fetches the whole tree and the dependencies its `fetch_sources.py` pins:
  glslang (BSD-3-Clause and others), SPIRV-Tools and SPIRV-Headers (Apache-2.0 and MIT), Amber
  (Apache-2.0), jsoncpp (MIT), Vulkan-Docs and the video parser (Apache-2.0 and MIT), built only to
  dump test programs, never linked into anything of ours);
  bgfx at e76ecedf (since run 8; BSD-2-Clause, its `LICENSE`; only `examples/runtime/shaders/glsl`
  and the licence are fetched);
  Godot at 73ca6be2 (since run 10; MIT, its `LICENSE.txt`; only `drivers/gles3/shaders`,
  `LICENSE.txt` and `COPYRIGHT.txt` are fetched; `COPYRIGHT.txt` names no other licence for those
  files, and `tonemap_inc.glsl` notes an ACES fit adapted from MJP's BakingLab, MIT);
  since run 11 also Godot's `servers/rendering/renderer_rd/shaders` (MIT) and the AMD FSR and FSR2
  code it includes (`thirdparty/amd-fsr`, `thirdparty/amd-fsr2`: Expat, that is MIT, by Godot's
  `COPYRIGHT.txt`; their own `license.txt` and `LICENSE.txt` fetched beside them), compiled by
  glslang (Nixpkgs' `glslang`, BSD-3-Clause and others; a tool run by `corpus/godot-forward-plus`,
  never linked);
  Sascha Willems' Vulkan samples at d2d49bf7 (MIT, `LICENSE.md`); piglit at 92510281 (since run 7;
  its `COPYING`: MIT for most of it, with some test shaders under their projects' own licences,
  e.g. GStreamer's in `glslparsertest`; piglit is for testing only, as the routed decision says,
  and never a dependency).
- Build tools only, on the Windows desktop (G9, `mesa/build-windows.ps1`, never linked into
  anything of ours; nothing installed, all under `C:\wc\graphics-nir\tools`): Python 3.13.1 from
  its NuGet package (PSF-2.0), Meson 1.12.1 (Apache-2.0), Ninja 1.13.0 (Apache-2.0), Mako 1.3.10
  (MIT), PyYAML 6.0.2 (MIT), packaging 25.0 (Apache-2.0 OR BSD-2-Clause), win_flex_bison 2.5.25
  (flex BSD-2-Clause; bison GPL-3.0-or-later, a generator whose output carries Bison's exception,
  as GCC's Bison on Linux), LLVM 21.1.8 for the Visual C++ runtime (Apache-2.0 WITH
  LLVM-exception; since G9 run 5 its release only gives bindgen's libclang and lldb, and
  llvmpipe links the same version built there from its source, the reference only; the
  release's `LLVMSupport.lib` holds rpmalloc, public domain or MIT, never linked now), Python
  3.10.11 from its NuGet package (PSF-2.0; only for lldb, which loads it, for debugging a
  case), and Visual Studio 2022 Build Tools, already on the desktop (Microsoft's licence).
- Build tools only, from the repository's locked nixpkgs: Meson 1.12.1 (Apache-2.0), Ninja (Apache-2.0),
  LLVM 21.1.8 (Apache-2.0 WITH LLVM-exception; llvmpipe and lavapipe use it, wcontainers-nir does not
  link it), GCC 16.2.0 (GPL-3.0 with the GCC runtime library exception).

## Departures from the plan

- G9 run 25: the stencil of a blit written a bit at a time takes the slow way without a counter
  or warning of its own, since the interface names no such slow path and this round makes no
  interface change; the name is asked of the interface track. Maps of multisampled textures
  through a single-sampled one are not counted as a slow path: no faster route exists on any
  back end, and only Mesa's own code makes such maps (OpenGL gives programs none).

- G9 run 17: a tessellation program's read of a control point at a constant index past the
  draw's input patch reads zero (route B, `missing_point_reads_zero`). OpenGL leaves the value
  undefined; the stage helper holds only the patch's own points, so the read cannot reach a
  real point, and without this the module is invalid. Fragment programs get storage buffers
  (the plan's screen gave them compute programs only), as `ARB_shader_storage_buffer_object`
  needs them and Mesa answers the tessellation stages' storage queries only with it.
- G9 run 16: every conformance list runs with `--deqp-terminate-on-device-lost=disable`, not
  the mustpass configuration alone. With glcts's default (on), its desktop packages call
  `glGetGraphicsResetStatus` after every case although they load it only from `GL_KHR_robustness`
  or OpenGL 4.5, so on a 3.3 context without it every case dies at its first frame. The option
  also stops glcts asking for a lose-context-on-reset context, which our driver cannot report
  either way. A lost device still ends its cases as crashes or failures; nothing is waived.
- G9 run 15: texel buffers take the interface's texture route (a copy per bind), not its storage
  route: the shaders crate's storage route decodes formats only inside its own DXBC and DXIL
  routes, and route B has no entry to it. A copy per bind is a counted slow path; a copy only
  when the buffer changed is not built.
- G9 run 15: the packed 10-10-10-2 vertex formats are unpacked in the vertex program by the
  driver (`packed.rs`), until the interface has them (Requests).
- G9 run 12: every storage buffer of a compute program is read-write in route B's module, so one
  buffer bound at a read-only and a written block of one dispatch is valid on WebGPU (OpenGL and
  Vulkan allow it). Its cost against read-only bindings is not measured.
- G9 run 12: a Mesa patch reports `OES_texture_storage_multisample_2d_array` only where the
  screen makes multisampled 2D arrays (st/mesa tied it to multisampled textures); it lives in
  `mesa/patches/` with Zink's until the fork 15.2 describes exists.
- G9 run 12: an integer cube map's gather through a program that also samples it (made a 2D
  array of faces) reads its face's edge texels past the edge, not the next face's; a program
  that only gathers keeps the cube.

- G9 run 11: indirect draws read their arguments back and draw as direct draws
  (`IndirectReadBack`), so every rewrite a direct draw takes (fans, restart, provoking vertex,
  stream output) applies; the direct host draw from the buffer is a later fast path for draws
  that need no rewrite.
- G9 run 11: storage buffers and images are reported in compute programs only (vertex and
  fragment report none, which OpenGL ES 3.1 allows): a fragment program's storage writes need
  the pass tracker told of each draw's storage uses, not built yet.
- G9 run 11: every single-sample colour texture whose format the back end can store to is made
  with storage use (OpenGL binds any texture as an image later and st/mesa names no image use at
  creation). Its cost on the RTX 3080 (a texture that may lose compression) is not measured; a
  texture made for storage only at its first image binding is the alternative.
- G9 run 11: an image of another format than its texture's, of the same texel size, is bound
  through a twin texture of its format, copied in through a buffer before the dispatch and back
  after it when written (`ReinterpretCopy`); WebGPU views change only between sRGB forms.

- G9 run 10: an integer cube map is sampled through a 2D array view of its faces, loaded at the
  texel OpenGL's cube map table and nearest rule pick, in a program variant keyed on the bound
  samplers (WebGPU loads no cube texel and gathers only the base level). Its level of detail comes
  from the face coordinates' derivatives, as OpenGL's does.
- G9 run 10: the driver ends the process when the back end reports its device lost; a context of
  OpenGL ES 3.0 without robustness has no way to tell a program, and going on would make every
  later call read nothing.
- G9 run 10: route B reads NIR's `fcanonicalize` as the value itself.

- G9 run 5: llvmpipe's LLVM on Windows is built from LLVM's source on the desktop rather than
  taken from LLVM's release (the plan names no source for it; the release's integrated allocator
  corrupted Mesa's heap, Where it stands). The Windows Mesa build keeps debug information
  (`debugoptimized`, assertions still off as in a release build), for the stacks of crashes; its
  speed is not what G9 measures.

- G9 run 1: the Windows build of Mesa is a script beside the Linux one
  (`crates/wcontainers-nir/mesa/build-windows.ps1`), sharing its pin and patches, not a step of
  the rig's setup (the rig's scripts are another track's). It fetches its own tools into
  `C:\wc\graphics-nir\tools` rather than installing them system-wide. llvmpipe is built with
  LLVM's own Windows release (static runtime), not an LLVM built here.

- G9 run 2: `wcontainers_screen_create` is in the crate's C bridge, not in a C file of Mesa's
  driver folder (15.2): the folder holds only its meson file and the public header, so every line
  of the driver's code is in this repository. The library reaches Mesa's link through one meson
  option, `wcontainers-link-args`, set after cargo builds it (the native libraries are known only
  then), not through a path Mesa's meson file computes.
- G9 run 2: each list runs with the first configuration its `mustpass.xml` gives it (for the
  KHR lists, 64 by 64 and seed 1; the dEQP lists have one). The KHR lists' other configurations
  (other sizes and seeds, FBO surfaces) are not run: 15.8's measure names the lists, and gex_wgpu's
  112,212 GLES cases are one configuration of each.
- G9 run 2: the pinned CTS is unpatched (Mesa's own CI patches its copy); a case that fails by a
  CTS fault is listed as such with its reason, not excluded.

- 6.4 states the binding rule for Direct3D. For Vulkan and OpenGL programs the NIR route reads it as
  Direct3D 12's: within the stage's set each kind's resources in the order of the program's own
  (set, binding), with the set in `ReflectedBinding::space` and the binding in `register`. A combined
  image sampler is a texture and a sampler reflected at the same set and binding.
- The NIR the routes take is fixed by one set of NIR compiler options in the bridge (Zink's list plus
  all 64-bit integer lowering and the instance index counting from the first instance), which 15.3
  does not name; G9 and G10 must hand over NIR made with the same options.
- The corpus is fetched by `corpus/fetch` into the build folder on the rented machine (each source
  pinned by commit), not kept in the Mac's scratch (15.8): the runs happen there, and the pins make
  it the same corpus everywhere.
- Mesa is not a git submodule at `third_party/mesa` (15.2). It is pinned by commit in
  `crates/wcontainers-nir/mesa/pin.toml` and fetched by the build script. Reasons: the literal check
  scans every Rust file and script of a submodule (Mesa has hundreds of Python scripts and Rust files),
  any new baseline or policy line is refused as growth, and an unchecked-out submodule is itself a
  finding; and where the fork 15.2 describes is kept is the owner's decision (15.9). Until then the
  patches live as files in `crates/wcontainers-nir/mesa/patches/` (one so far, Zink's), applied by
  the build script; they move into the fork when there is one.
- The Mesa build runs on the rented x86-64 machine, not the laptop (15.8 says laptop): the brief of
  2026-11-19 makes the rented machines the default. The choice's times (15.3: "measured on the
  laptop") are taken there too, alone on ten cores, as that machine's notes take timings.
- The choice was made on the SPIR-V corpus first (run 4) and made final over the whole corpus,
  GLSL and the other sources in, by `corpus/choose` over all 18,222 items (run 11's, read in run 13).
- Zink is built beside llvmpipe and lavapipe: its `nir_to_spirv` is candidate A's first step.
  SPIRV-Tools is off (Mesa takes it when present; nothing of ours needs it).
- Candidate B reads NIR through a flattened copy the bridge makes (`bridge/flat.c`), not through
  NIR's own structures from Rust: bindgen is out (below), and the copy keeps Mesa's numbering and
  layout on the C side, so a Mesa update changes C only.
- wcontainers-opengl does use bindgen, for Gallium's headers (G9 run 3), as 15.2 says: Gallium's
  structs are what the driver fills, and copying them by hand would define their layout twice. The
  literal check's objection (below) is met by reading `OUT_DIR` inline in `env::var_os`, which
  the check accepts, and by handing the bindings' path to the crate through its own variable.
- wcontainers-nir's bridge-only build for the driver is the absence of a default feature,
  `standalone`, not a feature of its own: Cargo unifies features, so a `driver` feature would have
  taken the SPIR-V steps and the GLSL front away from the corpus and tests whenever both crates
  are built together; an additive `standalone` cannot. Its tests and the corpus run require it.
- The bridge's functions are declared by hand in `src/mesa.rs` and `src/direct.rs`, not by bindgen: bindgen's output
  is included from Cargo's `OUT_DIR`, whose name the literal check refuses as a new code string here
  (its one constant, `CARGO_OUT_DIR`, is private to wcontainers-literals).

- Lowered IO keeps a stage value's interpolation only where a fragment input is loaded. Route B
  gives a vertex output Perspective (Flat for an integer one); where a fragment program reads it
  another way, the pipeline (G9, G10) must give the vertex side the fragment side's. The plan does
  not say; the comparison runs each stage alone, so it does not see this.
- The 16-bit test's reference is lavapipe running the same program with native 16-bit integers and
  Rust's 16-bit arithmetic, both, rather than a corpus mode (the program is the test's own).
- The route's module is run by lavapipe through naga's SPIR-V writer, not by the recording back
  end's interpreter (`wcontainers-gpu-test`) or the WebGPU back end on the RTX 3080 (15.3). Reasons:
  the interpreter's limits (no derivatives, implicit levels only on one-level views, no atomics or
  workgroup memory) would leave most real fragment programs to the GPU queue, while lavapipe runs
  every program the reference runs, on the same machine, with one harness for both sides. What this
  holds is the conversion (NIR to the common form); naga's SPIR-V writer is the same one wgpu's
  Vulkan back end uses. The WebGPU back end's own conversion is the shaders track's.
- The plan says "within the format's rounding" for floats; the targets are 32-bit, and two programs
  that differ only in the order of operations differ by more than one rounding. The tolerance is
  1024 units in the last place or 2^-20 absolute (`compare.rs`), stated and checked to still catch a
  change of constants (Where it stands).
- GLSL is compiled on Mesa's standalone scaffolding with st/mesa's steps after a link written out
  in `glsl.cpp` for one set of capabilities, not by st/mesa itself (which needs a gallium screen
  and context: G9's driver). The steps and their order are st/mesa's; the shader-variant steps
  st/mesa takes at draw time (clamped colours, flat shading, two-sided colour, user clip planes,
  point size), which depend on the draw's state, are not taken here: G9 hands its variants over
  and its own corpus run holds them.
- OpenGL's buffer accesses reach route B as arrays of words, not as typed blocks: after
  `gl_nir_lower_buffers` only byte offsets remain (the block types are gone from the
  instructions), so the bridge makes the words form, the one 15.4 names for robustBufferAccess.
  Program binding for OpenGL: uniform block index, storage block index after them.
- Vulkan capability classes: a program needing what our Vulkan 1.1 will not report is counted apart,
  not as a route failure (the pass bar's "named reason").

- llvmpipe is reached through EGL's software device (`EGL_EXT_platform_device`), not the surfaceless
  platform 15.2's options name: the rented machine has no render node, and surfaceless takes software
  only when told by an environment variable (`LIBGL_ALWAYS_SOFTWARE`), which a test process should
  not set for itself. The context is OpenGL 4.5 compatibility, llvmpipe's; the bridge reports 4.6
  (G9's), so a program needing 4.6 is refused by the reference and counted not compared.
- The GLSL comparison fills its inputs from OpenGL's reflection of the program on llvmpipe (names,
  types, offsets, strides) and the bridge's names for where each lands in the route's module; the
  SPIR-V comparison fills from the route's module alone.
- GLSL vertex and fragment programs are compared as a drawn program (the vertex program wrapped in
  its own source, a feeder or catcher written beside it), not each stage alone: names between two
  stages do not survive OpenGL's linker, and the draw is how llvmpipe runs a stage. In a vertex
  and fragment pair, what the vertex program computes is compared through what the fragment
  program makes of it; its position is compared only when it is alone (the catcher).
- The corpus's `sources` lines gained a paths column (`.` for the whole tree): bgfx is fetched
  sparsely (its shaders and licence only), as dEQP's data files will be.
- bgfx's 3 differences (infinities from seeded RGBE texels, and one unit in the last place made
  large by `fract`) are left as differences with their reasons, as piglit's 17 are.
- dEQP's cases are run on the corpus's seeded inputs, not on the values in each case's `values`
  block: the corpus holds the route to llvmpipe, not to dEQP's expected results, and seeded inputs
  reach more of each program. Where dEQP compares the outputs in the shader, the corpus writes them
  to colour targets (Done). Of a requirement's alternative extensions the first is taken (dEQP
  takes the first the context reports); a case whose first alternative Mesa's compiler refuses is
  counted as reaching no route with Mesa's log.
- ES programs are compared at full precision on both sides (`#define mediump highp` and the rest):
  llvmpipe lowers `mediump` to 16 bits, our driver does not; both are allowed by GLSL ES, and the
  reference must compute what the route is to compute.
- The 17 differences from undefined behaviour on seeded inputs are left as differences with their
  reason, not hidden: the seed gives indices out of range and negative square roots, which the
  programs' own tests never give.

- Godot's Compatibility shaders are made for desktop OpenGL (`#version 330`, what Godot gives a
  desktop driver; our OpenGL reports 4.6 compatibility), at Godot's default project settings, with
  each mode at the default specializations and with each one specialization flipped, not every
  combination (scene alone has 33 specializations: 2^33 programs, most never built by Godot).
  Combinations Godot never builds that Mesa refuses are counted as reaching no route with Mesa's
  log.
- A vertex program without a position gets (0, 0, 0, -1) in the bridge (NIR), not through the
  shaders crate's `stage::clip_unpositioned` (crate-private, and on naga); the same rule, the same
  value. It moves to the shaders crate's entry point when there is one (Requests).

- Godot's RenderingDevice variants are every variant each `initialize` call lists, at Godot's
  default desktop settings, with no material (as the Compatibility shaders are taken), compiled for
  a Vulkan 1.1 device (SPIR-V 1.3); Godot would compile only the groups a device and project
  enable. Texture streaming (a module the default build leaves out) adds nothing to the scene
  defines.
- The GLSL comment marker Godot's reader cuts is named `GLSL_COMMENT_START`, not imported from the
  literal check's `LINE_COMMENT` (Rust's, private there): the same two characters, another
  language's fact.
- Subgroup operations (`reduce` and the like) are refused by name by route B: 15.4 has the Vulkan
  driver (G10) lower them to their one-invocation meaning before the route, so the corpus, which
  hands over NIR as spirv_to_nir makes it, counts them as failures until G10's step exists.

- A storage cube is a 2D array of its faces in the common form (run 13): the plan does not say;
  WGSL has no storage cubes, and Vulkan and OpenGL address a storage cube's faces as layers.
- A reference that does not end on the seeded inputs is held on the next seeds, up to four (run
  13), rather than counted not compared: the program is the same and the inputs are as seeded as
  before; the seed used is in the results.
- Dolphin is left out of G8's corpus (run 13): no permissively licensed boot image was found, as
  the orchestrator's ruling of 2026-11-19 night requires; the choice is final without it.
- `direct::rewrite` takes only 15.4's part of a variant key (storage formats, texels through a
  buffer) and refuses the rest by name (run 14): the other rewrites read the program's stage
  values, which the driver's reflection holds (G9, G10); it gives the bindings, not a whole
  `Reflection`, so nothing reflected is left empty and passed on as if filled.
- A storage image without a format is declared `r32float`, `r32sint` or `r32uint` by its texel's
  kind until the variant names the bound view's format (run 14): the shaders crate's contract
  asks for any format of the texel's kind.

- G9 run 9: a blend reading the blend constant's colour and its alpha, on a back end without
  constant alpha factors, gives the alpha as the blend's second source from the fragment program
  and counts it as `BlendInShader`, whose description in the interface is blending that reads a
  copy of the target. It reads no copy: the blend itself stays the back end's; only one factor's
  value comes from the program. The interface has no closer name and this track adds none
  (routed decision 4); asked under Requests.
- G9 run 9: Mesa's blitter's viewport, which maps depth -1..1 to itself, is read as the
  zero-to-one viewport (`viewport` in draw.rs): exact for the blitter's own depths, which are 0
  or above, and OpenGL itself clamps every depth range it is given to 0..1, so no program's draw
  meets it.
- G9 run 9: integer textures are sampled through their nearest texel's load, made in NIR from
  the bound sampler's wraps and level bounds, which the program variant is keyed on (one variant
  per such sampler state). Cube maps and the border and mirrored-clamp wraps are refused by name
  until built.

## Interface changes

None.

## Requests to other tracks

- Shaders track (G9 run 31): the stage helper's pass-through vertex program
  (`stage::passthrough`) takes a geometry or evaluation program's every passed value as a vertex
  input beside the position; the host has 30 (wgpu's Direct3D 12: 32 input slots less vertex and
  instance index), so an OpenGL driver can report at most 29 passed values there (28 with points'
  `gl_PointCoord`), under OpenGL 3.2's least of 32 (128 components) for geometry outputs and
  fragment inputs. Asked: the pass-through reading its records from a storage binding by the
  vertex index (as pulled vertex input reads streams) where the values pass the host's vertex
  inputs, so that passed values are bounded by the inter-stage limit only. Read from KHR-GL33
  `transform_feedback.discard_geometry_test` (30 values) and KHR `geometry_shader.limits
  .max_output_components`, which lost the device at 31.
- WebGPU back end track (G9 run 31): `fill.rs` `quantise` (`PackedTargetQuantised`) makes a view of
  dimension `D2` of the target texture also when it is 3D, then sets `depth_slice` on its
  attachment; WebGPU makes no 2D view of a 3D texture, so the device is lost. Read from dEQP-GLES3
  `fbo.color.tex3d.rgb565` and `.rgb5_a1` (a 3D texture of a packed 16-bit format drawn into: the
  only packed formats of that group, the others pass). Asked: a `D3` view for a 3D texture (its
  `base_array_layer` 0, `depth_slice` as now); 2 device losses.
- Shaders track (G9 run 28): the carried naga's HLSL writer writes `MathFunction::Sign` as HLSL's
  `sign`, which returns `int` (or `intN`) for a float argument; a float `sign` whose bits are then
  taken (`asuint(sign(x))`, as a storage write or an integer target's output inlines it) gives
  `0x00000001` for 1.0. Read from dEQP-GLES31 `common.sign.float_*_tess_control` (`sign(1.0)`
  written as `0x00000001`); the same cause fits every other failing `sign` case of the table
  (dEQP-GLES3 `common.sign.vec*_fragment` 9 and `precision.sign.*_fragment` 9, dEQP-GLES31
  `common.sign.float_*_compute` 3 and `precision.sign.*_compute.scalar` 3) and `precision
  .normalize.*_compute.scalar` 3 (GLSL's `normalize` of a scalar is `sign`). Asked: write a float
  `sign` in the argument's float type (`float(sign(x))`, `floatN(sign(x))`); 30 cases.
- Shaders track (G9 run 28): an evaluation program writing storage, without a geometry program
  after it, is drawn as a vertex program, where the WebGPU back end writes no storage; so the
  driver reports no storage blocks or images for evaluation programs (KHR-GLES32
  `constant_expressions`' 232 evaluation cases NotSupported). `Stages::tessellated_with` already
  runs a domain program as compute without a geometry stage when it writes cull distances (the
  cull route, `domain_cull_programs`). Asked: the same route for a domain program writing storage
  (`writes_storage` of a domain module), as `stored_programs` is for a vertex program, under
  `SlowPath::VertexStorageAsCompute`. The driver then binds its storage as it does a geometry
  program's and reports it.

- WebGPU back end track (G9 run 27): `Rgb32Float`, `Rgb32Uint` and `Rgb32Sint` are held in
  four-channel textures, which the host renders into, but are reported without a render use, so
  st/mesa holds `GL_RGB32F` (which it asks a render use of) as `R32G32B32A32_FLOAT` while
  `GL_RGB32I` and `_UI` are `R32G32B32_*`: OpenGL's copies between them (one 96-bit view class)
  then meet textures of 16- and 12-byte texels, which no interface copy joins (dEQP-GLES31
  `copy_image.non_compressed.viewclass_96_bits` rgb32f to and from rgb32i and rgb32ui, 32
  crashes). Asked: the render use for the three formats where their held format has it (the
  copies are then of equal formats), or a copy between a three-channel format and its held
  four-channel one.
- Shaders track (G9 run 26): the multisampled array split (`variant/layers.rs`) adds a binding for
  every layer of `VariantKey::texture_layers` whatever the program does with the array; a program
  that only queries its size or layer count needs layer 0 alone, and one that loads needs only the
  layers it can reach. As it is, an array of more layers than the stage's free texture bindings
  is refused ("BindingsBeyondLimit"; dEQP-GLES31 `builtin_functions.texture_size.samples_*
  _texture_*2d_array` 6, which query only sizes). Asked: bind the layers only where a load reads
  them.
- Shaders track (G9 run 25): a staged built-in for the point size (`staged::BuiltIn` has none),
  passed between the stages the stage helper runs and from the vertex stage into them, and
  dropped at its final draw: a tessellation or geometry program reading `gl_in[].gl_PointSize`
  or writing `gl_PointSize` is refused ("not taken: a Evaluation program's input of the built-in
  PointSize", 144 process refusals in run 23, KHR-GLES31 and -GLES32 `tessellation_shader`).
  Placing it at a location of this route's own would need the vertex stage to write it there too
  and a location the device's inter-stage limit leaves free, which only the helper knows. Since
  run 30 wide points after tessellation or from a geometry program no longer need it (drawn as
  squares by this driver); programs reading `gl_in[].gl_PointSize` still do.
- Interface track (G9 run 25): a slow path name for a blit's stencil written by one draw per bit
  and sample (`util_blitter_stencil_fallback`, eight draws a sample where a program could write
  stencil, which WebGPU's language cannot), e.g. `SlowPath::StencilWrittenByBits`; the driver
  takes that route since run 25 without a counter of its own.

- Shaders track (G9 run 24): the stage helper sizes a geometry stage's output after tessellation
  for the largest factor of every patch (`patch_bounds(h.tessellation.max_factor)`, 64 where the
  program declares no maximum, as GLSL never does), so a draw of a few hundred patches asks for a
  buffer larger than `max_buffer_size` ("buffer larger than max_buffer_size", 56 process
  refusals): dEQP-GLES31 `primitive_bounding_box` 28 crashes (vertex, tessellation, geometry and
  fragment programs; 288 patches of lines, a geometry program of 5 vertices at most) and
  `geometry_shading.input.triangle_strip_adjacency` 3. Asked: bound the output by the levels the
  draw's patches give (the tessellator counts them first), or run such a draw in parts of
  patches that fit.
- Shaders track (G9 run 24): the stage helper lists the odd triangles of a triangle strip with
  adjacency in Direct3D's order, the first vertex kept and the other two swapped (`strips.wgsl`,
  `adjacent_place`: places i, i+3, i+4, i+6, i+2, i-2 for the triangle at strip place i); OpenGL
  gives a geometry program those triangles with the first two swapped instead (OpenGL 4.6, table
  10.1 "Triangles generated by triangle strips with adjacency": 2j+2, 2j-2, 2j, 2j+3, 2j+4,
  2j+6 for odd triangle j, that is i+2, i-2, i, i+3, i+4, i+6). The same triangle and winding,
  but `gl_in[]` in another order: dEQP-GLES31 `geometry_shading.input.triangle_strip_adjacency`
  fails from two triangles on (vertex counts 8 to 12, one triangle passes) and KHR
  `geometry_shader.adjacency.adjacency_*_triangle_strip`. Asked: OpenGL's order where the API is
  OpenGL (the helper knows the API's conventions already through `provoking`).
- Shaders track (G9 run 24): the stage helper draws a geometry program's records through vertex
  attributes, one a location and one for the position (`record_layout(words, slots)`), so a
  program writing as many locations as the screen reports for it (the inter-stage slots) needs
  one attribute more than the device has: "Vertex attribute location 30 must be less than limit 30"
  at `create_render_pipeline`, which loses the device (KHR-GL33
  `transform_feedback.capture_geometry_interleaved_test` and `query_geometry_interleaved_test`).
  Reporting fewer geometry outputs would put OpenGL 3.3's 128 output components out of reach.
  Asked: a pass-through that reads the records from storage by the vertex index where they need
  more attributes than the device has.
- WebGPU back end (G9 run 23, a finding, not yet traced to the object): wgpu refuses a submit
  because a texture a recorded pass uses "has been destroyed" (KHR-GLES3 `packed_depth_stencil`
  blit and renderbuffers, KHR-GL33 `texture_swizzle`: 20 lost processes in run 22). The
  interface's `destroy` keeps objects recorded work uses alive until it is done; the back end
  defers a texture's destruction only where `used_in` marks it, so some use is not marked (a
  view without its texture, or a use recorded before the pass begins). Until it is found the
  device is lost in those cases.
- Shaders track (G9 run 23): `rewrite` validates a fragment program with `early_fragment_tests`
  without `Capabilities::EARLY_DEPTH_TEST` ("Capability EARLY_DEPTH_TEST is not supported", 12
  process refusals in dEQP-GLES31's `image_load_store.early_fragment_tests`); where the back end
  lacks forced early tests (browsers' WebGPU), the refusal belongs to the back end's caps.

- WebGPU back end (G9 run 17): a block-compressed texture whose first level is not a whole number
  of blocks. The interface's `TextureDesc` allows any size, and OpenGL makes such textures (a
  7 by 7 `GL_COMPRESSED_RG_RGTC2`), but `create_texture` hands it to wgpu, which refuses it
  ("Width 7 is not a multiple of Bc5RgUnorm's block width (4)") and loses the device: KHR-GL33's
  packed_pixels has 198 such crashes. The back end already holds BC textures decoded with their
  blocks on the host (`blocks.rs`, `BlockDecoded`) on a device without `texture-compression-bc`;
  asked: take the same path for a texture whose level 0 is not a whole number of blocks, so
  that no front writes a second decoder. Until then the driver's textures of that shape fail.
- Interface track (G9 run 12): the 10-10-10-2 vertex formats OpenGL ES 3.0 requires that
  `VertexFormat` lacks: signed normalized, and signed and unsigned integers read as floats
  (Gallium's R10G10B10A2_SNORM, _SSCALED, _USCALED). Since G9 run 15 the driver unpacks them in
  the program (`VertexFormatConverted`); native formats would remove that.
- Shaders track (G9 run 18; DELIVERED as `Stages::provoking` in its run 108, used since G9 run
  19): flat values from the last vertex of each primitive a geometry or
  tessellation stage makes. OpenGL's provoking vertex is the last unless the program asks for the
  first (`glProvokingVertex`), the host's the first; the driver rewrites its own draws' indices,
  but the stage helper's final draw is made inside the helper. Asked: a choice of the provoking
  vertex for the helper's final draw (each triangle it lists rotated to start at its last vertex,
  which keeps its winding; each line reversed), so that no front copies the helper's records.
  dEQP-GLES31 has 122 process refusals of this ("flat values from the last vertex ...").
- Shaders track (G9 run 15; DELIVERED as `wcontainers_shaders::typed` in its run 104, used by route
  B for images of buffers since G9 run 19): an entry that takes a common-form module's typed buffers (a global named as one,
  with its format from the variant) through `variant::texel`'s storage route, so route B's texel
  buffers need no copy into a texture.
- WebGPU back end (G9 run 12, a finding): a multisampled resolve of an integer format reaches
  wgpu and loses the device ("R16Uint is not resolvable"); the driver now checks
  `FormatUse::RESOLVE` first, as every front must.

- Shaders track (G9 run 12; DELIVERED as `wcontainers_shaders::staged` in its runs 88 to 99, used
  by route B's geometry form since G9 run 13, its hull and domain forms since G9 run 17; its
  17-byte pulled inputs lifted this driver's refusal in G9 run 14): a public entry point for the
  stage helper's geometry and tessellation forms taken from a naga module, as `lower/geometry.rs`
  and `lower/hull.rs` already make them for the DXBC and DXIL routes (`pub(crate)` today, and fed
  from dxbc-spirv's IR). The NIR route would hand over the program as a naga module whose entry
  point calls placeholder emit and cut functions (geometry) or writes its patch values (hull),
  with its declarations (input and output primitive, most vertices, instances, streams; control
  points, domain, partitioning) and its stage values, and take back the `Converted` that
  `stage::Stages::new` and `Stages::tessellated` read. Without it OpenGL ES 3.2 and desktop
  OpenGL 3.2 and later cannot be reported (both require geometry shaders; 4.0 and ES 3.2
  tessellation), so KHR-GLES32 (1,405 cases), KHR-GL33 (9,886) and KHR-GL46 (19,714) run nothing
  and the bar's 92.9% cannot be met: with every other case of the six lists that start passing,
  the measure would be at most 88.5% (86,617 of 97,908). Writing the layout's numbers again in
  this track (the helper's bindings and value words are `pub(crate)` there) would define them
  twice, which the literal rule forbids.

- WebGPU back end (G9 run 11, a finding): its format caps give `STORAGE_WRITE` (from wgpu's
  `STORAGE_WRITE_ONLY`) for `Depth16Unorm` and `Depth32Float` on the RTX 3080, and a texture of
  those formats made with `TextureUsage::STORAGE` loses the device at its first use ("device lost:
  Reset"; all 288 depth16 and depth32f cases of dEQP-GLES3 texture.shadow, which pass without the
  use). The driver now makes no depth texture for storage; the caps should not report it.

- Shaders track (G9 run 10, a finding): naga's HLSL writer writes every constant texture offset
  as `int2(...)` (`back/hlsl/writer.rs`, the DXC workaround), so a 3D texture's `int3` offset
  becomes `int2(int3(...))`, which DXC refuses ("too many elements in vector initialization").
  40 logs of dEQP-GLES3 shaders.texture_functions; Direct3D 11's 3D textures with offsets reach
  the same writer.
- WebGPU back end (G9 run 10, findings): `create_sampler` clamps the largest level of detail to
  wgpu's 32 and not the smallest, so a sampler with a minimum over 32 is invalid (the driver now
  clamps both itself); `write_texture` of part of a stencil aspect, or of 16-bit depth, goes
  straight to wgpu, which refuses it (the driver now writes such parts through
  `copy_buffer_to_texture`); `copy_buffer_to_texture` of part of a 16-bit depth level still
  fails ("part of a 16-bit depth level written from a buffer").

- Interface track (G9 run 9): a slow path for a blend factor's value given by the program as the
  second source (the OpenGL driver's constant colour and alpha in one block on WebGPU), apart
  from `BlendInShader`, which names blending that reads a copy of the target. Counted as
  `BlendInShader` until then (Departures).
- Literals track (G9 run 9): the check reads the inside of `macro_rules!` bodies as code, so the
  driver's `unbuilt!` stub's `extern "C"` and its `eprintln!` text are findings (3); an ABI string
  and a message inside a macro body are what they are outside one.

- Shaders track (G9 run 7): the pulled-input fetch (`variant/pull.rs`, `fetch`) reads each
  component from one word at `address >> 2`, so a component that does not start at a multiple of
  its size (a float at byte 17) reads the wrong bits. Direct3D's alignment rules never make one;
  OpenGL allows any stride and offset (dEQP-GLES2's `multiple_attributes.stride.*_17`). A
  component across two words needs two loads joined by shifts. Until then the OpenGL driver
  refuses such an input by name.
- Interface track (G9 run 7): the alignment a back end needs of vertex strides, attribute offsets
  and stream offsets (WebGPU: 4, and an attribute's offset to its size up to 4, within the
  stride) as a limit or a documented rule of `VertexLayoutDesc`. The OpenGL driver takes
  `wgpu::VERTEX_ALIGNMENT` for it today, which is WebGPU's rule applied to every back end.
- WebGPU back end (G9 run 6; the driver no longer depends on it since run 7, which fetches such
  streams itself): `create_vertex_layout` accepts a stream whose stride is not a
  multiple of 4 (OpenGL allows any stride: dEQP-GLES2's `vertex_arrays` cases with strides 2 and
  17), and wgpu then refuses the pipeline ("Vertex buffer 0 stride 2 does not respect
  VERTEX_ALIGNMENT"), which fails the whole submit and every case after it in the process. The
  back end should take such a stream by a counted slow path (repacked, or fetched in the program)
  or refuse the layout so that a front can. (Run 5's request about 2D views of 3D textures is
  withdrawn: it was this driver's misread view target, fixed in run 6.)

- Literals track (G9 run 3): a lawful form for a build script's output folder. The check accepts
  `env!("OUT_DIR")` but not `env::var_os("OUT_DIR")`, the only way a build script can read it,
  and its own constant for the name (`CARGO_OUT_DIR`) is private; and `include!` takes only a
  literal, so the file a build script writes there is named twice. Either accept those two
  positions or export the constant; until then the two findings stay (Literal check).
- Interface track (15.7): everything 15.7 lists; G8 needs `HelperValue::BufferRange` and the slow path
  names `RobustAccessInShader` and `AlphaToOneInShader` before its robustness and alphaToOne parts can
  be reflected.
- Shaders track (a finding): naga's SPIR-V reader fails on HLSL's row-major array of matrices once
  it is decorated (`shadowmappingcascade/scene.frag`, "invalid access As" on candidate A); not needed
  by route B.
- Shaders track (run 14): `wcontainers_shaders::rewrite` validates without
  `Capabilities::IMMEDIATES`, so a module with push constants is refused ("rewrite: invalid: ...
  IMMEDIATES is not supported"): 20 of Godot's RenderingDevice programs with arrays of textures
  (`gi.glsl` and kin). The same as the `capabilities` request below, for `rewrite`'s validation.
- Shaders track (run 14, information): `rewrite` recomputes `sampled_pairs` only when it splits
  an array; a module with none keeps the pairs given. The NIR route gives none (its driver makes
  the reflection), so a public `sampled_pairs` of a module, or `rewrite` always recomputing them,
  would let G9 take them from one place.
- Interface track: `pub const fn stage_set(stage: ShaderStage) -> u32` in `reflect.rs` beside
  `helper_binding` (vertex and compute 0, fragment 1, plan 6.4). The shaders crate writes the rule as
  a literal in `lower.rs`; the NIR route defines `wcontainers_nir::bind::stage_set` until the
  interface has it, and will import it then.
- Shaders track: `stage::clip_unpositioned` as part of that entry point, so the NIR route gives a
  vertex program without a position the same position from one place (the bridge does it in NIR
  until then).
- Shaders track: the draw's bases rewrite (`HelperValue::DrawBase`) as part of the public entry point
  below, for Vulkan programs reading `BaseVertex` or `BaseInstance`.
- Shaders track: `wcontainers_shaders::capabilities` to add `Capabilities::IMMEDIATES` for a module
  with an `AddressSpace::Immediate` global, as it adds the other extensions a front asks for only
  where the back end has them; until then the NIR route adds it itself.
- Shaders track (a finding, not a request for this route): naga's SPIR-V reader turns a sampled
  array layer into an integer with a truncating `As` (`front/spv/image.rs`), where Vulkan and
  Direct3D round to the nearest even integer; a layer of 1.6 reads layer 1 instead of 2. The NIR
  route rounds in NIR first; dxil-spirv's and dxbc-spirv's output reaches the same reader.
- Interface or shaders track (later, when the Vulkan driver reports `vkCmdDispatchBase`): a helper
  value for the base workgroup; the routes now count dispatches from workgroup 0.
- Interface track: a helper value for the view index (e.g. `HelperValue::ViewIndex`, u32, written
  by the front for each view of a `MultiviewSerial` draw), so a program reading `gl_ViewIndex`
  (Vulkan 1.1 requires `multiview`) has it in the common form; route B refuses `load_view_index`
  by name until then (66 of Godot's RenderingDevice programs).
- Interface track: a public `CUBE_FACES` (the layers of one cube) beside `ViewDimension`;
  `resource.rs` writes it as a literal, and `wcontainers_nir::CUBE_FACES` defines it until the
  interface has it, and will import it then. Since the Direct3D 11 track's 7e4d8df6 its device
  defines one too (`pub(crate)`), and the literal check, on a tree holding both, reports the NIR
  crate's as a redefinition (run 15): neither front nor this crate can import the other's, so the
  interface's is the one place for it.
- Shaders track (a finding of run 15, from G8's second check): naga's SPIR-V reader maps
  `OpFUnordNotEqual` to `BinaryOperator::NotEqual` (`front/spv/convert.rs`), and naga's SPIR-V
  writer emits `NotEqual` on floats as `OpFOrdNotEqual` (`back/spv/block.rs`), so an unordered
  not-equal (true on NaN) becomes ordered (false on NaN) on the way through naga. Route B no longer
  uses `NotEqual` for NIR's `fneu`; dxil-spirv's and dxbc-spirv's output reaches the same reader
  (HLSL's `!=` and `isnan` on floats are unordered).
- Literals track: `CARGO_OUT_DIR` public (in a public module), so a build script can name `OUT_DIR`
  through it.

## Literal check

G9 run 20: exit 1 at 9901c6eb (x86-64 machine, from a git bundle of agents; the clone, its
build and the bundle deleted after; output in scratch `logs/g9r20-literals.out`). In
wcontainers-nir and wcontainers-opengl, 13: this run's two (the refusals of `image_buffer`, plain
strings) and the patches and pass-through refusals of earlier runs, all four made messages the
check reads in 2524183b (not run again after); left: the 7 of run 12 (`unbuilt!`'s three, the
two `OUT_DIR`-like ones, `CUBE_FACES` restating the interface's, which is not public) and
`GEOMETRY_OUTPUT_VERTICES` restating another crate's 256.

G9 run 19: exit 1 at 31ede8c6 (x86-64 machine, from a git bundle; 418 findings in the whole
workspace, most of them other crates'; the clone, its build and the bundle deleted after; output
in scratch `logs/g9r19-literals.out`). In wcontainers-nir and wcontainers-opengl, 15: the 7 of run
12, this run's one (the refusal in `bound_image`, returned through `to_string`: made a plain
message in 5b44e71d), and 7 that earlier runs of this batch left. Of those, 4 are fixed in the
commit after it (not run again after): `staged_value`'s "output" and "input" (two messages now),
`TessellatedKey`'s 4 and `run_tessellated`'s two 3s (from `TESSELLATED`, the stages before a
geometry program). Left: the refusals "patches from indices in memory or of 8 bits" (draw.rs) and
"a pass-through without a position" (position.rs), not yet read why the check takes them, and
`GEOMETRY_OUTPUT_VERTICES` restating another crate's 256 (screen.rs).

G9 run 12: exit 1 at 317f65fe with b7e3bb83's change (x86-64 machine, from a git bundle; marks 0; the clone, its build
and the bundle deleted after; output in scratch `logs/g9r12-literals.out`): the 7 findings of run
10 (`unbuilt!`'s three, the two `OUT_DIR` ones, `CUBE_FACES` twice). The run's one new finding,
the volume texture refusal (it held "3D" and "2D"), was reworded and checked again. Nothing new
since in the files the check reads but numbers derived or Gallium's. wcontainers-nir and
wcontainers-opengl have no baseline lines.

G9 run 11: exit 1 at f6fe1681 (x86-64 machine, from a git bundle; marks 0; the clone, its build
and the bundle deleted after; output in scratch `logs/g9r11-literals.out`): 8 findings, the 7 of
run 10 and one new, the image refusal "one slice of a 3D texture as a 2D image" returned as a
plain string, made a message the check reads in b4792714 (not run again after). The run's new
numbers are derived (the indirect arguments' sizes from OpenGL's command layouts, the grid's
dimensions from the interface's workgroup size) or Gallium's constants. wcontainers-nir and
wcontainers-opengl have no baseline lines.

G9 run 10: exit 1 at 9a5baaf7 (x86-64 machine, from a git bundle; marks 0; the clone, its build
and the bundle deleted after; output in scratch `logs/g9r10-literals.out`): the same 7 findings
as run 9, none new (`unbuilt!`'s three, the two `OUT_DIR` ones, `CUBE_FACES` twice).
wcontainers-nir and wcontainers-opengl have no baseline lines.

G9 run 9: exit 1 at cd5162cf, the run's last code commit (x86-64 machine, from a git bundle;
marks 0; the clone, its build and the bundle deleted after; output in scratch
`logs/g9r9-literals-2.out`, a first pass at 207e7587 in `logs/g9r9-literals.out`): 7 findings,
from 47 at run 8. Fixed this run: the
driver's refusal texts are now messages the check reads (`refuse` takes any `Display`, given
`format_args!`; `refused` returns one as an error), the colour arrays' 4 is `COLOR_CHANNELS`
(derived from Gallium's `pipe_blend_color`), the copy's entry points are named, its vertex count
is the shaders crate's `TRIANGLE_VERTICES`, and `QUERIES_PER_SET` (which shared a word and a value
with the Direct3D 11 front's `SET_SLOTS`) is `OCCLUSION_QUERY_BATCH`. Left: `unbuilt!`'s three
(inside `macro_rules!`, asked of the literals track), the two `OUT_DIR` ones (asked since run 3),
and `CUBE_FACES` twice (waiting on the interface). wcontainers-nir and wcontainers-opengl have no
baseline lines.

G9 run 8: exit 1 at dd5c5606 (x86-64 machine, from a git bundle; marks 0; clone, build and
bundle deleted after; output in scratch `logs/g9r8-literals.out`): 47 findings, 45 in
wcontainers-opengl (draw.rs 18, state.rs 12, query.rs 5, context.rs 3, present.rs 3, and one
each in build.rs, blit.rs, gallium.rs, view.rs; most are refusal texts passed to `refuse`, five
are a bare 4, and `QUERIES_PER_SET` restates the Direct3D 11 front's `SET_SLOTS`), and
`CUBE_FACES` twice (waiting on others). Not yet worked down: next run, after the provoking
vertex.

G9 run 7: not run (the run's budget went to the desktop). The Rust added names its numbers
(`QUERIES_PER_SET`, `VERTEX_ALIGNMENT` from wgpu's own constant, sizes from `size_of` and
Gallium's format table); its new refusal texts go through `refuse` like run 6's, which the check
reported. Run 6's 38 findings are still to be worked down; to run first next run.

G9 run 6: exit 1 at 30a4c084's successor 7ead95aa (x86-64 machine, from a git bundle; marks 0;
the clone and its build deleted after; output in scratch `logs/g9r6-literals.out`). 38
findings: `CUBE_FACES` twice (Waiting on others) and 36 in wcontainers-opengl, most from runs 3
to 5: the refusal texts given to the driver's own `refuse` and returned as `Err("...".into())`
(the check's documented false catch of a crate's own message helpers; to be made messages the
check reads, or named), `"C"` and the format string of the `unbuilt!` macro, the shader entry
names `"vs"`/`"fs"` in present.rs, `[u32; 4]` and `[f32; 4]` colour arrays, and run 3's two
`OUT_DIR` ones (build.rs's `"OUT_DIR"` and gallium.rs's `"/gallium.rs"`, still reported). Fixed
this run: the draw's binding sets (from `MAX_BINDING_SETS` by `array::from_fn`) and two zero
colours (`Default::default()`), 2062f8fe. wcontainers-nir and wcontainers-opengl have no
baseline lines.

G9 run 5: run at 99e5ff55 (x86-64 machine, from a git bundle; marks 0; the clone and
its build deleted after). Findings shown: `CUBE_FACES` twice (Waiting on others) and
`COMPONENTS` in `wcontainers-nir/src/mesa.rs` ("a second name for MAX"), now gone: the swizzle's
length is `naga::VectorSize::MAX` itself (d287ead9). Only the summary's last lines were kept, so
whether the two `OUT_DIR` findings of run 3 are still reported was not seen; the next run runs
it again whole. wcontainers-nir and wcontainers-opengl have no baseline lines.

G9 run 3: exit 1 at d6bd4c8a (x86-64 machine, from a git bundle; marks 0; the clone and its
build deleted after). The first try found 11; fixed this run: the cargo words and the Mesa
variable both build scripts defined (`MESA_VARIABLE`, `CHECK_CFG`, `CFG`,
`RERUN_IF_ENV_CHANGED`, `RUSTC_ENV`, `ASSIGN`) now live once in `mesa/meson_build.rs`, which both
include, with the defines bindgen takes (`mesa_defines`); the driver's `'\0'` (run 2's) is gone.
Left, 4 findings: `CUBE_FACES` twice (Waiting on others), and two of the driver's that no legal
form avoids today: `env::var_os("OUT_DIR")` in its build script (the check accepts `OUT_DIR` only
inside `env!`, which a build script cannot use for it, and its own name for it, `CARGO_OUT_DIR`,
is private) and `include!(concat!(env!("OUT_DIR"), "/gallium.rs"))` (a macro that takes only a
literal; handing the path through a variable of the build script's instead was found as a
restated name the same way). Asked of the literals track under Requests. wcontainers-nir and
wcontainers-opengl have no baseline lines.

G9 run 2: not run (the run's budget went to the desktop); the Rust added has no bare literal
(names, paths and flags are named constants; the build scripts share `mesa/meson_build.rs`
rather than defining its constants twice). To run first in the next run.

G9 run 1: exit 1 at 248050c5 (x86-64 machine, from a git bundle; marks 0), with the one finding
outside this track's reach, `CUBE_FACES` (Waiting on others). wcontainers-nir still has no
baseline lines; this run added no literal (the PowerShell script names its versions and paths once
at its top; the check does not scan PowerShell).

Run 15: exit 1 at 52c083b9 (x86-64 machine, from a git bundle; marks 0), for one finding outside
this track's reach: `CUBE_FACES` in `wcontainers-nir/src/lib.rs` (since run 9) is reported as a
redefinition of the Direct3D 11 device's, added on that branch at 7e4d8df6 and merged here this
run (Requests: the interface's one constant). Every other finding is the shaders crate's
submodules, not checked out in the bundle's clone (as run 6 found); this run added no literal.

Run 13: exit 0 at the run's last code commit, 86cefeb2 (x86-64 machine, from a git bundle;
marks 0). wcontainers-nir still has no baseline lines.
Run 12: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0).
wcontainers-nir still has no baseline lines.
Run 11: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A first
try found the Godot reader's `LINE_COMMENT` ("//") named like the check's own (Rust's comment, private
there): renamed for what it is, GLSL's (Departures). wcontainers-nir still has no baseline lines.
Run 10: exit 0 at the run's last code commit (x86-64 machine, from a git bundle; marks 0). A
first try found the new reader's comment marker defined beside dEQP's (`COMMENT`, '#'): Godot's
reader now imports dEQP's. wcontainers-nir still has no baseline lines.
Run 9: exit 0 with the run's code (x86-64 machine, from a git bundle; marks 0). A first try
found `"in"`, `"out"` and a decimal point in `tests/corpus/deqp.rs` restating constants of the
WebGL back end and the check (private there): GLSL's qualifiers are now written through one
function of format strings, and the dEQP grammar's one word before a stage list is taken as any
word. wcontainers-nir still has no baseline lines. Run 8: exit 0 at the run's last code commit, 9e0a7c6e's parent (x86-64 machine, from a git bundle; marks 0; a first
try found the GLSL version directive named twice, in `glsl.rs` and the new `bgfx.rs`: now imported);
wcontainers-nir still has no baseline lines. Run 7: exit 0 at the run's last code commit (x86-64 machine, from a git bundle of the branch; marks
0); wcontainers-nir still has no baseline lines. Run 6: exit 0 at 4294fcf0 (x86-64 machine, from a git bundle; marks 0; the 18 changes that could
add lines are the shaders branch's, as before). The bridge's numbers are `#define`s, named once;
the Rust added has no new bare literals (test data apart). Adopted with the branch (merged from the
shaders branch, which carries it). wcontainers-nir has no
baseline lines: the check passes with none for it (run on the Mac, runs 1 to 4, exit 0; in run 4 the
two new scripts' numbers were taken out (the timing cores are passed in, the load read by `read`), and
one baseline line the Direct3D 11 merge left stale was deleted, as the check asked;
it needs the git history, which the rented machines' copies lack). The list of
changes that could add lines is the 18 the shaders branch brought; this branch added none. C files are
not scanned; the bridge names its one version by `#define`s all the same.
