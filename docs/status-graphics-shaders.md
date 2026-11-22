# Status: graphics, shaders track (G6)

On branch `agents` in the agents' shared worktree (`wcontainers-agents`) since the evening of
2026-11-21 (this crate stays at `crates/wcontainers-shaders`; no track branches). Plan:
`docs/phase-2-graphics.md`, sections 6, 7, 8 and G6 in 12. The run-by-run record of runs 1 to 77
is in `docs/status-graphics-shaders-history.md` (history; where it disagrees with this file, this
file holds).

Milestone G6a, given again on 2026-11-21 evening with the FIRST check's findings carried (at
eb202617, 2026-11-19): run 75 compared each with the tip and found every one settled by earlier
runs (history); its check (at 9fb6e0d1) found the WebAssembly build broken and gap A27 incomplete,
which run 76 settled (history); the next check (at b5222b3d) found the own draw test skipping a
WebGL 2 gather for an untrue reason, and five minor points, which run 77 settled (history); run
78 (the section below), given the first check's findings once more, measured each again at the
tip after the owner's interfaces were merged; the 15.4 entry point the NIR track asked for is
`rewrite` (since run 57). The milestone is reported complete for its independent check.
Its third check's findings were settled in run 72 and its fourth check's in run 73 (history).
G6a's fourth check (at 5c70cc91) found five minor points, settled in run 79 (its section below).
G6b (gap A6: DXIL geometry, hull and domain shaders), given again on 2026-11-22 as its run 1 (run
79, the section below; runs 65 to 74 built it, history): DXIL geometry, hull and domain
shaders convert through the passes the DXBC route uses and draw equal to real Direct3D with dxc's
builds. Of the four Direct3D 12 rig scenes that failed for A6, two draw equal to real Direct3D 12
(geometry, streamout); raster-fill and tessellation still differ on the RTX 3080, only in cells
drawn as lines. The causes, picture by picture: in both Direct3D 12 rig pictures nearly every
differing pixel is a line pixel ours draws and Direct3D 12 leaves clear (tessellation 308 of 310),
the line rule of wgpu's Vulkan path where the Direct3D 12 front draws, a listed loss (plan 15.1);
the rig's topology picture, with no stage of A6, fails the same way. Raster-fill runs no stage of
A6. The order of a patch's triangles (the RTX 3080's own, which Direct3D leaves to the device; ours
is WARP's word for word, equal to Direct3D 11 on WARP in every texel) is the cause of the Direct3D
11 harness's tessellation differences (`tessellation_draw`, run 74), not of the rig picture's.
Not met as G6b writes it ("equal on the RTX 3080"), and not reachable by the converter: the
owner's ruling, or a line allowance in the rig's comparison, is asked ("New for the owner"); the
item stays open until one of them comes.

G6 (the rest of the gap list), given on 2026-11-22 as its run 1 (run 80, the section below):
G6b's check's findings settled or recorded, A4's `combine` drawn for single-sample targets, and
subnormal tessellation factors kept on the DXBC route. Run 2 (run 81, the section below): A28's
first part, `A8Unorm` storage views, built and drawn. Run 3 (run 82, the section below): A4's
`SwizzledDepth` measured on real Direct3D 12 and settled where the devices agree, gathers of a
depth texture's other components on both routes. Run 4 (run 83, the section below): A5's
geometry replay of streams past the first, the Direct3D 11 front's ask. Run 5 (run 84, the
section below): A5's alpha-antialiased lines on WebGL 2. Run 6 (run 85, the section below): A5's
cull distances and primitive numbers on WebGL 2, A4's multisampled integer twins found absent by
the plan. Run 7 (run 86, the section below): A9, vertex shaders writing storage, run as compute
through the stage helper; A7's typed buffers in geometry shaders. Run 8 (run 87, the section
below): arrays of constant and structured buffers on the DXIL route, and dxc's `vs_cull` drawn on
both cull routes. G6 given again on 2026-11-22 as its run 1 (run 88, the section below): the NIR
track's public entry for geometry, hull and domain programs from a naga module (`staged`), and its
three asks of `rewrite`. Run 2 (run 89, the section below): A13, geometry shaders taking patches
drawn without tessellation, and A14, hull shaders giving no control points, both on the DXBC route
(the Direct3D 12 runner's six files). Run 3 (run 90, the section below): A7's cull distances
written by geometry shaders (the Direct3D 12 runner's eight files). Not complete: the gap list
below.

## For the NIR track: geometry, hull and domain programs from a naga module (`wcontainers_shaders::staged`)

Built in run 88 (its G9 run 12 ask, "Requests to other tracks"). `staged::geometry`, `staged::hull`
and `staged::domain` take `(module, &program, reflection, &variant, &route_key)` and return the
`Converted` that `stage::Stages::new`, `Stages::tessellated` and `tessellated_with` read, made by
the same passes the DXBC and DXIL routes run (`lower::geometry`, `lower::hull`), then 15.4's
rewrites and the variant's as `rewrite` makes them; the modules carry `cache::rewritten_key` of
`route_key` (the geometry pass-through `module_key(.., 1)` of it, as `convert` gives).
- The module: the program's stage values are private globals (`StageVariable`: the variable,
  input or output, `Place::Location { location, component }` or `Place::BuiltIn(staged::BuiltIn)`,
  a geometry output's stream, `patch` for hull outputs and domain inputs held once a patch). Types:
  a scalar or vector, or an array of them one location a row; per-vertex and per-control-point
  values arrayed by vertex or control point outermost (GLSL's `gl_in[]`, `gl_out[]`).
- `GeometryProgram`: `main` (run once per input primitive and instance after its inputs are
  filled), `emit` (one empty function without arguments per stream, at least one), `cut` (empty),
  all three placed in the function arena before every function calling them; input and output
  primitive, most vertices, instances (`BuiltIn::GeometryInstance` is GLSL's `gl_InvocationID`
  there). Entry points of the module are replaced.
- `HullProgram`: `control_point` run once per output control point in order with
  `BuiltIn::ControlPointId` set, then `patch_constant` if given (`None` for a GLSL control program
  written as one function; it then runs after the last point); domain, partitioning and
  tessellator output (OpenGL declares them in the evaluation program: hand them over here), input
  and output control points. The factors are `BuiltIn::OuterFactors` (array of 4 floats) and
  `InnerFactors` (array of 2), as GLSL declares them; only the domain's count is read.
- `DomainProgram`: the module's one vertex entry point, its outputs its own (the given
  reflection's outputs are kept); domain, the patch's control points (the hull's output count),
  and the inputs (`BuiltIn::TessellationCoordinate` is a vec3 as `gl_TessCoord`).
- Refusals are the routes' own names (`refusal::GEOMETRY_OUTPUT`, `HULL_INPUT_SHAPE`,
  `DOMAIN_WITHOUT_CONTROL_POINTS`, ...); a program that does not hold to the above is `Malformed`.
- Tests: `tests/staged.rs` (any machine): own `gs_cut` and the hull and domain shaders of
  `tessellation.hlsl` written in WGSL as such a module (`tests/own/staged_*.wgsl`,
  `tests/forms/mod.rs`) give the same extras, stage values (but semantics), bindings and module
  stages as fxc's builds through `convert`, under the compute and replay forms; refusals by name.
  Drawn on the RTX 3080 against real Direct3D 11 drawing fxc's builds:
  `geometry_programs_from_a_module_draw_as_direct3d_does` (tests/geometry_draw.rs) and
  `hull_and_domain_programs_from_a_module_stream_as_direct3d_does` (tests/tessellation_draw.rs,
  every domain with three partitionings and each output, streamed and through a geometry shader).
- The other asks, in `rewrite` since run 88: the sampled pairs are always taken from the module;
  a vertex program writing no position is given (0, 0, 0, -1) as `convert` gives it (the NIR
  track's own copy of `clip_unpositioned` can go); `BuiltIn::BaseVertex` and `BaseInstance`
  arguments read the draw's bases from where `draw_base` says (the DXIL route's pass). Test
  `a_vertex_program_takes_its_sampled_pairs_position_and_draw_bases_as_convert_gives_them`
  (tests/rewrite.rs). Not done: the carried SPIR-V reader's three refusals and the array layer and
  unordered not-equal items (A26), below.

## G6 run 3 (run 90): cull distances written by geometry shaders (A7)

Logs and scripts in scratch `graphics-shaders/g6-r90` (run 89's with its folder: `gpu.sh`,
`gpu-noclippy.sh`, `tree-tar.sh`, `suite-laptop.sh` as `~/wcontainers-graphics/suite-r90.sh` writing
`out-r90`; `cull.cmd` compiles the three new shaders alone on the desktop; jobs `job1.log`,
`job2-mutant.log`; `arm1.log`, `arm2.log`, `arm-lit.log`; `chrome.log`). Builds, clippy and CPU tests
on the rented ARM64 machine from the shared worktree; nothing built on the Mac.
- Built: a DXBC or DXIL geometry shader writing `SV_CullDistance` converts (src/lower.rs,
  src/dxil/geometry.rs); its compute form's records end with the cull distances' slots after the
  clip distances' (src/lower/geometry.rs `distance_outputs`), reflected as the signature places
  them. The stage helper (src/stage/helper.rs) then runs the geometry cull program
  (src/stage/geometry_cull.wgsl, `stage::geometry_cull_program`) after the geometry stage: one
  invocation per geometry invocation walks the drawn stream's strip indices and lists each
  primitive's vertices, or an empty record's (outside every clip volume) where all its vertices are
  below zero on one distance, and the final draw is that list (`FinalDraw` of a list topology);
  stream output still takes every primitive from the strips. `SlowPath::CullDistanceAsGeometry`
  is counted. Also after a domain stage (the same path); not drawn there. The replay form (WebGL 2)
  refuses such a shader by name (`GeometryCullDistanceReplay`).
- Test: own `gs_cull` (triangle strips, a cut restarting the second triangle), `gs_cull_lines`
  and `gs_cull_points` (geometry.hlsl, fxc gs_5_0 and dxc gs_6_0; two clip and three cull
  distances, packed by fxc as the runner's are: clip and cull in one register), every fourth
  primitive dropped whole, a strip's single primitives dropped, primitives whose vertices are each
  below zero on another distance kept. `geometry_shaders_writing_cull_distances_draw_as_direct3d_does`
  (tests/geometry_draw.rs) on WebGPU against real Direct3D 11 drawing fxc's builds on the RTX 3080:
  six cases (lists and strips, indexed, instanced) on both builds, seeds 1 to 4, 48 comparisons, 0
  failed, every texel equal (job1.log). With the cull program made to keep every primitive (a
  mutant in the scratch clone) all 48 fail (job2-mutant.log). CPU test
  `geometry_shaders_write_cull_distances` (tests/geometry.rs; both builds, the reflection's two
  clip and three cull distances, the replay refusal).
- Corpus (laptop, `--test corpus`): the runner's `clip-cull-distance-1-18` to `-21-gs` (DXBC and
  DXIL, 8) convert; own records added for the three shaders' six builds. The corpus test's
  `writes_cull_distances` (which sends a vertex shader writing them to the cull route) now asks
  for a vertex program.
- Runs: ARM64 machine (load 15 to 24): clippy of this crate with warnings denied clean; `geometry`
  23 passed (1 ignored as before), the library 10 (arm2.log); the literal check prints no line of
  this crate (arm-lit.log). Desktop (RTX 3080, one GPU job, 3 cargo jobs): Windows clippy clean.
  Laptop (load 4 to 5.4, 4 jobs): `corpus` 13 passed with every corpus and glslang (11 in
  `logs/suite-r90.log` there, where own and runner failed on the corpus test's cull route; both
  then passed, with rig and games again, in `logs/suite-r90b.log`). Chrome 157 on the desktop
  compiles all 14 new WGSL files (chrome.log).
- For the Direct3D fronts: nothing new to call. Such a geometry shader is made and run through
  `Stages::new` (or `tessellated_with`) as any; its `FinalDraw` is then an indexed list (u32
  indices at `index_offset` of `buffer`) instead of strips, drawn with `passthrough()` and
  `vertex_layout()` as before.
- Not run: WebAssembly, the whole GPU suite (only the new drawn test; no other converted module
  changed, and the geometry stage's records and final draw change only for shaders writing cull
  distances).
- Clean-up: the desktop's `C:\wc\src\shaders-tip`, `C:\wc\target\shaders-tip`,
  `C:\wc\out\shaders-tip` and `C:\wc\scratch\shaders-cull`, the laptop's `src/shaders-tip`,
  `target/shaders-tip` and tar (its `out-r90` and logs kept), the scratch clone and tar. The ARM64
  machine's `/work/target/graphics-shaders` is kept.

## G6 run 2 (run 89): geometry shaders taking patches (A13), hulls giving no control points (A14)

Logs and scripts in scratch `graphics-shaders/g6-r89` (run 88's with its folder: `gpu.sh`,
`tree-tar.sh`, `suite-laptop.sh` as `~/wcontainers-graphics/suite-r89.sh` writing `out-r89`; jobs
`job1.log` to `job3.log`, `gpu-suite.log`; `arm1.log` to `arm4.log`, `arm-lit.log`; `chrome.log` and
`chrome-out/`). Builds, clippy and CPU tests on the rented ARM64 machine from the scratch clone
`tip`; nothing built on the Mac.
- A13 built (src/lower/geometry.rs `input_vertices`, src/stage/helper.rs): a DXBC geometry shader
  taking `InputPatch<T, n>` takes each patch's control points as its primitive's vertices, and the
  stage helper draws `InputTopology::PatchList(n)` into such a geometry shader when no hull stage is
  bound (lists, indexed or not; a patch count other than the shader's is `TopologyMismatch`, as for
  every topology). DXIL stays refused (`GeometryPatchInput`): dxil-spirv converts no geometry
  shader taking patches ("Unexpected input primitive"). The replay route (WebGL 2) still refuses a
  patch list (`TopologyMismatch`): not built or drawn there.
  Test: own `gs_patch` (geometry_patch.hlsl, fxc gs_5_0 and dxc gs_6_0; quadrilaterals of four
  control points picked at run time, coloured by the patch's number) with `vs_line` and `ps_line`,
  `geometry_shaders_taking_patches_draw_as_direct3d_does` (tests/stage_draw.rs) on WebGPU against
  real Direct3D 11 drawing `D3D11_PRIMITIVE_TOPOLOGY_4_CONTROL_POINT_PATCHLIST` with no hull stage on
  the RTX 3080: four cases a seed (24 points x1, 22 x2 cutting a patch short, 16 x3 and 18 x1
  indexed), seeds 1 to 4, 16 comparisons, 0 failed, every texel equal (job1.log). CPU test
  `geometry_shaders_take_patches_as_primitives` (tests/geometry.rs; compute and replay forms; dxc's
  build refused by name).
- A14 built (src/lower/hull.rs): a DXBC hull shader of no output control points converts; its
  control point function never runs (its body is dropped, as it would index arrays of none), and
  the patch's record holds the patch constants alone. A DXBC domain shader of no input control
  points converts too. dxc refuses to build such a hull shader ("number of control points 0 is
  outside the valid range"), and dxc's domain shader taking none stays refused
  (`DomainWithoutControlPoints`: dxil-spirv declares its patch an array of none).
  Test: own `hs_tri_none` (hull_none.hlsl, fxc only) and `ds_tri_none` (tessellation.hlsl, both),
  `hulls_giving_no_control_points_stream_as_direct3d_does` (tests/tessellation_draw.rs): streamed
  through the whole stage helper on WebGPU (RTX 3080) against Direct3D 11 on WARP streaming the same
  draw, seeds 1 to 4: 4 comparisons, 0 failed, every patch's primitives equal and in the same order
  (9,342 to 20,694 primitives a seed; job3.log). Against real Direct3D 11 on the RTX 3080 the test
  process ended with STATUS_INTEGER_DIVIDE_BY_ZERO (job2.log); only the reference device changed
  between that run and WARP's, so the RTX 3080's Direct3D 11 is inferred to end there (not traced
  further). CPU test `hulls_may_give_no_control_points` (tests/geometry.rs).
- Corpus (laptop, `--test corpus` with every corpus and glslang, load 1 to 4): the runner's six
  DXBC records (`geometry-shader-syntax-6`, `-7`, `-8`, `-10`, `patch-syntax-12`,
  `hull-shader-attributes-10`) now convert; own records added for `gs_patch`, `hs_tri_none` and
  `ds_tri_none` (fxc's convert; dxc's two refused by name). The other 11 corpus tests passed and
  every other record held (`logs/suite-r89.log` there). Chrome on the desktop compiles all nine new
  WGSL files (chrome.log). The runner's DXIL `patch-syntax-12-gs` and `patch-syntax-11-ds` stay
  refused by name, dxil-spirv's.
- Runs: ARM64 machine (load 2 to 11): rustfmt clean, clippy of this crate with warnings denied
  clean, CPU test binaries `geometry` 22 (1 ignored as before), `staged` 3, `variants` 40, `rewrite`
  13, `builtin` 5, `cache` 7 (1 ignored as before), `compute` 5, the library 10: all passed
  (arm4.log); the literal check prints no line of this crate (arm-lit.log). Desktop (RTX 3080, GPU
  jobs one at a time, 3 cargo jobs): Windows clippy clean; the whole suite at 829bf2e3 (one job,
  one test thread): 25 test binaries, 189 passed, 0 failed, 6 ignored as before (gpu-suite.log).
- Not run: WebAssembly (no converted module of an existing record changed).
- Clean-up: the desktop's `C:\wc\src\shaders-tip`, `C:\wc\target\shaders-tip` and
  `C:\wc\out\shaders-tip`, the laptop's `src/shaders-tip`, `target/shaders-tip` and tar (its
  `out-r89` and log kept), the scratch clone and tar (logs kept in `g6-r89`). The ARM64 machine's
  `/work/target/graphics-shaders` is kept (the machine's own build folder for this track).

## G6 run 1 again (run 88): the NIR track's entry for geometry and tessellation, its `rewrite` asks

Logs and scripts in scratch `graphics-shaders/g6-r88` (`gpu.sh` as run 87's, from the clone
`tip` on branch `shaders-tip`; `job1.log`, `job2.log`). Builds and CPU tests ran on the rented
ARM64 machine (`arm-run <checkout> --name graphics-shaders`), none on the Mac.
- Built: `src/staged.rs` (public `staged`, the section above); `lower::hull::call_domain_inputs`
  (the DXIL route's and `staged::domain`'s one way of calling a domain program's input function);
  `HullPass::entry_point` takes the patch constant function as optional (the DXBC route refuses
  its absence as before, as `Malformed`); `convert` and `rewrite` share their tail (`varied`,
  `finished`, `route_rewrites` in src/lib.rs).
- `rewrite` changed (the NIR track's asks): sampled pairs always from the module, the position
  for an unpositioned vertex program, the draw bases' rewrite.
- Runs (ARM64 machine, load 3 to 8): clippy of this crate with warnings denied clean; CPU test
  binaries `staged` 3, `geometry` 20 (1 ignored as before), `variants` 40, `rewrite` 13, `builtin`
  5, `cache` 7 (1 ignored as before), `compute` 5, and the library's 12: all passed. Desktop (RTX
  3080, one GPU job, 3 cargo jobs): Windows clippy clean; the two new drawn tests (job2.log):
  seeds 1 to 4, geometry 16 comparisons and tessellation 168, 0 failed; seeds 100 to 115, 64 and
  672, 0 failed; every geometry case 0 texels apart from Direct3D 11.
- Literals: the new files add no finding of this crate (one `position` name restating a stage
  constant was renamed before the commit).
- The whole suite at 3565e4bf: desktop (RTX 3080, one GPU job, one test thread, 3 cargo jobs): 25
  test binaries, 185 passed, 0 failed, 6 ignored as before (gpu-suite.log). Laptop (load 0 to 5.7,
  4 jobs): `corpus` 13 passed with every corpus and glslang, `fuzz` 3 (the mutation test skipped
  as before), `glsl_writer` 11 (`logs/suite-r88.log` there); its `out-r88` differs from `out-r87`
  only in the converter version, the cache keys and the logs' paths, so no converted module of a
  corpus changed and Chrome had nothing new to compile. WebAssembly not run (no module changed).
- Clean-up: the desktop's `C:\wc\src\shaders-tip` and `C:\wc\target\shaders-tip`, the laptop's
  `src/shaders-tip`, `target/shaders-tip` and tar (its `out-r88` and log kept), the scratch clone
  and tar (logs kept in `g6-r88`). The ARM64 machine's `/work/target/graphics-shaders` is kept
  (the machine's own build folder for this track).

## G6 run 8 (run 87): arrays of constant and structured buffers on the DXIL route, dxc's `vs_cull` drawn

Logs and scripts in scratch `graphics-shaders/g6-r87` (run 86's with its folder: `gpu.sh`,
`tree-tar2.sh` for the clone `tip2`, `suite-laptop.sh`; `x86-literals.sh` runs fmt, clippy, the
CPU test binaries and the literal check on the x86 machine from the bundle `shaders-r87.bundle`;
jobs `job1.log`, `job2.log`, `gpu-suite.log`; `x86-tests.log`).
- Interface changes merged at a00ab267 (readiness, binding settings, the resolver): none touches
  this crate or what it calls; it builds and its tests pass at the tip.
- Built (src/dxil/buffer_arrays.rs, new; src/dxil/arrays.rs; src/dxil.rs): dxc's arrays of
  constant and structured buffers (`ConstantBuffer<T> b[n]`, `StructuredBuffer<T> s[]`,
  `RWStructuredBuffer<T> u[n]`), which naga's reader takes as one buffer holding an array of
  blocks, are split into one buffer per element at its own register (an array without a length
  sized by `register_ranges`, as typed buffer arrays since run 55). An element picked by a
  constant is that buffer; each load, store, atomic operation and length taken through an element
  picked at run time is a call of a function switching on the index (the last element for an
  index past the array, which Direct3D leaves undefined). A pointer into a picked element passed
  to a function stays refused (`ResourceArray`; no shader of any corpus does so). A constant
  buffer element declared 64 KiB by dxil-spirv takes its array's declared size (`declared_sizes`
  reads the entry holding the element's register; dxc's metadata gives one element's size, as
  I read it, not measured with an element indexed at run time inside). The typing copy of a
  function, shared with the texture arrays' pass, is `arrays::typing_copy`.
- Tests: `buffer_arrays_use_the_elements_picked` (tests/compute_draw.rs) now runs dxc's
  `cs_buffer_array` too; new own shader `cs_structured_array` (arrays.hlsl, fxc cs_5_1 and dxc
  cs_6_0) and test `structured_buffer_arrays_use_the_elements_picked_on_both_routes`: reads,
  stores, atomic adds with the value returned, and `GetDimensions` through elements picked at run
  time, against the words Direct3D's rules give, on WebGPU on the RTX 3080. Seeds 1 to 4 (job1.log):
  cs_buffer_array DXBC and DXIL, cs_structured_array DXBC and DXIL, cs_typed_array both: 24
  comparisons, 0 words apart. Seeds 100 to 115 (job2.log): 96 comparisons, 0 words apart. CPU
  tests (tests/variants.rs): `buffer_arrays_are_a_binding_per_element` takes both routes (the DXIL
  route's refusal it asserted is gone) and new
  `structured_buffer_arrays_are_a_binding_per_element_on_both_routes`.
  Not drawn against real Direct3D 12 (as for the other arrays: Direct3D's rules, not its device).
- Corpus: own `cs_buffer_array.dxil` now records `UnboundedResourceArray` (it converts with
  ranges; the corpus gives none), new `cs_structured_array` records the same on both routes. The
  laptop's corpus test: 13 passed (`logs/suite-r87b.log` there, load 0.3 to 1.3). No other record
  changed, so Chrome had no new WGSL to compile.
- dxc's build of `vs_cull` (gap A5's leftover) is drawn on both cull routes:
  `geometry_shaders_draw_as_direct3d_does` (WebGPU) and
  `geometry_shaders_replayed_on_webgl_draw_as_direct3d_does` (ANGLE) now take the eight `CULLED`
  cases by both builds (`culled_by_both_builds`), and each must drop a primitive by its planes.
  Seeds 1 to 4 (job2.log): 224 and 216 comparisons, 0 failed; the DXIL cases within the allowance
  the DXBC ones have (largest 1, allowed 2, where printed).
- Literals: the resource list message is one named constant (`dxil::PAST_THE_LIST`), so two
  baseline lines of this crate are gone (8fb40546); the new file adds no finding (x86-tests.log:
  no line of this crate among the check's 20 findings, all other tracks').
- Runs, at 8fb40546: x86 machine (load 10 to 50, other tracks' jobs beside): rustfmt clean,
  clippy of this crate and the interface with warnings denied clean, the CPU test binaries
  `geometry`, `variants`, `rewrite`, `builtin`, `cache`, `glsl_writer` and `compute`: 100 passed,
  2 ignored as before. Desktop (RTX 3080, one GPU job at a time, 3 cargo jobs): Windows clippy
  clean; the whole suite (one job, one test thread): 24 test binaries, 179 passed, 0 failed, 6
  ignored as before (gpu-suite.log).
- Clean-up: the desktop's `C:\wc\src\shaders-tip`, `C:\wc\target\shaders-tip` and
  `C:\wc\out\shaders-tip`, the x86 machine's clone, build folders and bundle, the laptop's
  `src/shaders-tip`, `target/shaders-tip` and tar (its `out-r87` and `logs/suite-r87*.log` kept),
  the scratch clones, tar and bundle (logs kept in `g6-r87`).
- Not run: WebAssembly (no converted module of a corpus record changed).

## G6 run 7 (run 86): vertex shaders writing storage through the stage helper (A9), typed buffers in geometry shaders (A7)

Logs and scripts in scratch `graphics-shaders/g6-r86` (run 85's with its folder; jobs `job1.log`
to `job5.log`, `gpu-suite.log`, `chrome.log`; `x86-tests.log`, `x86-tests2.log`; the laptop's
corpus verdicts in `verdicts/`).
- Built (src/lib.rs, src/stage.rs, src/stage/helper.rs): `convert` and `rewrite` no longer refuse
  a vertex program writing storage (`VertexStorageWrite` is gone); `stage::writes_storage` names
  one; `Stages::stored` (and `Stages::new` without a geometry shader) runs it as compute and draws
  its records through the cull route's pass-through, the cull program listing every primitive
  (dropping only by cull distances the program writes); `stage::stored_programs` gives the
  programs. The helper counts `SlowPath::VertexStorageAsCompute` once a draw for any route whose
  vertex program writes storage, and `CullDistanceAsGeometry` only where cull distances are
  written. Refused by name: adjacency without a geometry shader (`VertexStorageAdjacency`). How
  the front calls it: "How the Direct3D fronts call `convert`".
- Test (tests/stage_draw.rs, own `vs_store_drawn`, fxc vs_5_0 and dxc vs_6_0 -Gis, in
  `vertex_storage.hlsl`): `vertex_storage_writes_through_the_stage_helper_match_direct3d` draws the
  seven primitive-number cases (lists, strips, points, instanced, indexed strips with restarts)
  through `Stages::stored` on WebGPU against real Direct3D 11.1 on the RTX 3080 (UAVs bound with
  `OMSetRenderTargetsAndUnorderedAccessViews`): the picture, and the raw buffer's words and the
  R32_UINT texture's texels of every vertex of a whole primitive of each instance, equal; elsewhere
  ours may hold zero (not shaded) or Direct3D's. Seeds 1 to 4: 56 comparisons, 0 failed; seeds 100
  to 115: 224 comparisons, 0 failed; every texel and stored word equal (job5.log). The test also
  asserts the slow path is counted once a draw.
- Corpus (laptop, `--test corpus` with every corpus and glslang): the eight own records and the
  runner's two `depth-bounds-1-vs` now convert, plus the new `vs_store_drawn` (own 161 and 150
  records); the WGSL written for them is the storage route's compute form (as for cull distances),
  and Chrome on the desktop compiles all ten (chrome.log). 13 passed (the corpus test's log
  `logs/suite-r86c.log` there, load 0.4 to 1.1).
- Runs, at the code of each commit: x86 machine (load 1 to 8 for the first, 22 to 53 for the
  second, other tracks' jobs beside): rustfmt clean, clippy of this crate and the interface with
  warnings denied clean, the CPU test binaries `geometry`, `variants`, `rewrite`, `builtin`,
  `cache`, `glsl_writer` and `compute`: 99 passed, 2 ignored as before; the literal check prints no
  line of this crate (x86-tests2.log, x86-tests3.log). Desktop (RTX 3080, GPU jobs one at a time, 3
  cargo jobs): Windows clippy clean; the whole suite (one job, one test thread): 24 test binaries,
  176 passed after A9 (gpu-suite.log) and 177 after the typed buffers (gpu-suite2.log), 0 failed,
  6 ignored as before. The corpus test on the laptop: 13 passed (suite-r86c.log after A9; after
  the typed buffers only `gs_typed`'s two new records differed, now added).
- Also built (src/lower.rs): typed buffers in DXBC geometry shaders (gap A7's
  `TypedBufferInGeometry`, refused since run 18 because the stage helper did not fill their helper
  values; the front fills the geometry stage's helper block since). Test (tests/stage_draw.rs, own
  `gs_typed`, fxc gs_5_0 and dxc gs_6_0 -Gis, `geometry_typed.hlsl`):
  `typed_buffers_in_geometry_shaders_draw_as_direct3d_does` draws `vs_line`, `gs_typed` (a
  `Buffer<float4>` of four elements read at the primitive's number modulo five, so every fifth
  reads past the view) and `ps_line` through the stage helper on WebGPU against real Direct3D 11
  on the RTX 3080, the triangle cases of the primitive-number test: seeds 1 to 4, 24 comparisons,
  0 failed; seeds 100 to 115, 96 comparisons, 0 failed; every texel equal (job6.log, job7.log).
  The DXIL build converted before this run too; both are drawn. Corpus: `gs_typed` added to the
  own records (both convert); Chrome compiles both (chrome2.log).
- Not run: WebAssembly (no converted module of any other shader changed; the corpus's new
  conversions are not compared with a WebAssembly build).
- Clean-up: the desktop's `C:\wc\src\shaders-tip`, `C:\wc\target\shaders-tip` and
  `C:\wc\out\shaders-tip`, the x86 machine's clone, build folder and bundle, the laptop's
  `src/shaders-tip`, `target/shaders-tip` and tar (its `out-r86` and `logs/suite-r86*.log` kept),
  the scratch clone, tar and bundle (logs kept in `g6-r86`).

## G6 run 6 (run 85): cull distances and primitive numbers on WebGL 2 (A5)

Logs and scripts in scratch `graphics-shaders/g6-r85` (run 84's with its folder: `gpu.sh`,
`x86-tests.sh`; jobs `job1.log`, `gpu-suite.log`; `x86-tests.log` to `x86-tests3.log`).
- Built (src/stage/replay.rs, src/stage.rs, src/stage/primitive_replay.wgsl):
  `GeometryReplay::culled` and `GeometryReplay::primitive_numbered`, the Direct3D 11 front's
  WebGL 2 item for test_combined_clip_and_cull_distances and the primitive route's twin. Each draw
  captures the vertex stage (records with the cull distances' slots after the clip distances'),
  copies the records into the records' texture, then captures `primitive_replay.wgsl` once a
  vertex of each primitive: a primitive all of whose vertices lie below zero on one cull distance
  is written as records outside every clip volume (as the compute route's empty record), and on
  the primitive route each record is followed by its primitive's number in its instance. The cull
  route's or the primitive route's own pass-through draws the capture. How the front calls it:
  "How the Direct3D fronts call `convert`", primitive numbers and cull distances without compute.
  The line replay program and the new one share one wrapper (`replay_program`).
- A4: multisampled `combine` into integer twins is absent by plan 4.12 (its entry); a logic
  operation into a multisampled target is what stays open there.
- Tests: `geometry_shaders_replayed_on_webgl_draw_as_direct3d_does` (tests/geometry_draw.rs) now
  replays the eight `CULLED` cases (`vs_cull`, fxc's build: lists, strips, indexed strips with
  restarts, points, instanced) on ANGLE against real Direct3D 11 on the RTX 3080: 184 comparisons,
  0 failed (32 of them cull cases; the planes drop 118 primitives over the four seeds, and every
  case drops some), and each cull case's figures are those of the compute route word for word (10
  of the 32 with texels a step apart, largest 1, allowed 2, as on the compute route; the comparison
  is shared, `compared_clipped`). `primitive_numbers_from_the_stage_helper_draw_as_direct3d_does`
  (tests/stage_draw.rs) draws each case by compute on WebGPU and replayed on WebGL 2 with and
  without EXT_clip_control, fxc and dxc builds: 168 comparisons, 0 failed, every texel equal.
  `geometry_shaders_draw_as_direct3d_does` 192, 0 failed; `antialiased_lines_draw_as_direct3d_does`
  144, 0 failed (unchanged figures). Default seeds 1 to 4 (job1.log).
- Runs: x86 machine (load 1 to 47, other tracks' jobs beside): rustfmt clean, clippy of this crate
  and the interface with warnings denied clean, the CPU test binaries `geometry`, `variants`,
  `rewrite`, `builtin`, `cache`, `glsl_writer` and `compute`: 99 passed, 2 ignored as before; the
  literal check prints no line of this crate (x86-tests3.log). Desktop (RTX 3080, GPU jobs one at
  a time, 3 cargo jobs): Windows clippy clean, job1 as above; the whole suite at the committed
  code (f264c3a8, one job, one test thread): 24 test binaries, 175 passed, 0 failed, 6 ignored as
  before (gpu-suite.log).
- Not run: WebAssembly, Chrome, the corpora (no converted shader changed; the stage helper's
  programs are in no corpus record). A DXIL build of `vs_cull` is not drawn on either cull route
  (as before this run).
- Clean-up: the desktop's `C:\wc\src\shaders-tip` and `C:\wc\target\shaders-tip`, the x86
  machine's clone, build folder and bundle, the scratch clone and bundle (logs kept in `g6-r85`).

## G6 run 5 (run 84): alpha-antialiased lines on WebGL 2 (A5)

Logs and scripts in scratch `graphics-shaders/g6-r84` (run 83's with its folder; jobs `job1.log`
to `job3.log`, `gpu-suite.log`; `x86-checks.log`, `x86-tests.log` with `x86-tests.sh`, the CPU
tests and checks from a bundle of the clone's `x86tip`).
- Built: `GeometryReplay::antialiased_lines` and `run_antialiased_lines` (src/stage/replay.rs),
  the Direct3D 11 front's remaining 10_0 need on WebGL 2. Each draw captures the vertex stage
  (`stage::line_capture_programs`, records as the compute route's), copies the records into the
  records' texture, then captures `src/stage/line_replay.wgsl` once a corner of each line's two
  triangles (`lines.wgsl`'s rectangle, written by a vertex program; the corner record's slot count
  is declared in front of the text, `line_replay_program`), which the line pass-through draws. On
  a back end without clip depth from zero the records carry `CLIP_DEPTH_REWRITE`'s `2z - w`, and
  the program cuts lines at z = -w there (`near_w`). How the front calls it: "How the Direct3D
  fronts call `convert`", alpha-antialiased lines.
- Fixed on the way, both found by the new test on ANGLE: (1) every stage helper pass-through
  wrote each value as four components, where GLSL ES links a varying only of the same type, so a
  pixel shader reading a narrower value (dxc's `ps_line` reads its `ID` as one u32) did not link
  behind the replay; pass-throughs now write each value at the width the vertex program declares
  (`DrawnOutputs::passthrough_values`; the geometry shader's pass-through, from its reflection,
  keeps four). (2) The replay's capture states asked for clip depth from zero, which the WebGL 2
  back end refuses without EXT_clip_control (`GpuError::Absent(CLIP_DEPTH_ZERO_ONE)`), so every
  replay route failed on such a context; they now take the back end's convention.
- Test (tests/stage_draw.rs): `antialiased_lines_draw_as_direct3d_does` draws each case by the
  compute route on WebGPU, replayed on WebGL 2 (ANGLE on the RTX 3080) and replayed on WebGL 2
  without EXT_clip_control, fxc and dxc builds, against real Direct3D 11 on the RTX 3080 within
  the allowance (64) and beside WARP: 144 comparisons, 0 failed, default seeds 1 to 4 (job3.log);
  each replayed picture differs from the RTX 3080's in exactly as many texels and by as much as
  the compute route's (for example seed 1's list 508 of 528, largest 28). Every vertex lies at
  depth 0.5, so no line crosses the near plane: the cut is not exercised by any drawn test, on
  either route.
- Runs: x86 machine (load 0.4 to 2): rustfmt clean, clippy of this crate and the interface with
  warnings denied clean, the CPU test binaries `geometry`, `variants`, `rewrite`, `builtin`,
  `cache`, `glsl_writer` and `compute`: 99 passed, 2 ignored as before; the literal check prints
  no line of this crate (x86-tests.log). Desktop (RTX 3080, GPU jobs one at a time, 3 cargo jobs):
  Windows clippy clean; `geometry_draw` with the width change: 9 passed (job2.log); the whole
  suite at the committed code (74d7df22, one job, one test thread): 24 test binaries, 175 passed,
  0 failed, 6 ignored as before (gpu-suite.log).
- Not run: WebAssembly, Chrome (no converted shader changed; the stage helper's programs are in
  no corpus record).
- Clean-up: the desktop's `C:\wc\src\x86tip` and `C:\wc\target\x86tip`, the x86 machine's clone,
  build folder and bundle, the scratch clone and bundles (logs kept in `g6-r84`).

## G6 run 4 (run 83): the geometry replay's streams past the first (A5)

Logs and scripts in scratch `graphics-shaders/g6-r83` (run 82's with its folder; jobs `job1.log`,
`job2.log`; `x86-checks.log`, `x86-tests.log`).
- Built: on WebGL 2 a geometry shader writing several streams (points, as Direct3D allows) with a
  stream output declaration no longer stops at `GeometryReplayStreams` (removed). The replay form
  keeps the vertices of the stream the values block's new word `word::STREAM` names
  (src/lower/geometry.rs, `replay_emit` gated on it; a shader of one stream converts as before);
  `GeometryReplay::run` replays once a stream when stream output is bound, later streams first and
  stream 0 last (the one drawn), and `stream_out` writes each stream's buffers and its written and
  made counts at `STREAM_BUFFERS + 2k` on the host (src/stage/replay.rs). How the front calls it:
  "How the Direct3D fronts call `convert`", geometry shaders on a back end without compute.
- Tests (tests/geometry_draw.rs): `streams_past_the_first_match_direct3d` now runs the replay
  route on ANGLE beside the compute route on WebGPU, against real Direct3D 11 (fxc) and 12 (dxc)
  on the RTX 3080: 64 comparisons, 0 failed (32 replayed, every word and count equal; the unbound
  fourth stream's made count equal and its written count 0, as gap A15 says), 16 known from A22's
  zero positions. `geometry_shaders_with_rows_stream_as_direct3d_does` replays
  `gs_rows_streams` (three streams) on both routes too, no longer expecting a refusal: 72
  comparisons, 0 failed, 4 known (A22); its refusals are dxc's arrays of arrays alone
  (`GeometryReplayArrayOfArrays`). Default seeds 1 to 4 (`WCONTAINERS_GEOMETRY_SEED`,
  `WCONTAINERS_GEOMETRY_CASES`).

Runs (code at the scratch clone's e1925a6a, the checkout's 960220c4 with this run's four files):
- Desktop (RTX 3080, GPU jobs one at a time, 3 cargo jobs): Windows clippy of this crate with
  warnings denied clean; `geometry_draw`'s `streams_past_the_first`, `geometry_shaders_with_rows`,
  `stream_output_after_replay`, `geometry_shaders_replayed_on_webgl` and
  `stream_output_matches`: 5 passed (job1.log; counts from job2.log with `--nocapture`).
- x86 machine (load 6 to 31, other tracks' jobs beside): rustfmt clean, clippy of this crate and
  the interface with warnings denied clean, the literal check prints no line of this crate
  (baseline lines of this crate 4,786, unchanged); the CPU test binaries `geometry`, `variants`,
  `rewrite`, `builtin`, `cache`, `glsl_writer` and `compute`: 99 passed, 2 ignored as before
  (x86-tests.log).
- Not run: the corpus records (the change touches only the stage helper's programs, which no
  record holds), WebAssembly, Chrome.
- Clean-up: the desktop's `C:\wc\src\shaders-tip` and `C:\wc\target\shaders-tip`, the x86
  machine's clone, build folder and bundle, the scratch clone and bundle (logs kept in `g6-r83`).

## G6 run 3 (run 82): mapped depth views measured on Direct3D 12, depth gathers

Logs and scripts in scratch `graphics-shaders/g6-r82` (clone `tip`, `gpu.sh`, `gpu-noclippy.sh`,
`tree-tar.sh`, `suite-laptop.sh`; jobs `job1.log` to `job6.log`).
- A4's `SwizzledDepth` (the Direct3D 12 front's ask) measured: tests/shadow_draw.rs now draws real
  Direct3D 12 (`direct3d12`: an R16 depth resource read through an R16_UNORM view with
  `Shader4ComponentMapping`, static point samplers) on the RTX 3080 and on WARP, for every program
  reading a depth view (`MAPPED_SHADERS`: whole, first component, gathers of each component,
  comparison gathers of each), 12 mappings, `Less` and `Greater`, both routes. What the devices do
  through a mapping (job4.log, job5.log):
  - Plain reads and gathers of component c give the mapped c-th component of (depth, 0, 0, 1): the
    RTX 3080 so, and vkd3d's tests expect so. WARP instead applies the mapping to a gather's four
    results as if they were one texel (GatherAlpha through `0r00` gives (0, 1, 0, 0)), even for a
    red gather with red kept; taken as a WARP fault, not followed.
  - A plain comparison through a mapping whose first component is zero or one gives that constant
    whatever the function, on both devices. Through green, blue or alpha the RTX 3080 gives their
    constant (0, 0, 1) while WARP compares the depth: they disagree.
  - A comparison gather of component c: the RTX 3080 compares the depth whatever c, then maps each
    result by the mapping's first component (as vkd3d's gather-cmp test says of NVIDIA); vkd3d's
    expectations compare the mapped c-th component. They agree only where both components are red.
- Built (src/variant/swizzle.rs, `depth_mapped`): every read the devices agree on converts (a
  gather or depth-alone read through a constant becomes the constant; a plain comparison through
  zero or one becomes the constant); the rest is refused by name, `SwizzledDepth` through a
  mapping, `DepthGatherComponent` for a comparison gather of another component through none.
- Gathers of a depth texture's green, blue and alpha (refused `DepthGatherComponent` on both routes
  before) now convert, plain ones to their default constant, comparison ones where the mapping
  keeps red in both places. The pass runs for such gathers without a mapping too.
- Found and fixed: the DXIL route converted `GatherCmpGreen`, `Blue` and `Alpha` as red comparison
  gathers without a word, because dxil-spirv drops the call's channel (its dxil_sampling.cpp,
  "TextureGatherCmp has a component here"). cpp/dxil.cpp now reads each `TextureGatherCmp` call's
  channel from the bitcode (`compared_gather_channels`), and src/dxil/depth.rs gives it to the
  comparison gathers (`compared_gathers`); a program whose calls name different channels, or one
  not constant, is refused by name (`DepthGatherComponent`; no file of any corpus).
- Tests: `depth_views_with_component_mappings_read_as_direct3d_12_maps_them` (default seeds 1 to
  4: 544 drawn comparisons equal to the RTX 3080 in every texel, 800 refusals with the expected
  name, WARP printed beside), `depth_views_with_component_mappings_read_as_mapped` (now the three
  red-keeping mappings against Direct3D 11 mapped on the CPU), and in tests/variants.rs
  `mapped_depth_views_compose_what_they_read` and
  `depth_gathers_of_other_components_convert_where_exact`. New own programs in
  tests/own/shadow.hlsl: `ps_shadow_gathers` and `ps_shadow_compared_red` to `_alpha`, fxc and dxc.
- The test harness binds by register: a DXIL program's slots rank the registers it declares, so a
  program declaring only `s1` has it at slot 0.
- Records: own and own DXIL gain the five new programs each (the green, blue and alpha comparison
  gathers `DepthGatherComponent`, the rest converting, GLSL compiling); runner DXIL's
  `gather-cmp-3` to `-7` go from a wrong conversion to `DepthGatherComponent`, as on DXBC. Every
  other record holds.

Runs (code at 5d618f15, from the scratch clone):
- Desktop (RTX 3080, GPU jobs one at a time, 3 cargo jobs): Windows clippy of this crate with
  warnings denied clean; `shadow_draw` on the final rule (job6: 544 drawn comparisons, 0 failed,
  800 refusals named as expected; job5, before plain comparisons through green and blue were
  refused, drew 640 equal to the RTX 3080, which is where WARP's disagreement showed),
  `variant_draw` (1,358 comparisons, 0 failed, 96 known anisotropic), `filter_draw`,
  `rewrite`: passed (job6.log; its one variants failure was the new test's own wrong mapping,
  fixed and passed on the laptop).
- Laptop (i7-8750H, load 4 to 5): the whole CPU suite with every corpus and glslang 8.13
  (`suite-laptop.sh`, `--skip mutated --skip damaged_records_decode_without_a_panic`): every test
  passed but the own and runner DXIL records before they were updated; after: `corpus` 13 passed,
  `variants` 39 passed, `rewrite` 12 passed (logs/suite-r82b.log, suite-r82c.log there).
- x86 machine (load 17 to 27, other tracks' jobs beside): rustfmt clean, clippy of this crate and
  the interface with warnings denied clean, the literal check prints only the line this run's
  baseline commit deletes (it fails on other tracks' lines); baseline lines of this crate 4,786.
- Clean-up: build folders on the desktop, the x86 machine and the laptop, the scratch clone and
  bundle (logs kept in scratch `g6-r82`).
- Not run: WebAssembly (cpp/dxil.cpp gained a reader using only the bitcode parser it already
  used), Chrome (no new WGSL beyond the own programs, whose GLSL compiles), the whole GPU suite.

## G6 run 2 (run 81): `A8Unorm` storage views

Logs and scripts in scratch `graphics-shaders/g6-r81` (clone `tip`, `gpu.sh`, `x86-checks.sh`).
- A28 (1), the Direct3D 11 front's ask, built (da38fb54): a storage view of `A8Unorm` in
  `storage_formats` is declared `rgba8unorm`, the form the WebGPU back end holds `A8Unorm` in (value
  in alpha, colour channels zero; its packed.rs), on both routes and through `rewrite`
  (`lower::storage_form`). The variant pass `src/variant/alpha.rs` makes every store into such a
  view write `(0, 0, 0, a)` of its value, as Direct3D stores the fourth component into the format's
  one channel, so the colour channels stay zero and a load reads `(0, 0, 0, a)`. Through a buffer
  (`texels_through_buffer`) it is still `StorageFormatAbsent`, as is the built-in storage clear (the
  component Direct3D's clear of an A8 view takes was not measured).
- Tests: `alpha_storage_views_store_their_alpha_alone_on_both_routes` (tests/compute.rs: the type,
  each store's value, WGSL round trip, the buffer route's refusal) and
  `alpha_storage_textures_store_their_alpha_as_direct3d_does` (tests/compute_draw.rs: `cs_images`
  with u0 an `A8_UNORM` view against real Direct3D 11 on the RTX 3080, both routes, default seeds 1
  to 4; ours drawn into an rgba8unorm texture as the back end holds it, every texel equal to
  Direct3D's widened to `(0, 0, 0, a)`, so the colour channels are checked zero too): passed
  (job1.log). Not drawn: a load of an A8 storage view (no test program loads one).
- The WebGPU back end still gives `A8Unorm` no storage use (the Direct3D 12 track asked it), so a
  front must create the texture through the back end's `A8Unorm`, which refuses `STORAGE`, or hold
  it itself as `Rgba8Unorm` with the value in alpha (the Direct3D 11 front does), binding that view
  with `A8Unorm` named in `storage_formats`.

Runs (at da38fb54, from the scratch clone):
- x86 machine (load 40 at the start, 19 at the end, other tracks' jobs beside): rustfmt of every
  file of this crate clean, clippy of this crate and the interface with warnings denied clean, the
  whole suite with every corpus (`--include-ignored --skip mutated --skip
  damaged_records_decode_without_a_panic`, A16's known failure): 125 passed, 0 failed, the four
  corpus record tests among them (13 passed); the literal check prints no line of this crate (it
  fails on 413 lines of other tracks); this crate's baseline lines 4,787, 356 under src
  (x86-checks.log).
- Desktop (RTX 3080, one GPU job, 3 cargo jobs): Windows clippy of this crate with warnings denied
  clean; `compute_draw`'s four storage texture tests passed (job1.log).
- Not run: WebAssembly and Chrome (no corpus file names an `A8Unorm` storage view; the verdicts
  hold), the whole GPU suite.

## G6 run 1 (run 80): G6b's check's findings, `combine` drawn, subnormal factors kept

Logs and scripts in scratch `graphics-shaders/g6-r80` (clone `tip`, `gpu.sh`, `gpu-noclippy.sh`,
`x86-checks.sh`). G6b's check's findings (at 13d2f093), each:
- The two Direct3D 12 rig scenes (raster-fill, tessellation): still open, waiting on the owner's
  ruling or a line allowance in the rig (A6); nothing of the converter stands in the way.
- The lead paragraph now says which cause belongs to which picture (line rule in the Direct3D 12
  rig pictures, triangle order in the Direct3D 11 harness), with the check's per-cell counts in A6.
- Subnormal factors: the stated cause was wrong. It was dxbc-spirv's clamp of each tessellation
  factor (`IoMap::applyMaxTessFactor`), which the back end's compiler flushes; the lowering now
  keeps a subnormal factor's bits past that clamp, and the drawn tests no longer excuse or flush
  subnormal factors (4284c582). At the check's seed 80701, 20 cases (RTX 3080, job2.log):
  `stream_output_after_the_domain_stage_matches_direct3d` 1,040 comparisons, 0 failed (the check
  had 120 known, all DXBC); `stage_values_hold_wherever_the_data_ring_wraps` 1,520, 0 failed.
- A10's and A16's figures at the check's seeds: added to those entries; nothing of a new kind.
- Hand-rolled SPIR-V on the DXIL route: new gap A33 (rspirv 0.13 fits; not ported this run, by
  priority).
- The x86 machine's corpora: all three copied there, and "How to run" says how to copy them.

New work: A4's `combine` for single-sample targets is drawn against real Direct3D 11 (four blends
into the four integer twins within one code, all sixteen logic operations exact; both routes,
both back ends; 4694656f; A4 has the figures). The Direct3D 11 front can now blend into integer
twins and do logic operations ("How the Direct3D fronts call `convert`").

Runs (at 4694656f's code, from the scratch clone):
- Desktop (RTX 3080, GPU jobs one at a time, 3 cargo jobs): Windows clippy of this crate with
  warnings denied clean; the tests above (job2.log, job4.log).
- x86 machine (load 17 at the start, 29 at the end, other tracks' jobs beside): rustfmt clean,
  clippy of this crate and the interface with warnings denied clean, the whole suite with every
  corpus (`--include-ignored --skip mutated --skip damaged_records_decode_without_a_panic`, the
  last being A16's known failure): every test passed, the four corpus record tests among them (13
  passed); the literal check prints no line of this crate (it fails on 413 lines of other
  tracks); this crate's baseline lines 4,787, 356 under src (x86-checks.log).
- Not run: WebAssembly and Chrome (the converter changed only for DXBC hull programs' factor
  clamp; the corpus verdicts hold), the whole GPU suite.
- Clean-up: desktop `C:\wc\target\shaders-tip` and `C:\wc\src\shaders-tip`; x86
  `/work/target/shaders-tip`, its clone and bundle; the scratch clone (logs kept). The corpora
  stay on the x86 machine at `/work/probes/shaders-corpus`.

## G6b again, run 1 (run 79): G6a's fourth check, and G6b's bar at the tip

Logs, scripts and the written files in scratch `graphics-shaders/g6b-r79`. Code runs at 0b071580
(laptop, desktop) and 2db605c2 (x86 machine); the two differ only in tests/geometry.rs, which the
x86 machine ran at 2db605c2, and the commits after them change only this file.

G6a's fourth check (at 5c70cc91), each settled:
- Run 78's section named a00ab267, not the pushed tip: it now says so, with the check's rerun at
  5c70cc91 that reproduced every figure.
- The stale departure about the corpus test copying the WebGL 2 options: corrected (only
  `naga_refusal` copies naga's options, for naga's own reason).
- `SWIZZLED_DEPTH` was defined in lib.rs: it is now in src/refusal.rs with the others
  (`wcontainers_shaders::refusal::SWIZZLED_DEPTH`; no other crate used the old path), so the claim
  that every refusal name is defined there holds.
- Seed 40961 of `geometry_draw` and seed 60606 of the fuzz test: the check's figures are in A10,
  A11 and A12. Nothing of a new kind.

G6b's bar, item by item at the tip:
- The rig's DXIL shaders refused for A6 convert: all 16 of rig-dxil (`DxilGeometryShader` 6,
  `DxilHullShader` 6, `DxilDomainShader` 3, the reader's "unsupported builtin" 1, in the records
  before 7c988bce) are `ok`.
- The runner's and own files refused for A6 give their recorded verdicts: runner-dxil's 40
  (`DxilGeometryShader` 10, `DxilHullShader` 22, `DxilDomainShader` 6, "unsupported builtin" 2):
  32 convert, 8 are refused by the DXBC route's names (`CullDistance` 4, `ClipDistance` 1,
  `PrimitiveId` 1, `GeometryPatchInput` 1) and `DomainWithoutControlPoints` 1 (A14). Own-dxil's
  11 (`DxilGeometryShader` 10, "unsupported builtin" 1): all convert; own-dxil now holds dxc's
  builds of 17 geometry, 35 hull and 5 domain shaders, every one `ok`. The three old names are
  gone from src. Counted from the records (before: `git show 7c988bce^`), which the laptop's
  corpus test holds at the tip (below).
- Every refusal that remains has a name, defined once in src/refusal.rs, and is counted per
  corpus in the records, which the corpus test holds.
- Drawn against real Direct3D at seeds of this run's own (79001, 16 cases; the checker picks its
  own; RTX 3080, gpu-suite.log, one GPU job): `geometry_draw`'s geometry cases, both routes, 768
  comparisons, 9 failed, every one gap A10's triangle edge pattern with 0 texels at a clipped line
  end (79005 `gs_clip` TriangleList 30 from 3 x2, DXBC and DXIL, 6 of 920 texels, largest 127;
  79015 `gs_clip` TriangleStrip 14 from 2, DXBC and DXIL, 5 of 802, largest 48; `vs_cull` DXBC at
  79008, 79012 and 79015 three times, 2 to 18 texels, largest 167); the replay on ANGLE, 608
  comparisons, 4 failed, the same `gs_clip` cases; depth clamping, 96 comparisons, 2 failed, gap
  A12's pattern (79008 and 79016 `gs_cut` TriangleStrip, 24 and 30 steps); stream output after a
  geometry shader 256, 384, 448 and 128 comparisons, 0 failed (16 and 64 known, A22);
  `tessellation_draw` 832, 1,216 and 1,024 comparisons, 0 failed (80 known, subnormal factors). No
  failure of a kind A10 and A12 do not list; each DXIL failure mirrors a DXBC one.
- A mutated copy of each test proven to fail (scratch `mutant-*.diff`, applied alone; x86
  machine, x86-mutants2.log at 2db605c2, where the unmutated test binary passes, 20 tests): the
  hull's winding swapped for DXIL fails `hull_and_domain_shaders_declare_what_they_are` and
  `hull_values_make_the_same_records_on_both_routes`; DXIL domain inputs read as patch constants
  fails `hull_values_make_the_same_records_on_both_routes`; a DXIL geometry shader's instance
  count taken as 1 passed every test that runs without a GPU, so `geometry_shaders_declare_what_they_are`
  now checks dxc's build of each of its seven shaders too (2db605c2), and fails on that mutant
  (gs_instances.dxil, instances 1 against 3). Drawn with the instance and domain input mutants
  both applied (gpu-mutants.log, default seeds 1 to 4): `geometry_shaders_draw_as_direct3d_does`
  fails exactly the four DXIL `gs_instances` cases (460 to 826 texels apart), the replay its four,
  the rows stream test 8 of 64 (DXIL `gs_rows_lines`); every tessellation scene test fails for
  dxc's builds (the solid patches up to 2,940 texels missed a cell) with fxc's cells all equal, and
  stream output after the domain stage refuses (`DomainInputShape`). Run 66's drawn mutants
  (winding, layer from the stage) are in the history.
- The four Direct3D 12 rig scenes through the Direct3D 12 front built from the tip against real
  Direct3D 12 (scenes1.log, RTX 3080): geometry (7 outputs) and streamout (3) pass with largest
  difference 0; raster: cull, scissor and viewports pass, fill fails 64 of 16,384 pixels;
  tessellation fails 310 of 32,768. The same figures as run 66 and every run since: the line cells
  of A6 (gap entry), waiting on the owner's ruling, not reachable by the converter.

Runs:
- Laptop (i7-8750H, load 4.1 at the start, 5.0 at the end, another track's job beside; 4 build
  jobs under `choom`): the whole suite with every corpus and glslang, `--include-ignored --skip
  mutated` (laptop-suite.log): 124 passed, 1 failed, the known failure
  `damaged_records_decode_without_a_panic` (A16; `ps_levels_past.dxil` case 17). Every corpus's
  records hold (13 passed). WGSL files written: corpus 185, games 527, games DXIL 441, own 145, own
  DXIL 136, rig 79, rig DXIL 72, runner 1438, runner DXIL 1418, rewrite 3. Every written WGSL and
  GLSL file and every verdict is byte-equal to run 78's (`diff -r` with g6a-r78/out: only
  `cache.txt` and `converter-version.txt` differ, as the crate's sources changed, besides
  glslang's `.log` paths), so Chrome's compile of run 78's files stands for these.
- x86 machine (load 2.0 at the start, 9.2 at the end): rustfmt of every file of this crate clean,
  clippy of this crate and the interface with warnings denied clean (0b071580, and this crate
  again at 2db605c2), the literal check prints no line of this crate (it fails on other tracks'
  lines); this crate's baseline lines 4,787, 356 under src (x86-checks.log, x86-mutants2.log).
- Desktop (RTX 3080, one GPU job, 3 cargo jobs, queued 23:00, ran 23:02 to 23:16): Windows clippy
  of this crate with warnings denied clean; every test binary at default seeds with
  `--test-threads 1`: 171 passed, 0 failed, 6 ignored (gpu-suite.log), then the seeded draws
  above.
- Not run: WebAssembly (no input of its build changed but the refusal's place; run 78's figures),
  the fuzz test (A11 holds the check's new figures).
- Clean-up: laptop `target/shaders-tip` (4.9 GB), its source copy and `out-r79`; x86
  `/work/target/shaders-tip` (3.2 GB), its clone and probes; desktop `C:\wc\target\shaders-tip`,
  `C:\wc\src\shaders-tip`, `C:\wc\out\shaders-tip`; the scratch clone and tars (the written files
  kept as `out`).

## G6a run 10 (run 78): the first check's findings at the tip after the interface merge

Every figure here is from runs at a00ab267 (the tip of `agents` when the run started, after the
merge of the owner's reviewed interfaces), not at the pushed tip 5c70cc91; the two differ for this
crate's build inputs only in `literals.baseline` (66 lines of other tracks deleted), and the fourth
check reran everything at 5c70cc91 and got every figure below exactly. Nothing this crate builds changed since run 77's
793e1bfe: `git diff 793e1bfe a00ab267` is empty for this crate, `wcontainers-gpu`, `-gpu-webgl`,
`-gpu-webgpu`, `-gpu-test` and `-graphics-test`, and the lock file differs only in the name of
another track's crate. None of the changed interfaces (readiness, sockets, resolver) is used here.
No code changed in this run. Logs and the files Chrome compiled (`out`, 52 MB) in scratch
`graphics-shaders/g6a-r78`. The first check's findings (at eb202617), each against the tip:
- The four corpus record tests: pass. The corpus test, every corpus, 13 passed (laptop-suite.log);
  the games DXIL corpus is checked.
- Run 46's figures taken before its code: superseded; every figure below is from a00ab267.
- Gap A3's cause: closed since run 47. The corpus test writes GLSL with the WebGL 2 back end's own
  writing under its key, so its bit functions and gathers are what Chrome compiles; Chrome refuses
  none of them (its 40 refusals: multisampled reads 23, the primitive index 8, vertex cull
  distances 9).
- GLSL refusals hidden by a class: the records carry each refusal's reason (the needs the back end
  reads and glslang's first error, or the writer's reason), so another refusal changes the record.
  The three examples are no longer refused: `ps_shadow` and `ps_shade` compile, and the fine
  derivative refusals name `FINE_DERIVATIVES`, which the WebGL 2 back end declares absent.
- The unbuilt parts of the milestone: refused by name (constants in src/refusal.rs) and listed with
  causes in the gap list (A1, A2, A4, A5, A7, A8; A6 closed in run 66).
- The vertex-stage storage write route: gap A9 (closed in run 86).
- Minor: `geometry_draw` seeds beyond 1 to 4 (A10); the tessellator's MIT notice is at the top of
  `src/stage/tessellator.wgsl`; a cache file cut inside its header loads as empty (`Cache::load`,
  test `a_cut_file_keeps_every_whole_entry_before_the_cut`); refusal names are defined once in
  src/refusal.rs; `filter_draw` counts a known failure only for a mode filtered over an anisotropic
  footprint (`anisotropic_footprint`).

The 15.4 entry point for the NIR track: `wcontainers_shaders::rewrite` (since run 57).

Runs at a00ab267:
- Laptop (i7-8750H, load 4.0 at the start, 1.2 at the end; 4 build jobs, under `choom`): the
  whole suite with every corpus and glslang 8.13, `--include-ignored --skip mutated`
  (laptop-suite.log): 124 passed, 1 failed: the known failure
  `damaged_records_decode_without_a_panic` (ignored by default, gap A16; `ps_levels_past.dxil`
  case 17). Written files: WGSL corpus 185, games 527, games DXIL 441, own 145, own DXIL 136, rig
  79, rig DXIL 72, runner 1438, runner DXIL 1418; GLSL 126, 463, 300, 69, 57, 55, 53, 1275, 1229.
  Every written WGSL and GLSL file and every verdict is byte-equal to run 76's (`diff -r` of the
  two `out` folders; only `cache.txt` and `converter-version.txt` differ, as the converter's
  version changed with run 77's reader names).
- Laptop, WebAssembly (wasi-sdk 25, wasmtime; another track's job also running; wasm.log):
  `wasm_imports` links (6,875,458 bytes); the corpus test for `wasm32-wasip1` on every corpus, 12
  passed; WGSL bytes differ from the native files in 27 files, exactly A8's (corpus 1, own 1, games
  3, runner 22; no DXIL file), and `cache.txt` in those four corpora only.
- Desktop (RTX 3080, one GPU job, 3 cargo jobs): Windows clippy of this crate with warnings denied
  clean; every test binary with `--test-threads 1` (gpu-suite.log): 171 passed, 0 failed, 6 ignored.
- Chrome on the RTX 3080 (`web/check` on the laptop's written files, one GPU job; chrome.log):
  every WGSL file compiles (the counts above and the 3 of `rewrite`); WebGL 2 compiles 3,587 GLSL
  files and refuses 40, the same 40 files as runs 75 and 76, each a file whose record names the
  need.
- x86 machine (load 5.2 at the start, 7.9 at the end): rustfmt of every file of this crate clean,
  clippy of this crate and the interface with warnings denied clean, the literal check prints no
  line of this crate (it fails on 415 lines of other tracks); this crate's baseline lines 4,787,
  356 under src (x86-checks.log, literals.txt).
- Not run: the fuzz test and the seeded `geometry_draw` (A10 and A11 hold the checks' figures; no
  input changed), the Direct3D 12 scenes (A6's figures). Gap A29 not met again: the GPU suite ran
  with one test thread.
- Clean-up: this run's build folders deleted (laptop `target/shaders-tip`, 6.6 GB, its source copy,
  `out-r78` and `outw-r78`; x86 `/work/target/shaders-tip`, 1.2 GB, and its clone; desktop
  `C:\wc\target\shaders-tip`, `C:\wc\src\shaders-tip`, `C:\wc\out\shaders-tip`; the scratch clone).

## New for the owner

- Run 88: a public entry for geometry, hull and domain programs another route brought into the
  common form (`wcontainers_shaders::staged`), which the NIR track's OpenGL geometry and
  tessellation stages wait on; drawn equal to real Direct3D 11 with programs written that way.
  `rewrite` now also gives the sampled pairs, an unpositioned vertex program's position and the
  draw bases as `convert` does (the NIR track's asks). No interface change.

- Run 82: depth views with component mappings measured on real Direct3D 12 (RTX 3080 and WARP)
  and converted wherever the devices agree; where they disagree (a comparison through a moved red
  source) it stays refused by name, as the RTX 3080 and WARP give different answers and vkd3d's
  tests a third. The DXIL route converted comparison gathers of green, blue and alpha as red
  without a word (dxil-spirv drops the channel); it now reads the channel from the bitcode and
  converts or refuses as the DXBC route does. No interface change.

- Run 81: `A8Unorm` storage views convert and store as Direct3D 11 does (A28's first part), for
  the Direct3D 11 front. The WebGPU back end still declares no storage use of `A8Unorm`. No
  interface change.

- Run 80: single-sample `TargetInShader::combine` is drawn equal to Direct3D 11 (blends into
  the integer twins within one code, logic operations exact), so the Direct3D 11 front may set it
  ("How the Direct3D fronts call `convert`"). A DXBC hull program's subnormal tessellation factor
  now reaches the tessellator unflushed, as on the DXIL route. The ruling on A6's two rig scenes
  is still open. New gap A33 (rspirv for the DXIL passes). No interface change.

- Run 79: G6b given again; its bar measured at the tip (section above). Unchanged since run 74:
  of the four Direct3D 12 rig scenes, raster-fill and tessellation still differ on the RTX 3080 in
  line cells only, which the converter cannot reach (A6); the ruling asked in run 74 is still
  open. `SWIZZLED_DEPTH` moved to `wcontainers_shaders::refusal` (no other crate used it). The
  geometry declaration test now checks dxc's builds too. No interface change.

- Run 78: nothing new, no code changed. The owner's interface merge (a00ab267) touches nothing
  this crate builds; the whole suite, every corpus, WebAssembly, Chrome and the checks were run
  again at that tip (section above). No interface change.

- Run 76: the crate links for WebAssembly again (a `thread_local` another track added in
  260d36d8 needed a C++ thread exit hook wasi-sdk's runtime lacks; WebAssembly without threads now
  keeps that list in a plain global). Gap A27 lists every refusal name src has (27 were missing);
  new gap A30 (DXIL `saturate` of a NaN on WebGL 2); A11 names a naga reader panic. No interface
  change.

- Run 75: nothing new in behaviour, no code changed. The first check's carried findings are all
  settled at the tip (section above). New gap A29: three drawn test binaries crash in ANGLE when
  their tests run in parallel threads and pass with `--test-threads 1`. The 15.4 entry point is
  `rewrite` (below). No interface change.

- Run 74: G6b's last item cannot be met by the converter as written. The tessellation scene's
  remaining texels on the RTX 3080 are the GPU's own order of a patch's triangles (Direct3D leaves it
  to the device); our order is WARP's word for word, and the scene drawn on WARP equals Direct3D on
  WARP in every texel. Raster-fill runs none of G6b's shaders. Both scenes' lines are also drawn by
  wgpu's Vulkan path for the Direct3D 12 front, a listed loss. Asked: rule the WARP proof the bar
  for these line cells, or have the rig allow for them. New test helper `webgpu_on_warp` (tests
  only). No interface change, no change to the converter.

- Run 73: a DXBC `f32tof16` keeps infinities on WebGL 2 too (the clamp to the half's range is
  made by this crate's lowering with a bit test, dxbc-spirv's own lowering of it turned off). The
  stated case counts of `early_draw` are now drawn cases; the corpus test's rule names fewer
  outcomes (only the reader's "unsupported" and glslang refusals citing a need). The tessellation
  scene's remaining differences are measured as the order of triangles within a patch (A6). No
  interface change.

- Run 72: nothing new in behaviour; the status file corrected where the third check of G6a found
  it untrue (item 2's figures, gap A7) and given the gaps the check found unlisted: A26 (a DXBC
  `ne`, and dxc's unordered comparisons, drawn as ordered ones where an operand is NaN), A27 (93
  refusals by name that no corpus meets) and the `gs_instances` texel (A21). Another track
  committed to this crate again (260d36d8: a DXIL module's views read in one dxil-spirv call and
  kept in the cache file, `container.rs`, `cache.rs`, `cpp/dxil.cpp`); this run's suites at
  1130f618 include it. Not this track's work and not reviewed here.

- Run 71: nothing new in behaviour; the corpus test holds a glslang refusal to the needs the WebGL
  2 back end lacks or declares through a directive the file carries (above). Another track works
  in this crate in the shared checkout: it committed f4302b4a and a1b731f1 (the container reader
  public as `wcontainers_shaders::container` for the Direct3D fronts, md-5 moved to the crate's
  dependencies, the fuzz test signing with it), which this run's suites at 277a2891 include, and
  at this run's end it has more uncommitted edits here (build.rs, cpp/boundary.h,
  src/container.rs, tests/fuzz.rs: a DXIL view list and two more DXBC enums). Not this track's
  work, not reviewed here; the next check should know its tip may hold more of it.

- Run 70: nothing new in behaviour. The paused combine commit's 45 literals are named (two
  interface constants, `BLEND_HALF_CODES` and `BLEND_CONSTANT_COMPONENTS`), and the drawn tests
  build on Windows again (the paused commit had broken them). The 15.4 entry point the NIR track
  asked for is `rewrite` (below, since run 57, called by that track); its newer ask, an entry point
  for the stage helper's geometry and tessellation forms from a naga module (its G9 run 12,
  "Requests"), is new work after this milestone's check.

- Run 69: the stage helper writes each stage's values into the data ring just before its set
  (stream output after a domain stage could read none when the ring wrapped). The converter
  version no longer depends on where a checkout keeps git's data, so it changes once. A DXIL
  geometry stream past Direct3D's is refused as malformed. No interface change.

- Run 68: a DXIL geometry shader's values may be arrays of rows (`float3 c[2] : COLOR0`), one
  register a row, as fxc's builds of the same programs. New refusal `ComponentPastVector` (A23): a
  DXBC program dxbc-spirv addresses past a vector's components, which `convert` returned as an
  invalid module before. The stream tests take Direct3D 12's zero positions from Direct3D 11 (A22)
  and allow dxc's builds 4 ulp at the operands' scale (A21). No interface change.
- Run 67: stream output from a DXIL geometry shader of several streams works (each stream's
  outputs at their own registers and semantics); `container::Element` has the stream. DXBC hull
  shaders' patch constant functions read every output control point (dxbc-spirv's I/O bound check
  off). Tests draw dxc's builds' stream output against real Direct3D 12. No interface change.
- Run 66: DXIL hull and domain shaders convert (A6 closed), and a DXIL pixel shader reads its layer
  and viewport. A DXBC domain shader's `StageExtras::Domain::input_control_points` is now its
  declared count (it was 0). New refusal `DomainWithoutControlPoints`. `wcontainers_shaders::token`
  is public (a DXBC instruction word's fields; the fuzz test imports it). No interface change.
- Run 65: DXIL geometry shaders convert (A6's geometry part): a front gets the same compute and
  replay forms, pass-through and reflection from dxc's build as from fxc's. Normalised codes
  are encoded with halves toward zero, as the RTX 3080 does (the Direct3D 11 front's ask; A19).
  `rewrite` refuses an array longer than its kind's slots by name (`BindingsBeyondLimit`).
  `stage::LIST_VARIABLE` is public (a test reads it). No interface change.
- `wcontainers_shaders::rewrite(module, reflection, &variant, &route_key)` (built in run 57,
  src/lib.rs): the 15.4 rewrites (arrays of textures, storage textures and samplers split into an
  element each, gathers at run-time offsets moved to a shifted coordinate, formatless storage
  textures given the key's format) and every rewrite of 6.4's variant key, on a module another
  route brought into the common form, with `cache::rewritten_key`. This is the public entry point
  the NIR track asked for in the brief of 2026-11-19 night, and that track calls it on route B's
  output since its run 14 (its status, "the 15.4 rewrites are now made on route B's output by the
  shaders crate's `rewrite`"). Contract and tests: history, "Run 57"; drawn on the RTX 3080 by
  `rewrite_draw`. A vertex program writing a storage buffer or a storage texture converts, as
  `convert` gives it since run 86, for the stage helper (`stage::writes_storage`, gap A9).
  Changed in G6a run 6:
  - After a split it turns WGSL's derivative uniformity check off, as the DXBC and DXIL routes do
    (`lower::derivatives_anywhere`, now kept below any filter the module has): an implicit-level
    sample of an element picked at run time moves into a switch on the index, which Chrome refused
    where the index may differ between lanes (the sixth check: a pixel program picking by a flat
    input). Test `a_texture_picked_by_an_input_is_sampled_in_any_control_flow` (tests/rewrite.rs),
    whose WGSL Chrome compiles (G6a run 6 above).
  - `capabilities` adds `Capabilities::IMMEDIATES` for a module with an `AddressSpace::Immediate`
    global, so `rewrite` validates the NIR route's push constants (its run 14: 20 of Godot's
    RenderingDevice programs with arrays of textures refused "IMMEDIATES is not supported"). Test
    `immediates_are_taken_and_named_as_a_capability`. The DXBC and DXIL routes make no such global.
- `refusal::GLOBALLY_COHERENT` (`GloballyCoherent`, G6a run 5): a DXBC program declaring a
  `globallycoherent` view is refused by name, as the DXIL route already refused it. No shader of
  any corpus declares one.
- For the NIR track: on its NIR-to-SPIR-V candidate route, three of the four 15.4 features cannot
  reach `rewrite` yet, because the carried naga SPIR-V reader refuses glslang's SPIR-V for them
  before a module exists (the third check, at 64787579): an array of combined image samplers
  indexed at run time (`uniform sampler2D tex[4]; texture(tex[u.i], uv)`) gives `InvalidId(28)`,
  a gather with a uniform offset gives `UnsupportedCapability(ImageGatherExtended)`, and a
  formatless storage image gives `UnsupportedCapability(StorageImageWriteWithoutFormat)`. Route B
  (NIR to the common form directly) does not pass through that reader. Taking the three in the
  carried reader is this track's work (it carries naga's patches), new work after this
  milestone's check.
- The NIR track's other asks of this crate (its status, "Requests to other tracks", "Shaders
  track"): `rewrite` taking `sampled_pairs` from the module, `stage::clip_unpositioned` and the
  draw's bases rewrite in the entry point were built in run 88 (the section "For the NIR track");
  not built: naga's reader rounding an array layer and keeping an unordered not-equal (the DXIL
  route reaches the same reader; the DXBC route does not, and drops the unordered not-equal in
  this crate's own lowering instead: gap A26).

## G6: the pass bar item by item, and the gap list (checked at G6a run 6; A10, A11 and A19 at run 65; A6 at run 66; A4 and the figures of items 1, 2 and 4 at run 70; item 2's figures, A7, A21 and A26 to A27 at run 72; item 6, A8, A10 to A12, A27 and A30 at run 76; A10, A11, A31 and A32 at run 77)

The pass bar, item by item:

1. The 191 corpus shaders and real game shaders (DXBC and DXIL) convert or fail with a named reason
   (`tests/corpus.rs`, records in `tests/expected/`; each record is the whole verdict, refusal
   reason and GLSL ES outcome included, and the test fails on any verdict that differs from its
   record or is not a conversion or a named refusal). Met but for gap A1.
   - Corpus (Wine's tests, DXBC): 185 of 191 convert; 6 named: `SampleCount` 4 (a pixel shader
     asking the target's sample count, given by the front's `samples`; the corpus has no key),
     `RenderTargetArrayIndex` 1 (a vertex shader writing the layer: absent, plan 4.12),
     `StencilRef` 1 (absent on WebGPU and WebGL 2).
   - Real programs' shaders (the rig's programs, `examples/containers.rs`): DXBC 527 of 529
     convert, DXIL 441 of 443 (since run 55; 519 and 426 before, the rest views of mixed kinds,
     now read by their bits). Named: `MultisampledArrayTexture` 1 and 1 (converts with the
     front's `texture_layers`; the corpus has no key), `RenderTargetArrayIndex` 1 and 1 (absent,
     as above).
   - Also recorded: the rig's (79 DXBC, 72 DXIL), our own (166 and 154 records, 155 and 143
     converting, counted in run 89 with `gs_patch`, `hs_tri_none` and `ds_tri_none`) and vkd3d's
     shader runner files (1503 DXBC, 1517 DXIL; 1445 and 1414 converting, counted in run 89, the
     DXBC six more than run 86's by A13 and A14). Every record holds at run 89's code (its section
     above).
2. Every converted shader compiles in Chrome for each back end that can run it
   (`web/check`, one GPU job). WGSL: every converted shader of every corpus. GLSL ES 3.00 (WebGL 2):
   since run 47 the corpus test writes it with the WebGL 2 back end's own writing
   (`wcontainers_gpu_webgl::glsl::write`, on a module converted under the WebGL 2 front's key,
   `TEXTURE_INFO` set, on a context with EXT_clip_control and NV_shader_noperspective_interpolation
   as Chrome's on the RTX 3080), so the bitfield operations, gathers and loads at a level are the
   back end's GLSL (run 46's gap A3 is gone). Each verdict names the features the back end's
   `needs` reads from the shader beyond GLSL ES 3.00 (and a cube array view, which the interface
   has no feature for); a GLSL failure is a named outcome only where such a need is named, any
   other fails the test. Every shader with no such need compiles (glslang 8.13, and Chrome on the
   files written at the tip: run 70 above; each of Chrome's refusals is of a shader whose
   record names the need).
   Those with needs (absent at feature level 10_0 on WebGL 2, plan 4.11, or given by an extension):
   storage written by or read in a fragment or vertex program (`FRAGMENT_STORAGE_WRITE`,
   `VERTEX_STORAGE_READ`), compute, `SAMPLE_SHADING`, `SUBGROUPS`, `MULTISAMPLE_READ`,
   `PRIMITIVE_INDEX`, `FINE_DERIVATIVES`, cube array views, and clip and cull distances (Chrome
   has WEBGL_clip_cull_distance; glslang 8.13 lacks the extension). A vertex shader writing cull
   distances (`vs_cull`) reaches WebGL 2 only through the stage helper there (gap A5); Chrome
   refuses the written vertex shaders that write cull distances for a type error in the written
   GLSL as well, so they would fail where the extension is enabled too (the WebGL 2 track's and
   naga's writer, "Asked of other tracks", run 77). Since G6a
   run 2 a glslang refusal is recorded with glslang's first error (so a new or other refusal
   changes the record), and a write refusal is a named outcome only where a need is one the WebGL
   2 back end never declares (not `DUAL_SOURCE_BLEND`, `POINT_SIZE` or `SAMPLE_SHADING`; the
   test's own copy of that list, asked of the WebGL 2 track). Since run 71 a glslang refusal is
   held to the same rule, with one more case: a need the back end declares through an extension
   (`SAMPLE_SHADING`, `DUAL_SOURCE_BLEND`), only where the written file carries that extension's
   directive for each form of the need it shows (otherwise the outcome is `undeclared` with the
   directive, which fails the test before glslang runs; test
   `a_written_file_without_its_needs_directive_is_not_a_named_outcome`). glslang 8.13 refuses
   OES_sample_variables's built-ins in GLSL ES 3.00 even with the directive (37 records,
   `'gl_SampleID'` and `'gl_SampleMask' : undeclared identifier`); Chrome, which has the
   extension, decides those. Since G6a run 3 a write refusal's
   record also gives naga's own reason (its GLSL ES 3.00 writer on the module as converted,
   `; naga alone: ...`), so a write refusal with another cause changes the record. The nine
   records hold 211 write refusals (counted with `grep` over `tests/expected` in run 72: corpus
   28, games 7, games DXIL 21, own 10, own DXIL 14, rig 5, rig DXIL 5, runner 58, runner DXIL 63;
   the 197, 184 and 143 stated before run 72 did not match them, as the third check found). 198 end at the back end's "naga
   could not write the shader as GLSL ES 3.00"; naga's own reasons across all 211 are features
   GLSL ES 3.00 lacks (storage buffers, `BUFFER_STORAGE`, 157; image load and store 56; subgroups
   23; derivative control 7; cube array textures 5; each counted once per record naming it).
   The other 13 (Godot DXIL pixel shaders, all needing `FRAGMENT_STORAGE_WRITE` too) end at
   a path the WebGL 2 back end has not built: "one expression of a texture is sampled through two
   samplers (not built)". They are named outcomes for their storage need, but that path is the
   back end's own gap, told to its track under "Asked of other tracks".
   Since run 73 (the fourth check of G6a) a glslang refusal is named only where its recorded error
   cites a form of one of its record's needs (`CITED_FORMS` in tests/corpus.rs), and a reader
   verdict only as `spirv-reader unsupported ...`. Each of the 98 glslang refusals at the tip cites
   its need (`gl_SampleID` or `gl_SampleMask` 37, `sampler2DMS` or `usampler2DMS` 23,
   `GL_ANGLE_clip_cull_distance` 30, `gl_PrimitiveID` 8; counted with `grep` in run 72; the
   records are unchanged since but for `ps_half`'s two, which compile). One hole stays: a
   `SAMPLE_SHADING` need shown only by the `sample` qualifier is not checked for
   OES_shader_multisample_interpolation's directive, as that directive is the WebGL 2 back end's
   constant `SAMPLE_QUALIFIER_EXTENSION`, private to its `shader` module at the tip (asked of the
   WebGL 2 track to export). Takes: the constant exported and added to `DIRECTIVES`.
3. Each variant field and flag of 6.4 has a test showing the rewritten program's result equals the
   unrewritten one's with the native feature, or real Direct3D's picture. Met for every field and
   flag but three, for `swizzled` but the comparisons the devices disagree on, and for `targets_in_shader`'s `combine` but
   multisampled targets (gap A4; `combine` drawn since run 80; multisampled integer twins are
   absent by plan 4.12, a logic operation into a multisampled target is the open part). Where each is tested (drawn on the RTX 3080; test binaries of
   `crates/wcontainers-shaders/tests`):
   - `variant_draw` (`variants_draw_as_the_native_feature_does` unless named): `CLIP_DEPTH_REWRITE`
     (WebGL 2 with and without EXT_clip_control against WebGPU), `VIEWPORT_DEPTH_FLIP`,
     `DEPTH_BIAS` (24-bit step against real Direct3D 11), `COUNTING`, `DUAL_SOURCE` (native on
     WebGPU), `SAMPLE_MASK` (native on WebGPU), `VIEWPORT_ADJUST` (native fractional viewport),
     `TEXTURE_INFO` (native queries, and empty slots against Direct3D 11), `CLIP_DISTANCE` with
     `linked` (native clip distances on wgpu's Vulkan path), `DEPTH_CLAMP` with `linked`,
     `lod_bias`, `mirror_once_in_shader`, `constants_as_storage`, `constants_as_texture`,
     `min_lod_clamped` (Direct3D 11's SetResourceMinLOD), `targets_in_shader`, `swizzled`,
     `samples` (and `inputs_evaluated_elsewhere_match_direct3d`), `draw_base` (`Helpers` and
     `Arguments`, non-zero bases, direct and indirect), `texel_formats` and `texel_buffers`
     (`typed_buffers_draw_as_direct3d_does`; since run 55 every second seed binds both views of
     the other kind), `texture_layers`
     (`multisampled_array_layers_read_as_direct3d_does`), `targets_in_shader` into normalised
     targets held as integers (since run 54, against Direct3D 11 drawing into the normalised
     format, both back ends). Since run 56 `DEPTH_CLAMP` and `CLIP_DISTANCE` also without a pixel
     shader, through `builtin::pixel_program_for_variant`, against Direct3D 11 drawing with none
     (both routes, both back ends).
   - `compute_draw`: `storage_formats` (`storage_textures_match_direct3d`, and since run 55
     `storage_textures_of_another_kind_read_and_write_their_bits_as_direct3d_does`), `texel_buffers`
     and `texel_formats` (`typed_buffers_match_direct3d`, and since run 55
     `typed_buffers_of_another_kind_read_and_write_their_bits_as_direct3d_does`; since run 56
     `textures_of_another_kind_load_their_bits_as_direct3d_does`: `texel_formats` at texture
     slots, both routes),
     `texels_through_buffer` (`storage_textures_through_buffers_match_direct3d`), `STORAGE_OFFSET`
     (`storage_views_at_any_first_element_match_direct3d`), `register_ranges`
     (`buffer_arrays_use_the_elements_picked`: arrays of constant and typed buffers, two declared
     without a length, sized by the ranges, DXBC and since run 87 DXIL; since run 55
     `typed_buffer_arrays_use_the_elements_picked_on_both_routes`, typed buffers alone, DXBC and
     DXIL; since run 87 `structured_buffer_arrays_use_the_elements_picked_on_both_routes`, arrays
     of structured buffers read, stored to, added to atomically and measured, DXBC and DXIL;
     against Direct3D's rules, not against real Direct3D 12, which no drawn test here drives
     yet).
   - `filter_draw`: `filter_in_shader`, `border_in_shader`, `decoded` (since run 54: R16_UNORM,
     R16G16_SNORM, R16G16B16A16_UNORM and R8G8B8A8_SNORM held as their integer formats, every
     sampling mode and loads; against real Direct3D 11 and WARP;
     anisotropic filtering a known failure, Known gaps: since run 47 only the modes that take a
     footprint from derivatives are counted as known, an explicit level or a gather must match;
     the comparison against WebGPU's own sampler at seeds other than the defaults: gap A18).
     `shadow_draw`: `filter_in_shader` on
     depth textures compared and read plainly, `swizzled` on depth views.
   - `pull_draw`: `pulled` (every vertex format against what Direct3D 11 fetches).
   - `early_draw`: `EARLY_TESTS` (with storage writes, against Direct3D 11).
   - `stage_draw`: `LINE_COVERAGE` (against the RTX 3080 within a stated allowance),
     `PRIMITIVE_ID_FROM_STAGE`. `geometry_draw`: `LAYER_FROM_STAGE`
     (`layers_past_the_views_draw_into_the_first_as_direct3d_11_does`), `CLIP_DISTANCE` behind a
     geometry stage (met at the default seeds 1 to 4 only: gap A10), `DEPTH_CLAMP` behind a
     geometry stage (since run 56, `depth_clamped_behind_a_geometry_shader_draws_as_direct3d_does`,
     within four units in the last place of the interpolated depth at the default seeds 1 to 4
     only: gap A12).
   - `draw`: `output_interpolation` (every drawn pair links through it; `vs_shade`/`ps_flat` with
     flat, linear and centroid values against Direct3D 11).
   - `linked` is the place the flag-carried values travel; it is exercised by every test of
     `CLIP_DISTANCE`, `DEPTH_CLAMP`, `LINE_COVERAGE`, `PRIMITIVE_ID_FROM_STAGE` and
     `LAYER_FROM_STAGE`.
4. Geometry, tessellation and stream output tests match real Direct3D on the RTX 3080: met on
   WebGPU for DXBC (`geometry_draw`: geometry shaders, layered geometry, stream output;
   `tessellation_draw`: the tessellator against the GPU's, tessellated patches, the rig's
   tessellation scene, stream output after a domain stage, which since run 69 also holds wherever
   the data ring wraps and at the fifth check's seeds 4401 to 4424 and 9100 to 9123; `stage_draw`).
   On WebGL 2 since run 52,
   geometry shaders by replay (`geometry_draw`,
   `geometry_shaders_replayed_on_webgl_draw_as_direct3d_does`, on ANGLE: every case of the WebGPU
   test but the strips with adjacency, which the route refuses by name; indexed strips since run
   54), and
   stream output after them since run 53 (`stream_output_after_replay_matches_direct3d`), and
   since run 58 stream output from a vertex shader alone there, drawn from its capture (the same
   test and `geometry_shaders_replayed_on_webgl_draw_as_direct3d_does`).
   DXIL since G6b: geometry, hull and domain shaders with dxc's builds (A6 closed); stream output
   from dxc's builds against real Direct3D 12 running them since G6b run 3 (A21, A22).
   Alpha-antialiased lines on WebGL 2 since run 84 (`stage_draw`, replayed on ANGLE).
   Cull distances and primitive numbers on WebGL 2 since run 85 (`geometry_draw`, `stage_draw`,
   replayed on ANGLE); tessellation is not offered on WebGL 2 (plan section 7).
   Since run 86, vertex shaders writing storage through `Stages::stored` (`stage_draw`, A9).
   Not met: some topologies and streams (A7), single texels on
   the edges of clipped triangles and a few of culled lines (A10), clamped depth behind a
   geometry stage at some seeds (A12), geometry shaders taking patches and hull shaders giving no
   control points on the DXIL route (A13, A14: dxil-spirv's) and the written count of a stream
   with no buffer bound (A15).
5. The built-in programs: the storage clear (`clear_draw`, every storage format and dimension
   against WARP and the GPU's Direct3D 11), and since run 56 the pixel program of a pipeline
   without a pixel shader (`builtin::pixel_program_for_variant`, `tests/builtin.rs` and the
   `variant_draw` cases above). Met. The fronts do not use them yet (theirs).
6. The cache of section 8 (`tests/cache.rs`): one file that round-trips
   (`converted_shaders_round_trip_through_the_file`), merges by union in both orders
   (`caches_merge_by_union_in_either_order`), survives a cut at seeded places
   (`a_cut_file_keeps_every_whole_entry_before_the_cut`, `WCONTAINERS_CACHE_SEED`,
   `WCONTAINERS_CACHE_CUTS`, default 10; since run 47 also every cut inside the header, which
   loads as an empty cache with the cut bytes dropped) and a damaged entry (a damaged byte ends
   the file at that entry by its CRC; a record damaged under a matching checksum decodes to a
   conversion or an error natively, while in WebAssembly a panic in naga's validator can end the
   instance: gap A16); state combinations recorded once. The header's and an entry's field offsets come from `#[repr(C)]` layouts.
   Conversion gives the same bytes under each back end's keys: the corpus test converts every
   shader under the WebGPU key, the WebGL 2 key (`TEXTURE_INFO`, since run 46), the bases from the
   helpers and the view keys, twice each, and decodes each record back to the same bytes;
   `cache.txt` holds each record's hash so the native and WebAssembly builds can be compared. Met
   but for gaps A8 and A16 (the WebAssembly build links and is compared again since run 76; it was
   broken from 260d36d8 to run 76).

The gap list (every item of the pass bar not met):

- A1. `hlsl+distance-4-ps.dxil` (runner corpus) ends "convert rejected: Unknown type!", not a named
  refusal. Since G6a run 2 the corpus test names it as a known unnamed verdict with this gap
  (`KNOWN_UNNAMED` in tests/corpus.rs): its verdict must still equal its record, and the runner
  test passes and catches any other change. Cause: dxc
  10.0.26100 writes a `PPC_FP128` record into the program's LLVM type table, which dxil-spirv's
  bitcode reader does not know; the program never uses the type. Waits: nobody (a vkd3d runner file
  of the Direct3D 12 track's runner; no real program seen with it). Takes: dxil-spirv's reader
  taking the record (upstream, or a patch carried as naga's are).
- A2. Closed in run 55: a typed buffer or storage texture bound as a view of another kind than
  the program declares is read and written by its bits, as Direct3D gives it (no refusal by kind
  is left), so every such shader converts under the corpus's one-kind view keys.
- A3. Closed in run 47. Run 46 named the WebGL 2 back end's bitfield operations, gathers and loads
  at a level as its cause, but that back end has written all three since its commit 2080826c; the
  refusals came from the corpus test writing GLSL with naga itself. The test now uses the back
  end's writing (item 2), and every shader needing nothing beyond GLSL ES 3.00 compiles.
- A4. Three parts of the variant key, and one case each of two more, are refused by name and have
  no test of a result (`decoded` built in run 54, single-sample `combine` drawn in run 80):
  - `PROVOKING_LAST` (`ProvokingRewrite`): no rewrite of the programs alone can give a flat input the
    last vertex's value, since a pixel program sees one value per primitive. The exact route is the
    front's (indices rotated per primitive, strips and fans made lists). Waits: nobody (both fronts
    route it themselves or report it). Takes: a decision to drop the flag from the interface.
  - `storage_packed` (`StoragePacked`): open question; separate guest buffers become one GPU buffer
    only if the front keeps them in one arena, and the binding the packed buffer takes is not named.
    Waits: nobody yet (programs with more storage views than the back end binds). Takes: settling
    the arena with the interface track, then the program's offsets from the helpers.
  - `layout` and a root signature (`RootSignatureLayout`): root constants read as immediates.
    Waits: nobody (the Direct3D 12 front binds root constants as a uniform buffer). Takes: the
    immediates in the program from the root signature's parameters.
  - `TargetInShader::combine` (plan 6.4 and 4.12): a target combined with its old value in the
    program, read from a copy of the target at `TARGET_COPY_BINDING`, by the factors and operations
    of `HelperValue::TargetCombine`. Single-sample targets met in run 80: `src/variant/targets.rs`
    `combine` loads the old value at the fragment's position and applies `combine.wgsl`'s blend,
    logic operation and write mask before the encoding `targets_in_shader` does, and
    `variants_draw_as_the_native_feature_does` draws it against real Direct3D 11 on the RTX 3080,
    both routes, WebGPU and WebGL 2 (g6-r80/job4.log): four blends (every factor kind but the
    second source's, every operation, partial write masks, a signed blend constant) into
    R16_UNORM, R16G16_SNORM, R16G16B16A16_UNORM and R8G8B8A8_SNORM held as their integer twins,
    from old values, each within one code of Direct3D in every texel (Direct3D's float-to-integer
    tolerance; up to 882 of 4,096 texels a code apart); and all sixteen logic operations into
    R32G32B32A32_UINT with a write mask, against Direct3D 11.1's, equal in every texel. A
    multisampled target is refused by name (`BlendInShaderMultisampled`), as is a key without a
    sample count (`SampleCount`). For the formats held as integer twins this is the plan's own
    rule, not a gap (read in run 85): plan 4.12, "16-bit normalised formats and rendered signed
    normalised ones", says "Multisampling of these formats is reported absent on such adapters",
    which is what the Direct3D 11 front does (it withdraws the multisample bits of the nine
    formats), and WebGL 2 reads no multisampled texture in a program at all (plan section 6). Left
    open: a logic operation into a multisampled target on WebGPU (`LogicOpInShader`, which plan
    4.12 does not list as absent there). Waits: nobody (no front program or test file seen with
    one). Takes: every sample's old value from a multisampled copy, the program run once a sample,
    and a drawn test against Direct3D 11.1.
  - `swizzled` on a depth texture (`SwizzledDepth`, src/variant/swizzle.rs `depth_mapped`):
    measured on real Direct3D 12 in run 82 (its section). Met, drawn equal to the RTX 3080: plain
    reads, gathers of any component and reads used as the depth alone through any mapping, and
    comparisons through mappings whose first component is red, zero or one (comparison gathers:
    red in the first and the gathered component). Refused by name, as the RTX 3080, WARP and
    vkd3d's tests disagree there: a plain comparison through green, blue or alpha, and a
    comparison gather otherwise; also any mapping on a texture slot the program declares as
    neither sampled nor depth. Waits: the Direct3D 12 front's runner (vkd3d's gather-cmp files
    3 to 7 map `0r00`-like views, refused); no real program seen. Takes: a device that settles
    the disagreement (an AMD or Intel GPU's Direct3D 12), then the rule it gives.
- A5. The stage helper on WebGL 2 (plan section 7). Geometry shaders run there since run 52
  (`stage::GeometryReplay`, `SlowPath::GeometryAsReplay`, drawn equal to Direct3D 11 on ANGLE).
  Indexed strips run there since run 54 (listed on the host from their indices read back).
  Not built, each refused by name or absent: strips with adjacency (`GeometryReplayAdjacentStrip`);
  a replay form holding an array of arrays (`GeometryReplayArrayOfArrays`, since run 68: dxc's
  build of a geometry shader whose inputs are arrays of rows, one array per vertex, which GLSL ES
  3.00 cannot declare; fxc's build of the same program replays, its rows separate registers);
  records wider than one capture target
  (`GeometryReplayComponents`, past `capture_components`, 16 slots on WebGL 2's minimum);
  layered geometry (`selects`, built on the same pass-through, not drawn on WebGL 2 yet); stream
  output from a vertex shader alone runs since run 58 (`GeometryReplay::streamed`), refusing
  adjacency (`StreamOutputAdjacency`, as the compute route) and cull distances
  (`StreamOutCullDistance`); stream
  output from a geometry shader writing several streams runs since run 83 (the replay form keeps
  the stream `word::STREAM` names, replayed once a stream, stream 0 last; its section);
  alpha-antialiased lines run since run 84 (`GeometryReplay::antialiased_lines`: the vertex stage
  captured, then `line_replay.wgsl` captured once a corner of each line's two triangles, as
  `lines.wgsl` writes them; its section); cull distances and primitive numbers from a vertex
  shader alone run since run 85 (`GeometryReplay::culled` and `primitive_numbered`: the vertex
  stage captured, then `primitive_replay.wgsl` captured once a vertex of each primitive, writing
  a dropped primitive's vertices outside every clip volume or each with its number; its section).
  Absent by the plan: tessellation on WebGL 2 (plan section 7: "Hull and domain shaders are not
  offered on WebGL 2 (feature level 10_0 does not have them)"). Also open on both line routes: a vertex program
  converted with `VIEWPORT_DEPTH_FLIP` (a swapped depth range) is cut at the wrong plane where a
  line crosses the near plane, as both line programs take Direct3D's near plane z = 0 (or z = -w
  under `CLIP_DEPTH_REWRITE` on the replay route); not tested. Waits: nobody (the Direct3D 11
  front at feature level 10_0 on WebGL 2 now has its geometry shaders, their stream output and
  its alpha-antialiased lines). Takes: each its route and its drawn test on ANGLE. dxc's build of
  `vs_cull` is drawn on both cull routes since run 87.
- A6. Converter part closed in G6b (runs 65, 66, 68 and 69): DXIL geometry, hull and domain shaders convert through the
  passes the DXBC route uses and draw equal to real Direct3D with dxc's builds of every program
  (`geometry_draw`, `tessellation_draw`); a DXIL pixel shader reads its layer and viewport. Since
  run 68 a geometry shader's inputs and outputs may be arrays of rows (`float3 c[2] : COLOR0`, which
  dxc writes as one signature element of several rows), one register a row, on both routes; an array
  of arrays among them is refused by name (`OutputArray`, no file of any corpus has one). Of the four
  Direct3D 12 rig scenes that failed for A6, two (geometry, streamout) draw equal to real Direct3D 12.
  Not met: raster-fill (64 of 16,384 pixels beyond 1) and tessellation (310 of 32,768 beyond 4) differ
  on the RTX 3080, as since run 66 (the fifth check's scenes.log at 8c498a03, and run 79's
  scenes1.log at 0b071580, the same counts), only in cells drawn as
  lines (wireframe, and the isoline patch). Tessellation, measured to the end in run 74 (its
  section): the tessellator's points and indices equal WARP's in order for every topology
  (`tessellator_matches_direct3d`), and the rig's scene drawn on WARP through the stage helper
  equals Direct3D 11 on WARP in every texel, both builds
  (`rig_tessellation_scene_draws_as_direct3d_11_on_warp_does`); on the RTX 3080 the scene with
  order made irrelevant is equal in every texel (`..._in_any_order`), and its 191 texels apart
  unblended are which of two triangles sharing an edge is drawn last: the RTX 3080 orders a patch's
  primitives its own way (inner ring first, the outer backwards; no rule of the reference's), which
  Direct3D leaves to the device. Raster-fill runs no geometry, hull or domain shader; its line cells
  are the wireframe of wgpu's Vulkan path, where the Direct3D 12 front draws (the WebGPU track's
  measurement: 214 pixels of that picture apart on Vulkan, none on wgpu's Direct3D 12 path), the
  listed loss of plan 15.1, which the Direct3D 12 tessellation scene's lines meet too. The order
  measurement above is of the Direct3D 11 harness (`tessellation_draw`); the Direct3D 12 rig
  picture's differences are another cause. G6b's check counted them per 64-pixel cell (its
  cats.py, threshold 4): 308 of the tessellation picture's 310 are pixels Direct3D 12 left at the
  clear colour and ours drew, all in line cells, so a rasterisation (line rule) difference, not
  an order one; raster-fill's 64 are 44 drawn by ours only, 6 by Direct3D 12 only and 14 by both;
  and the rig's topology picture, which runs no geometry, hull or domain stage, fails the same way
  (43 of 65,536 beyond 1, largest 229). Waits:
  nobody (no front program seen depending on it). Takes: nothing of the converter. Either the
  owner rules the WARP proof the bar for line cells whose order the device chooses, or the rig's
  comparison of the Direct3D 12 scenes takes an allowance for line cells (the rig's work; plan
  15.1's loss); copying the RTX 3080's order is not proposed (one GPU's choice, undocumented).
- A7. The stage helper refuses by name: several streams of lines or triangles (`GeometryStreams`,
  which Direct3D does not allow; several streams of points convert since run 49); indexed strips
  and triangle strips with adjacency run since run 50; indirect draws through the stages, the
  primitive's number behind a geometry shader (the rig's geometry scene's `ps_id`, the Direct3D 11
  front, which draws it with the back end's native primitive index today). Typed buffers in
  geometry shaders convert and run since run 86 (`TypedBufferInGeometry` gone; drawn equal to
  Direct3D 11, `typed_buffers_in_geometry_shaders_draw_as_direct3d_does`, both builds; the front
  gives the view's `HelperValue::TexelBufferView` in the geometry stage's helper block, set 3 of
  `geometry_sets`). Cull distances written by a geometry shader convert and are culled since run
  90 (its section; the runner's `clip-cull-distance-1-18` to `-21-gs`, both builds, convert):
  the geometry stage's records end with their slots and the stage helper's geometry cull program
  lists the kept primitives for the final draw. Still refused by name: cull distances written by a
  domain shader (`CullDistance`; no file of any corpus or real program seen with one; takes the
  same cull program over the tessellator's listed primitives and a drawn test), and a geometry
  shader writing them in the replay form (`GeometryCullDistanceReplay`, WebGL 2; takes a replay
  slot that sees every vertex of its primitive).
- A8. The WebAssembly build converts 27 shaders, all DXBC, to modules whose expression order
  differs from the native build's (G6a run 4, wasm.log, every corpus including the runner's; no
  DXIL file differs; the same 27 and nothing else at run 76's tip, g6a-r76/wasm.log, where every
  verdict's WebGPU part equals the native one in all nine corpora). The build did not link from
  260d36d8 (2026-11-21, another track's `thread_local` in cpp/dxil.cpp needing
  `__cxa_thread_atexit`, which wasi-sdk's C++ runtime for `wasm32-wasip1` lacks) until run 76. Five outside the runner corpus, as since G6a run 1: Wine's
  `116_cs_group_index_code`, the games' `d599fbc7efa911be`, `bfa45b98c14fa486` and
  `f092830b5c83e017`, our own `cs_one_d`. In the runner corpus, measured first in G6a run 4, 22
  more: `arithmetic-int-uniform-1` to `-6` and `-8` to `-11`, `arithmetic-uint-5`,
  `clip-cull-distance-1-11` to `-15` and `-17`, `eval-attrib-1`, `function-return-5` and `-6`,
  `instance-id-3`, `interface-packing-8`. What differs: the two results of the unsigned divide
  helper come out in the other order (11 of the 22 and the first five), the two comparisons of a
  range check joined by a logical and come in the other order (9), and the loads of a select's
  operands are made in the other order, which renumbers them (2, and with the and in 8). Same
  meaning (read in four of the runner's, the rest sorted by the lines that differ); a cache made
  natively and one made in the browser disagree on those entries (counted, the first kept),
  against section 8's "same bytes on both". Cause: dxbc-spirv builds operands inside one call's
  arguments, whose order C++ leaves free (gcc and MSVC right to left, clang left to right). Waits:
  nobody until a native and a browser host share one cache. Takes: the operands sequenced in
  dxbc-spirv (upstream, or a patch carried as naga's are).
- A9. Closed in run 86 (its section): vertex shaders writing storage buffers or storage textures
  convert (no longer `VertexStorageWrite`), `stage::writes_storage` names them, and the stage
  helper runs them as compute and draws their records through the cull route's pass-through
  (`Stages::stored`, `SlowPath::VertexStorageAsCompute`, plan section 7), as do its other routes
  that run the vertex stage as compute. Drawn equal to real Direct3D 11.1 on the RTX 3080 (the
  picture, and every word and texel each vertex of a whole primitive writes), fxc and dxc builds
  (`vertex_storage_writes_through_the_stage_helper_match_direct3d`, tests/stage_draw.rs). Left:
  adjacency without a geometry shader (`VertexStorageAdjacency`, as the cull route refuses it);
  WebGL 2 has no storage (the back end's `Absent(VERTEX_STORAGE_WRITE)`, feature level 10_0).
- A10. Clip distances through the stage helper (`gs_clip` behind a geometry shader, `vs_cull` on
  the cull route) match Direct3D 11 at the default seeds 1 to 4, but at other seeds a few texels
  are drawn on one side only. The third check (seed 9001, 16 cases, at 64787579) found four more,
  all this pattern: seed 9002 `gs_clip` TriangleStrip (10 of 693 texels, largest 60), 9002
  `vs_cull` TriangleList indexed (5 of 868, largest 49), 9012 `vs_cull` TriangleStrip (3 of 1,140,
  largest 4), 9013 `gs_clip` TriangleStrip (7 of 459, largest 147); the replay on WebGL 2 fails
  9002 and 9013 `gs_clip`. G6a run 1 (gpu-all.log, `geometry_shaders_draw_as_direct3d_does` at
  seed 4747, 12 cases): 336 comparisons, 8 failed. Four are this gap, the check's own: seed 4749
  `vs_cull` TriangleList indexed, 4753 and 4756 `gs_clip` TriangleStrip, 4756 `vs_cull`
  TriangleList indexed (largest 75, a few texels of 600 to 1,700 drawn). The other four are
  scenes Direct3D itself draws nothing of (0 texels drawn, 0 differ: seed 4747 and 4758 `vs_cull`
  TriangleStrip indexed, 4756 and 4758 `gs_adjacent` TriangleStripAdjacency indexed), which the
  test counts as failures so that no case passes empty; the seeded scenes, not the conversion. The
  texels seen in run 47 lie away from the clip line (texel (14, 41) of seed 4749 is 0.65 from
  vs_cull's plane, (27, 48) of seed 4753 at least 0.16 from each of gs_clip's three), on the edges
  of drawn triangles. G6a's check found more at seed 5101, 12 cases: 5106 `vs_cull` TriangleStrip
  (1 of 1,158 texels, largest 147), 5108 `vs_cull` TriangleList indexed (2 of 762, largest 121),
  5110 `gs_clip` TriangleList. The fourth check (seed 6201, 20 cases, at 2bc9756e) found four more
  triangles of this pattern: 6205 `gs_clip` TriangleList (6 of 841 texels, largest 147), 6210
  `gs_clip` TriangleList (6 of 602, largest 67), 6211 `gs_clip` TriangleStrip (7 of 516, largest
  192), 6214 `gs_clip` TriangleStrip (7 of 335, largest 111); the replay on WebGL 2 fails the same
  four. Cause, inferred and not measured: Direct3D clips the triangle into new
  vertices, whose rounding moves its edges by a fraction of a texel, where the pixel program clips
  by the interpolated distance and leaves the edges as drawn. Waits: nobody (a texel on an edge).
  Takes: clipping the triangles in the stage helper's compute (the polygon cut at each plane, as
  the rasteriser does), then the same seeds compared again. (Seeded strips are drawn again until
  they hold a whole primitive since G6a run 2, so no scene is empty for want of one.)
  On lines, besides the clipped ends Known gaps lists: the fourth check found one texel of a
  `vs_cull` line strip that Direct3D draws and ours leaves as background, away from the clipped
  ends and from the plane (seed 6202, `vs_cull` LineStrip 12 from 0, indexed, base 0: 1 of 146
  texels, texel (24, 22), Direct3D [22, 113, 71, 191], ours [32, 64, 127, 255]; its centre is 0.26
  from the plane, the allowance near it 0.044). Cause: not found; it is not the clip at the plane.
  Takes: that seed's two pictures and the line's vertices compared at the texel (which segment
  Direct3D drew it from, and whether ours culled or clipped that segment), then the fix and the
  seed again. The fifth check (seed 7301, 24 cases, at e337c38a) found the opposite on `vs_cull`
  lines, away from the clipped ends: ours draws texels Direct3D leaves as background. Seed 7313
  `vs_cull` LineList 20 from 0: 239 texels drawn by Direct3D, 5 differ, 2 at a clipped end, the
  others up to 158 (allowed 2), for example (17, 17) and (23, 20), Direct3D background, ours [215,
  54, 55, 191] and [190, 47, 51, 191]; seed 7313 `vs_cull` LineStrip 9 from 1 x2: 271 drawn, 6
  differ, 4 at a clipped end, for example (25, 17), ours [199, 70, 55, 191]. G6a run 5 drew
  the same seeds at its tip and got the same figures (g6a5/gpu.log; in the LineList ours also
  leaves (13, 21) and (23, 21) blank where Direct3D draws). Cause: not found
  (not measured whether ours keeps a segment Direct3D culls).
  Takes: as for seed 6202, which segment each texel lies on and that segment's cull distances
  at both ends. The same check also failed seeds 7308 `vs_cull` TriangleList and 7319
  `vs_cull` TriangleStrip indexed (the triangle edge pattern above), and its seed 8801 run 8
  triangle edge cases. The sixth check (seed 9901, 32 cases, at 211e0656) found the same pattern
  at about the same rate and nothing new: 9906, 9909 (20 texels differ, largest 75) and 9924
  (largest 133) `gs_clip` TriangleList, 9906, 9920 and 9931 `vs_cull` triangles, and 9932 `vs_cull`
  LineStrip indexed (2 of 159, largest 88); the replay on WebGL 2 fails the same five `gs_clip`
  cases. The G6a fourth check of run 64's tip (seed 31337, 16 cases): 448 comparisons, 3 failed,
  all this pattern (31344 `vs_cull` TriangleList indexed 3 of 778, largest 132; 31351 `gs_clip`
  TriangleStrip 8 of 816, largest 81; 31351 `vs_cull` TriangleList 3 of 1,669, largest 68); the
  replay on WebGL 2 failed 31351 `gs_clip` (352 comparisons).
  The G6b fourth check (seed 8501, 16 cases, at 3afa2f5a; both routes since G6b): 768 comparisons,
  8 failed, all this pattern on both routes: 8501 `gs_clip` TriangleStrip 14 from 2 (DXBC and DXIL,
  10 of 571 texels, largest 120), 8503 `vs_cull` TriangleList 30 from 3 x2 DXBC (3 of 1,646, largest
  45), 8503 `vs_cull` LineList 20 from 0 DXBC (1 of 220, largest 64), 8505 and 8508 `gs_clip`
  TriangleList 30 from 3 x2 (both routes); the replay on ANGLE, 608 comparisons, 6 failed, the same
  `gs_clip` cases.
  The G6b fifth check (seeds 4401 to 4420, at 8c498a03): 960 comparisons, 12 failed, all this
  pattern, each DXIL failure mirroring a DXBC one: 4406 `gs_clip` TriangleList (6 of 1,081 texels,
  largest 171), 4409 `vs_cull` TriangleStrip (3 of 1,161, largest 89), TriangleList indexed base 4
  (2 of 1,068, largest 144) and TriangleStrip indexed (4 of 1,107, largest 128), 4414 `gs_clip`
  TriangleStrip (5 of 730, largest 64), 4416 `vs_cull` TriangleStrip indexed (largest 102), 4419
  `gs_clip` TriangleList and TriangleStrip; the replay, 760 comparisons, 8 failed, the same `gs_clip`
  cases.
  G6a's second check (seed 3131, 8 cases, at 47ed2fce): the same pattern and nothing new, seed 3135
  `gs_clip` TriangleStrip 14 from 2 x1, DXBC and DXIL alike (749 texels drawn by Direct3D, 4 differ,
  none at a clipped line end, largest 64, allowed 2); the replay on WebGL 2 fails the same two.
  G6a's second check of the first's findings (seed 2718, 16 cases, at 9fb6e0d1): 768 comparisons,
  3 failed, all this pattern: 2732 `gs_clip` TriangleList 30 from 3 x2, DXBC and DXIL (989 texels
  drawn by Direct3D, 6 differ, largest of the others 96, allowed 2), 2732 `vs_cull` TriangleList 30
  from 3 x2 DXBC (10 of 1,770, largest 126); the replay on ANGLE, 608 comparisons, 2 failed, the
  same `gs_clip` case on both routes.
  G6a's third check (seed 5555, 20 cases, at b5222b3d): 7 failed, 6 of this pattern; the replay
  fails 5569 `gs_clip` on both routes; depth clamping stayed within A12's allowance in all 120
  comparisons. The seventh, 5562 `vs_cull` LineStrip 9 from 1 x2, was an empty scene: the planes
  drop all 8 primitives, Direct3D draws nothing and ours nothing (0 differ). The re-draw above
  covers only indexed scenes; since run 77 the test counts an empty scene as agreement where the
  planes dropped primitives of it and ours drew nothing either (an empty scene with no primitive
  dropped still fails). Run 77 drew the same seeds with that change (g6a-r77/gpu-draw.log; one GPU
  job): 960 comparisons, 6 failed, all this pattern: 5561 `vs_cull` TriangleStrip 15 from 2 (4 of
  1,326 texels, largest 47), 5562 `vs_cull` TriangleList 24 from 3 indexed base 4 (1 of 1,143,
  largest 49), 5564 `vs_cull` TriangleList 30 from 3 x2 (16 of 1,420, largest 56), 5567 `vs_cull`
  TriangleList 24 from 3 indexed base 4 (1 of 270, largest 109), 5569 `gs_clip` TriangleList 30
  from 3 x2, DXBC and DXIL (17 of 917, largest 97); 5562's LineStrip passes (0 drawn, 0 differ).
  G6a's fourth check of run 78 (seed 40961, 24 cases, at 5c70cc91; gpu2.log): 1,152 comparisons,
  14 failed, all this pattern, 0 texels at a clipped line end in each: 40961 `vs_cull`
  TriangleStrip 16 from 1 x2 indexed base 1 DXBC (7 of 1,122, largest 81), 40976 `vs_cull`
  TriangleList 24 from 3 indexed base 4 (3 of 1,268, largest 113), 40981 `gs_clip` TriangleList 30
  from 3 x2 DXBC (14 of 1,071, largest 111), and `gs_clip` on both routes at 40962, 40980, 40982 and
  40983; the replay on ANGLE, 912 comparisons, 12 failed, the same `gs_clip` cases.
  G6b's check (seed 80701, 24 cases, at 13d2f093; its gpu1.log): 1,344 comparisons, 63 failed, 48
  of them the checker's own degenerate input and 15 this pattern, each DXIL failure mirroring a
  DXBC one: 80703 `gs_clip` TriangleStrip 14 from 2 (4 of 689, largest 141, both routes), 80721
  `gs_clip` TriangleList 30 from 3 x2 (6 of 1,027, largest 130, both routes), 80704 `vs_cull`
  TriangleList 24 from 3 indexed base 4 (7 of 795, largest 96), 80715 `vs_cull` LineList 20 from 0
  (3 differ, 1 at a clipped end, largest of the others 103, the line pattern of seeds 7313 and
  8503); the replay on ANGLE, 1,104 comparisons, 60 failed, 48 the checker's and 12 the same
  `gs_clip` cases. Depth clamping passed all 24 seeds.
- A11. `mutated_bytecode_converts_or_fails_without_ending_the_process` (tests/fuzz.rs, ignored by
  default as a long run) fails: signed but malformed bytecode can end the process (G6a run 1,
  fuzz.log: seed 7, case 301, a mutated `blend.vs.dxil`, SIGSEGV; the second check, seed 8128,
  4,000 cases, restarted past each crash: 26 crashes, all DXIL; the third check, seed 5555, 6,000
  cases: 27, 24 SIGSEGV and 2 `std::terminate` on DXIL and 1 on DXBC; the fifth check, seed
  24680, 8,000 cases: 31, 26 SIGSEGV and 4 `std::terminate` on DXIL and 1 on DXBC; the fourth
  check of run 64's tip, seed 86420, 6,000 cases: 27, all DXIL inside dxil-spirv, 20 SIGSEGV and
  7 `std::terminate`, none on DXBC or in this crate's Rust; G6a's second check at 47ed2fce, seed
  97531, 2,500 cases: 14, all DXIL inside dxil-spirv, 11 SIGSEGV and 3 `std::terminate` (for
  example case 20, `textures.ps_2d_array.dxil` with a byte flipped at 3834); G6a's second check
  of the first's findings at 9fb6e0d1, seed 2718, 4,000 cases: 24, all DXIL inside dxil-spirv, 16
  SIGSEGV and 8 `std::terminate` (for example case 172, `ps_mirror.dxil` with a byte flipped at
  3700); G6a's third check at b5222b3d, x86 machine, seed 4321, 5,000 cases, `ulimit -v 8000000`:
  31, all on DXIL inputs, 26 SIGSEGV and 5 `std::terminate` (for example case 0,
  `hs_line_integer_point.dxil`, and case 3638, `hs_quad_integer_ccw.dxil`, "terminate called
  without an active exception"); G6a's fourth check of run 78 at 5c70cc91, x86 machine, seed 60606,
  6,000 cases: 41, every one a DXIL input, 30 SIGSEGV and 11 `std::terminate` (case 179
  `vs_texels.dxil` word@2274, case 2705 `vs_rows_io.dxil` word@3992, case 2830 `ps_dual.dxil`),
  and the three backtraced stop inside dxil-spirv under `wcs::contained` (cpp/dxil.cpp): case 179
  in `NamedMDNode::getNumOperands` on a null node (bc/metadata.cpp, from `get_entry_points`), case
  2705 in `cast<ConstantAsMetadata>` (from `emit_stage_output_variables`), case 2830 in
  `cast<MDNode>` (from `emit_stage_input_variables`), all metadata the reader takes unchecked; Known
  gaps has the earlier counts). Causes:
  - DXIL: dxil-spirv's bitcode reader faults or calls `std::terminate` on such input.
  - DXBC, the third check's case: this crate's own lowering took a resource array's declared count
    unchecked and asked Rust for gigabytes for its register list. Fixed in G6a run 3: a declared or
    ranged count past the slots its kind binds is refused (`BindingsBeyondLimit`) and the register
    end is a checked add (`a_resource_array_longer_than_its_kind_binds_is_refused_by_name`).
  - DXBC, the fifth check's case (seed 24680, case 1397, `113_cs_1_group_code.dxbc` `word@476`):
    not ours. The word replaced is the register index of `r1` in that program's `iadd r1.y, r1.y,
    l(1)` (read from the file in G6a run 5), and dxbc-spirv's `RegisterFile::getOrDeclareTemp`
    (dxbc/dxbc_registers.cpp) resizes its list of temporaries to that index times four, unchecked
    against `dcl_temps`; its lists of indexable temporaries, labels, function bodies and
    tables and `g` registers are sized the same way. Natively with no limit the process grew until the
    kernel's out-of-memory killer ended it (the check's dmesg); under `ulimit -v` the same case is
    a rejection (G6a run 5 too), taken to be dxbc-spirv's `bad_alloc` caught at the boundary
    (cpp/boundary.h; inferred, the message not printed). In WebAssembly,
    where the C++ has no exceptions, an allocation the module cannot make ends the instance.
  Only bytecode whose checksum matches reaches the converters, so these need a program that signs
  malformed bytecode. Waits: nobody. Takes: on DXIL, the reader hardened upstream (not yet
  reported), or the converters run where a fault cannot end the host; on DXBC, the register
  indices checked against their declarations in dxbc-spirv (upstream, or a patch carried as naga's
  are); this crate cannot bound them without reading the bytecode itself.
  Also from that check (seed 2718, case 3700, `ps_textured.dxil` flipped at 1296 and 3357, a word
  replaced at 2358): the carried naga SPIR-V reader panicked on dxil-spirv's output
  (third_party/naga/src/front/spv/image.rs:775, an `Option::unwrap` of `None`). Natively
  `contained` turns it into `Malformed` ("the conversion panicked"), so the test passes that case;
  in WebAssembly, where a panic aborts, it would end the instance, as A16's validator panics do.
  Cause: the reader looks up its own tables by ids it does not check there. Waits: nobody. Takes:
  the reader's lookups checked (a carried patch, then upstream).
- A12. `DEPTH_CLAMP` behind a geometry stage (`depth_clamped_behind_a_geometry_shader_draws_as_direct3d_does`)
  is within four units in the last place of Direct3D 11's depth at the default seeds 1 to 4 (16
  comparisons), but not at all seeds. The test prints, for each case, the texels Direct3D clamped
  to either end of the range, then the texels of the whole picture whose depth differs at all
  (clamped or not, so the second count may pass the first), then the largest difference, and in
  brackets that difference in steps (units in the last place); the allowance is the largest
  difference, 4.77e-7 (four units in the last place at a depth of 1), so a case of 8 steps
  at smaller depths passes (2.38e-7 at seed 3131, run 71). Figures so read: G6a's check, seed 5103 `gs_cut`
  TriangleList, 470 clamped, 37 differ, largest 33 steps; seed 5104 `gs_cut` TriangleStrip, 177
  clamped, 173 differ, 59 steps; the third check, seed 9014 `gs_cut` TriangleStrip, 40 clamped, 24
  differ, 20 steps; the fourth check, seed 6212 `gs_cut` TriangleStrip, 781 clamped, 173 differ, 20
  steps (largest 5.96e-7, allowed 4.77e-7); and seed 4753 `gs_cut` TriangleStrip, 1,101 clamped, 72
  differ, 23 steps (in G6a runs 2 and 3's seed 4747 runs and again in run 4); the fifth check and
  G6a run 5 at its tip, seed 7305 `gs_cut` TriangleList, 26 steps (the only one of 144 past the
  allowance; the others at 1 to 4 steps), and the check's seed 8801, 18 and 11 steps; the sixth
  check (seed 9901, at 211e0656), 4 of 192: 9920 (32 steps), 9926 (44) and 9931 twice (20), all
  `gs_cut`; the G6b fifth check (seeds 4401 to 4420, at 8c498a03), seed 4418 `gs_cut` TriangleList,
  139 clamped, 83 differ, 26 steps (largest 7.75e-7, allowed 4.77e-7); G6a's second check (seed
  3131, 8 cases, at 47ed2fce), 2 of 48: seed 3137 `gs_cut` TriangleList, 97 clamped, 189 differ, 28
  steps (largest 8.34e-7), and seed 3138 `gs_cut` TriangleStrip, 165 clamped, 208 differ, 10 steps;
  G6a's second check of the first's findings (seed 2718, 16 cases, at 9fb6e0d1), 2 of 96: seed 2727
  `gs_cut` TriangleList, largest 6.56e-7 (22 steps), and seed 2731 `gs_cut` TriangleStrip,
  7.15e-7 (12 steps); G6a's fourth check of run 78 (seed 40961, 24 cases, at 5c70cc91), 1 of 144:
  seed 40981 `gs_cut` TriangleStrip, 155 clamped, 219 differ, largest 6.56e-7 (11 steps). Cause:
  not found. Both are `gs_cut`; the pixel program clamps a depth
  interpolated as a value between stages from the pass-through's unclamped depth, where Direct3D
  clamps its rasteriser's own depth, so the two interpolations may round apart by more on some
  triangles (inferred, not measured). Waits: nobody (depths some millionths
  apart). Takes: the depths compared texel by texel at those seeds to find whether the
  interpolation or the clamp moves them, then the rewrite changed and every seed compared again.
- A13. DXBC part closed in run 89 (its section): a geometry shader taking patches converts and the
  stage helper draws a patch list into it without tessellation, drawn equal to real Direct3D 11 on
  the RTX 3080 (`geometry_shaders_taking_patches_draw_as_direct3d_does`); the runner's five DXBC
  files convert. Left: the DXIL route (`GeometryPatchInput`, runner `patch-syntax-12-gs.dxil`, 1
  file): cause, dxil-spirv converts no geometry shader taking patches (its input primitive switch
  ends "Unexpected input primitive"); and the replay route on WebGL 2 (`TopologyMismatch` for a
  patch list, not built or drawn). Waits: the Direct3D 12 front's runner (that file); nobody for
  WebGL 2 (no program seen). Takes: dxil-spirv taking patch input (upstream, or a patch carried
  as naga's are, which needs the way to carry C++ patches A8 also waits on), then dxc's `gs_patch`
  drawn; for WebGL 2, the replay capturing a patch list as points, as lists with adjacency are,
  and a case of the replayed test.
- A14. DXBC part closed in run 89 (its section): a hull shader of no output control points and a
  domain shader of none convert, streamed equal to Direct3D 11 on WARP
  (`hulls_giving_no_control_points_stream_as_direct3d_does`); the runner's
  `hull-shader-attributes-10` converts. Real Direct3D 11 on the RTX 3080 ended the test process
  dividing by zero on the same draw, so the RTX 3080 gives no reference for it. Left: dxc's domain
  shader of no input control points (`DomainWithoutControlPoints`, runner `patch-syntax-11-ds.dxil`,
  own `ds_tri_none.dxil`), as dxil-spirv declares its patch an array of none; dxc builds no hull
  shader of none. Waits: the Direct3D 12 front's runner (that file). Takes: the array of none
  dropped from dxil-spirv's SPIR-V before naga reads it (the domain program reads no control
  point), then the own pair drawn with dxc's domain shader.
- A15. Stream output of a stream whose buffers are all unbound (`streams_past_the_first_match_direct3d`,
  tests/geometry_draw.rs): the primitives made match Direct3D 11, the primitives written do not.
  On the RTX 3080 real Direct3D 11 counts some of such a stream's primitives as written, ours 0
  (the fifth check at e337c38a, and G6a run 5 at its tip, seeds 7301 to 7324: 96 comparisons, 24
  differ, each the case "30 from 0 x1, buffers 1", whose stream 3 has no buffer: Direct3D's
  written and made [25, 54], ours [0, 54], the other streams and every made count equal). The
  test compares only the made count of such a stream, so it passes. Cause: no rule found for the
  GPU's count (25 of 54 there, 0 of 84 in another case); ours writes nothing and counts
  nothing written, as no buffer takes it. Waits: nobody (no program seen reading the written count
  of an unbound stream). Takes: the same draws on WARP as a second reference; then either the
  count matched or the difference recorded as the GPU's own.
- A16. The cache survives a damaged entry natively, not in WebAssembly, where a panic aborts and
  ends the instance (pass bar item 6). Each entry's CRC drops a damaged one before decoding, so
  this needs a record whose checksum matches but whose contents are malformed (a crafted, merged
  or foreign file). Decoding validates the modules with naga's validator, which indexes such a
  module unchecked; natively `contained` (catch_unwind) turns the panic into a refusal. Our own
  `capabilities` read the types unchecked too until G6a run 6 (now `get_handle`). Tests:
  `damaged_records_decode_to_a_conversion_or_an_error` (no panic reaches the caller; passes
  natively, ignored on wasm32) and `damaged_records_decode_without_a_panic` (no panic at all,
  contained or not; ignored as a known failure, failing). Measured on the laptop at G6a run 6's
  tip (load 4.5 before, the other track's work beside), seeds 4242 and 1 to 5, 1,000 damaged copies of
  each of 189 records (g6a6/panic-sites.log, the strict test with its assertion removed for the
  count): 1,049 panics among 1,134,000 decodes, all in naga (analyzer.rs:641, a function argument's
  index past the arguments, 935; typifier.rs:122 57; compose.rs:22, a composed type's handle, 35;
  function.rs:617 and :592, an atomic's result type, which naga's handle check skips, 9;
  analyzer.rs:1116, a called function's information before it is made, 6; expression.rs:1326 3;
  analyzer.rs:1247 3; constant_evaluator.rs:768 1), none in this crate. G6b's check reproduced
  it on other files: x86 machine, seed 80777, 80 damages, `cs_divide.dxbc` case 18 (analyzer.rs:641,
  index 76 past a length of 1); laptop, seed 80778, `ps_levels_past.dxil` case 6.
  Waits: nobody (no host shares a cache file it did not write yet). Takes: either naga's validator
  checking every index and handle before use (carried patches, then upstream), or a record
  signed with a key the host holds so a foreign record is never decoded.
- A17. Aliased lines (`MultisampleEnable` off) into a multisampled target: Direct3D 10 and 11
  cover every sample of each pixel the diamond-exit rule picks; WebGPU draws lines into a
  multisampled target by its own rule. The plan lists the rule as a loss (4.12, "Which line
  rasterisation rule"), so it is not on the pass bar's words, but the Direct3D 11 front waits on it
  (its known failure `aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does`,
  "Needs an aliased-line route (the shaders track and the interface)"). Cause: not built; the
  raster state has no word for the rule, and no stage helper route draws lines by it. Takes: the
  interface's word for it (the interface track's), then a route: the line's vertices captured by
  the stage helper as for the cull route, each segment drawn as the pixels the diamond-exit rule
  picks (a quadrilateral per segment, its pixel program discarding the pixels the rule leaves and
  writing every sample of those it keeps), and a drawn test against Direct3D 11 into 4x targets.
- A18. `filtering_in_the_program_matches_direct3d`'s comparison against WebGPU's own sampler has
  no level-of-detail allowance, where the main comparison has one (a texel still apart from both
  references after the level of detail is moved by 1/256 either way). At the sixth check's seed
  9911 (10 cases) it failed one case that is not anisotropic: 9913, `Case { linear: [true, true,
  false], anisotropy: 1, address: [2, 1, 2], bias: 0.0, lod: [0.0, 3.4e38], value: 1.5, scale:
  [1.0, 0.5] }`, Rgba8Unorm `bias`, both routes: 4 of 4,096 texels past 1/128 from WebGPU's own
  sampler and from warp. G6a run 6 at its tip, the same seed (gpu.log, FILTER): see the run's
  section above. The main comparison of the same case finds the same 4 texels ("4 before moving
  the level of detail") and passes them with the allowance; WebGPU's own sampler equals Direct3D
  on the GPU there (0 texels apart), so the 4 are texels whose level of detail ours computes on
  the other side of a level decision from both. Cause: the program's level of detail (from
  derivatives, as warp computes it) is not bit for bit the GPU's. Waits: nobody (a texel's level
  of detail at a decision). Takes: either the GPU's level of detail computation matched exactly
  (not documented for the RTX 3080), or the owner ruling that the main comparison's allowance
  applies to this comparison too; the test is left as it is meanwhile.
- A19. Normalised codes with halves toward zero (`variant::texel::encode`, run 65, the Direct3D 11
  front's ask from its measurement on the RTX 3080: 0.5 into a 16-bit normalised texture held as
  its integer twin gives 0x7fff there): built, but no drawn test of this crate writes an exact
  half, so the rule is the front's measurement, not this crate's; the drawn tests that run the
  encoding (`targets_in_shader`, storage formats, typed buffers) pass with it. Waits: the
  Direct3D 11 front (its difference list). Takes: an own program writing 0.5 and -0.5 into 8- and
  16-bit normalised targets, storage textures and typed buffers, compared exactly with real
  Direct3D 11 (fxc and dxc builds).
- A20. A geometry shader after a domain stage reading `SV_PrimitiveID`
  (`GeometryPrimitiveIdAfterDomain`, src/stage/helper.rs `tessellated_with`): refused by name. Own
  `gs_values_tess` (both builds) and the check's `gs_own_tess` meet it. Cause: the domain stage's
  records do not hold the patch's number, which Direct3D gives such a geometry shader. Waits:
  nobody (no corpus or real program seen with it). Takes: the patch's number written into each
  domain record by the domain program, read by the geometry stage as its primitive, and a drawn
  test against Direct3D 11 (`gs_values_tess` with `hs_values_*`).
- A21. Float arithmetic is not bit-exact across compilers: Direct3D and the back ends' compilers
  may each contract a multiply and an add and divide by a constant's reciprocal, and a program
  built IEEE-strict (`-Gis`, `precise`) forbids it, which WGSL cannot say. Measured (G6b run 3,
  draw3.log): dxc's `-Gis` builds of `gs_unplaced` and `gs_cut` on Direct3D 12 against ours, 1 to
  4 ulp in words of a multiply and an add; the same programs built without `-Gis`, up to 2 ulp on
  WebGPU and 4 on the replay route (ANGLE) at seeds 1 to 4. The fourth check found 8 ulp on the
  replay route at its seeds 8501 to 8516 (`gs_cut` strips, 4 of 16 seeds; for example 0.11680809
  against 0.116808146): `gs_cut`'s lerp from the centre takes a difference of positions near 1, so
  the result keeps their rounding and its own last place is no measure of it. Since run 68 the
  allowance on dxc's builds' normal floats is 4 units in the last place of the larger of the two
  values and 1 (`CONTRACTED_ULPS`, `OPERAND_MAGNITUDE`: the tests' operands are positions in the
  clip volume, colours and constants of at most about 1), where it was 4 ulp of the value itself.
  At the check's seeds 8501 to 8516 the stream tests then pass on the replay route (384
  comparisons, 0 failed) and on WebGPU (448, 0 failed); every DXBC case is exact (g6b4/probe1.log).
  The same may hold for one colour texel the third check of G6a found and run 72 drew again at its
  tip (seed 5252, 12 cases, gpu-err.log): seed 5257 `gs_instances` TriangleList 6 from 0 x2, dxc's
  build on WebGPU, 1 of 1,269 texels apart from the test's Direct3D 11 reference by 1, where colours are allowed 0; fxc's
  build of the same case is exact, and the replay on ANGLE is exact for both builds. The program
  multiplies the colour by `instance / 2.0` and offsets the position by `instance * 0.3 - 0.3`;
  that dxc's build rounds one of them apart is inferred, not measured. The test fails at that seed
  (`geometry_shaders_draw_as_direct3d_does`, its other ten failures A10's). Takes: that case's vertex values
  compared between the two builds' conversions, to find the word that rounds apart.
  Waits: nobody (no program seen depending on it). Takes: nothing the converter can do in WGSL; a
  native back end could keep `NoContraction`.
- A22. Direct3D 12 on the RTX 3080 writes zeros for the position in the stream output of a
  geometry shader of several streams, whichever compiler built it and whichever stream holds the
  position (the fourth check's measurement: Direct3D 12 running fxc's build of `gs_streams` is 80
  words apart from Direct3D 11 running it, all of them zeros at the position; its own
  `gs_c4_streams_flat`, the position in stream 1, the same), where ours writes the values and
  equals Direct3D 11 running fxc's build word for word. Every count and every other word of
  Direct3D 12's equal ours. So the converter is right and this reference is the odd one. Since run
  68 the tests of several streams take each such zero from Direct3D 11 running fxc's build of the
  same program (`reference_of_streams` in tests/geometry_draw.rs: only words Direct3D 12 left zero at
  the position's places in a buffer, by the declaration and stride) and count the case as known:
  seeds 1 to 4, 32 comparisons, 0 failed, 16 known (draw1-err.log); the check's seeds 8501 to 8516,
  128, 0 failed, 64 known (probe1.log). Cause within Direct3D 12 or the driver: not found (WARP's
  Direct3D 12 not drawn). Waits: nobody (our reference only). Takes: the same draws on WARP; if WARP
  writes the positions, the difference recorded as the driver's.
- A23. A DXBC program reading a dynamically indexed range of input registers whose component mask
  has a gap (own `ds_rows_gap`: `c[1 - k].arr[k + 1].z + c[k].arr[2].x`, fxc declares the range
  `.xz`) is refused by name (`ComponentPastVector`, src/lower.rs at `COMPOSITE_EXTRACT`); before run
  68 `convert` returned an invalid module ("Index 2 is out of bounds"), since G6a at least. dxc's
  build converts. Cause, read in dxbc-spirv's IR (scratch g6b4/probe-ir.txt): the range's load
  function returns the mask's components packed (`makeVectorType`, two for `.xz`), but
  `IoMap::computeRegisterAddress` (dxbc/dxbc_io_map.cpp) addresses a component by its distance from
  the mask's first (`tzcnt`, 2 for `z`), where its own interpolation path counts the mask's bits below
  it (`popcnt`, 1). Waits: nobody (no corpus or real program seen with it; the fourth check's
  `ds_c4*`). Takes: that one line in dxbc-spirv (upstream, or a patch carried as naga's are), then
  the refusal removed and the check's `ds_c4` drawn against Direct3D 11.
- A24. The stage helper's refusals by name not listed above (G6a's second check). No file of any
  corpus and no drawn test meets one; each is a form Direct3D allows or may give, and none is
  built. Waits: nobody yet (no real program seen with one). Each takes its route and a drawn test
  against Direct3D 11:
  - `StreamOutputSystemValue` (src/stage/helper.rs, `stream_parts`): a stream output declaration
    naming a system value other than the position, for example `SV_ClipDistance`, which Direct3D
    streams. Cause: the stage helper's records hold the position and the user locations only, so
    no other system value has a place to be read from (the vertex records carry no clip distances,
    Known gaps). Takes: a record slot for each streamed system value on every route that makes
    records, read by the stream table.
  - `StreamOutputEntries`: a declaration of more than 64 entries (`STREAM_ENTRIES`, the stream
    program's table in its block). Direct3D 11 takes up to D3D11_SO_STREAM_COUNT times
    D3D11_SO_OUTPUT_COMPONENT_COUNT (4 times 128) entries, as Microsoft's documentation of
    `CreateGeometryShaderWithStreamOutput` gives it (read, not measured). Cause: the table's size
    was chosen for the tests' declarations. Takes: the table sized to Direct3D's limit.
  - `CullDistanceStreamLocations` (the cull route) and `StreamOutCaptureLocations` (the replay
    route, src/stage/replay.rs): stream output of a vertex program whose records do not hold its
    reflected output locations first. Cause: the records hold every output location of the module
    as converted, sorted, where the stream table addresses a reflected output by its place among
    the reflected ones; a key adding outputs below them (values linked for the pixel program)
    would move them. Takes: the stream table addressing each value by its record slot.
  - `AntialiasedLinesSlots` (src/stage.rs, `line_programs`): a vertex program with more than 32
    record slots (its outputs plus the slots of its clip distances) drawn as antialiased lines.
    Direct3D 11 allows 32 output registers besides them. Cause: the line route's masks of
    interpolated and linear slots are one 32-bit word each. Takes: wider masks.
  - `VertexOutputShape`, `VertexOutputForGeometry` and `VertexInputForGeometry` (src/stage.rs, the
    compute forms of a vertex program): an output that is neither a scalar nor a vector (other than
    the clip and cull distance arrays), an output built-in other than the position and those
    distances, and an input built-in other than the vertex and instance numbers. Cause: the records
    have no place for them. Whether either converter gives such a vertex program is not known
    (dxc's arrays of rows, which run 68 took for geometry shaders, are the likely first; not tried
    for vertex programs). Takes: a place in the records for each, as for geometry shaders' rows.
- A25. `rewrite` (the 15.4 entry point) and the DXIL route refuse a sample whose texture and
  sampler are picked from arrays at indices not known to be one value, or from arrays of different
  lengths (`ResourceArrayPair`, src/dxil/arrays.rs): the second check's probe
  `textureSample(t[u.pick[k].x], s[k & 1u], ...)` is refused so. A common Vulkan pattern of
  separate texture and sampler arrays. Cause: the split turns a pick into a switch over one
  array's elements; two independent picks need a switch over every pair, not built. Waits: the
  NIR track's route B on Vulkan programs (its status names no program refused so). Takes: the
  nested switch (or one switch over the pairs) and a case of `rewrite_draw`.
- A26. A float not-equal comparison that Direct3D defines as unordered (true where either operand
  is NaN) is drawn as an ordered one (false there) on wgpu's Vulkan path, on both routes (the
  third check of G6a, read in the code, not drawn). DXBC: dxbc-spirv's IR `FNe` is Direct3D's `ne`
  (its own SPIR-V writer emits `OpFUnordNotEqual`, spirv/spirv_builder.cpp), and this crate lowers
  it to naga's `BinaryOperator::NotEqual` (src/lower.rs, `c::F_NE`), which naga's SPIR-V writer
  emits as `OpFOrdNotEqual` (third_party/naga/src/back/spv/block.rs). DXIL: dxil-spirv writes
  dxc's `fcmp une` as `OpFUnordNotEqual`, and the carried naga reader takes it, and its other
  unordered comparisons, as the ordered operators (front/spv/convert.rs; the NIR track's finding,
  "New for the owner"). What a browser does with WGSL's `!=` on a NaN is not measured. No test
  covers it, no corpus record shows it (a verdict is not a drawing). Waits: nobody (no program
  seen depending on a NaN compared so). Takes: `ne` lowered as the negation of an ordered
  equality (`!(a == b)`, unordered on every back end that keeps NaN), the reader patched the
  same way for its unordered forms, and a drawn test: a pixel shader `return (a != b) ? 1 : 0;`
  with a NaN in a constant buffer, built by fxc and dxc, against real Direct3D 11 and 12.
  That test alone does not reach the gap (the fourth check of G6a, its NaN probe drawn on both
  routes and back ends, 0 of 4096 texels apart): fxc writes `a != b` as the negation of `eq`, not
  as `ne`, and wgpu's Direct3D 12 path, which the drawn tests' WebGPU runs on, writes HLSL `!=`,
  unordered already. The drawn test needs a DXBC `ne` (assembled by hand or from another compiler)
  and a back end emitting `OpFOrdNotEqual` (wgpu on Vulkan, `webgpu_on(Backends::VULKAN)`).
- A27. Refusals by name that no file of any corpus meets and that no entry above lists (the third
  check of G6a counted 92 such names in src; run 72, with the check's `git grep` of src at its tip,
  93 refusals besides five names of the helper's own programs that are not refusals; G6a's second
  check of the first's findings found 24 more unlisted and run 76 three more (`StorageFormat`,
  `TypedBuffer`, `StorageImageAtomics`), all now below, and `CompareExchangeResult`
  listed by mistake: it is a type name the DXIL route gives its compare-exchange result, not a
  refusal; run 76 checked the list against every name src refuses by, see its section). Each refuses, by name, a form `convert`, `rewrite` or the stage helper does not build; no
  record in `tests/expected` holds one, so each is untested beyond the code that refuses it. Waits:
  nobody (no corpus or real program seen with one). Takes, for each: its lowering and a drawn test
  against real Direct3D 11 or 12 (or, for the absent ones, nothing: they are reported as the guest
  expects). By kind:
  - Absent on WebGPU and WebGL 2, reported as the guest expects: `Type64BitInteger` (DXBC 64-bit
    integers), `Int16Storage`, `Int16StageValue` and `Int16Bitcast` (DXIL 16-bit integers; WGSL has
    `f16` only), `InnerCoverage` (`SV_InnerCoverage`), `MultisampledStorage` (multisampled storage
    textures), `SubgroupsInVertex` and `SubgroupBarrier` (WebGPU's subgroups serve pixel and
    compute programs only and have no subgroup barrier), `Stage` (an entry point of a naga stage
    other than vertex, fragment and compute where the route takes no other: src/dxil.rs after its
    hull, domain and geometry passes, src/lib.rs in `rewrite`), `SpirV` (a SPIR-V source).
  - Control flow dxbc-spirv may give that the lowering into naga's structured statements does not
    take (src/lower.rs): `SwitchFallthrough` (a case falling into the next), `SwitchWithoutMerge`,
    `LoopLatch` (a loop whose back edge is not the form the lowering reads), `ContinueConstruct`.
  - Arithmetic, types and memory (src/lower.rs unless named): `MulExtended` (`imul` and `umul`
    with both halves), `Type16Bit` (dxbc-spirv's minimum-precision types), `AtomicType` and
    `StructAtomic` (an atomic declaration that is not one 32-bit integer), `StructConstant`,
    `NestedArrayConstant` and `NonFiniteConstantArray` (an immediate constant buffer of structs, of
    arrays of arrays, or with a float that is not finite), `StructMembersAsVector`,
    `VectorLoadAcrossElements` and `VectorStoreAcrossElements` (a vector access spanning elements
    of an array), `BuiltInArray` and `InputArray` (stage values of an array shape the reflection
    does not take), `ClipDistanceType`, `InputTarget`, `PushData` and `StreamOutput` (dxbc-spirv IR
    declarations of an input target, of a specialisation constant or push data, codes 36 and 37,
    and of transform feedback, code 47: the lowering has no form for them; stream output goes
    through the stage helper), `ReadWriteStorageFormat` (a storage texture both read and written in
    a format other than the three one-channel 32-bit ones, the only read-write formats of WebGPU's
    core; DXBC and DXIL), `TexelsThroughBufferInGeometry`.
  - Sampling (src/lower.rs and src/dxil/depth.rs): `SampleCompareBias`, `SampleCompareGradient`,
    `SampleCompareLevel` (a comparison sample with a bias, gradients or a level other than zero,
    which WGSL's comparison samples do not take), `DepthSampleBias`, `DepthSampleGradient`,
    `DepthSampleLevel` (a depth texture sampled without comparison with a bias, gradients or a
    level that is not a whole constant, which WGSL's depth textures do not take).
  - The DXIL route's resource passes (src/dxil): `CounterUse`,
    `TexelPointerUse`, `TypedBufferUse`, `TexelsThroughBufferUse` (a use of a counter, texel
    pointer or typed buffer the passes do not rewrite), `Texture1DAtomic`, `PackedVertexInput` (a
    vertex input at a component other than the first), `StageValueType`.
  - Hull, domain and geometry values (src/lower/hull.rs, src/lower/geometry.rs,
    src/dxil/tessellation.rs, src/dxil/geometry.rs): `HullInput`, `HullInputShape`, `HullOutput`,
    `HullOutputLocation`, `DomainInput`, `DomainInputShape`, `DomainInputLocation`,
    `DomainPointCount`, `GeometryInput`, `GeometryInputShape`, `GeometryOutput` (a value of a
    built-in, shape or location the records have no place for), and `GeometryVariant` (a key
    asking the vertex stage before a geometry shader for `VIEWPORT_ADJUST`, `VIEWPORT_DEPTH_FLIP`,
    `CLIP_DEPTH_REWRITE`, pulled inputs or draw bases, which the pass-through does not make;
    src/lib.rs).
  - Variant key combinations (src/variant.rs and src/variant/): `ClipDistancePerSample` and
    `EarlyTestsPerSample` (the in-program clip and early tests of a pixel program run per sample),
    `ClipDistanceSamples` and `EarlyTestsSamples` (a sample count other than 1 and 4),
    `LinkedClipDistanceCount`, `LinkedLocationTaken`, `LodBiasOnComparison`,
    `LodBiasOnDepthLevel`, `MirrorOnceOnDepth`, `FilterInShaderDimension`, `FilterInShaderOnCube`,
    `FilterInShaderOnDepth`, `FilterInShaderOnInteger`, `PulledInputInStruct`, `PulledInputType`,
    `PulledInputWidth`, `SamplerArgument`, `TextureArgument` and `SampledThroughArgument` (a texture
    or sampler reached through a function argument where a rewrite needs its binding),
    `TextureInfoOfStorage`, `TypedBufferAtomicFormat`, `TypedBufferFormat`,
    `TypedBufferFormatAbsent`, `TypedBufferStorageAsTexture`, `StorageOffsetOfBlock`,
    `ConstantsAsTextureLayout`, `EvaluatedCentroidInput` (a centroid input evaluated in the
    program at more than one sample), `SamplePosition` (the sample position built-in read by a
    program; src/lower.rs).
  - The stage helper (src/stage.rs, src/stage/helper.rs): `StageIndexU8` (8-bit indices),
    `StageIndirect` (an indirect draw through the stages, A7), `CullDistanceAdjacency` (the cull
    route drawn with adjacency), `VertexInputNotPulled` (a vertex program's compute form given an
    input that is not pulled), `VertexWithoutPosition` (a vertex program's compute form without a
    position), `AntialiasedLinesTopology` and `AntialiasedLinesCullDistance` (the line route drawn
    with another topology than lines, or after a vertex program writing cull distances),
    `PrimitiveIdAdjacency` and `PrimitiveIdCullDistance` (the primitive route drawn with adjacency,
    or after a vertex program writing cull distances).
  - More of the DXBC route (src/lower.rs): `BuiltIn` (a dxbc-spirv built-in the lowering has no
    form for, past those named elsewhere), `DynamicOffset` (a constant buffer or structured offset
    that is not a constant where the lowering needs one, also src/common.rs), `RegisterSpace` (a
    resource in a register space other than 0 under Direct3D 11's numbering, which knows one).
  - More of the DXIL route (src/dxil.rs, src/dxil/): `ConstantComponentIndexing` (a constant
    buffer component picked at run time through dxil-spirv's second, scalar view of the buffer,
    whose 4-byte stride WebGPU's uniform arrays do not take), `NonFiniteConstant` (an infinite or
    NaN constant the reader made, where the route has no form for it), `FloatToIntWidth` (a float
    converted to an integer of another width than 32 bits).
  - More variant key combinations (src/variant.rs and src/variant/): `DecodedFormat` (a `decoded`
    texture of a format that is not normalised, or BGRA, which no integer format holds),
    `DecodedTexture` (a `decoded` slot that is not a single-sample float texture, or a depth
    texture filtered in the program), `SampledViewKind` (a texture sampled whose view's kind
    differs from its declared one), `NormAsInteger` (a target of a normalised format no integer
    format holds, written in the program), `DualSourceWithoutTwoTargets` (dual-source blending
    without targets 0 and 1), `LineCoverageTarget` (`LINE_COVERAGE` without a `vec4` target 0),
    `PrimitiveIdInStruct` (the primitive index inside an input struct where the rewrite needs it
    alone), `MinLodOnComparison` (a minimum level on a comparison sample),
    `MirrorOnceOnComparison`, `MirrorOnceOnGather` and `MirrorOnceWithOffset` (mirror-once
    addressing in the program on a comparison, a gather or a sample with a texel offset),
    `FilterInShaderOnComparison` and `BorderInShaderOnComparison` (filtering or a border colour in
    the program on a comparison sample, src/variant/filter.rs).
  - Keys a front gives without what the program needs (both routes): `StorageFormat` (a storage
    texture whose slot the key's `storage_formats` does not name), `TypedBuffer` (a typed buffer
    under `TexelBufferRoute::Native`, which neither back end has), `StorageImageAtomics` (a DXBC
    atomic on a storage texture not read through the texel buffer route; WGSL's core has no
    texture atomics).
  - The built-in programs (src/builtin.rs): `FloatClearOfInteger` (a float clear value for a
    storage view of an integer format).
  - The built-in programs (src/builtin.rs): `ClearDimension` (a storage clear of a cube or cube array view),
    `SnormMostNegative` (a clear of a signed normalised format at its most negative code, below
    minus its largest).
  - `StorageFormatAbsent`, a storage view whose format has no storage form here: raised by the
    storage clear (src/builtin.rs) and by `convert` on both routes, where a key's
    `storage_formats` or `texel_buffers` names such a format (src/lower.rs `storage_format` and the
    typed buffer layout, src/dxil/images.rs). Since run 81 `A8Unorm` storage views convert
    natively (A28); through a buffer and by the clear they still meet this refusal.
- A28. Two asks of the Direct3D 11 front (its status, "Asked", and its known failure
  `yuy2_view_sizes_read_as_direct3d_reads_them`), not on the pass bar's words, unlisted here until
  the fourth check of G6a found them. (1) A storage form of `A8Unorm`: the front holds A8 as
  `rgba8unorm` with its value in alpha, and `storage_format` has no `A8Unorm`, so a typed storage
  view of it was `StorageFormatAbsent` (A27). Built in run 81 (its section): declared
  `rgba8unorm`, stores writing `(0, 0, 0, a)`, drawn equal to Direct3D 11 for stores on both
  routes; loads not drawn; through a buffer and by the built-in clear still absent. (2) A helper value giving a program a view's width
  other than its texture's (`GetDimensions` of a YUY2 view: Direct3D gives the texture's width in
  pixels, ours the view's pixel pairs). Waits: the Direct3D 11 front. Takes: an interface helper
  value (the interface track's) and `TEXTURE_INFO` reading it in place of the texture's width.
  The second is not built.
- A29. Three drawn test binaries, `draw`, `geometry_draw` and `variant_draw`, crash on the desktop
  when their tests run in parallel threads (cargo's default): ANGLE asserts in its Direct3D 11
  state manager (`StateManager11.cpp:1201`, `mVertexArray11 == ...`, and `:2968`,
  `hasPixelExecutableForCachedOutputLayout`), then an access violation or exit code 2173 (run 74
  twice, run 75 once; run 73 ran them in parallel without it). With `--test-threads 1` every test of
  the three passes (run 75: the whole suite so, 171 passed). Cause: not found; each of these
  binaries' tests makes its own WebGL 2 back end on ANGLE (`common::webgl`), so several ANGLE
  contexts draw at once from several threads, which the mozangle build used here may not take
  (inferred, not measured; whether the WebGL 2 back end or ANGLE is at fault is not known). Waits:
  nobody (the tests, not the converter). Takes: the cause found (one context per thread kept
  current, or ANGLE's own lock), or the tests' WebGL 2 use serialised in `common` by a lock that a
  test making two contexts in turn can take twice; until then run the three with
  `--test-threads 1` (How to run).
- A30. On the DXIL route through WebGL 2 (ANGLE), `saturate` of a NaN gives 1.0 where Direct3D 12
  gives 0; the same DXIL through WebGPU gives 0, and both DXBC builds are exact against Direct3D 11
  (G6a's second check of the first's findings, at 9fb6e0d1: its own `ps_chk_round`, dxc `ps_6_0`,
  drawn by `own_shaders_draw_as_direct3d_does` with the case added, vs_transform, Rgba32Uint:
  webgpu-dxil 0 of 4,096 texels differ, webgl-dxil 1,315, only the component holding
  `saturate(sqrt(uv.y - 0.5))`). Cause, read in the code and not drawn: dxil-spirv writes DXIL's
  saturate as GLSL.std.450 `NClamp` (opcodes/dxil/dxil_arithmetic.cpp), which defines a NaN
  operand to give the low bound; the carried naga reader reads `NClamp` as a plain `Clamp`
  (front/spv/next_block.rs), whose NaN result is left open, and the GLSL ES writer writes
  `clamp(x, 0.0, 1.0)`, which ANGLE's compiled program gives 1.0 for. Waits: nobody (a NaN into a
  saturate on WebGL 2). Takes: the reader writing `NClamp`, `NMin` and `NMax` in a form that keeps
  their NaN rule on every back end (a max with the low bound before a min with the high one, if
  each back end's min and max give the number over the NaN, to be measured), the check's case added
  to the own shaders, and that case drawn on both routes against Direct3D 12.
- A31. Gathers on WebGL 2 differ from Direct3D in a few texels, on both routes: own `ps_gatheroff`
  (offsets known only at run time) 7 of 4,096 texels (largest 9), the third check's
  `ps_c3_gather` 6 of 4,096 (largest 36); WebGPU is exact for both. Before run 77 the draw test
  did not draw `ps_gatheroff` on WebGL 2, giving "GLSL ES 3.00 has no textureGather", untrue since
  the back end writes gathers (A3); since run 77 every case is tried on WebGL 2 and a stated reason
  the back end does not refuse fails the test, and `ps_gatheroff` is drawn there as a known failure
  naming this gap. Cause, measured in run 77 from the check's pictures (scratch
  `graphics-shaders/g6a-r77/snap.py`): the WebGL 2 back end writes a gather as four samples at the
  texel centres of the footprint whose corner is `floor(p * size - 0.5)` at full float precision
  (`gathers_by_samples`, crates/wcontainers-gpu-webgl/src/shader.rs), where Direct3D, and the GPU's
  own gather that WebGPU uses, round the position to 8 bits of subtexel first. Each of
  `ps_c3_gather`'s 6 texels lies where `v * 8 - 0.5` is 0.14 to 0.45 of a 1/256 step below a whole
  number (Direct3D rounds up to it, the back end floors below it, so the footprint is a row lower);
  no texel 0.5 of a step or more below differs. The converter's run-time offset adds whole texels
  and does not move that fraction, so a gather with no offset differs the same way. Waits: the
  WebGL 2 track (asked under "Asked of other tracks"). Takes: the corner taken from the position
  rounded to 1/256 of a texel, then `ps_gatheroff` drawn again and its known failure removed.
- A32. The DXIL route's `f32tof16` of a product that overflows differs between our back ends (the
  third check's `ps_c3_half`, dxc `ps_6_0 -Gis`): real Direct3D 12 on the RTX 3080 keeps `big *
  1e35` finite (f32tof16 gives 0x7bff and 0xfbff, `isinf` false), where Direct3D 11 with fxc's
  build gives infinity (0x7c00 and 0xfc00, `isinf` true). Our WebGPU gives Direct3D 11's answer for
  the DXIL build (107 of 4,096 texels differ from Direct3D 12), our WebGL 2 gives Direct3D 12's;
  both DXBC routes are exact. Cause: not found. The DXBC route's `f32tof16` tests finiteness by its
  bits (since 0b412922); the DXIL route goes through dxil-spirv's half conversion and the back ends'
  compilers, and it is not measured whether Direct3D 12's compiler or ANGLE's keeps the product
  finite. Waits: nobody (an overflow to infinity in a dxc build). Takes: dxc's disassembly of the
  case read for the product's form, the converted WGSL and GLSL compared at it, the case added to
  the own shaders and drawn on both routes against Direct3D 12.
- A33. The DXIL route's passes on dxil-spirv's SPIR-V walk and re-write raw words by hand
  (src/dxil/staged.rs, geometry.rs, tessellation.rs, and before them registers.rs and widths.rs:
  instruction walking, literal strings, entry points and execution modes, stub functions, the id
  bound), where rspirv loads and assembles modules (G6b's check). rspirv 0.13.0+sdk-1.4.341.0
  (Apache-2.0, crates.io, 2026-03-13) is built on the same `spirv` 0.4 header crate this crate
  uses, so it fits without a second header version. Not done in run 80, by priority: the fronts
  wait on A4's `combine` and the other items below, while these passes are proven by the drawn
  tests and the corpus records today. Waits: nobody. Takes: each pass on rspirv's `dr::Module`
  (load, edit, assemble), proven by every corpus record and written file staying byte-equal.
- Also open but not on the pass bar's words, for the fronts' real programs (arrays of constant and
  structured buffers on the DXIL route convert since run 87, its section): `rewrite` refusing
  arrays of buffers (its input's binding arrays of blocks; `src/dxil/buffer_arrays.rs` would serve
  it with the elements numbered as `common::split_arrays` numbers images; no front has asked),
  `InterpolateAt` on the DXIL route,
  `CalculateLevelOfDetail`, `SampleClamp`, `globallycoherent` views (refused by name on both
  routes: on DXBC `GloballyCoherent` since G6a run 5, before which the flag was dropped without a
  word; on DXIL the reader's "unsupported memory operand .. at scope .."; no shader of any corpus
  declares one), DXIL signed atomic minimum
  and maximum on unsigned words, on DXBC the same on a typed view (`AtomicSignedness`, runner
  `uav-atomics-5-cs`: a typed view holds one scalar kind in the common form, and the
  compare-exchange loop that gives buffers the other signedness is not built over a typed view's
  texel), early tests on 24-bit depth (not measured), comparison filtering
  with a border colour (needs the comparison in `SamplerState`); and dxil-spirv's crashes on signed
  but malformed DXIL (fuzz, Known gaps). The Direct3D 12 front's runner items besides (its status,
  "Asked of other tracks"): `DepthGatherComponent`, `SparseFeedback`, a clip distance read by a
  pixel shader, a DXIL pixel shader's layer and viewport index, and values that differ without a
  refusal (a centroid `SV_Position` per sample, `min16float` constants, subnormals not flushed,
  DXIL `frexp` of a NaN), and the shader model 4.0 multisampled load in a loop that its front reads at
swapped coordinates (runner texture-load; this track's run 32 found fxc's build of that shader
converts right, test `shader_model_4_multisampled_loads_keep_their_coordinates`, so the bytecode
the runner compiles needs dumping by the front). Its list also names `TypedBufferFormatKind`, gone
since run 55 (A2), and `SwizzledDepth` (A4). The Direct3D 11 front's atg-simplecomputepc division (just below 4 where
  Direct3D gives 4; inferred there, its shader not in the games corpus) and Wine's
  test_atomic_instructions (its refusal of `AtomicExchangeLoop` is the front's to lift).

Every refusal in the nine records (`tests/expected`, at the tip), by name and count, with where it
is accounted for (counted with `grep` over the records, G6a run 4; the records are unchanged at
G6a run 5's tip):
- On the gap list: none left of A6 (`DxilGeometryShader` 26 gone in run 65; `DxilHullShader` 28
  and `DxilDomainShader` 9 gone in run 66: 36 convert, 1 is `DomainWithoutControlPoints`, next to
  A14's `HullWithoutControlPoints`, gone in run 89);
  `VertexStorageWrite` 8 gone in run 86 (A9: they convert); `GeometryPatchInput` 2 (A13; DXIL
  `patch-syntax-12-gs` and own `gs_patch.dxil`; the DXBC 5 convert since run 89);
  `DomainWithoutControlPoints` 2 (A14; runner `patch-syntax-11-ds.dxil`, own `ds_tri_none.dxil`;
  `HullWithoutControlPoints` gone in run 89); `ComponentPastVector` 1
  (A23, own `ds_rows_gap.dxbc`, since run 68); the stage helper's `GeometryReplayArrayOfArrays` (A5, at
  `GeometryReplay::new`, not a conversion verdict: dxc's `gs_rows_lines` on the replay route);
  `OutputArray` 0 (an array of arrays among a geometry shader's values; no file of any corpus); the stage helper's
  `GeometryPrimitiveIdAfterDomain` (A20, at `Stages::tessellated_with`, not a conversion verdict); `GeometryStreams` 1, `PrimitiveId` 2 (a geometry shader
  writing the primitive's number, `primitive-id-2-gs`, DXBC and DXIL) (A7; `CullDistance` 8 of
  geometry shaders writing cull distances, `clip-cull-distance-1-18` to `-21-gs`, DXBC and DXIL,
  gone in run 90: they convert); "convert rejected: Unknown type!" 1 (A1).
- Absent on both back ends, reported as the guest expects (plan 4.12 and section 9's absent
  lists): `Float64` 42 and
  `Int64` 35 (no 64-bit types), `RasterizerOrderedView` 12 (no fragment interlock), `StencilRef` 7,
  `RenderTargetArrayIndex` 5 and `ViewportArrayIndex` 2 (written by a vertex shader).
- Converting once the front gives the key its value: `SampleCount` 16 (`samples`),
  `MultisampledArrayTexture` 5 (`texture_layers`), `UnboundedResourceArray` 4 (`register_ranges`).
- In "Also open": `DepthGatherComponent` 10 (vkd3d's `gather-cmp-3` to `-7`, DXBC and, since
  run 82, DXIL; A4's `swizzled`) and own 6 (expected: the comparison gathers of green, blue and
  alpha without a mapping); `CalculateLevelOfDetail` 4 (DXBC) and the reader's
  "unsupported instruction ImageQueryLod" 4 (DXIL); `SparseFeedback` 3 (DXBC) and the reader's
  "ImageSparseFetch" 2 and "ImageSparseSampleImplicitLod" 1 (DXIL); `InterpolateAt` 3;
  `SampleClamp` 2 (DXBC) and the reader's "image sample operand 0x80" 2 (DXIL, the same minimum
  level operand); `AtomicSignedness` 1; `ResourceArray` 0 (own `cs_buffer_array.dxil` converts
  since run 87, recorded `UnboundedResourceArray` as the corpus has no ranges); a clip or cull distance read as an input
  (`ClipDistance` 4, `CullDistance` 3: pixel shaders and `clip-cull-distance-1-16-gs`, DXBC and,
  since run 65, DXIL). A DXIL pixel shader's layer or viewport index (the reader's "unsupported
  builtin" 4) converts since run 66.

## Next

1. G6's gap list in the order the fronts need it: A28's second part (with the interface track, a
   view's width for YUY2) and A17's aliased lines into multisampled targets (the Direct3D 11
   front's asks, both waiting on the interface track; A5's routes on WebGL 2 were finished in runs
   83 to 85, A4's multisampled integer twins are absent by plan 4.12, A9 closed in run 86), A7
   (its typed buffers in geometry shaders done in run 86; left: indirect draws through the stages,
   cull distances from geometry or domain shaders, the primitive's number behind a geometry
   shader), A1 (the DXIL route's buffer arrays done in run 87), A33 (rspirv), A8 (needs a way to carry dxbc-spirv patches), a logic operation
   into a multisampled target (A4), the line routes' near-plane cut under `VIEWPORT_DEPTH_FLIP`
   (A5), stream output from a vertex shader writing cull distances on WebGL 2 (A5); the next
   Direct3D 12 runner items are A20 and the DXIL parts of A13 and A14 (dxil-spirv's; A13 and A14's
   DXBC parts closed in run 89, A7's cull distances from geometry shaders in run 90). A20 takes:
   the geometry compute form reading the patch's primitive number from the tessellator's points
   (word 3 of the point its first vertex's record is, `Tessellator::points`) when listed, which
   needs the points bound at `POINTS_BINDING` of its set for every geometry shader reading
   `SV_PrimitiveID` (so their converted modules change: Chrome again), and a drawn test
   (`gs_values_tess` with `hs_values_*`).
2. A6's two Direct3D 12 rig scenes when the owner rules or the rig takes a line allowance.
3. Besides, as asked: the NIR track's reader items (the carried SPIR-V reader's three refusals,
   the array layer's rounding; its geometry and tessellation entry and `rewrite` asks were built
   in run 88), A26 (the unordered not-equal, both routes, with its drawn NaN
   test), A30 (`NClamp` keeping its NaN rule, drawn on both routes), A32 (the DXIL overflow's
   cause), A31 when the WebGL 2 track has rounded its gathers, A23's line of dxbc-spirv and
   A8 (both want a way to carry dxbc-spirv patches), A22 on WARP.

## Asked of other tracks

- For the Direct3D 12 front (run 89, A13 and A14; its runner): the runner's DXBC
  `geometry-shader-syntax-6`, `-7`, `-8`, `-10`, `patch-syntax-12` and `hull-shader-attributes-10`
  convert now. A geometry shader taking patches is drawn through the stage helper as any geometry
  shader: pass the patch list as `InputTopology::PatchList(n)` with no hull stage (`Stages::new`
  with the geometry shader); drawn equal to Direct3D 11 here. A hull shader of no output control
  points and its domain shader go through `Stages::tessellated` as any pair. The same holds for
  the Direct3D 11 front. On WebGL 2 a patch list into a geometry shader is still `TopologyMismatch`.

- The WebGL 2 track (run 77, gap A31): `gathers_by_samples` finds the footprint's corner as
  `floor(p * size - 0.5)` at full precision; Direct3D rounds the position to 8 bits of subtexel
  first, so where `p * size - 0.5` lies less than half a 1/256 step below a whole number the gather
  reads the row or column before Direct3D's (6 and 7 texels of 4,096 in two own shaders, both
  routes). Taking the corner from `floor(round((p * size - 0.5) * 256.0) / 256.0)` would match.
- The WebGL 2 track (run 77): own `ps_cases` (fxc's build, a `switch` in a loop with a `continue`
  in a case) is written as valid GLSL ES 3.00, but ANGLE on Direct3D 11 translates it to HLSL that
  fxc refuses ("error X3708: continue cannot be used in a switch") and the device is lost at the
  first draw. Chrome's compile check (`web/check`) passes the file, as it does not draw; Chrome
  drawing it on Windows is expected to fail the same way (not measured). naga's HLSL writer carries such a
  `continue` out of the switch through a flag; writing the GLSL the same way would avoid it.
- The WebGL 2 track and naga's GLSL writer (run 77, the third check of G6a): Chrome refuses the
  written vertex shaders that write cull distances (own `vs_cull.vert` on both routes, the
  runner's `hlsl+clip-cull-distance-1-1`, `-5`, `-7` and `-15` vertex shaders) with a type error,
  not for a missing feature: `gl_CullDistance` is declared with 8 elements and assigned a
  `float[N]` ("cannot convert from 'highp array[3] of float' to 'CullDistance highp array[8] of
  float'"). They would not compile where WEBGL_clip_cull_distance is enabled either; each element
  assigned on its own would. The records name the need, so the corpus test counts them as named.

- For the Direct3D 12 front (run 86, A9, its runner's `depth-bounds-1-vs`): vertex shaders
  writing unordered access views now convert (no longer `VertexStorageWrite`), so drawing one
  directly on WebGPU now fails when the pipeline is made; route a vertex shader for which
  `stage::writes_storage` is true through `Stages::stored` ("How the Direct3D fronts call
  `convert`"), drawn equal to Direct3D 11.1 here. The same holds for the Direct3D 11 front at
  feature level 11_1.
- Answered for the Direct3D 11 front (run 84, its ask "atg-simplecomputepc's division", still
  listed in its status): measured twice, not the converter's. This track's own `cs_divide` and
  `cs_escape` (the program's division and its Mandelbrot loop as written, fxc and dxc builds)
  give real Direct3D 11's bits on the RTX 3080, both routes (runs 41 and 44, history), and the
  WebGPU track measured real Direct3D 11 itself giving 1200 / 300 = 3.9999998 for a divisor read
  at run time (its status, `divisions_match_direct3d`). What is left for that program is the colour
  map's sample at u just below 1 (the front's sampler state and the texture's addressing), which
  the front can compare with a real run's.
- Answered for the Direct3D 11 front (run 85, A5; its WebGL 2 item "Cull distances
  (`CullDistanceAsGeometry` needs compute): test_combined_clip_and_cull_distances"): cull distances
  from a vertex shader alone run on WebGL 2 through `GeometryReplay::culled`, and a pixel shader's
  primitive number without a geometry stage through `GeometryReplay::primitive_numbered` ("How the
  Direct3D fronts call `convert`"). Multisampled `combine` into integer twins is absent by plan
  4.12 ("Multisampling of these formats is reported absent on such adapters"), which the front
  already reports; nothing waits on this track there.
- Answered for the Direct3D 11 front (run 84, A5): alpha-antialiased lines on WebGL 2 run through
  `GeometryReplay::antialiased_lines` ("How the Direct3D fronts call `convert`").
- Answered for the Direct3D 11 front (run 83, its ask "the geometry replay's streams past the
  first"): `GeometryReplay::new` takes a stream output declaration naming several streams, and
  `run` writes every stream; give the filled block `stage::STREAM_FILLED_WORDS` words, as on the
  compute route ("How the Direct3D fronts call `convert`"). Also answered earlier: a primitive
  naming a layer past the bound view's is drawn into the first layer (`passthrough` with
  `VariantFlags::LAYER_FROM_STAGE`, test `layers_past_the_views_draw_into_the_first_as_direct3d_11_does`),
  `A8Unorm` storage views (run 81) and single-sample `combine` (run 80).
- Answered for the Direct3D 11 front (its asks of this track, run 65): halves toward zero are
  built (A19 for the drawn proof); `ClearView` of a storage view's rectangles takes
  `builtin::storage_clear` with each rectangle's `origin` and `end` (one dispatch a rectangle),
  which exists since run 56: the rest is the front's.

- The Direct3D 11 track (run 58): stream output from a vertex shader alone on WebGL 2 is
  `GeometryReplay::streamed(backend, &vertex, &so)` (the vertex shader converted as for a replay,
  its inputs as drawn); `run` with a `ReplayDraw` whose `geometry_sets` are all `None` writes the
  stream output and returns the capture as a `FinalDraw`, drawn through `passthrough()` and
  `vertex_layout()` as after a replayed geometry shader (or not drawn, for
  `D3D11_SO_NO_RASTERIZED_STREAM`). Drawn equal to Direct3D 11 on ANGLE. Its refusals are
  `StageError::Unsupported` with the name. Also: the pass-through's clip distances at the linked
  location behind a geometry stage (your round 2 run 3 ask) were built here in run 38: convert the
  geometry shader and the pixel shader with `VariantFlags::CLIP_DISTANCE` and `linked`.
  And (G6a run 6): your status says the converter refuses `DrawBaseSource::Arguments`, so an
  indirect draw reading `SV_VertexID` or `SV_InstanceID` is read back (`IndirectReadBack`); it is
  built (`variant::draw_arguments`, both routes; drawn in `variant_draw`, direct and indirect, the
  `draw_base` case of pass bar item 3).

- The NIR track (G6a run 6): `rewrite` now validates immediates (`capabilities` adds
  `Capabilities::IMMEDIATES` for an `AddressSpace::Immediate` global), so Godot's 20 programs with
  arrays of textures and push constants should pass it without your own validation's capability;
  and after a split it turns WGSL's derivative uniformity check off, as Chrome needs for a sample of
  an element picked by a non-uniform index. `sampled_pairs` whatever `rewrite` splits is among the
  NIR track's asks on this track's "Next" (2).

- The Direct3D 12 track (run 56): (1) a pipeline without a pixel shader whose variant needs one
  (`DEPTH_CLAMP`, `CLIP_DISTANCE` where the back end lacks them) takes
  `builtin::pixel_program_for_variant(&key)` as its pixel program, keyed as the pixel shader's
  would be (`linked`, `samples`), with no colour writes, and its helper block from the returned
  reflection; drawn equal to Direct3D 11 without a pixel shader. (2) A geometry shader converts
  with `DEPTH_CLAMP` and `linked` (the pass-through clamps), drawn equal to Direct3D 11; the cull,
  line and tessellation routes take the clamp from the vertex or domain program's key. (3) For
  texture-load lines 235 to 267: give a texture slot's bound view format in `texel_formats` where
  its component kind differs from the program's declaration and drop `read_as`; loads read the
  view's value by its bits (drawn equal to Direct3D 11 on both routes); a sample of such a slot is
  refused `SampledViewKind`.

- The Direct3D 12 track (run 55): a typed buffer or storage texture bound as a view of another
  kind than the program declares now converts and reads and writes the view by its bits, as
  Direct3D does (drawn equal to real Direct3D 11 on both routes); give the bound view's format in
  `texel_formats`/`storage_formats` as for any view, with no check of its kind, and rerun the
  runner's srv-buffers, tgsm, uav-rwbuffer and wave-ops-float. Arrays of typed buffers convert on
  the DXIL route too, those without a length sized by `register_ranges` as on DXBC: rerun
  unbounded-array-5.1's shader model 6.0 run with the ranges in its key. Arrays of constant or
  structured buffers stay refused there (`ResourceArray`). Runner `uav-atomics-5-cs.dxbc` now
  ends at `AtomicSignedness` instead of the kind.
- Both Direct3D tracks (run 54): `decoded` and normalised targets in `targets_in_shader` are built
  (see "How the Direct3D fronts call `convert`"), drawn equal to real Direct3D 11 on both back
  ends. The Direct3D 11 front can hold 16-bit normalised textures as their integer formats where
  the back end lacks them and un-ignore
  `typed_storage_stores_without_texture_formats_tier1_or_16_bit_norm_as_direct3d_does` (its
  storage views of them go through `texels_through_buffer`, built since run 18). The Direct3D 12
  front's view-typed reads (texture-load lines 235 to 267) still need an interface field.
- The Direct3D 11 track (run 54): indexed strips run through `GeometryReplay` on WebGL 2 now
  (drawn equal to Direct3D 11 on ANGLE, stream output after them too); make index buffers with
  `BufferUsage::COPY_SRC` there, as the route reads an indexed strip's indices back.

- The Direct3D 11 track (run 53): geometry shaders run on WebGL 2 by replay (`GeometryAsReplay`):
  `stage::GeometryReplay`, called as under "How the Direct3D fronts call `convert`", drawn equal
  to Direct3D 11 on ANGLE for every own geometry shader. Its refusals (indexed strips, strips with
  adjacency, indirect draws) are `StageError::Unsupported` with the name; report them through the
  front's own mechanism. Stream output after it runs too (run 53, as under "How the Direct3D
  fronts call `convert`"; drawn equal to Direct3D 11 on ANGLE, counts and overflow included), so
  with the front's own transform feedback for vertex shaders alone, 10_0's geometry stage and
  stream output are there. Alpha-antialiased lines on WebGL 2 are not built yet (A5).
- The Direct3D 12 track (run 51), its ask of clip distances through `Stages::culled`'s
  pass-through on a back end without `CLIP_DISTANCE`: convert the vertex shader with
  `VariantFlags::CLIP_DISTANCE` and `linked` (and the pixel shader with the same): its clip
  distances then leave it as linked values, which the cull route's pass-through passes on as
  locations, and the pixel program discards by them. Expected from the code (run 39), not drawn
  here on a back end without clip distances; report what fails.

- The Direct3D 11 track (run 50): the stage helper takes indexed strips on every route (geometry
  shaders, stream output, cull distances, alpha-antialiased lines, primitive numbers), so the
  front can drop its `NotImplemented("StageIndexedStrip")` and un-ignore
  `an_indexed_alpha_antialiased_strip_blends_its_coverage_as_direct3d_does`. Pass the draw's index
  buffer in `StageDraw::index` as for lists; the helper restarts at the format's largest index.
  Triangle strips with adjacency into a geometry shader run too (no longer
  `GeometryTriangleStripAdjacency`).
- The Direct3D 12 track (run 50): its "Next" 5 (a DXBC `Buffer<float4>` vertex shader with
  `DrawBaseSource::Arguments` failing validation, "Index 3 is out of bounds") was fixed here in
  run 29; nothing waits on it. Indexed strips through the stage helper work as above (its
  refusals of indexed strips there, if any, can go).
- The Direct3D 11 track (run 49), for test_so_statistics_query: geometry shaders writing several
  streams of points convert and run through `Stages` now. Give the stage helper a filled block of
  `stage::STREAM_FILLED_WORDS` words (the front's own `FILLED_BYTES = 32` in context/geometry.rs
  covers one stream only; a shorter block for a shader with more streams is refused as
  malformed) and read stream `k`'s primitives written and made at words `STREAM_BUFFERS + 2k` and
  `STREAM_BUFFERS + 2k + 1`; pass `stream_output` with every buffer `None` when a statistics query
  runs and nothing is bound (Direct3D then counts what was made and writes nothing); draw the
  rasterised stream 0 as before, nothing for `D3D11_SO_NO_RASTERIZED_STREAM`, and refuse by name
  a later rasterised stream (its points are vertices `k * count` on, but the pass-through reads
  stream 0's values; not built or tested here); link the pixel stage with stream 0's reflected outputs
  (`StageValue::stream`).

- The Direct3D 12 track (run 48): for resource_arrays and the runner's unbounded-array-5.1, give
  every shader model 5.1 program's key the bound root signature's descriptor ranges as
  `VariantKey::register_ranges` (kind, space, registers; a range of unbounded length cannot size
  an array and stays refused), or only after a plain conversion is refused
  `UnboundedResourceArray`, and bind each element of an array at its reflected (space, register)
  as any binding. Since run 87 the same holds for dxc's arrays of constant and structured buffers
  (`ConstantBuffer<T> b[n]`, `StructuredBuffer<T> s[]`, read and written; each element is a
  binding of its own, nothing else to do). For shader_eval_attribute nothing new: the pixel program's reflection now asks
  `PerspectiveSample` for such an input, which the front's `output_interpolation` already passes.

- The WebGL 2 track (run 47): keep the public `glsl` module this branch added to
  `wcontainers-gpu-webgl` (re-exports of `shader::{write, needs, GlslSettings, ShaderText}`; the
  shaders corpus test writes and checks every converted shader's GLSL through it), or give the
  same under a name of your choice and say so. And the write error drops naga's reason (it is only
  printed): carrying it in the error would let callers name it (until then the corpus test records
  naga's own reason beside it, G6a run 3).
- The WebGL 2 track (G6a run 3): a public list, in the `glsl` module, of the features `caps` can
  declare (`DUAL_SOURCE_BLEND`, `POINT_SIZE`, `SAMPLE_SHADING` today, each where its extension is
  there). The corpus test keeps its own copy (`WEBGL_DECLARABLE` in tests/corpus.rs) to decide
  that a GLSL write refusal of a shader needing only such features is a failure; with the list
  public it would read yours and could not go stale when caps.rs declares another (for example
  `CLIP_DISTANCE`, held back for a Chrome bug).
- The WebGL 2 track (run 71): `SAMPLE_QUALIFIER_EXTENSION` (and `NOPERSPECTIVE_EXTENSION`) in the
  public `glsl` module, so the corpus test can check that a file using the `sample` qualifier
  carries its directive, as it checks naga's own directives (run 71 section, item 1).
- The WebGL 2 track (G6b run 4): GLSL ES 3.00 has no arrays of arrays, and naga's writer refuses
  them; dxc's build of a geometry shader reading input arrays of rows (own `gs_rows_lines.dxil`
  with `vs_rows`) has one in its replay form, so the stage helper refuses it by name
  (`GeometryReplayArrayOfArrays`). If your writer flattened such arrays (an array of arrays as one
  array, its indices joined), the refusal could go.
- The WebGL 2 track (G6a run 4): your writer refuses 13 of the games' Godot DXIL pixel shaders with
  "one expression of a texture is sampled through two samplers (not built)"
  (`tests/expected/games-dxil.txt`, the records with that text, for example
  `godot-pixel-3ce334b77ff19481.dxil`). They also need `FRAGMENT_STORAGE_WRITE`, so WebGL 2
  cannot run them today either way, but the path is yours to build or name; the converted modules
  are written with `WCONTAINERS_SHADERS_OUT` (How to run) if you want them.
- The interface track (run 47): no feature names a cube array view; WebGL 2 cannot make one, and
  the corpus test names it itself (`CUBE_ARRAY_VIEW`, 5 shaders).

## How the Direct3D fronts call `convert`

- Vertex shaders writing unordered access views (Direct3D 11.1's UAVs in every stage, Direct3D
  12's; since run 86): they convert as any vertex shader (no longer `VertexStorageWrite`), and
  `stage::writes_storage(&vertex)` says so. Where the back end lacks
  `Features::VERTEX_STORAGE_WRITE` (WebGPU always), convert the vertex shader with every input
  pulled, as for the cull route, and draw through `Stages::stored(backend, &vertex,
  stream_output)`: `run` runs it as compute (its storage writes with it) and returns a `FinalDraw`
  drawn through `passthrough()` and `vertex_layout()`, as after the cull route; bind its sets with
  `StageSet::COMPUTE`. `Stages::new` with no geometry shader makes the same, so stream output from
  such a shader also ends in a `FinalDraw`; behind a geometry, hull or domain stage, or on the line
  and primitive routes, its compute form writes the storage as well. The helper counts
  `SlowPath::VertexStorageAsCompute` once a draw. Its refusal by name: adjacency without a geometry
  shader (`VertexStorageAdjacency`). Direct3D runs a vertex shader any number of times per vertex;
  the stage helper runs it once per place in the draw (index list), and not for vertices of no
  whole primitive. On WebGL 2 there is no storage: the back end refuses the program
  (`GpuError::Absent(VERTEX_STORAGE_WRITE)`).
- Typed buffers in geometry shaders (since run 86; `TypedBufferInGeometry` is gone): convert the
  geometry shader with the typed buffer route and the view's format, as for any stage, bind the
  buffer in set 0 of `geometry_sets` and its `HelperValue::TexelBufferView` in the geometry
  stage's helper block (set 3, `helper_binding(ShaderStage::Compute)`), both `StageSet::COMPUTE`.
- Views of depth textures with a component mapping (Direct3D 12's `Shader4ComponentMapping`; since
  run 82 measured on real Direct3D 12): name `(slot, mapping)` in `swizzled`. Every read converts
  but comparisons through a first component of green, blue or alpha and comparison gathers whose
  gathered or first component is not red, which are `SwizzledDepth` (`DepthGatherComponent`
  without a mapping). A program's slots on the DXIL route rank the registers it declares.
- `A8_UNORM` typed unordered access views (since run 81): hold the texture as `Rgba8Unorm` with the
  value in alpha and the colour channels zero (as the WebGPU back end holds `A8Unorm`), bind its
  `Rgba8Unorm` view, and name `(slot, Format::A8Unorm)` in `storage_formats`; stores write alpha
  alone. Not through `texels_through_buffer`, and not cleared by `builtin::storage_clear`.
- `TargetInShader::combine` (blending or a logic operation in the program; since run 80 drawn
  equal to Direct3D 11 for single-sample targets). Set it on the target's entry with the guest's
  format and the key's `samples` (a multisampled target is refused, `BlendInShaderMultisampled`);
  copy the target before the draw into a texture of the stored format bound at
  `TARGET_COPY_BINDING`, and give `HelperValue::TargetCombine { target }` the value
  `TargetCombineValue::new(&blend, logic_op, blend_constant)`. The pipeline's own blending stays
  off. The program reads one old value per fragment from that copy, so split a draw whose
  primitives overlap (one primitive per draw, the copy taken again between). Blending takes a
  float or normalised output; a logic operation an integer output into an integer target (Direct
  3D 11.1's rule); a float output with combine into an integer target is refused
  (`TargetsInShader`).
- Normalised textures and targets the back end lacks (`NormAsInteger`, plan 4.12; since run 54).
  Hold the texture as the integer format of the same bits: unsigned for UNORM (`R16_UNORM` as
  `r16uint`, `R10G10B10A2_UNORM` as `rgb10a2uint`), signed for SNORM (`R16G16_SNORM` as `rg16sint`,
  `R8G8B8A8_SNORM` as `rgba8sint`). A program reading it takes `VariantKey::decoded` with
  (texture slot, the guest's format): the slot is declared as that integer format, loads are
  decoded as Direct3D converts, and samples and gathers are filtered in the program from them,
  exactly as `filter_in_shader` (so the slot's samplers leave `sampled_pairs`, and the helper
  values are the same: `SamplerState`, `BorderColor`, `TextureComponents`, `LodBias` with
  `lod_bias`, `TextureLevels` with `TEXTURE_INFO`). A program writing it as a target takes a
  `TargetInShader` with the guest's normalised format: its float output is encoded into the
  integers (the reflection's output says `Uint` or `Sint`); draw into the integer target. BGRA,
  a format with no integer twin, is refused `NormAsInteger` (targets) or `DecodedFormat`
  (`decoded`); a decoded slot that is not a single-sampled float texture `DecodedTexture`;
  combining with the old value stays `BlendInShader`.
- Geometry shaders (WebGPU, `Features::COMPUTE`): convert the pixel shader first, then the geometry
  shader with `output_interpolation` from the pixel shader's inputs (as for a vertex shader), and the
  vertex shader with every input pulled (`pulled`, as for pulled inputs) and `draw_base: Helpers`.
  `Stages::new(backend, &vertex, &geometry)` once per pair. Per draw, outside the pass:
  `stages.run(backend, &StageDraw { vertex_sets, geometry_sets, index, topology, range })`, where
  the vertex sets are its set 0 (pulled streams) and 3 (helper block: `PulledStream`s, `DrawBase`)
  and the geometry sets its set 0 (its slots) and 3 (its helper block, if any), all with
  `StageSet::COMPUTE`; an index buffer must have `BufferUsage::STORAGE` and a size that is a multiple
  of 4. Then, in the pass, draw the returned `FinalDraw`: the program of `stages.passthrough()` and
  the pixel shader, `stages.vertex_layout()`, `buffer` as stream 0, indices (u32) from `buffer` at
  `index_offset` when given, its topology, restart on, `count` vertices or indices, one instance.
  `tests/geometry_draw.rs` does exactly this.
- Geometry shaders on a back end without compute (WebGL 2, `Features::TRANSFORM_FEEDBACK`; since
  run 52): convert the pixel shader first, then the geometry shader as above with
  `VariantFlags::GEOMETRY_REPLAY`, and the vertex shader with its inputs as drawn (nothing
  pulled) and `draw_base: Helpers`. `GeometryReplay::new(backend, &vertex, &geometry)` once per
  pair. Per draw, outside the pass: `replay.run(backend, ReplayDraw { vertex_sets, vertex_layout,
  vertex_buffers, index, immediates, geometry_sets, topology, range })`: the vertex stage's own
  layout, streams and indices as for the API draw, its set 3 the helper block (`DrawBase`: the
  base vertex, or the first vertex when not indexed, then the first instance); the geometry
  sets its set 0 and 3, with `StageSet::VERTEX`; set 1 is the route's. Then, in the pass, draw the
  returned `FinalDraw`: the program of `replay.passthrough()` and the pixel shader,
  `replay.vertex_layout()`, `buffer` as stream 0, no indices, its list topology, `count`
  vertices, one instance. `None` means nothing to draw. Refused by name: strips with adjacency,
  indirect draws, records past `capture_components`. Indexed strips (since run 54): the route
  reads the draw's indices back, so the index buffer needs `BufferUsage::COPY_SRC`; it restarts
  at the format's largest index, as Direct3D does. Stream output (since run 53):
  pass the declaration to `new` and `ReplayDraw::stream_output` as on the compute route, but with
  buffers and the filled block made with `COPY_SRC` and `COPY_DST` (no storage); the route reads
  the filled block, writes each stream's whole primitives on the host as Direct3D does (a primitive
  only while every bound buffer of its stream holds it, gaps keeping their bytes) and writes back
  the filled sizes and each stream's written and made counts. A shader writing several streams
  (since run 83) is replayed once a stream, stream 0 last for the draw: give a filled block of
  `stage::STREAM_FILLED_WORDS` words (stream `k`'s counts at words `STREAM_BUFFERS + 2k` and
  `+ 1`, as on the compute route; a shorter one is refused as malformed), and `stream_output` with
  every buffer `None` when a statistics query runs and nothing is bound.
  `geometry_shaders_replayed_on_webgl_draw_as_direct3d_does` does exactly this.
- Alpha-antialiased lines (Direct3D 10 and 11's `AntialiasedLineEnable` with `MultisampleEnable`
  off, on a line list or strip without a geometry or hull stage; WebGPU, `Features::COMPUTE`; since
  run 45): convert the pixel shader with `VariantFlags::LINE_COVERAGE` and `linked` (its location
  one past every location either program lists, as for clip distances), and the vertex shader with
  every input pulled, `draw_base: Helpers` and the same `linked`. `Stages::antialiased_lines(backend,
  &vertex, &pixel_key)` once per pair; per draw `stages.run_antialiased_lines(backend, &draw,
  [viewport width, viewport height])`, then draw the `FinalDraw` as after a geometry stage, with
  culling off (a triangle list, u32 indices). The pixel program multiplies target 0's alpha by the
  coverage; a target 0 that is not four floats is refused (`LineCoverageTarget`). Refused by name:
  cull distances on the same vertex shader (`AntialiasedLinesCullDistance`), other topologies
  (`AntialiasedLinesTopology`). Indexed strips run (since run 50; pass the index buffer in
  `StageDraw::index`). `tests/stage_draw.rs` does exactly this.
  On a back end without compute and with `Features::TRANSFORM_FEEDBACK` (WebGL 2; since run 84):
  convert the vertex shader with its inputs as drawn (`draw_base: Helpers`, the same `linked`) and
  the pixel shader as above; `GeometryReplay::antialiased_lines(backend, &vertex, &pixel_key)` once
  per pair; per draw `replay.run_antialiased_lines(backend, ReplayDraw { .. }, [viewport width,
  viewport height])` (`geometry_sets` all `None`, `stream_output` `None`), then draw the
  `FinalDraw` through `replay.passthrough()` and `replay.vertex_layout()` with culling off (a
  triangle list without indices). Make index buffers with `BufferUsage::COPY_SRC`, as indexed
  strips are read back. Convert that vertex shader without `CLIP_DEPTH_REWRITE` and
  `VIEWPORT_DEPTH_FLIP` (the line program cuts lines at Direct3D's near plane, z = 0, on both
  routes); WebGL 2 without EXT_clip_control is then not served by this route. Refused by name
  besides those above: records wider than one capture target (`GeometryReplayComponents`).
  `tests/stage_draw.rs` (`ours_replayed`) does exactly this.
- A pixel shader reading `SV_PrimitiveID` without a geometry or hull stage where the back end lacks
  `Features::PRIMITIVE_INDEX` (WebGPU, `Features::COMPUTE`; since run 45): convert the pixel shader
  with `VariantFlags::PRIMITIVE_ID_FROM_STAGE` and `linked` (as for clip distances), the vertex
  shader with every input pulled, `draw_base: Helpers` and the same `linked`.
  `Stages::primitive_numbered(backend, &vertex, &pixel_key)` once per pair; per draw `stages.run`,
  then draw the `FinalDraw` as after a geometry stage (a list, no indices: `index_offset` is
  `None`). Each primitive's vertices are copied with its number, so the front's own provoking
  vertex rule holds. Refused by name: adjacency (`PrimitiveIdAdjacency`), cull distances on the same
  vertex shader (`PrimitiveIdCullDistance`), the built-in inside a struct of inputs
  (`PrimitiveIdInStruct`). Indexed strips run (since run 50). `tests/stage_draw.rs` does exactly
  this. Behind a geometry shader the pixel shader's number is still not passed (next).
- The same, and cull distances from a vertex shader alone, on a back end without compute (WebGL 2,
  `Features::TRANSFORM_FEEDBACK`; since run 85): convert the vertex shader with its inputs as drawn,
  `draw_base: Helpers`, `CLIP_DEPTH_REWRITE` where the back end lacks clip depth from zero, and the
  pixel shader's `linked` (for primitive numbers the pixel shader as above);
  `GeometryReplay::primitive_numbered(backend, &vertex, &pixel_key)` or
  `GeometryReplay::culled(backend, &vertex)` once per vertex shader (and pixel key); per draw,
  outside the pass, `replay.run` with a `ReplayDraw` as for a vertex shader alone (the API draw's
  layout, streams and indices, `geometry_sets` all `None`, `stream_output: None`), then draw the
  returned `FinalDraw` through `replay.passthrough()` and `replay.vertex_layout()` (a list without
  indices; a dropped primitive's vertices lie outside every clip volume). Make index buffers with
  `BufferUsage::COPY_SRC`, as indexed strips are read back. Refused by name: adjacency
  (`PrimitiveIdAdjacency`, `CullDistanceAdjacency`), indirect draws (as every replay), records wider
  than one capture target (`GeometryReplayComponents`), both on one vertex shader
  (`PrimitiveIdCullDistance`); stream output from a vertex shader writing cull distances stays
  refused (`StreamOutCullDistance`). `tests/stage_draw.rs` (`ours_replayed`, `Route::Primitives`)
  and `tests/geometry_draw.rs` (`ours_replayed` with the `CULLED` cases) do exactly this.
- Cull distances from a vertex shader without a geometry or hull stage (WebGPU, `Features::COMPUTE`,
  the back end without `Features::CULL_DISTANCE`; since run 39): the vertex shader converted as for
  a geometry shader (every input pulled, `draw_base: Helpers`, its other flags as for a plain draw,
  `CLIP_DISTANCE` with `linked` where clip distances are not native), then
  `Stages::culled(backend, &vertex, stream_output)` once per vertex shader and stream output
  declaration, and per draw `stages.run` with no geometry sets; draw the returned `FinalDraw` as
  after a geometry stage (indexed, list topology). `tests/geometry_draw.rs` (`CULLED`) does this.
- Hull and domain shaders (WebGPU, `Features::COMPUTE`): convert the pixel shader first, the domain
  shader with `output_interpolation` from the pixel shader's inputs, the hull shader plain, the
  vertex shader with every input pulled. `Stages::tessellated(backend, &vertex, &hull, &domain)`
  once per triple. Per draw, outside the pass: `stages.run` as for geometry, with the hull shader's
  sets (its slots in set 0, its helper block in set 3, `StageSet::COMPUTE`) as `geometry_sets` and
  topology `InputTopology::PatchList(n)` (Direct3D's `n_CONTROL_POINT_PATCHLIST`). The returned
  `FinalDraw` has `tessellated: Some(t)`: in the pass draw the domain program (`domain.modules[0]`)
  with the pixel shader, a vertex layout with no streams, the domain shader's own sets, and in set
  3 `t.patches` at `DOMAIN_PATCHES_BINDING` and `t.points` at `DOMAIN_POINTS_BINDING` (read-only
  storage, `StageSet::VERTEX`, beside the domain shader's helper block if it has one); indices u32
  from `FinalDraw::buffer` at offset 0, topology `FinalDraw::topology`, range
  `DrawRange::IndexedIndirect { args: &t.arguments, offset: 0 }` (the arguments start past the
  index list's header, so bind the buffer at offset 0). `tests/tessellation_draw.rs` (`scene_ours`)
  does exactly this. Patches may have more control points than the hull shader declares (Direct3D
  12 draws them; the hull shader reads the first of each patch); fewer is `TopologyMismatch`.
- A geometry shader or stream output after a domain shader (since run 30): convert the domain
  shader as above, the geometry shader as for geometry; `Stages::tessellated_with(backend, &vertex,
  &hull, &domain, geometry, stream_output)`; per draw `stages.run_after_domain(backend, &draw,
  &DomainSets { domain_sets, geometry_sets })`, where `draw.geometry_sets` are the hull shader's
  and `domain_sets` the domain shader's own sets (its slots in set 0, its helper block in set 3,
  `StageSet::COMPUTE`, without the patches and points: the helper binds those). With a geometry
  shader the returned `FinalDraw` is drawn as after any geometry shader; without one it is the
  tessellated draw above (drawn only when the stream is rasterized). A geometry shader there reading
  `SV_PrimitiveID` is refused (`GeometryPrimitiveIdAfterDomain`). `tests/tessellation_draw.rs`
  (`ours_staged`) does this.
- A geometry shader writing `SV_RenderTargetArrayIndex` or `SV_ViewportArrayIndex` (since run 30):
  `stages.selects()` is true. Run the stages once, then draw the `FinalDraw` once per layer of the
  bound target views and per viewport (`LayerSplit`, `MultiViewportSplit`; count and warn), each
  run into that layer's views with that viewport alone; the pass-through's helper constants are
  `stage::PASSTHROUGH_HELPERS` (the run's layer and viewport, u32, a 16-byte uniform block at
  `helper_binding(Vertex)` in set 3) and the pixel shader's `HelperValue::Layer`/`Viewport` are the
  run's. Each run draws only the vertices that name it (others are placed outside the clip volume).
  `tests/geometry_draw.rs` (`layered_geometry_draws_as_direct3d_11_does`) does this.
  A primitive naming a layer at or past the views' layer count (since run 43): set
  `VariantFlags::LAYER_FROM_STAGE` and the same `linked` (location one past every location either
  program lists, as for clip distances) in the geometry shader's key and the pixel shader's. The
  pass-through then reads `stage::PASSTHROUGH_LAYER_COUNT` (the views' layer count, u32, at byte 8
  of the same block) beside `PASSTHROUGH_HELPERS`, keeps such primitives in layer 0's run and passes
  the named layer as a flat u32 at `linked.location` plus the linked values' slots; the pixel
  shader reads its layer from there and no longer lists `HelperValue::Layer`
  (`layers_past_the_views_draw_into_the_first_as_direct3d_11_does`; Wine's test_layered_rendering).
- `Stages::destroy(backend)` releases its programs and buffers.
- A vertex shader writing no `SV_Position` (feeding stream output, a geometry or a hull shader)
  converts: its program writes the position (0, 0, 0, -1), outside every clip volume, so drawn it
  rasterises nothing (not checked against Direct3D drawing such a shader), and its records carry that
  position. `reflection.outputs` lists no position for it.

```rust
use wcontainers_shaders::{convert, Source, ConvertError};
let converted = convert(Source::Dxbc(bytecode), &variant_key, None)?;
let module: &wcontainers_gpu::ShaderModule = &converted.modules[0];  // hand to Backend::create_shader
let reflection = &converted.reflection;
```
- `variant_key` is `wcontainers_gpu::VariantKey`; `VariantKey::default()` for the plain program. Built:
  `draw_base` (`None`, `Helpers`, `Arguments`), `output_interpolation`, `lod_bias`, `pulled`,
  `constants_as_texture`,
  `constants_as_storage`, `samples`, `linked`, `mirror_once_in_shader`, and the flags
  `CLIP_DEPTH_REWRITE`, `VIEWPORT_DEPTH_FLIP`, `VIEWPORT_ADJUST` (vertex programs; ignored on
  others), `COUNTING`, `SAMPLE_MASK`, `DUAL_SOURCE`, `DEPTH_BIAS` (pixel programs), `CLIP_DISTANCE`
  and `DEPTH_CLAMP` (both), `TEXTURE_INFO` and `min_lod_clamped`. Every other field or flag
  set, and a root signature, is refused by name (the list is under Run 10). Each helper value a variant needs is in `reflection.helpers` at its offset in the
  stage's block (`helper_size` bytes, bound at `helper_binding(stage)` in set 3), after `DrawBase` when
  both are asked: the front writes `ViewportAdjust` as (scale x, scale y, offset x, offset y) of clip
  x and y (offsets times w), `SampleMask` as a u32, `LodBias { sampler }` as an f32,
  `TextureLevels { texture }` as a u32, `DepthBias` as (constant units, slope scale, clamp, the depth
  format's step) in f32, `DrawArguments` as (word offset of the draw's arguments, 1 when indexed, draw
  number) in u32.
- `min_lod_clamped`: for each pair n of `reflection.sampled_pairs` whose texture is clamped, bind a
  sampler like the pair's own but with its level range starting at max(its own, the clamp in the
  view's numbering) at `CLAMPED_SAMPLER_BINDING` + n in set 3; write `MinLod { texture }` as (the
  clamp's f32 bits, the view's level count). A clamped texture that is compared is refused
  (`MinLodOnComparison`).
- Clip distances: a vertex program writing `SV_ClipDistance` converts to a module with
  `@builtin(clip_distances)`, which needs the back end's `Features::CLIP_DISTANCE` (wgpu's
  `CLIP_DISTANCES`; WebGL's WEBGL_clip_cull_distance); without it the front asks for the
  `CLIP_DISTANCE` variant in both programs' keys. `wcontainers_shaders::capabilities(module)` gives
  what a module needs beyond WebGPU's core.
- `CLIP_DISTANCE` and `DEPTH_CLAMP` (without `Features::DEPTH_CLAMP`): set the flag in both keys and
  the same `linked` in both: `location` one past every location of the vertex program's
  `reflection.outputs` and the pixel program's `reflection.inputs` with no `system` value (convert
  both plain first, as `linked_location` in `tests/variant_draw.rs` does), `clip_distances` the vertex
  program's count with `CLIP_DISTANCE` (zero otherwise). The pixel program needs `samples` (1 or 4)
  with `CLIP_DISTANCE`. With `DEPTH_CLAMP` write `DepthRange` as (near, far) f32 of the API's viewport;
  the raster block keeps depth clipping on (the program keeps z inside), and those draws lose early
  depth tests (the program writes depth). Check the vertex program's locations plus the added ones
  against the back end's inter-stage limit.
- `CLIP_DISTANCE` behind the stage helper (built since runs 38 and 39; both fronts asked for it
  in their latest runs, from creation-time conversions): the stage helper's pass-throughs take
  the key of the conversion they come from, so the key must carry `CLIP_DISTANCE` and the pixel
  shader's `linked`. Geometry stage: convert the geometry shader per pixel shader with
  `CLIP_DISTANCE` and `linked` (as the vertex shader is converted); its `modules[1]`, the
  pass-through `Stages::new` takes, then passes the distances at `linked.location` and writes no
  `ClipDistances` built-in. Cull route (`Stages::culled`): convert the vertex shader with every
  input pulled, `draw_base: Helpers`, `CLIP_DISTANCE` and `linked`; `cull_programs` passes its
  linked values on as any output location. `tests/geometry_draw.rs` draws both (`vs_cull` writes
  a clip distance beside its cull distances; the `gs_clip` cases) on wgpu's Direct3D 12 path,
  which has no clip distances, against real Direct3D 11.
- `mirror_once_in_shader` (without `Features::MIRROR_ONCE`): bind the sampler with each mirror-once
  axis made `Clamp`, and write `SamplerState { sampler }` (nine words: min, mag, mip filter, the three
  address modes of the API's sampler as `AddressMode as u32`, anisotropy, then the level range as
  f32) at its offset; the member is 48 bytes (three four-word vectors).
- `pulled` (inputs past the back end's vertex buffer or attribute limits, step rates it lacks): list
  `(location, format)` per pulled input (location = the input's register). Each gets a read-only
  storage binding in set 0 (`reflection.bindings`, `added: Some(PulledStream(location))`): bind its
  stream's buffer from the stream's offset rounded down to `min_storage_offset_alignment`, to the
  buffer's end (a multiple of 4 bytes). Write `PulledStream { input }` as four u32: the offset's
  remainder plus the attribute's offset, the stride, then per vertex (0, 0), per instance with step
  rate n (n, (n - 1) * first instance), step rate 0: stride 0, divisor 1, bias 0 and the first
  instance's element added to the offset. The program reads `instance_index`, which counts from the
  first instance, so it needs no other base. Each pulled input takes a storage binding of the vertex
  stage: the front checks the total against the back end's per-stage storage limit (WebGPU's
  default is 8) and refuses past it.
- `filter_in_shader` (a texture whose view's format the back end cannot filter, sampled through a
  sampler with a linear filter: R32_FLOAT and depth read as colour on WebGPU) and
  `border_in_shader` (a sampler with a border address mode where the back end has no border colour,
  every sampler on WebGPU): bind the texture as before (it is only loaded now, so WebGPU's layout
  makes it unfilterable) and the program's real sampler at its slot; write `SamplerState { sampler }`
  as for mirror-once, `BorderColor { sampler }` (four f32), `TextureComponents { texture }` (a bit
  per component the view's format has, red first: R32_FLOAT 1, RGBA 15), `LodBias` with `lod_bias`,
  `MinLod` with `min_lod_clamped`, and with `TEXTURE_INFO` (WebGL 2) `TextureLevels`. The texture's
  pair leaves `reflection.sampled_pairs`. Comparisons, cubes, depth and integer textures are
  refused by name (Run 15).
- `constants_as_texture` (WebGL 2, past its uniform block count or size): the slot's
  `reflection.bindings` entry becomes kind `Texture` with `added: Some(ConstantsAsTexture(register))`;
  bind an RGBA32 unsigned 2D texture holding the buffer's bytes, row r at (r % 256, r / 256)
  (`CONSTANTS_TEXTURE_WIDTH`), copied when the buffer changed.
- `DEPTH_BIAS` writes the depth itself, so the front draws with the raster block's bias at zero and
  without early depth tests for those draws.
- `draw_base: Arguments` reads the bases from the indirect argument buffer, bound read-only (storage)
  at `DRAW_ARGUMENTS_BINDING` in set 3 for the vertex stage; the helper block holds where the draw's
  arguments start.
- `samples` is the targets' sample count where the program reads it (`GetRenderTargetSampleCount`);
  a program that reads it converts only with it set (`Unsupported("SampleCount")` otherwise).
- `COUNTING` programs have no side effects left (`extras.side_effects` false); `DUAL_SOURCE` needs
  WebGPU's dual-source-blending (the module is validated with naga's `DUAL_SOURCE_BLENDING` then) and
  a program writing targets 0 and 1 (`Unsupported("DualSourceWithoutTwoTargets")` otherwise).
- `constants_as_storage` moves each named constant buffer to the storage range: its
  `reflection.bindings` entry becomes kind `Storage`, the first storage binding past the program's
  own, `added: Some(ConstantsAsStorage(register))`; bind the same buffer as read-only storage.
- `SAMPLE_MASK` is the plan's `SampleMaskInShader` route, on WebGL 2 too: the program writes
  `gl_SampleMask`, which the WebGL 2 back end runs where it declares `Features::SAMPLE_SHADING`
  (OES_sample_variables and OES_shader_multisample_interpolation: Chrome 157 on the RTX 3080 has
  both, chrome.log of G6a run 1; mozangle, the ANGLE the drawn tests here use, lacks them). The
  front asks for it there only with `SAMPLE_SHADING`; without it the plan has it report no
  multisampling. Drawn here on WebGPU only (`variant_draw`); the WebGL track's
  `programs_run_per_sample_and_write_the_sample_mask` passes in Chrome.
- Linking a vertex and a pixel shader: convert the pixel shader first, then ask for the vertex
  shader with `output_interpolation` = every `(location, interpolation)` of the pixel shader's
  `reflection.inputs` with no `system` value whose interpolation is not `Perspective` (as
  `tests/draw.rs`'s `ours` does). WebGPU and GLSL ES require the two stages to agree; without it a
  pixel shader reading `nointerpolation`, `noperspective`, `centroid` or `sample` values fails
  pipeline creation or program linking.
- Bindings follow 6.4 for Direct3D 10 and 11: set 0 for vertex and compute shaders' slots, set 1 for
  pixel shaders' (including their UAVs), binding = `SlotKind::first_binding()` + register. Each is
  listed in `reflection.bindings` (kind, register, set, binding). Resource arrays and slots beyond a
  kind's range are refused.
- Inputs and outputs: every user value lives at location = its register, as a four-component value
  of its scalar type (so the types of a vertex and a pixel shader converted apart always match,
  whatever components each uses; their interpolation matches through `output_interpolation`). `reflection.inputs` and `outputs` give semantic, index, register, mask,
  location and component. Pixel shader output n is `@location(n)`, `SystemValue::Target(n)`.
- With `draw_base: Helpers`, a vertex shader reading `SV_VertexID` or `SV_InstanceID` reads the
  draw's bases from its helper block (set 3, `helper_binding(Vertex)`), `HelperValue::DrawBase` at
  offset 0, 16 bytes (`reflection.helpers`, `helper_size`). Without it, `SV_VertexID` is
  `vertex_index` and `SV_InstanceID` is `instance_index` as they are (right only when the bases are 0).
- `reflection.sampled_pairs` lists (texture register, sampler register) pairs in first-use order.
  `reflection.extras` for pixel shaders: early tests, side effects (UAV writes), discards, per sample.
- DXIL: `convert(Source::Dxil(bytecode), &variant_key, None)`, same result shape. Bindings are
  numbered by Direct3D 12's rule (run 22): stage set as for Direct3D 11; within it each kind's
  slots are the program's own resources of that kind in (space, register) order from the kind's
  first binding (shader resource views, buffers among them, are `Texture`, unordered access views
  `Storage`, counters `Counter` at their view's slot). Find each binding's descriptor by its
  `space` and `register`; per-slot variant fields take `ReflectedBinding::slot()`. `draw_base` `None` (bases read as zero), `Helpers` and
  `Arguments` are taken as for DXBC. Inputs and outputs are listed as for DXBC
  (semantic names from the container's signatures, location = register), but unlike DXBC each value
  keeps its own width and type (`vs_transform.dxil` writes `@location(1) vec2<f32>`), so a
  Direct3D 12 vertex shader writing a float4 that the pixel shader reads as float2 fails WebGPU's
  pipeline check: until that is fixed (first under Next), the front must link only stages whose
  shared locations have the same width and type, comparing `reflection.outputs` with the pixel
  shader's `reflection.inputs` (mask and type), and answer a mismatch as the API's refusal. A
  pixel shader's unread `SV_Position` may be listed where the DXBC route leaves it out. Sampled
  pairs are listed as for DXBC, by register (a pair's register spaces are not in the pair).
- DXIL-only refusals: `NonFiniteConstant`, `ConstantComponentIndexing`, `FloatToIntWidth`,
  `PackedStageValues`, `PrimitiveId`, `VertexWithoutPosition`, `Texture1DArray`, and `Reader(..)`.
- On WebGL 2, the back end's GLSL writing refuses programs using fine derivatives
  (`MissingFeatures(DERIVATIVE_CONTROL)`), as GLSL ES 3.00 has none; a front on WebGL 2 reports a
  feature level without them.
- Constant buffers are declared with the size the bytecode gives them, on both routes (DXIL's
  dynamically indexed ones since run 6), so binding the program's own buffer (or any range at least
  that long) satisfies WebGPU.
- Errors: `Rejected` (the container's checksum does not match, which Direct3D answers with
  `E_INVALIDARG` too, or dxbc-spirv or dxil-spirv refused the bytecode, with its message, including
  their failed assertions and exceptions natively), `Unsupported(name)` (a named feature or variant
  not provided yet, for example `GeometryShader`, `StorageImage`, `TypedBuffer`, `Atomics`,
  `RegisterSpace`, `PackedStageValues`), `Reader(text)` (a gap of naga's SPIR-V reader, DXIL
  only), `Malformed` and `Invalid` (our lowering's own checks or naga's validation refused the
  converter's output, a panic in the lowering included; on real bytecode these are our bugs, and
  the corpus tests fail on any of them). A front answers each with the API's own refusal
  (`E_INVALIDARG` at shader creation).
- On WebAssembly a converter failure that is not an error return (dxil-spirv's crashes on signed but
  malformed DXIL, see Known gaps; a panic in our lowering or in naga's validator, which natively
  is a `Malformed` error, gap A16) traps the shaders module; the host reports it as a failed
  conversion.

## Measurements

Run 45, 2026-11-19, counts only (laptop i7-8750H, load 0.3 to 2.7, 4 cargo jobs; desktop shared
through the queue, load not taken): the stage helper's drawn tests (`stage_draw`, both tests, 4
seeds) 1.25 s of tests in one GPU job, 8 seeds of the line test 0.8 s; geometry_draw's 4 seeds
3.8 s (run45/gpu4.log, gpu5.log). Neither route was timed against another: the counts are
correctness only.

Run 43, 2026-11-19, counts only (laptop i7-8750H, load 0.5 to 5 with another track building, 4
cargo jobs; desktop shared through the queue, load not taken): the laptop's whole suite with every
corpus about 6 minutes of tests after the build, the corpus test 268 and 329 s of it; one GPU job of
compute, contention, geometry and variant draws 13 s of tests; Chrome's compile of the own corpora
4.4 s in the queue.

Run 40, 2026-11-19, counts only (laptop i7-8750H, load 3 to 14 with another track building, 2 to
4 cargo jobs; desktop shared through the queue's hazard group, 3 cargo jobs, load not taken), logs in
scratch `graphics-shaders` (run40-*):
- Desktop, RTX 3080: `depth_views_with_component_mappings_read_as_mapped` (run40-gpu4.log, seeds 1
  to 4): 48 comparisons, all 0 texels apart, and the refusal of the mapping moving red checked on
  both programs and routes; `textures_compared_and_read_plainly_draw_as_direct3d_does` 32
  comparisons, all pass; `wave_operations_draw_as_direct3d_12_does` (run40-gpu2.log): `ps_wave` 0
  of 4096 texels apart (2074 drawn), `ps_wave_signed` 0 of 4096 (2591 drawn).
- Laptop, the whole suite but the fuzz with every corpus and glslang (run40-suite1.log): 20 test
  binaries pass, the corpus binary fails only on the runner corpus's known `distance-4`.
- Desktop, at 3f73914a: Windows lint clean, then the whole suite as one GPU job
  (`cargo test -p wcontainers-shaders --no-fail-fast -- --test-threads 1 --skip mutated`,
  run40-suite-gpu.log): 21 test binaries, all pass.
- Chrome's compile of every written file (`web/check`, run40-chrome.log, one GPU job): every WGSL
  file of every corpus compiles (corpus 185, games 519, games-dxil 425, own 118, own-dxil 59, rig
  79, rig-dxil 55, runner 1432, runner-dxil 1375), the new `ps_shadow_wide`, `ps_wave_signed` and
  the runner's wave-ops-int among them. WebGL 2 fails 117 GLSL files, each of a kind it lacks
  (bitfield operations and `findLSB`/`findMSB`, gathers, multisampled samplers, the primitive
  index) or a cull-distance vertex shader's plain GLSL ("array size mismatch" at the cull
  distance array; WebGL 2 has no cull route, so nothing runs it).
- Clean-up: the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders` (5.7 GB) and
  the desktop's `C:\wc\target\phase-2-graphics-shaders` deleted; scratch keeps the logs and
  `run40/out` (47 MB, the written files Chrome compiled).
Run 39, 2026-11-19, counts only (laptop i7-8750H, load 3 to 11 with another track building, 4 or 5
cargo jobs; it was unreachable twice for some minutes; desktop shared through the queue, 3 cargo
jobs, load not taken), logs in scratch `graphics-shaders/run39`:
- Desktop, RTX 3080, `geometry_draw` (geomdraw5.txt, seeds 1 to 4): 84 drawn comparisons and 36
  stream output comparisons, all pass; the six cull cases 0 to 15 texels apart per case by at most
  1, two line cases one texel each at a clipped line end.
- Laptop, every corpus with glslang (corpus2.txt, then suite1.txt and suite2.txt after the record
  update): all pass but the runner corpus's known `distance-4`.
- Clean-up at the end: the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders`
  (5.3 GB) and the desktop's `C:\wc\target\phase-2-graphics-shaders` and
  `C:\wc\scratch-shaders-out` deleted; no build folder was made on the Mac or in scratch.
- Desktop, after the sweep step: Windows lint clean (winlint2.txt), then the whole suite as one
  GPU job (`cargo test -p wcontainers-shaders --no-fail-fast -- --test-threads 1 --skip mutated`,
  suite-gpu.txt): 21 test binaries, all pass.
- Literal check (check5.txt): passes; changes that could add lines: 16 (the 12 of run 38, and
  4bcacbf5 the start, 2cc8ad76 the init after the baseline arrived by a merge, bd53fbb4 and
  949667f2 new rules, for literals already in the tree, all from the merged tracks).
Run 38, 2026-11-18, counts only (laptop i7-8750H, load 3 to 39 with another track building, 5
cargo jobs, 2 when the load was 8 or more; desktop shared through the queue, 3 cargo jobs, load
not taken), logs in scratch `graphics-shaders/run38`:
- Laptop, the whole suite with every corpus and glslang after this run's changes (suite-2.log):
  every test passes but the runner corpus's known `distance-4` (a reader rejection, not a named
  refusal). Records updated in this run: own and own-dxil (the new `gs_clip`, `vs_clip_six`), runner
  (four geometry shaders now `CullDistance`).
- Laptop, WebAssembly against native (suite-2.log, `wasm-run10.sh` with 2 jobs after a first try
  at 5 was killed for memory): rig, rig DXIL, own DXIL and games DXIL 0 files apart; corpus 2,
  games 3 and own 1, all the known gap (dxbc-spirv's unspecified evaluation order in the
  division and remainder helper): `116_cs_group_index_code` and `d599fbc7efa911be` as before, and
  the new `gs_clip`'s line in own's `cache.txt` (its `primitive % 4`; compared natively and under
  wasmtime: only the helper's operand order differs). The wasm corpus run stops at the runner test,
  which needs `WCONTAINERS_RUNNER_SHADERS` (the script does not give it), after the others ran.
- Desktop, RTX 3080, `variant_draw`, `geometry_draw`, `tessellation_draw` (gpu4.log, default
  seeds and counts): all pass; `gs_clip` 1 to 13 texels apart per case by at most 1 over 4
  seeds; `vs_clip_six` natively 0 apart, in the program 14 of 4096 by at most 2 (both routes).
- Desktop, Windows lint (`cargo clippy -p wcontainers-shaders --all-targets -- -D warnings`)
  clean, then the whole suite as one GPU job after every change of this run (`cargo test -p
  wcontainers-shaders --no-fail-fast -- --test-threads 1 --skip mutated`, 22:57 to 23:07,
  suite-gpu.log): 20 test binaries, all pass.
- Literal check on the desktop (check3.txt, check4.txt after the last changes): passes.
- Chrome's compile of every corpus as one GPU job (chrome.log before the WGSL writer fix,
  chrome2.log after it and the `VertexStorageWrite` refusal; written files from the laptop's
  suite run): every converted shader's WGSL compiles in Chrome's WebGPU: corpus 185, games 519,
  games DXIL 425, own 116, own DXIL 56, rig 79, rig DXIL 55, runner 1429, runner DXIL 1369. GLSL
  ES files Chrome's WebGL 2 refuses: corpus 15, games 10, games DXIL 1, own 11, own DXIL 4, rig 3,
  rig DXIL 3, runner 35, runner DXIL 26, every one for what GLSL ES 3.00 lacks (bitfield
  operations, `findMSB`/`findLSB`, `textureGather`, multisampled samplers, `gl_PrimitiveID`; 6.5
  gives those to the WebGL 2 back end, or they are absent there), as before.
- After those: the laptop's corpora once more (suite-4.log, records updated: the runner's
  `depth-bounds-1-vs` now `VertexStorageWrite`, nine runner shaders now report
  `RasterizerOrderedView` instead of a view kind) and the desktop's whole suite as one GPU job
  with Windows lint first (suite-gpu2.log, 00:03 to 00:08): lint clean, every test binary passes.
- Clean-up at the end: deleted the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders`
  (and run 37's), its `outw` and this run's literal-check clone, the desktop's
  `C:\wc\target\phase-2-graphics-shaders` and `C:\wc\out\phase-2-graphics-shaders\chrome`.
  Kept in scratch `run38`: logs and the last written files with Chrome's verdicts (`out2`).

Run 37, 2026-11-18, counts only (laptop i7-8750H, load 1.0 to 5.4 with another track building, 5
cargo jobs, until it went down at about 20:25 and stayed down to the end of the run; desktop shared
through the queue, 3 cargo jobs), logs in scratch `graphics-shaders/run37`:
- Laptop, the whole suite with every corpus and glslang, before `ps_two` existed (suite-1.log):
  every test passes but the own corpus (the new `cs_readonly`, then unrecorded; both builds
  `ok-wgsl glsl-write`, now recorded) and the runner corpus's known `distance-4`. Every other
  corpus verdict as recorded, so the narrower DXIL targets changed none (runner rt-format-mismatch
  1 to 16 stay `ok`).
- Desktop, RTX 3080: `compute_draw` (run37-compute.log, default seed 1, 4 seeds): 32 comparisons
  of each test, 0 failed, `cs_readonly` 0 words apart on both routes at the views' start and at
  seeded first elements. `draw` (draw.log): 7 tests pass; `ps_two` 0 of 4096 texels apart from
  real Direct3D 12 on WebGPU and WebGL 2 (its two written components). Not checked: that the
  WebGPU pipeline fails without the change (the runner's line 53 is that evidence).
- `ps_two`'s records (`ok`, both builds) were taken on the desktop, without glslang (the laptop
  was down); its GLSL compile by glslang has not run.
- Desktop, the whole suite as one GPU job after every change of this run (`cargo test -p
  wcontainers-shaders --no-fail-fast -- --test-threads 1 --skip mutated`, default seeds and case
  counts, 21:00 to 21:05, suite-gpu3.log): 21 test binaries, all pass. Windows lint (`cargo clippy
  -p wcontainers-shaders --all-targets -- -D warnings`, suite-gpu2.log): clean.
- Not rerun this run: the corpora on the laptop after `ps_two` and the stage.rs and boundary
  literal steps (the laptop was down; the desktop has no corpus but own and rig), Chrome's compile
  and the WebAssembly comparison.
- Clean-up at the end: deleted the desktop's `C:\wc\target\phase-2-graphics-shaders` and
  `C:\wc\out\shaders37`. The laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders`
  (built in this run) is still there: the laptop was down at the end; the next run deletes it
  after its own use. No build output was made in scratch.

Run 36, 2026-11-18, counts only (laptop i7-8750H, load 0.2 to 1.8, 5 cargo jobs; desktop shared
through the queue, whose hazard group held each job for 10 to 30 minutes), logs in scratch
`graphics-shaders/run36`:
- Laptop, the whole suite with every corpus and glslang (suite-start.log, suite-2.log,
  suite-4.log): every test passes but the runner corpus's known `distance-4` (Run 36 above). The
  runner corpus as run 35 ended it: DXBC 1503 (1430 converting, 73 refused by name), DXIL 1517
  (1370 converting, 133 refused by name, 13 refused by naga's reader, 1 rejected by dxil-spirv).
  The games corpus: DXBC 529 as recorded; DXIL 443 (430 as recorded, and the 13 new Godot 3D
  fragment shaders, which convert and need storage buffers and subgroups GLSL ES lacks:
  `ok-wgsl glsl-write`). The 191-shader corpus, rig and own: as recorded.
- Desktop, RTX 3080 (draw1-pueue.log, draw2.log): `draw`, 5 of 6 tests pass (every own, rig,
  primitive index and wave case 0 texels apart, as before). `native_16_bit_integers_draw_as_direct3d_12_does`
  (new in run 35, never run before) failed twice the same way: after its real Direct3D 12
  reference draw, wgpu offered only WARP. The test now makes both back ends before the reference.
- Desktop, the whole suite as one GPU job (`cargo test -p wcontainers-shaders -- --test-threads 1
  --skip mutated`, default seeds and case counts, 19:45 to 19:47, suite-gpu2.log): 21 test
  binaries, all pass, among them `native_16_bit_integers_draw_as_direct3d_12_does`, the own cases
  with run 35's `ps_narrow` and `ps_component`, and the tessellation tests (a job of run 33 or 34 at
  11:28 had failed 4 of 66 `stream_output_after_the_domain_stage` comparisons at 2 seeds; at the
  default seed it passes here, as in run 34; 2 seeds not rerun).
- Chrome 157.0.8103.78 on the desktop (WebGPU on Direct3D 12, chrome.log): the 13 new Godot
  shaders' WGSL all compile (`web/check` on a directory of only those 13).
- Windows lint (`cargo clippy -p wcontainers-shaders --all-targets -- -D warnings`, inside the
  suite job's build): clean.
- Not rerun this run: Chrome's compile of the other corpora (unchanged verdicts) and the
  WebAssembly comparison.
- Clean-up at the end: deleted the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders`
  and the copied Godot shaders there, the desktop's `C:\wc\target\phase-2-graphics-shaders`,
  `C:\wc\shaders-dump36` and this branch's `programs` and `chrome` output folders under
  `C:\wc\out`; no build output was made in scratch (logs, dumps and records kept there).
- The rig: two jobs of one branch sent at once clobber each other's tree (the second sends its
  tree, the first then fails with 125, "holds tree ..., not the sent ..."); send them one after the
  other.

Run 35, 2026-11-18, counts only (desktop shared through the queue, 3 cargo jobs, load not taken;
the laptop down from 16:52), logs in scratch `graphics-shaders/run35`:
- The runner corpus (verdict classes; DXBC 1503, DXIL 1517), at the start of the run (laptop, with
  glslang) and at its end (desktop, glslang absent there, the records keep the laptop's
  `glsl-compile` verdicts where the desktop says ok): DXBC converting 1429 then 1430, refused by
  name 61 then 73, other failures 13 then 0. DXIL converting 1346 then 1370, refused by name 35
  then 133, refused by naga's reader 93 then 13 (wave minimums and maximums of the other
  signedness 2, a pixel shader's layer and viewport index 2, `CalculateLevelOfDetail` 4,
  `SampleClamp` 2, sparse feedback 3), other failures 45 then 1 (dxil-spirv's rejection of distance-4).
- The games corpus (DXBC 529, DXIL 430, desktop): every verdict as recorded; 3 DXIL changed this
  run (one converts, one now `RenderTargetArrayIndex`, one past `ConstantComponentIndexing` to
  `TypedBufferFormatKind`, the corpus test's views of mixed kinds); the run 34 record update (2
  `PackedStageValues` now converting) was found by the laptop's first run.
- The 191-shader corpus, rig and own: as recorded (laptop at the start; rig and own on the desktop
  after every change). Not rerun this run: glslang over the new verdicts, Chrome's compile, the
  WebAssembly comparison (the laptop was down).

Run 33, 2026-11-18, counts only (laptop load 4 to 8, another track building; desktop shared through
the queue's hazard group), logs in scratch `graphics-shaders/run33`:
- Desktop, RTX 3080 (draw3.log, then draw4.log after the allowance): `shadow_draw` seeds 1 to 4, 4
  filter scenes, fxc and dxc builds, 32 comparisons against real Direct3D 11: point plain sampling
  (with point and with linear comparisons) 0 of 4096 texels apart in all 16; linear plain sampling
  through `filter_in_shader` 2361 to 2376 of 4096 texels differ in the green channel (two linear
  samples summed), largest 0.0065, inside twice `FILTER_ALLOWANCE` (1/128 each); every other
  channel exact. `variant_draw`: the three `targets_in_shader` cases 0 of 4096 texels apart, the
  other variants as before (4 tests pass). `filter_draw` 2 seeds: 108 comparisons, 0 failed.
- Laptop (suite1.log, the tree before the last layout and SPIR-V changes; suite2.log after):
  `cargo test --no-fail-fast -p wcontainers-shaders -- --include-ignored --skip mutated` all pass,
  the corpus test 9 of 9 (every corpus's verdicts as recorded, so dxbc-spirv's numbers read from
  its header convert every shader as before), 412 s for the corpus binary. suite3.log, after the
  helper blocks' sizes and alignments were derived: the same, all pass, clippy with warnings denied
  clean (corpus binary 509 s, laptop load about 6).
- Desktop, RTX 3080 (draw5.log, after that change, Windows clippy clean): `variant_draw`,
  `early_draw` (8 scenes), `pull_draw`, `clear_draw` (64 cases), `geometry_draw` (2 seeds),
  `compute_draw` (2 seeds) and `shadow_draw` all pass against real Direct3D 11 (every comparison
  printed 0 failed). draw4.log: `draw` 5 of 5 pass.

Run 32, 2026-11-18, counts only (laptop load 1.8 to 4.1, another track building; desktop shared
through the queue), logs in scratch `graphics-shaders/run32`:
- Laptop (suite2.log, then the own corpus test again after recording ps_shadow.dxil): `cargo test
  --no-fail-fast -p wcontainers-shaders -- --include-ignored --skip mutated` 68 passed, then the one
  failing own-verdicts test passed with the new records (corpus, rig, rig DXIL, own, own DXIL and
  games verdicts as recorded; own ps_shadow, ps_msloop, ps_msswap and own DXIL ps_shadow newly
  recorded); clippy with warnings denied clean. The corpus test binary took 313 s.
- Desktop, Chrome on the RTX 3080 (chrome.log, one GPU job, 30 s once started): every written WGSL
  file compiled, 1398 of 1398 (corpus 185, games DXBC 519, games DXIL 409, own 107, own DXIL 45,
  rig 79, rig DXIL 54), so every converted shader, the newly converting geometry shaders and
  ps_id/ps_primid included, compiles for WebGPU. GLSL ES 3.00: 1016 compiled, 45 failed, each for
  what GLSL ES 3.00 lacks (bitfield operations, `textureGather`, multisampled samplers) or for
  `gl_PrimitiveID` (4: ps_primid and geometry.ps_id, both builds; WebGL 2 has no primitive index),
  the same classes as glslang's `glsl-compile` verdicts.

Run 31, 2026-11-18, counts only (laptop load 2 to 11, another track building; desktop shared
through the queue, whose hazard group held each of our jobs for 10 to 40 minutes), logs in
scratch `graphics-shaders/run31`:
- Laptop (test1.log, test2.log): `cargo test -p wcontainers-shaders -- --include-ignored --skip
  mutated` 67 passed, 0 failed (corpus, rig, rig DXIL, own, own DXIL and games verdicts as
  recorded, after recording rig geometry.ps_id (both builds) as converting and own ps_contend and
  ps_primid); clippy with warnings denied clean. Rerun after the last changes: geometry, variants
  and the library's tests pass, own corpus as recorded.
- Desktop, RTX 3080 (draw1.log): `contended_exchange_loops_match_direct3d`, seeds 1 to 8, fxc and
  dxc builds: 16 of 16 equal to real Direct3D 11 (every word and texel; 21 to 1412 pixels ran per
  seed), every loop ended, no display driver reset. `compute_draw` (4 seeds, cs_atomics' exchange
  loops in compute) and `variant_draw` (typed buffers' narrow stores) pass unchanged.
- Desktop, RTX 3080 (draw2.log): `primitive_index_draws_as_direct3d_does`: fxc build 0 of 4096
  texels apart from Direct3D 11, dxc build 0 of 4096 from Direct3D 12, 32 primitives each.
  `draw`'s other tests, `geometry_draw` (52 and 36 comparisons, 0 failed) and `tessellation_draw`
  (132 comparisons with 14 known subnormal-factor cases, and 128, 0 failed) pass.

Run 30, 2026-11-18, counts only (laptop load 1 to 10, another track building; desktop shared
through the queue, whose hazard group held our jobs for up to an hour), logs in scratch
`graphics-shaders/run30`:
- Laptop: `cargo test -p wcontainers-shaders` all pass; corpus with `--include-ignored` (corpus,
  rig, rig DXIL, own, own DXIL, games): every verdict as recorded after recording rig's
  geometry.gs_layers and raster.gs, own gs_layers and gs_tess_*, and 3 game geometry shaders
  writing a layer, which now convert (their WGSL validates; not yet compiled in Chrome).
- Desktop, RTX 3080 (draw1.log, draw3.log): `stream_output_after_the_domain_stage_matches_direct3d`
  at seeds 1 and 2 (`WCONTAINERS_TESSELLATION_CASES=2`): 66 cases (tri, quad and isoline domains,
  several partitionings and outputs, each streamed directly and through gs_tess_*, and 3-point
  patches into a 1-point hull shader), 62 equal to real Direct3D 11 (each patch's primitives and the
  written and made counts), 4 known (subnormal factors, Known gaps), 0 failed.
  `layered_geometry_draws_as_direct3d_11_does`: 4 layers, 0 bytes apart. `geometry_draw` (4 seeds)
  and the other tessellation tests (2 seeds) still pass after the geometry program's new binding
  and the tessellator's index header.

Run 29, 2026-11-18, counts only (laptop load about 0.2 to 5.6; desktop shared through the queue,
whose hazard slot other tracks' long runs held for most of the run), logs in scratch
`graphics-shaders/run29`:
- Laptop: the full suite of both crates with the corpora (`--include-ignored --skip mutated
  --no-fail-fast`, suite2.log): everything passes except the own-shader records, which lacked the
  four new files of each route; the records were copied from `classes.txt` and the corpus test then
  passed 9 of 9 (corpus3.log). `ps_spaces.dxbc` is now `ok`; no game, rig or Wine corpus verdict
  changed with shader model 5.1 numbering. Variant tests 25 of 25, the interface's 36 of 36, clippy
  of both crates clean (suite2.log, variants4.log).
- Recompiling every own shader on the desktop (`compile.cmd`) gave the committed bytes; new are
  `vs_texels`, `vs_early`, `ps_early` and `ps_mark` (fxc and dxc).
- Desktop (one GPU job each, RTX 3080): `early_draw` 32 seeded scenes (early3.log): in 21 Direct3D
  ran the shader for 44 to 2017 of 4096 pixels, in 11 for none; in every scene the marks and the
  target's texels of the fxc and the dxc build through WebGPU equal real Direct3D 11's (0 apart).
  The first 16 scenes of the first version of the test (small triangles, 11 of them empty) were also
  all equal (gpu3.log). `compute_draw` 5 of 5 (gpu3.log), among them
  `storage_views_at_any_first_element_match_direct3d` (8 seeds, every view of `cs_atomics`,
  `cs_shared` and `cs_decrement` at a seeded first element, 0 words apart by both routes).
- WebAssembly (laptop, wasmtime; wasm1.log): the corpus test passes 9 of 9 under wasmtime, and the
  written files equal the native ones except the known gap (`116_cs_group_index_code` WGSL,
  `d599fbc7efa911be` WGSL and GLSL, and the two `cache.txt` holding them).
- Not rerun this run: Chrome's compile of the written files.
- At the end of run 29 the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders` (7.3 GB)
  and the desktop's `C:\wc\target\phase-2-graphics-shaders` were deleted.

Run 28, 2026-11-18, counts only (laptop load about 1 to 5; desktop shared through the queue), logs in
scratch `graphics-shaders/run28`:
- Desktop (one GPU job each): `draw` 4 of 4 (gpu6.log): `ps_wave` through WebGPU on wgpu's Vulkan 0 of
  4096 texels apart from real Direct3D 12 (2074 texels drawn), `ps_gatheroff`'s dxc build 0 of 4096
  apart from real Direct3D 12 (draw1.log, gpu4.log), the other cases as before; `compute_draw` 4 of 4,
  24 comparisons 0 failed (gpu4.log); the Windows clippy of the crate clean (gpu4.log, gpu6.log).
- Laptop: clippy of both crates clean and the full suite of both crates with the corpora
  (`--include-ignored --skip mutated`; suite7.log): the records of own-dxil and games-dxil were
  copied from `classes.txt` after the deliberate changes (ps_gatheroff.dxil and ps_wave.dxil
  convert; Godot: the fragment shader with wave operations and two compute shaders convert, two
  compute shaders move from the reader to `ResourceArray`).
- Recompiling every own shader on the desktop (`compile.cmd`) gave the committed bytes; only
  `ps_wave.dxil` is new.
- Not rerun this run: the WebAssembly comparison and Chrome's compile of the written files.
- At the end of run 28 the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders` (5.7 GB)
  and the desktop's `C:\wc\target\phase-2-graphics-shaders` were deleted.

Run 27, 2026-11-18, counts only (laptop load about 1; desktop shared through the queue), logs in
scratch `graphics-shaders/run27`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; the full suite of both crates with the corpora (`--include-ignored --skip mutated`): every
  test passes but the own-shaders record of `ps_gatheroff.dxil`, which the reader change made
  `spirv-reader unsupported` and which was then recorded (suite3.log; the record copied from
  `classes.txt`).
- Desktop (one GPU job each): `tessellation_draw` 3 of 3 on wgpu's Direct3D 12, 512 comparisons
  (tess1.log); `geometry_draw` 2 of 2, 52 and 36 comparisons (geom1.log); `draw` 3 of 3 with the
  new cases (draw3.log), and the Windows clippy of the crate clean (draw4.log, which also holds the
  DXIL gather failure that led to the reader change).
- Not rerun this run: the WebAssembly comparison and Chrome's compile of the written files.
- At the end of run 27 the laptop's `~/wcontainers-graphics/target/phase-2-graphics-shaders` and the
  desktop's `C:\wc\target\phase-2-graphics-shaders` and `C:\wc\probe` were deleted.

Run 26, 2026-11-18, counts only (desktop shared through the queue; the laptop unreachable), logs in
scratch `graphics-shaders/run26`:
- Desktop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu --no-fail-fast` (every test not
  ignored, GPU ones included, default seeds) passes (gpu4.log): `compute_draw` 24 comparisons 0
  failed (`cs_atomics` with its exchange loops, both builds, 0 words apart at seeds 1 to 4),
  `variant_draw` 48 comparisons 0 failed (`swizzled` both builds and back ends 0 of 4096 texels),
  the multisampled array 0 texels apart on both builds, filtering 396 comparisons with the 32 known
  anisotropic failures. `cargo check -p wcontainers-direct3d-11 -p wcontainers-direct3d-12
  --all-targets` builds (fronts.log). Not rerun: the corpus, games and WebAssembly runs and Chrome's
  compile (they need the laptop).

Run 25, 2026-11-18, counts only (laptop load 1 to 4; desktop shared through the queue), logs in
scratch `graphics-shaders/run25`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu -- --include-ignored --skip mutated`
  with the corpus, the games (Godot's added) and glslang: every test passes (corpus test 9 of 9,
  208 s; full.log).
- Desktop: `cargo clippy -p wcontainers-shaders --all-targets -- -D warnings` clean, and `cargo
  check -p wcontainers-direct3d-11 -p wcontainers-direct3d-12 --all-targets` builds (fronts.log).
  `compute_draw` 4 of 4, `draw` 3 of 3, `variant_draw` 3 of 3 at seeds 1 to 4 (gpu2.log, after the
  raw buffer change): every comparison within its allowance, as in run 24; DXIL builds new to the
  drawn tests: `cs_texels` 0 elements apart in 4 seeds, `ps_table` 0 of 4096 texels on both back ends.
- Chrome 157.0.8103.78 on the RTX 3080 (chrome.log): every WGSL file compiles (corpus 181, games
  516, games DXIL 406 with Godot's 58, own 89, own DXIL 33, rig 76, rig DXIL 53); the GLSL ES
  failures are the known classes only (bitfield operations, `textureGather`, multisampled samplers:
  corpus 13, games 10, games DXIL 1, own 5, own DXIL 2, rig 2, rig DXIL 2).
- WebAssembly (laptop, `wasm-run10.sh`, wasm.log): the corpus test under wasmtime 9 of 9 (331 s),
  every verdict as native; written files equal to the native ones except corpus 2 and games 3, the
  counts of the known gap of run 24 (files not listed again by name).
- At the end of run 25 the build folders were deleted (laptop
  `~/wcontainers-graphics/target/phase-2-graphics-shaders`, 7.1 GB; desktop
  `C:\wc\target\phase-2-graphics-shaders`, 6.1 GB); `out` and the logs are kept.

Run 24, 2026-11-18, counts only (laptop load 2 to 9; desktop shared through the queue), logs in
scratch `graphics-shaders/run24`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu -- --include-ignored --skip mutated`
  with the corpus, the games and glslang: every test passes (corpus test 9 of 9, 232 s; full.log).
- Desktop: `cargo clippy -p wcontainers-shaders --all-targets -- -D warnings` clean;
  `compute_draw` 4 of 4 at seeds 1 to 4 (draw3.log): atomics and counters 20 comparisons (fxc and
  dxc builds of `cs_atomics`, `cs_shared`, `cs_decrement`; `cs_atomics.dxil` the known refusal),
  0 words apart; storage textures 4 seeds both builds 0 apart; typed buffers 4 seeds 0 elements
  apart; storage textures through buffers, both builds, 4 seeds, 0 texels apart in every view
  (R32Uint with atomics, R32Float read and written, the seeded array and volume formats).
  `variant_draw` 3 of 3 (draw2.log): the layer and viewport test 0 texels apart in all four layers;
  the run's other variant comparisons 48, 0 failed. `cargo check -p wcontainers-direct3d-11 -p
  wcontainers-direct3d-12 --all-targets` builds (fronts.log).
- WebAssembly (laptop, `wasm-run10.sh`, wasm.log): the corpus test under wasmtime 9 of 9 (330 s),
  every verdict as native; written files equal to the native ones except the known gap's two
  shaders (Known gaps: `116_cs_group_index_code` WGSL, `d599fbc7efa911be` WGSL and GLSL, and their
  lines in each `cache.txt`).
- Chrome 157.0.8103.78 on the RTX 3080 (chrome.log): every WGSL file compiles (corpus 181, games
  516, games DXIL 348, own 89, own DXIL 30, rig 76, rig DXIL 53); the GLSL ES failures are the known
  classes only (bitfield operations, `textureGather`, multisampled samplers: corpus 13, games 10,
  games DXIL 1, own 5, own DXIL 2, rig 2, rig DXIL 2).

Run 23, 2026-11-18, counts only (laptop load 3 to 5; desktop shared through the queue), logs in
scratch `graphics-shaders/run23`:
- Laptop: `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -- --include-ignored --skip mutated` with the corpus,
  the games and glslang: every test passes (corpus test 9 of 9, 278 s), verdicts as recorded
  (test3.log).
- Desktop: `cargo clippy -p wcontainers-shaders --all-targets -- -D warnings` clean;
  `tessellation_draw` 3 of 3 (tess6.log; the tessellator at the default 4 seeds, 128
  comparisons), `geometry_draw` 2 of 2 at 4 seeds (geom2.log); `cargo check -p
  wcontainers-direct3d-11 -p wcontainers-direct3d-12 --all-targets` builds (fronts.log).
- Chrome 157 on the RTX 3080 (chrome.log): every WGSL file compiles (corpus 177, games 469, games
  DXIL 346, own 87, own DXIL 28, rig 75, rig DXIL 52), the 10 newly written programs as GLSL ES
  too; the GLSL ES failures are the known classes only (bitfield operations, `textureGather`,
  multisampled samplers: corpus 13, games 10, games DXIL 1, own 5, own DXIL 2, rig 2, rig DXIL 2).

Run 22, 2026-11-18, counts only (laptop load 1 to 3; desktop shared through the queue), logs in
scratch `graphics-shaders/run22`:
- Laptop, `cargo clippy -p wcontainers-shaders -p wcontainers-gpu --all-targets -- -D warnings`
  clean; `cargo test -p wcontainers-shaders -p wcontainers-gpu -- --include-ignored --skip mutated`
  with every corpus: all pass once the records took the new verdicts (games-dxil: 10 from
  `unsupported RegisterSpace` to `ok` 9 and `ok-wgsl glsl-write` 1; own-dxil: `ps_spaces.dxil` ok,
  `ps_twospaces.dxil` ok; own: `ps_twospaces.dxbc` ok).
- Chrome 157 on the RTX 3080 (`web/check`, one GPU job): WebGPU compiles every written WGSL file
  (corpus 171, games 469, games-dxil 346, own 86, own-dxil 28, rig 74, rig-dxil 51); WebGL 2
  fails only on bitfield operations, `textureGather` and multisampled samplers as before (corpus
  13, games 10, games-dxil 1, own 5, own-dxil 2, rig 2, rig-dxil 2).
- Desktop, `draw`, `compute_draw` and `variant_draw` (one GPU job, all pass): `ps_twospaces` 0 of
  4096 texels differ on WebGPU and WebGL 2, DXIL against real Direct3D 12 and DXBC against real
  Direct3D 11; every earlier case as before.

Run 21, 2026-11-17, counts only (desktop shared with the other tracks through the queue; laptop
load 1 to 4), logs in scratch `graphics-shaders/run21`:
- `storage_textures_match_direct3d`, seeds 1 to 4 (draw2.log): fxc's DXBC and dxc's DXIL of
  `cs_images` through the WebGPU back end, 32 textures, every one equal word for word to real
  Direct3D 11's on the RTX 3080.
- `typed_buffers_draw_as_direct3d_does`, seeds 1 to 8 (draw2.log): 48 comparisons (DXBC and DXIL,
  each through storage on WebGPU and a texture on WebGPU and WebGL 2), all equal texel for texel.
- Corpora (laptop, corpus3.log, suite1.log): games DXIL 361: 288 ok, 47 ok as WGSL only
  (compute), 26 named refusals; own DXIL 44, rig DXIL 72 (2 more convert than run 20). The whole
  suite passed on the laptop (corpus 9 tests, variants 17, cache 6, the rest).
- Chrome 157 (chrome1.log, one GPU job): every written WGSL compiles (games DXIL 336, own DXIL 26,
  rig DXIL 51); GLSL ES fails only where it did (multisampled samplers 2, `textureGather` 3).

Run 20, 2026-11-17, counts only (desktop shared with the other tracks; laptop load 2 to 6):
- `tessellator_matches_direct3d` (`run20/tess5.log` in scratch `graphics-shaders`), seeds 1 to 16,
  32 topologies each: 512 draws of 8 patches, every patch's primitives equal the RTX 3080's
  (Vulkan through wgpu for ours), about 12 s of GPU job.
- The laptop suite (`run20/suite4.log`): 57 tests pass, none fail (the fuzz test skipped as in How
  to run); the corpora's records changed only for the hull and domain shaders (9 corpus, 9 rig, 35
  own) and `vs_patch` (`VertexWithoutPosition`).
- Chrome 157 on the desktop (`run20/chrome2.log`): every new WGSL file compiles with WebGPU: corpus
  9, own 35, rig 10, and `tessellator.wgsl` and `stream_out.wgsl` as written. A first attempt found
  no WebGPU adapter in Chrome; the retry a few minutes later did.
Run 19, 2026-11-17, counts only (the desktop shared with the other tracks, no timings taken):
- `storage_clears_match_direct3d` (`run19-clear4.log` in scratch `graphics-shaders`), seeds 1 to
  256: every case passes; all 40 storage formats on both (WebGPU on wgpu's Direct3D 12 has
  `STORAGE_WRITE` for each, the device a typed storage view of each); 62 float clears of integer
  formats refused `FloatClearOfInteger`, each left unchanged by Direct3D 11 too; 74 cases where the
  GPU's Direct3D 11 differs from WARP (integer clears saturated or read as floats, float clears
  rounded down at half a code), ours equal to WARP's integer clears and to the GPU's float clears.
  The first runs (`run19-clear1.log` to `-3.log`) found the denormal flush (R32_FLOAT, 1e-40 cleared
  as 0 by both) and the two references' disagreement.
- The corpora with view formats (laptop, `run19-corpus1.log`, `-2.log`, records updated): of the
  shaders refused `StorageFormat` or `TypedBuffer` before, games 51 of 60 now convert (9 refused
  `StorageFormatKind` 1 and `TypedBufferFormatKind` 8: they declare views of more than one kind,
  which one format for every slot cannot give; a front gives each slot its view's format), the
  Wine corpus 19 of 21 (2 refused `StorageImageAtomics`: atomics on a storage texture without
  `texels_through_buffer`), the rig 1 of 2, own shaders 0 of 4 (all four mix kinds). Compute shaders
  among them are `ok-wgsl glsl-write` (WebGL 2 has no compute), so their class does not show the
  views; `verdicts.txt` does.
- Chrome on the desktop (`run19-chrome.log`, one GPU job, RTX 3080): every written WGSL file
  compiles with WebGPU: corpus 162, games 469, games DXIL 289, own 50, own DXIL 26, rig 65, rig
  DXIL 49. WebGL 2 refuses 35 GLSL files (corpus 13, games 10, games DXIL 1, own 5, own DXIL 2,
  rig 2, rig DXIL 2), each for `bitfieldExtract`/`bitfieldInsert`/`bitfieldReverse`/`findMSB`,
  `textureGather` or `sampler2DMS`, which GLSL ES 3.00 lacks (6.5 gives the first two to the WebGL
  2 back end's writing); `web/check` exits 1 on them as before.
- The whole suite on the laptop before the push (`run19-suite2.log`, then the corpus test again
  after the records): every test passes, the corpora with glslang included; clippy with warnings
  denied passes on the laptop and on Windows.

Run 18, 2026-11-17, counts only (the desktop shared with the other tracks, no timings taken):
- `typed_buffers_match_direct3d`: seeds 1 to 64 (`texels-draw-2.txt`, `-3.txt`, `-4.txt` in scratch
  `graphics-shaders`), every result word and every written byte equal to real Direct3D 11 on the
  RTX 3080, every u1 atomic word equal; every loadable format at t0 to t3 and 20 stored formats at
  u0 drawn (half, 11- and 10-bit float stores exact, NaN and infinities included). Normalised stores:
  the RTX 3080 gives the code below where ours (the specification's formula) gives the one above, at
  fractions from 0.514 to 0.548 of a code (6 components in seed 4, 13 in seed 7, 0 to 9 elsewhere);
  the specification permits 0.6, so the test holds both to it (Departures). The first run found that
  past the length Direct3D gives the defaults where the format lacks components (fixed).
- `storage_textures_through_buffers_match_direct3d`: seeds 1 to 16 (`through-2.txt`), every texel
  of all four views equal (formats drawn: R16, R32, RG16, RG32, RGBA16 and RGBA32 floats; R8, RG8,
  RGBA8, RG16, RGBA16, R32 and RG32 unsigned).
- The whole drawn suite after it (`run18-gpu-2.txt`): `compute_draw` 4 tests (4 seeds), `draw` 3,
  `filter_draw` (2 seeds, 108 comparisons, 0 failed), `geometry_draw` 2 (4 seeds), `pull_draw`,
  `variant_draw` 2 (8 typed buffer seeds) pass. The laptop suite with every corpus
  (`run18-tests-2.txt`) passes.
- `typed_buffers_draw_as_direct3d_does`: seeds 1 to 40 (`texels-pixel-1.txt`, `run18-gpu-1.txt`),
  120 comparisons (storage on WebGPU, texture on WebGPU and WebGL 2), every texel equal.
- After the harness change (`run18-gpu-1.txt`): `draw` 3 tests, `compute_draw` 3, `geometry_draw` 2
  (4 seeds), `variant_draw` 2 pass.

Run 17, 2026-11-17, counts only (the desktop shared with the other tracks, no timings taken):
- `tests/geometry_draw.rs` (scratch `run17/gd3.log`), seeds 1 to 8: the geometry pictures 104
  comparisons, 0 texels differ; stream output 64 comparisons, every byte of both buffers and both
  statistics equal to real Direct3D 11's. Before `mad` became `fma` (`run17/gd2.log`): 8 of 32
  stream output comparisons had one or two words 1 ulp apart (all `gs_cut`, the lerp).
- After the `fma` change (`run17/draws1.log`): `draw` 3 tests pass; `variant_draw` passes;
  `filter_draw` seeds 1 to 8, 396 comparisons, 0 failed, 32 anisotropic known failures as before.
- Chrome's compile of every written file after run 17's changes (`run17/chrome.log`, one GPU job):
  every WGSL file compiles (corpus 143, games 418, games DXIL 289, rig 64, rig DXIL 49, own 50, own
  DXIL 26); WebGL 2 refuses only what GLSL ES 3.00 lacks (bitfield operations, `textureGather`,
  multisampled samplers): corpus 13, games 10, games DXIL 1, rig 2, rig DXIL 2, own 5, own DXIL 2.
  Geometry, atomics and counters convert to WGSL only (`ok-wgsl glsl-write`: compute).
- Corpus records after run 17 (laptop, `run17/linux-all3.log`): 56 storage textures now refused
  `StorageFormat` (no format under the default key) where they were `StorageImage`; 10 geometry
  shaders and 13 shaders with atomics or counters now convert; 2 refused `AtomicSignedness`; 3 game
  geometry shaders now refused `RenderTargetArrayIndex`.
- WebAssembly against native after run 17 (laptop, `run17/wasm.log`, scratch script
  `wasm-run10.sh`): the corpus test passes under wasmtime (9 tests); written files equal byte for
  byte in corpus, rig, rig DXIL, own, own DXIL and games DXIL; in games 3 differ, all from one shader
  (`d599fbc7efa911be.dxbc`: its WGSL, its GLSL and its line in `cache.txt`, Known gaps).
- The whole suite before the push (`run17/final-gpu.log`, one GPU job, about 100 s): `draw`,
  `variant_draw`, `filter_draw` (396 comparisons, 32 anisotropic known failures), `pull_draw`,
  `geometry_draw` (52 and 32 comparisons) and `compute_draw` (8 and 4 seeds) all pass; on the laptop
  every test passes with the corpora and glslang (`run17/linux-all3.log`, then the corpus test
  again after the records).
- `tests/compute_draw.rs` (`run17/cd2.log`), seeds 1 to 4: atomics and counters 8 comparisons, 0
  words apart, counters equal (256, 256, 0; 0); storage textures 16 comparisons (4 formats), 0 words
  apart.

Run 16, 2026-11-17, counts only:
- `tests/geometry_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 2 s,
  scratch `run16/geometry7.log`): seeds 1 to 4, thirteen cases each, WebGPU (wgpu's Direct3D 12
  path) against real Direct3D 11: 52 comparisons, 0 of 4096 texels differ in every one (allowance
  0); Direct3D draws 59 to 1468 texels per picture. Before the test gave the vertex stage the draw's
  bases (`run16/geometry5.log`), the cases with a first vertex, base vertex or first instance
  differed, as Direct3D's vertex and instance numbers leave the bases out.
- Laptop (i7-8750H, load average 0 to 0.6): the suite without the ignored corpora passes after
  recording the new own and rig verdicts; clippy with warnings denied passes. Windows clippy with
  warnings denied passes.

Run 15, 2026-11-17, counts only:
- `tests/filter_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 92 s,
  scratch `run15/desktop-final.log`), seeds 1 to 8, each by the DXBC and the DXIL route on WebGPU
  (wgpu's Direct3D 12 path) and WebGL 2 (ANGLE), R32_FLOAT and RGBA8, six sampling modes: 396
  comparisons, 0 failed, 32 known failures (all of seed 8, the one anisotropic seed: gather and
  explicit levels pass there). Of the 336 isotropic pictures: no texel equals neither Direct3D; in
  24 pictures 48 texels equalled neither until the level of detail moved by 1/256; in 8 pictures one
  texel each (one place, seed 3's gradients) is a third answer where the GPU and WARP differ. The
  GPU and WARP themselves differ in 68 of the 84 isotropic pictures, by up to 3758 texels (largest
  0.67). Against WebGPU's own sampler (12 RGBA8 comparisons): 0 texels apart from both it and WARP;
  the own sampler against the GPU's Direct3D 11: 0 texels apart in all 12 (allowance 1/128 per
  channel throughout). Seeds 1 to 16 (`run15/filter12.log`): seeds 1 to 13 pass; seed 14
  (anisotropy 4) lost ANGLE's device (DXGI_ERROR_DEVICE_REMOVED, Known gaps).
- Before the fixes (scratch `run15/filter3.log` to `filter11.log`): mip-point selection at an exact
  .5 rounded down (the GPU rounds up); DXIL on WebGL 2 read level count 0 (the helper packing bug);
  R32_FLOAT borders gave all four border components (Direct3D gives (r, 0, 0, 1)); a quad whose two
  triangles were not one plane put a seam of differing derivatives across the picture (the test's
  own geometry, fixed). Rounding the level of detail to 8 bits, or point coordinates to 8 subtexel
  bits, made more texels differ, so neither is done.
- `tests/variant_draw.rs` and `tests/draw.rs` on the desktop after the helper packing change: both
  pass (`run15/desktop-final.log`). Windows clippy with warnings denied passes.
- Chrome 157 (`web/check`, one GPU job, about 3 s, `run15/chrome.log`): the filter variants of
  `ps_filter` by both routes (by texture with a bias; by sampler with a resource clamp and
  `TEXTURE_INFO`) and of the rig's 2D array, 3D and gather shaders, written by a test kept out of
  the repository: WGSL 7 of 7 compile in Chrome's WebGPU, GLSL ES 2 of 2 in its WebGL 2.
- Laptop (i7-8750H, load average 0.1 to 0.7): the whole suite with every corpus (`--include-ignored
  --skip mutated`, the plan's corpus and the real programs' shaders) passes after recording the new
  own verdicts (`vs_filter` converts; `ps_filter`'s plain GLSL ES has `textureGather`) and four rig
  shaders that now compile as GLSL ES (the rig track recompiled its shaders at lower shader models,
  so fxc no longer writes bitfield operations in them); every other verdict and cache record as
  recorded (corpus test 179 s; `run15/full-suite2.log`). Clippy with warnings denied passes. Not
  re-run: the WebAssembly comparison (plain conversions did not change).

Run 14, 2026-11-17, counts only:
- `tests/pull_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 2.5 s,
  scratch `run14/pull5.log`): seeds 1 to 50, by the DXBC and the DXIL route on WebGPU (wgpu's
  Direct3D 12 path): 0 of 4096 texels differ from real Direct3D 11 in all 100 comparisons
  (allowance 0, the texels are the inputs' bits). Before the exact conversions (seed 1,
  `run14/pull3.log`): 635 of 4096 differed, normalised values by one step (Direct3D's are correctly
  rounded) and half-float NaNs (payload lost). Windows clippy with warnings denied passes.
- Laptop (i7-8750H, load average 0 to 0.4): the suite without the ignored corpora passes after
  recording the new own verdicts (`vs_pull`, `ps_pull`: convert, by both routes); clippy with warnings
  denied passes.
- `tests/variant_draw.rs` on the desktop (one GPU job, about 3.7 s, `run14/draw1.log`): 104
  comparisons, all within their allowances; new: constants through a texture in both stages
  (`vs_transform` with `ps_textured`, and with `ps_rows`, whose row is picked by the pixel's column)
  against the plain programs' uniform blocks, on WebGL 2 and WebGPU, by both routes: 0 of 4096 texels
  differ in all 8 (allowance 0). Every earlier comparison as in run 13. Windows clippy with warnings
  denied passes.
- `tests/draw.rs` on the changed shared harness: all 3 tests pass (`run14/draw-plain.log`).
- Laptop (i7-8750H, load average 0 to 0.4), end of run: the whole suite with every corpus
  (`--include-ignored --skip mutated`, the plan's corpus and the real programs' shaders) passes, every
  verdict and cache record as recorded (corpus test 175 s; `run14/full-suite.log`); clippy with
  warnings denied passes. Not re-run: Chrome's compile and the WebAssembly comparison (the plain
  conversions did not change; only the new variants' paths did).

Run 13, 2026-11-17, counts only:
- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, about 2.3 s,
  scratch `run13/draw8.log`; 96 comparisons, all within their allowances), each by the DXBC and the
  DXIL route: clip distances in the program (`vs_clip`, `ps_textured`) against real Direct3D 11's
  native clipping: WebGPU (wgpu's Direct3D 12 path, which has no clip distances) and WebGL 2, 11 of
  4096 texels differ, by at most 2 (allowance 2 in any number: the same pixels are drawn, an attribute
  over the unclipped triangle rounds apart; a pixel drawn on one side only would differ by far more);
  four samples on WebGPU against the native distances on wgpu's Vulkan path, resolved: 12 differ by
  at most 2. Depth clamping (`vs_deep`, depth range 0.25 to 0.75) against Direct3D 11 with
  DepthClipEnable false, depth as f32 steps: WebGPU 648 of 4096 within 2 steps, WebGL 2 570 within 2
  (allowance 2); clipping changes 791 of Direct3D's texels. Mirror-once in u and repeat in v on a
  4-level texture (`ps_mirror`) against Direct3D 11's MIRROR_ONCE: WebGPU and WebGL 2, 5 of 4096
  differ by 1 (allowance 1: explicit gradients' level of detail rounds apart); the mode changes 1693
  of Direct3D's texels against clamping. Every earlier comparison of the file as before.
- During the run the desktop's shared ANGLE (`C:\wc\tools\angle-mozangle-0.7.1`, rebuilt by another
  track at 16:30) crashed in libGLESv2.dll at offset 0x1f83f5 on making a context, as did a copy
  built here from the same recipe (`C:\wc\tools\angle-shaders-track`), the WebGL back end's own
  suite too, before and after today's merge of that back end. The new comparisons were first drawn
  on WebGPU alone with the WebGL draws taken out of an uncommitted copy of the test
  (`run13/draw7.log`, the same numbers as above); after the next rebuild of the shared ANGLE (16:59)
  the whole test ran and passed as committed (`run13/draw8.log`).
- Laptop (i7-8750H, load average 0 to 1): the suite without the ignored corpora passes after
  recording the new own verdicts (`vs_deep`, `ps_mirror`: convert), clippy with warnings denied
  passes; Windows clippy with warnings denied passes. At the end of the run the whole suite with
  every corpus (`--include-ignored --skip mutated`, the plan's corpus and the real programs'
  shaders) passes, every verdict and cache record as recorded (corpus test 179 s;
  `run13/full-suite.log`). Not re-run: Chrome's compile and the WebAssembly comparison (the
  plain conversions did not change).

Run 12, 2026-11-17, counts only:
- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, scratch
  `run12/draw2.log`), each by the DXBC and the DXIL route, against Direct3D 11's resource clamp:
  `ps_minlod` with clamps 0, 1.5, 3 and 3.5 on WebGPU and WebGL 2, and `ps_mingather` with clamps
  0.5, 1 and 3.5 on WebGPU: 0 of 4096 texels differ in every case (allowance 1). Each clamp past the
  first changes Direct3D's picture (1.5: 2162 texels, 3: 2530, 3.5: 2442; gather 1 and 3.5: 2071).
  Every earlier comparison of the file as in run 11. Windows clippy with warnings denied passes.
- Clip distances natively (`run12/draw4.log`): `vs_clip` with `ps_textured` on wgpu's Vulkan path,
  0 of 4096 texels differ from Direct3D 11 by both routes (allowance 0); the planes change 1071 of
  Direct3D's texels. Every other comparison of the file as before (68 exact comparisons in the log).
- Chrome 157 (`web/check`, one GPU job, about 3 s, own corpora only, `run12/chrome2.log`): WGSL own
  33 of 33, own DXIL 20 of 20 (`vs_clip` with `enable clip_distances` included); GLSL ES own 26 of
  30, own DXIL 17 of 18: `vs_clip` compiles after the naga change (before it, both builds failed on
  the extension's name), `ps_minlod` compiles, the failures are bitfield operations and
  `textureGather` (`ps_mingather`). The other corpora were not re-run: their verdicts did not
  change.
- Laptop (i7-8750H, load average 0.4 to 3): the suite with every corpus (`--skip mutated`) passes
  after recording the new own verdicts (`vs_clip` converts; glslang 8.13 knows neither clip
  distance extension, so its GLSL ES verdict is a glslang failure), every other verdict unchanged,
  so no corpus or real program shader used clip distances. Earlier in the run, after recording the four
  new own verdicts (`ps_minlod` converts and compiles as GLSL ES; `ps_mingather`'s GLSL ES has
  `textureGather`, which ES 3.00 lacks); every other verdict unchanged. Clippy with warnings denied
  passes.

Run 11, 2026-11-17, counts only:
- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, scratch
  `run11/draw4.log`), each by the DXBC and the DXIL route: depth bias against Direct3D 11's
  D24_UNORM_S8_UINT buffer, in 24-bit codes, on WebGPU and WebGL 2: (2000 units, slope 0.25, no
  clamp) 355 of 4096 texels differ by 1 code, (clamp 0.0002) 471 by 1, (-2000, -0.25, clamp -0.0002)
  478 by 1, never more than 1 (allowance 1); the clamp changes 1610 of Direct3D's texels. Draw bases
  against Direct3D 11, 0 of 4096 differ in every case: vertices from 48 and indexed (first index 6,
  base vertex 48), two instances from 1 or 2, directly on WebGPU and WebGL 2 and indirectly (the
  arguments 4 words into their buffer) on WebGPU. Every run 10 comparison as in run 10. Windows clippy
  with warnings denied passes. `tests/draw.rs` on the changed shared harness: all 3 tests pass
  (`run11/draw-plain.log`).
- Laptop (i7-8750H, load average 0.5 to 4.2): the whole suite with every corpus passes (`--skip
  mutated`; corpus test 214 s), every verdict as recorded; own DXBC and DXIL each two more
  (`vs_ids`, `ps_ids`, converting). Clippy with warnings denied passes.
- Not re-run this run: Chrome's compile of the written files and the WebAssembly comparison (the
  default conversions did not change; only the new variants' paths did).

- `tests/variant_draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, scratch
  `run10/draw8.log`), each by the DXBC and the DXIL route: equal texel for texel (or, for depth, in
  f32 steps) to the stated reference: viewport adjustment 0 of 4096 on WebGPU and WebGL 2; clip
  depth on WebGL 2 with and without EXT_clip_control and WebGPU natively, 0 steps from Direct3D 11's
  depth buffer; swapped depth range on WebGPU 843 of 4096 within 3 steps of the definition (allowance
  4); sample mask 0 on WebGPU; constants as storage 0; level-of-detail bias 2.5 on a 4-level texture 0
  on WebGPU and WebGL 2 against Direct3D 11; counting 1610 of 1610 samples Direct3D 11's query
  counts, on both back ends; level counts 0 on WebGPU and WebGL 2 (WebGPU's own query 0 against
  Direct3D 11); sample count (DXBC) 0 on both; dual-source blending 0 on WebGPU against Direct3D 11.
  `tests/draw.rs` after its move onto the shared harness: all 3 tests pass, every comparison equal as
  in run 9. Windows clippy with warnings denied passes.
- Laptop (i7-8750H, load average 0.3 to 4): the whole suite with every corpus passes (`--skip
  mutated`), clippy with warnings denied passes. Verdicts: corpus 127 of 191 convert (unchanged), rig
  56 of 79, rig DXIL 49 of 72 (unchanged), own 28 of 30 (`ps_samples` needs `samples`, `ps_spaces`),
  own DXIL 15 of 20; the real programs' shaders: DXBC 418 of 529 convert (named: render target array
  index 47, storage images 39, typed buffers 21, geometry shaders 3, multisampled arrays 1), DXIL 289
  of 361 (reader: storage images without a format 38, typed buffers 19, a load operand 1, viewport
  index from a vertex stage 1; register spaces 10, packed stage values 2, multisampled arrays 1).
  Cache records written and read back equal: corpus 254, rig 112, rig DXIL 98, own 56, own DXIL 30,
  games 836, games DXIL 578.
- Chrome 157 (`web/check`, one GPU job, about 9 s, `run10/chrome.log`): WGSL compiles for every
  converted shader: corpus 127, rig 56, rig DXIL 49, own 28, own DXIL 15, games 418, games DXIL 289.
  GLSL ES: corpus 89 of 102, rig 47 of 53, rig DXIL 46 of 48, own 22 of 25, own DXIL 13 of 13, games
  406 of 416, games DXIL 288 of 289; every failure a bitfield operation, `textureGather` or
  `sampler2DMS`.
- WebAssembly (`wasm-run10.sh`, laptop): `wasm_imports` (release) 5,112,617 bytes; the corpus test
  under wasmtime passes (all 9), every written file equals the native build's except one real
  program shader, `d599fbc7efa911be.dxbc`: its udiv helper function computes the quotient and the
  remainder from the same loads in the other order (the same values; WGSL, GLSL and the cache record
  differ). Known gaps.

Run 9, 2026-11-17, counts only:
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job, `run9/draw4.log`, and
  again after the last change to the file, `run9/draw5.log`, all 3 tests passing): DXBC route equal
  to Direct3D 11 texel for texel on WebGPU 22 of 22 and WebGL 2 17 of 17 drawn (`ps_size` included);
  DXIL route equal to Direct3D 12 drawing the same DXIL on WebGPU 12 of 12 and WebGL 2 11 of 11; the
  12 Direct3D 12 pictures equal Direct3D 11's of the fxc builds. Not drawn on WebGL 2: bitfield
  operations 3, `noperspective` 1, fine derivatives 2. Windows clippy with warnings denied passes.
  The rig's triangle from DXIL and DXBC equal to Direct3D 12's picture on both back ends. (Drawn
  before the reader's id-order change, which only orders the expression arena.)
- Laptop (i7-8750H, load average 0.6 to 3.8): the suite with the corpus passes, every verdict as
  recorded (own DXBC and DXIL each one more: `ps_negate`, converting); `tests/cache.rs` 6 of 6 with
  the default 10 seeded cuts (seed 1). Records written: own 52, own DXIL 26, rig 112, rig DXIL 98 and
  the corpus's, each the same bytes after decoding and after a second conversion.
- WebAssembly (`wasm-run5.sh`): the corpus test under wasmtime passes and every written file,
  `cache.txt` included, equals the native build's (corpus, rig, rig DXIL, own, own DXIL: 0 differ),
  after the reader's id-order fix (before it, the DXIL `cache.txt` files differed).
- Chrome 157 (`web/check`, one GPU job, about 4 s, `run9/chrome.log`): WGSL corpus 127 of 127, rig
  DXBC 56 of 56, rig DXIL 49 of 49, own 26 of 26, own DXIL 13 of 13; GLSL ES corpus 89 of 102, rig
  DXBC 47 of 53, rig DXIL 46 of 48, own 21 of 24, own DXIL 12 of 12. Every GLSL ES failure is a
  bitfield operation, `textureGather` or `sampler2DMS`.

Run 8, 2026-11-17, counts only:
- Laptop (i7-8750H, load average 0.2 to 4.2), the suite with the corpus: corpus 127 convert and 64
  named (unchanged), of which 87 compile as GLSL ES in glslang, 15 do not and 25 are not written (one
  more unwritten: `106_ps_code_ddy_fine`, fine derivatives); rig DXBC 56 of 79 and rig DXIL 49 of 72
  (unchanged); own DXBC 25 of 26, own DXIL 12 of 16 (`ps_table` `NonFiniteConstant`, `ps_spaces`
  `RegisterSpace`, `vs_pack`/`ps_pack` `PackedStageValues`).
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job): equal to Direct3D 11
  texel for texel on WebGPU in 21 of 21 by the DXBC route and 11 of 11 by the DXIL route, on WebGL 2
  in 15 of 16 drawn by the DXBC route (`ps_size` the known failure, 2591 of 4096 texels, largest 111)
  and 10 of 10 by the DXIL route; not drawn on WebGL 2: bitfield operations 3, `noperspective` 1,
  fine derivatives 2 (`ps_deriv`, both builds). New and equal everywhere they are drawn: `vs_carry`
  (both builds, the check's loop case), `ps_ftoi` (both builds; NaN, both infinities and 2^31 from
  the constant buffer, finite values past the limits), `ps_table` (fxc's table holding NaN and
  infinities), `ps_deriv` (WebGPU), `ps_coarse`. The rig's triangle from DXIL and DXBC equal to
  Direct3D 12's picture on both. Log in scratch `graphics-shaders/run8/draw1.log`.
- Chrome 157 (`web/check`, one GPU job, about 2 s): WGSL corpus 127 of 127, rig DXBC 56 of 56, rig
  DXIL 49 of 49, own 25 of 25, own DXIL 12 of 12; GLSL ES corpus 89 of 102, rig DXBC 47 of 53, rig
  DXIL 46 of 48, own 20 of 23, own DXIL 11 of 11. Every GLSL ES failure is a bitfield operation,
  `textureGather` or `sampler2DMS`. Verdicts in scratch `graphics-shaders/run8/out`.
- WebAssembly (`wasm-run5.sh`, laptop): `wasm_imports` (release) 4,959,946 bytes, the same WASI
  imports; the corpus test under wasmtime 38.0.4 passes, and every written file equals the native
  build's (corpus, rig, rig DXIL, own, own DXIL: 0 differ), `converter-version.txt` included: both
  builds give `0.1.0+7e003573...` for the same sources (they differed before by the SDK's path).
- Clippy with warnings denied passes on the laptop for the crate; the carried naga's changed files
  give no new clippy warnings with the GLSL, HLSL, WGSL writers and SPIR-V reader built.
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000` under `ulimit -v 4000000` (new own
  files, so new cases; laptop load below 2): 7 cases ended the process, all DXIL in dxil-spirv (4
  segmentation faults: 1604, 1843, 2040, 2597; 3 `std::terminate`: 240, 309, 2637); no DXBC case.
  Our own checks refused 11: `malformed` 5 (`unknown stage` 3, `no instruction 0` 2), `invalid` 6
  (a constant index past an array's end 3, a compute workgroup size out of range 2, `VertexIndex` in
  a pixel shader 1). Log `~/wcontainers-graphics/fuzz-run8-seed7.log`, refusals `fuzz-ours-7.txt`.
  The check of run 7 (seed 4242, 8000 cases) found 39 DXIL cases that ended the process, none DXBC.

Run 7, 2026-11-17, counts only:
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings; one GPU job): WebGPU equal to Direct3D
  11 texel for texel in 16 of 16 by the DXBC route and 7 of 7 by the DXIL route; WebGL 2 in 11 of 12
  drawn by the DXBC route (`ps_size` the known failure, 2591 of 4096 texels, largest 111) and 7 of 7
  by the DXIL route; 4 not drawn on WebGL 2 for the reasons in the cases (bitfield operations,
  `noperspective`). The rig's triangle from DXIL and DXBC equal to Direct3D 12's picture on both.
  Logs in scratch `graphics-shaders/run7/draw*.log`.
- Chrome 157 (`web/check`, one GPU job, about 3 s; the first attempt failed with "no WebGPU
  adapter", the retry a minute later worked): WGSL corpus 127 of 127, rig DXBC 56 of 56, rig DXIL 49
  of 49, own 20 of 20, own DXIL 8 of 8; GLSL ES corpus 90 of 103, rig DXBC 47 of 53, rig DXIL 46 of
  48, own 16 of 19, own DXIL 8 of 8. Every GLSL ES failure is a bitfield operation, `textureGather`
  or `sampler2DMS`. Verdicts in scratch `graphics-shaders/run7/out`.
- Laptop (i7-8750H, load average 0.4 to 4.5), the suite with the corpus: corpus, rig DXBC and rig DXIL
  verdicts unchanged from run 5; own DXBC 20 of 21, own DXIL 8 of 11. Clippy with warnings denied
  passes on the laptop and on Windows (draw test included).
- WebAssembly (`wasm-run5.sh`): `wasm_imports` (release) 4,930,913 bytes, the same 10 WASI imports;
  the corpus test under wasmtime 38.0.4 passes and every written file equals the native build's
  (corpus, rig, rig DXIL, own, own DXIL: 0 differ).
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000` (new own files, so new cases; laptop
  load 3 to 4.5): 9 cases ended the process, all DXIL in dxil-spirv (7 segmentation faults: cases
  517, 553, 1357, 1564, 1705, 2234, 2737; 2 `std::terminate`: 726, 1648); no DXBC case. Our own
  checks refused 17: `malformed` 10 (`no instruction 0` 5, `unknown stage` 4, `opcode 416 result`
  1), `invalid` 7 (a constant index past an array's end 6, an image operation on a signed integer
  texture 1). Log `~/wcontainers-graphics/fuzz-run7-seed7.log`, refusals `fuzz-ours-7.txt`.

Run 6, 2026-11-17, counts only (laptop i7-8750H, load average 1 to 4):
- The whole suite with the corpus (`--include-ignored --skip mutated`), after the run's fixes and
  before the new own shaders: every test passes and every verdict equals its record, unchanged from
  run 5 (corpus 127 convert and 64 named, rig DXBC 56 of 79, rig DXIL 49 of 72; none of the rig's DXIL
  discards, so the demote fix changes no rig verdict).
- WebAssembly (`wasm-run5.sh`): `wasm_imports` (release) is 4,930,913 bytes (naga's compaction is
  linked now), the same 10 WASI imports; the corpus test under wasmtime 38.0.4 passes and every file
  it writes equals the native build's (corpus, rig, rig DXIL, own, own DXIL: 0 differ).
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000`, the same inputs as run 5 (the new own
  shaders not yet in): 11 cases ended the process, the 10 DXIL cases of run 5 exactly (8 segmentation
  faults, 2 `std::terminate`) and DXBC 934 killed for memory; run 5's DXBC memory cases 1539 and
  2228 ended as `rejected` this time (how much memory the laptop had free differs between runs). Our
  own checks refused the same 13 cases as in run 5. Log `~/wcontainers-graphics/fuzz-run6-seed7.log`.
- The check of run 5 (seed 31337, 6000 cases, before run 6): 19 ended the process (9 segmentation
  faults and 5 `std::terminate`, all DXIL; 5 killed for memory, 4 DXBC and 1 DXIL), 26 refused by our
  own checks.

Run 5, 2026-11-17, counts only:
- Laptop (i7-8750H, load average 1 to 5), the whole suite with the corpus (`--include-ignored --skip
  mutated`): every test passes; every verdict of corpus, rig DXBC and rig DXIL equals its record,
  unchanged from run 4 (corpus 127 convert and 64 named, rig DXBC 56 of 79, rig DXIL 49 of 72). Own
  DXBC 18 of 19 (`ps_spaces` `RegisterSpace`; `ps_cases` converts now that loops are left from switch
  cases), own DXIL 5 of 8 (`ps_spaces` `RegisterSpace`, `vs_pack` and `ps_pack`
  `PackedStageValues`).
- Chrome 157 on the RTX 3080 (`web/check`, one GPU job, about 3 s): WGSL corpus 127 of 127, rig DXBC
  56 of 56, rig DXIL 49 of 49, own 18 of 18, own DXIL 5 of 5; GLSL ES corpus 90 of 103, rig DXBC 47
  of 53, rig DXIL 46 of 48, own 14 of 17, own DXIL 5 of 5. Every GLSL ES failure is a bitfield
  operation (`findMSB`, `bitCount` and `findLSB` among them), `textureGather` or `sampler2DMS`.
  Verdicts in scratch `graphics-shaders/run5/out`.
- `tests/draw.rs` on the desktop (RTX 3080, shared, no timings): every own pair equal to Direct3D 11
  texel for texel by the DXBC route on WebGPU (14 of 14) and WebGL 2 (9 drawn, `ps_depth` the known
  back-end failure, 2591 of 4096 texels, largest 77, as in run 4), and by the DXIL route
  (`ps_signed`, `ps_offset`, `ps_special`, `ps_cases`) on WebGPU (4 of 4) and WebGL 2 (3 drawn;
  `ps_cases` is not drawn there because fxc's build extracts bits). The rig's triangle from DXIL and
  DXBC equal to Direct3D 12's picture on both. Log in scratch `graphics-shaders/run5-draw.log`.
- WebAssembly (laptop, `run5/wasm.sh`): `wasm_imports` (release) is 4,853,449 bytes, the same 10 WASI
  imports. The corpus test under wasmtime 38.0.4 passes, and every file it writes equals the native
  build's (corpus, rig, rig DXIL, own, own DXIL).
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000` (scratch `fuzz/resume.sh`, laptop load
  below 5). The inputs now include the 13 new own files, so the cases are not those of run 4. 13
  cases ended the process: 10 DXIL in dxil-spirv (8 segmentation faults, 2 `std::terminate`: cases
  434, 743, 1457, 1904, 1938, 1964, 2158, 2600, 2786, 2997) and 3 DXBC killed for memory (934,
  1539, 2228). 13 cases were refused by our own checks: `malformed` 7 (`no instruction 0` 4, `unknown
  stage` 3) and `invalid` 6 (4 constant indices past an array's end, 1 duplicate switch case value, 1 `tanh` of a
  type it does not take;
  scratch `fuzz/run5-seed7-ours.txt`). The independent check's run with seed 20261117 and 12000
  cases (before run 5) ended the process in 55 (0.46%) and refused 38 by our own checks.

Run 4, 2026-11-17, on the Linux laptop (i7-8750H, load average 1 to 2), counts only:
- Corpus test (all verdicts as recorded in `tests/expected`): corpus 127 convert (88 GLSL ES compile
  in glslang, 15 do not, 24 not written: 3 fewer unwritten than run 3, from the level-zero fold), 64
  named failures with the same counts by reason as below; rig DXBC 56 of 79 (unchanged); rig DXIL 49
  of 72 (the rig's DXIL grew from 27; the 23 others: execution models of geometry and tessellation 15,
  typed buffers 2, a storage image without a format 1, an unsupported built-in 1, the sample mask as
  an array 1, `PrimitiveId` 1, `VertexWithoutPosition` 1, `Texture1DArray` 1; run 4's record of
  this line said 17 execution models, which was wrong); own DXBC 12 of 13
  (`ps_spaces` is `RegisterSpace`), own DXIL `ps_spaces` `RegisterSpace`.
- Fuzz, `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000`, inputs the rig's, our own and the
  corpus, every mutation re-signed so it reaches the converters, restarted after each crash
  (scratch `graphics-shaders/fuzz/resume.sh`): 16 of the 3000 cases ended the process, 15 of them
  DXIL (10 segmentation faults and 5 `std::terminate` in dxil-spirv: cases 751, 943, 1113, 1632,
  1651, 1656, 1732, 1846, 1937, 1955, 2443, 2693, 2774, 2775, 2810) and one DXBC case killed for
  memory (1996, `024_ps_ld_sint8_code_1.dxbc`). No assertion abort, foreign exception or Rust panic
  ended it (run 3's check found 43 foreign exceptions, 25 assertions and 4 panics in 5,950 DXBC
  cases, and 292 assertions in 2,000 DXIL cases, all unsigned mutations, which the checksum now
  refuses). Replay one case: `WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_FIRST=751
  WCONTAINERS_FUZZ_CASES=752`.
- Chrome 157 on the RTX 3080 (`web/check`, one GPU job, about 3 s), the files of the run above:
  WGSL corpus 127 of 127, rig DXBC 56 of 56, rig DXIL 49 of 49, own 12 of 12; GLSL ES corpus 90 of
  103, rig DXBC 47 of 53, rig DXIL 46 of 48, own 9 of 11. Every GLSL ES failure is a bitfield
  operation, `textureGather` or `sampler2DMS`. Verdicts in scratch `graphics-shaders/run4/out`.
- `tests/draw.rs` on the desktop (RTX 3080; the desktop is shared, no timings): every own pair equal
  to Direct3D 11 texel for texel on WebGPU (9 of 9) and on WebGL 2 (6 drawn: `ps_flat`, `ps_local`,
  `ps_size` among them; `ps_depth` differs in 2591 of 4096 texels, largest 77, the back end's depth,
  recorded as a known failure of that back end); the rig's triangle from DXIL and DXBC equal to
  Direct3D 12's picture on both. Real Direct3D 11 (`CreatePixelShader`) and 12
  (`CreateGraphicsPipelineState`) answer a container with one flipped byte with `E_INVALIDARG`
  (0x80070057), and `convert` with `Rejected`.
- WebAssembly: `wasm_imports` (release) is 4,837,106 bytes, the same 10 WASI imports. The corpus test
  built for `wasm32-wasip1` under wasmtime 38.0.4 passes (all records met) and writes every file equal
  to the native build's; its verdicts differ only where glslang, which it cannot run, would refuse.

Run 3, 2026-11-17, after all of run 3's changes:
- Laptop corpus test (i7-8750H, load average below 6): corpus 127 convert (85 GLSL ES compile in
  glslang, 15 do not, 27 not written), 64 named failures, the same counts by reason as below; rig
  DXBC 56 of 79 (unchanged); rig DXIL 24 of 27 (the rig now has 27 DXIL shaders; the 3 others are
  the reader's gaps: typed buffers twice, a storage image without a format).
- Chrome 157 on the RTX 3080 (one GPU job, about 2 s): WGSL corpus 127 of 127, rig DXBC 56 of 56,
  rig DXIL 24 of 24, own 6 of 6; GLSL ES corpus 87 of 100, rig DXBC 47 of 53, rig DXIL 23 of 24
  (`textureGather`), own 4 of 6. Verdicts in scratch `graphics-shaders/run3/out-final/*/chrome.txt`.
- `tests/draw.rs` on the desktop (RTX 3080, driver 32.0.16.2101; ANGLE from mozangle 0.7.1): every
  comparison exact, as listed above. The desktop is shared; no timings taken.
- WebAssembly: `wasm_imports` (release) is 4,853,482 bytes, the same 10 WASI imports.

Chrome, run 2, 2026-11-17: Chrome 157.0.8103.78 (the rig's pinned build), headless, on the desktop
(RTX 3080, WebGPU on D3D12, WebGL 2 on ANGLE D3D11), every adapter feature requested and every WebGL
extension enabled. Verdicts per file in `chrome.txt` beside the written files. After run 2's fixes:
- corpus: WGSL 127 of 127 compile; GLSL ES 87 of 100 (13 fail: bitfield operations 8, gather 4,
  `sampler2DMS` 1). Before the fixes WGSL was 125 (the NaN constant and the f32::MAX literal) and
  GLSL 86 (the sample mask).
- rig DXBC: WGSL 56 of 56; GLSL ES 47 of 53 (bitfield insert 3 and extract 1, gather 1,
  `sampler2DMS` 1).
- rig DXIL: WGSL 11 of 11, GLSL ES 11 of 11 (before run 2's DXIL entry point wrapper; run 3's re-run
  after it gave 24 of 24 and 23 of 24, above). Own: WGSL 5 of 5, GLSL ES 3 of 5 (bitfield operations).
- Each run of the 196 files' compile took about 2 s of GPU job time.

All on the Linux laptop (i7-8750H, Ubuntu 20.04), load average below 4 during the runs, 2026-11-17.
Counts only, no timings.

Direct route (this crate) on the plan's 191-shader DXBC corpus (`WCONTAINERS_SHADER_CORPUS`):
- 127 convert and their WGSL validates (naga, WebGPU core capabilities).
- Of those, 100 are written as GLSL ES 3.00 by naga and 85 compile in glslangValidator 8.13. The 15
  that do not: bitfield extract, insert, reverse and find-MSB (8), `textureGather` (4),
  `gl_SampleID` and `gl_SampleMask` (2, which need OES_sample_variables; run 2 made naga enable it,
  and Chrome compiles them, above), and one more. The 27
  naga does not write, by the features it names: texture level queries 14, storage buffers 7, compute
  3, sample counts 2, cube arrays 1 (all absent on WebGL 2, or back-end fixes of 6.5).
- 64 fail with a named reason: StorageImage 15, Atomics 9, GeometryShader 7, VertexWithoutPosition 6,
  TypedBuffer 6, HullShader 5, DomainShader 4, SampleCount 4, StorageCounter 2, DynamicOffset 2,
  RenderTargetArrayIndex 2, StencilRef 1, ViewportArrayIndex 1.
- The SPIR-V route at the same dxbc-spirv commit (6.3, measured in G0): naga read 123, wrote 112 as
  WGSL (111 compiled in Chrome), 95 as GLSL ES (80 compiled in Chrome).
- Decision: the direct route, as the plan chose. It converts 127 to valid WGSL against 112 written,
  and 100 to GLSL ES against 95, with every failure named, and none of the reader's gaps (compute
  shaders, typed buffers' and storage images' formats read wrongly, built-ins WGSL cannot express).

The rig's 79 compiled DXBC shaders (`crates/wcontainers-graphics-test/shaders/dxbc`): 56 convert to
valid WGSL (45 also compile as GLSL ES in glslang); 23 named: GeometryShader 6, HullShader 6,
DomainShader 3, StorageCounter 3, StorageImage 2, Atomics 1, PrimitiveId 1, VertexWithoutPosition 1.

WebAssembly: the conversion alone (`examples/wasm_imports.rs`, release, `wasm32-wasip1`, both
converters linked) was 4,842,413 bytes in run 2 (4,853,482 in run 3) (2,356,683 with dxbc-spirv alone) and imports from `wasi_snapshot_preview1`: environ_get, environ_sizes_get, fd_close,
fd_fdstat_get, fd_prestat_get, fd_prestat_dir_name, fd_read, fd_seek, fd_write, proc_exit (10;
the plan expected proc_exit and fd_write only: libc++'s streams, which dxbc-spirv's logger uses, pull
in wasi-libc's stdio and start-up). The corpus test built for `wasm32-wasip1` and run under wasmtime
38.0.4 gives the same verdicts and the same written files as the native build, for the corpus and
the rig's DXBC and DXIL.

## naga as carried

`third_party/naga` is naga 30.0.2 exactly as published on crates.io (commit "carried naga 30.0.2 in
the tree as published"), plus these changes, each a later commit touching only `third_party/naga`:
- GLSL writer, `BuiltIn::VertexIndex` without `WriterFlags::DRAW_PARAMETERS` is written
  `(uint(gl_VertexID) + naga_vs_base_vertex)`, with `uniform uint naga_vs_base_vertex;` declared in
  vertex shaders that read it (`back::glsl::BASE_VERTEX_BINDING`, feature `VERTEX_INDEX`), as naga
  already does for `instance_index` with `naga_vs_first_instance`. The back end sets it to each indexed
  draw's base vertex (0 otherwise); GL leaves an unset uniform 0. With `DRAW_PARAMETERS` nothing
  changes (desktop GL's `gl_VertexID` already includes the base vertex).
- GLSL writer, `BuiltIn::SampleIndex` is written `uint(gl_SampleID)` (it is a `u32` in naga and an
  `int` in GLSL; 6.3's corpus had one shader failing on it).
- GLSL writer, `BuiltIn::SampleMask`: written `gl_SampleMask[0] = int(...)` and read
  `uint(gl_SampleMaskIn[0])` (GLSL's are `int` arrays), requesting GL_OES_sample_variables on ES.
- WGSL writer, a structure's member bindings are written only where the structure is an entry
  point's argument or result (run 38): Tint checks them wherever written, and DXIL's reader keeps
  its own output structure (the sample mask as a one-element array) as an inner value after the
  entry point's result is rebuilt, which Chrome refused (`@builtin(sample_mask)` must be `u32`).
- WGSL writer, an f32 literal whose shortest digits name a number past f32::MAX (f32::MAX itself) is
  written with the f64 digits of its exact value, which WGSL compilers accept.
- HLSL writer (wgpu's Direct3D 12 path), a fragment entry point's position is passed on as
  `float4(p.xyz, 1.0 / p.w)`: the IR's fragment position has 1/w in w (SPIR-V's and GLSL's rule,
  which dxbc-spirv inverts back to Direct3D's w), HLSL's `SV_Position` has w.
- WGSL writer, the module's diagnostic filters are written as `diagnostic(...)` directives
  (`Severity::to_wgsl_ident` added beside `from_wgsl_ident`).
- SPIR-V reader (run 5): integer instructions read in the signedness they name, the result given its
  type's (`convert::op_signedness`, `glsl_signedness`, `Frontend::retype_int`); atomic minimum and
  maximum of the other signedness refused (`Error::UnsupportedSignedness`), subgroup ones (run 40)
  read their value in the signedness they name and give the result its type's; texel offsets
  on `OpImageFetch` added to the coordinate; unknown load operands refused
  (`Error::UnsupportedImageLoadOperand`) instead of skipped with a log warning.
- WGSL writer (run 5), `isnan`/`isinf` of f32 scalars and vectors written as bit tests (WGSL has
  neither).
- SPIR-V reader (run 6), `OpDemoteToHelperInvocation` read as `Statement::Kill` (WGSL's `discard`
  demotes too), its extension and capability accepted.
- GLSL and HLSL writers (run 8), a loop's `continuing` block reads the values its body made in that
  iteration from variables declared before the loop (`compact::values_for_continuing`), instead of
  writing their expressions again where the body's variables have changed.
- MSL writer (run 9), the same `continuing` fix as the GLSL and HLSL writers (run 8).
- SPIR-V reader (run 9), `OpSNegate` of an unsigned operand negated as signed; global variables'
  and constants' expressions made in id order (the same module on 32-bit and 64-bit targets).
- SPIR-V reader (run 10), an `OpUndef` in a function ends the running Emit range before its zero value
  (a zero value is never emitted); a texel coordinate's vector type of another scalar kind is made
  where the reader needs it, instead of panicking that it should exist.
- GLSL writer (run 12), WebGL programs with clip or cull distances enable
  `GL_ANGLE_clip_cull_distance` (WEBGL_clip_cull_distance's name) instead of
  `GL_EXT_clip_cull_distance`.
- GLSL writer (run 8), fine derivatives need `Features::DERIVATIVE_CONTROL` (GLSL 4.50, not ES)
  instead of being written as the plain functions.
- SPIR-V reader (run 25), an image taken from an array of images (`Access`/`AccessIndex` on a
  binding array) has the array's element type. This one is in commit 182398b1 together with the
  crate's `arrays.rs`, not a commit of its own.
- SPIR-V reader (run 27), `OpIsHelperInvocationEXT` reads a private bool named
  `front::spv::HELPER_INVOCATION`, made at its first use; the module's user sets it (this crate's
  `dxil.rs` from the coverage and at each demotion). Before, the reader refused the instruction.
- SPIR-V reader (run 27), an image sample operand it does not know is refused
  (`unsupported image sample operand`) instead of skipped with a warning.
- SPIR-V reader (run 28), a gather's run-time `Offset` moves the coordinate by the offset in texels of
  level 0 (low six bits, signed); availability and visibility operands at workgroup scope or less
  taken on storage and texels (WGSL's memory model gives every access that), wider ones refused
  (`Error::UnsupportedMemoryScope`); `OpGroupNonUniformBallotBitCount` read as a subgroup add of the
  ballot's predicate (or the ballot's bits counted, reduce only); `OpGroupNonUniformAllEqual` read as
  an all of each lane's value equal to the first lane's; ballot find LSB and MSB from the words' trailing and leading zeros.
- WGSL writer and reader (run 28), `enable subgroups;` written for modules with subgroup operations or
  built-ins (`Module::uses_subgroups`) and taken by the reader (`ImplementedEnableExtension::Subgroups`).
Not yet sent upstream: that needs a GitHub account and fork the owner chooses, which this track does
not have. Each change is its own commit touching only `third_party/naga`, so each can be sent as it
stands (`git diff d380816 HEAD -- third_party/naga`).
- SPIR-V reader (run 32), an image both compared against and sampled plainly becomes a depth image
  as one only compared against does (it was `InconsistentComparisonSampling`); a sampler used both
  ways is still refused. Its plain reads then give one value where the module expects four, so the
  caller widens them (`dxil/depth.rs`): not for upstream as it stands.

## How to run

- Run 88's scripts in scratch `graphics-shaders/g6-r88` are run 87's with its folder and the clone
  `tip` (updated with `git fetch <checkout> agents && git reset --hard FETCH_HEAD` inside it):
  `gpu.sh`, `tree-tar.sh`, `suite-laptop.sh` (copied to `~/wcontainers-graphics/suite-r88.sh`,
  writing `out-r88`). Builds, clippy and the CPU test binaries ran on the rented ARM64 machine
  instead of the x86 one: `arm-run <checkout> --name graphics-shaders -- '<cargo command>'`
  (orchestrator's `cloud/arm-run`; it copies the shared worktree as it is, other tracks' edits
  included). The literal check needs a git repository: in that copy, `git init` and commit the
  tree first, then read only this crate's lines (the copy also lists the converters' submodules'
  insides, which the real checkout does not).
- Run 79's scripts in scratch `graphics-shaders/g6b-r79` are run 78's with its folder and
  `out-r79`, plus `scenes.sh <n> [scenes]` (the Direct3D 12 rig scenes through the Direct3D 12
  front built from the clone `tip`), `gpu-noclippy.sh` (one GPU job of `geometry_draw` and
  `tessellation_draw` without clippy, for a mutant applied with `git -C tip apply
  ../mutant-<name>.diff`, then `git -C tip checkout -- crates/wcontainers-shaders`) and
  `x86-mutants2.sh` (each mutant alone against tests/geometry.rs on the x86 machine, from a bundle
  of the checkout's HEAD sent as `/work/probes/shaders-tip2.bundle`, the patches as
  `/work/probes/shaders-mutant-<name>.diff`).
- Run 78's scripts in scratch `graphics-shaders/g6a-r78` are run 77's with its folder, `out-r78`
  and `outw-r78`, the clone `tip` (`tree-tar.sh`, and `x86-checks2.sh` on a bundle of its branch
  `x86tip`); Chrome ran as `crates/wcontainers-shaders/web/check ../out` from that clone after
  copying the laptop's `out-r78` into `out`.
- Run 77's scripts in scratch `graphics-shaders/g6a-r77` are run 76's with its folder, `out-r77`
  and `outw-r77`; `tree-tar2.sh` tars the clone `tip2`, and `x86-checks2.sh` fetches the bundle's
  branch `x86tip` (a bundle made with `git bundle create <file> x86tip` in the clone). `snap.py`
  (run with `python3 -I`) diffs a gather case's WebGL 2 picture against Direct3D's and lists the
  texels whose position lies near a 1/256 step (gap A31).
- Run 76's scripts in scratch `graphics-shaders/g6a-r76` are run 75's (below) with its folder and
  `out-r76`, plus `wasm.sh` (copied to `~/wcontainers-graphics/wasm-r76.sh`, run after the
  laptop suite): it builds `wasm_imports` and the corpus test for `wasm32-wasip1` with wasi-sdk 25
  (`~/wcontainers-graphics/tools/wasi-sdk`), runs the corpus test under wasmtime
  (`~/wcontainers-graphics/tools/wasmtime`) on every corpus into `outw-r76`, and counts the files
  that differ from the native `out-r76`.
- Run 75's scripts in scratch `graphics-shaders/g6a-r75`, from its clone `tip` made as below:
  `gpu.sh <log> "<command>"` (as run 73's), `tree-tar.sh > tree.tar` (submodules taken from the
  agents' worktree, never from the owner's checkout), `suite-laptop.sh` (copied to
  `~/wcontainers-graphics/suite-r75.sh`, writing `out-r75`), `x86-checks.sh` (sent with `x86-run
  --ssh 'bash -s' < x86-checks.sh` after the bundle). The whole GPU suite is one job:
  `./gpu.sh gpu-suite.log "cargo test -j 3 -p wcontainers-shaders --no-fail-fast -- --test-threads 1"`
  (about 11 minutes; gap A29 for why one thread). Never run `tools/graphics-rig/queue-check`: it
  submits test jobs and sends a tree, which stops a waiting job.
- Run 74's scripts are run 73's `gpu.sh` and `gpu-noclippy.sh` in scratch `graphics-shaders/g6b7`,
  from its clone `tip` made as below. The WARP tests need nothing more: `common::webgpu_on_warp`
  asks wgpu for its software adapter on Direct3D 12, and the rig's `Driver::Warp` gives Direct3D
  11 on WARP. A mutant is tried by applying its patch (`git -C tip apply ../mutant-....patch`) and
  running `gpu-noclippy.sh`, then `git -C tip checkout` of the file. Since run 74, run `draw`,
  `geometry_draw` and `variant_draw` with `-- --test-threads 1` if they crash in ANGLE (run 74's
  section).
- Run 73's scripts (scratch `graphics-shaders/g6b6`), all from the scratch clone `tip` (made as
  below, on its branch `shaders-tip`, its remote removed; copy edits not yet committed into it to
  try them): `gpu.sh <log> "<command>"` is one GPU job of Windows clippy with warnings denied,
  every test binary built, then the command (`gpu-noclippy.sh` without the clippy step, for a
  mutated copy); `tree-tar.sh > tree.tar` writes the clone's working tree with the converters'
  submodules for the laptop, where `suite-laptop.sh` (copied to
  `~/wcontainers-graphics/suite-g6b6.sh`, writing `out-g6b6`) runs the suite as run 72's did;
  `x86-checks.sh` as below, from a bundle of the checkout's `HEAD`. Compiling the own shaders:
  `tools/graphics-rig/remote "crates\wcontainers-shaders\tests\own\compile.cmd"` from the
  clone, then copy the new files back from `C:\wc\src\shaders-tip\crates\wcontainers-shaders\tests\own`
  with `rig_scp` (tools/graphics-rig/common.sh).
- At the committed tip, not the shared tree (since run 70; other tracks' uncommitted edits are in
  the one checkout, this crate's among them). Scripts in scratch `graphics-shaders/g6a-r72`:
  `tip-tar.sh` writes HEAD with the converters' submodules as one tar; `suite-laptop.sh` (copied
  to `~/wcontainers-graphics/suite-r72.sh`) unpacks `~/wcontainers-graphics/shaders-tip.tar` into
  `src/shaders-tip`, lays `~/wcontainers-graphics/overlay.tar` over it if there is one (files not
  yet committed, for a trial), and runs the suite with every corpus and glslang, writing into
  `out-r72` (arguments go to `cargo test`, for example `--test corpus`): `./tip-tar.sh > tip.tar`,
  `scp tip.tar tom@192.168.2.240:wcontainers-graphics/shaders-tip.tar`, then `ssh
  tom@192.168.2.240 'cd ~/wcontainers-graphics && (nohup bash suite-r72.sh > logs/<log> 2>&1 <
  /dev/null &)'`. Never start a second run while one runs: both write the same build folder and
  log. `x86-checks.sh` (fmt, clippy of this crate and the interface with warnings denied, the
  literal check) runs on the x86 machine on a clone of a bundle of the tip sent to
  `/work/probes/shaders-tip.bundle` (made with `git bundle create <file> HEAD shaders-tip` in the
  scratch clone: without `HEAD` in it the clone checks out nothing) (it makes the clone `/work/shaders-tip` if missing and checks
  out with `-f`, as cargo rewrites the lock there). `gpu.sh` runs every test binary on the RTX 3080
  and a seed of `geometry_draw`, from a clone of the checkout in scratch `g6a-r72/tip` on its own
  branch `shaders-tip` with its remote removed (so the rig sends the whole commit as a bundle and
  builds in `C:\wc\src\shaders-tip`, apart from other tracks' `main` copy; move it with `git fetch
  <checkout> refs/heads/main && git checkout -B shaders-tip <commit>`; make it, as each run's
  clean-up deletes it, with `git clone -q --no-checkout <checkout> tip`, `git checkout -q -B
  shaders-tip <commit>` and `git remote remove origin`);
  `crates/wcontainers-shaders/web/check <dir>` run from that clone compiles the laptop's `out-r72`
  in Chrome.
- The literal check on the x86 machine (since run 66, as the desktop's copy must not change while a
  GPU job waits): send the tip as a bundle (`git bundle create <scratch>/tip.bundle HEAD`), then
  `x86-run --ssh 'cat > /work/shaders-lit.bundle && cd /work/shaders-lit && git fetch -q
  /work/shaders-lit.bundle HEAD && git checkout -q FETCH_HEAD && CARGO_TARGET_DIR=/work/target/shaders-lit
  nix develop /work/src/graphics-shaders -c cargo run -q -j 32 -p wcontainers-literals -- check' <
  <scratch>/tip.bundle`. A new machine needs the clone first (`git clone /work/shaders-lit.bundle
  /work/shaders-lit`, then `git submodule update --init` of both converters and
  `git -C crates/wcontainers-shaders/dxil-spirv submodule update --init third_party/spirv-headers`:
  the baseline holds their scripts' lines).
- The Direct3D 12 rig scenes through this branch's Direct3D 12 front: scratch
  `graphics-shaders/g6b2/scenes.sh <n> [scenes]` (log `scenes<n>.log`); `draw.sh <n>` runs
  `geometry_draw` and `tessellation_draw` in one GPU job.
- The literal check on the desktop (before run 66): `tools/graphics-rig/remote "cargo run -q -j 3 -p
  wcontainers-literals -- check > C:\wc\incoming\shaders-check.txt 2>&1"`, then copy that file
  back with scp (its output is long). It runs on the desktop because the laptop's git 2.25 lacks an
  `ls-files` option it uses. Never run `remote` while one of this branch's GPU jobs waits in the
  queue: it changes the desktop's tree and the waiting job then stops ("holds tree ..., not the
  sent ..."). Lines it prints with `-` are to be deleted from `literals.baseline`. The desktop's
  copy `C:\wc\src\phase-2-graphics-shaders` has `core.autocrlf` set to false (run 39): with
  Windows line ends the check reads multi-line strings differently and fails on other crates'.
- A GPU job running several test binaries needs `--no-fail-fast`, or the first failing binary
  ends the job before the others run.
- The rented x86 machine (the default for work without a GPU since 2026-11-19; run 54): from the
  checkout, `<orchestrator>/cloud/x86-run . --name graphics-shaders -- '<command>'` copies the
  working tree (submodules included) and runs the command in the checkout's `nix develop`, cargo
  building into `/work/target/graphics-shaders`. Clippy and the crate's tests run as on the
  laptop. The corpora go to `/work/probes/shaders-corpus/{dxbc,games,runner}`, and a new machine
  has none or only some of them (G6b's check found only `dxbc`; run 80 copied all three): copy them
  from the laptop each run with `ssh tom@192.168.2.240 'cd ~/wcontainers-graphics/corpus && tar
  --exclude="._*" -cf - dxbc games runner' > <scratch>/corpus.tar`, then `x86-run --ssh 'mkdir -p
  /work/probes/shaders-corpus && cd /work/probes/shaders-corpus && tar -xf -' <
  <scratch>/corpus.tar` (191, 973 and 3,020 files); a missing corpus fails its record test, it
  never skips. `C=/work/probes/shaders-corpus; export WCONTAINERS_SHADER_CORPUS=$C/dxbc
  WCONTAINERS_GAME_SHADERS=$C/games WCONTAINERS_RUNNER_SHADERS=$C/runner
  WCONTAINERS_SHADERS_OUT=$PWD/target/shaders-out; cargo test --no-fail-fast -p
  wcontainers-shaders -- --include-ignored --skip mutated` (about 3 minutes). It has no glslang,
  so GLSL ES outcomes are not compiled there; the laptop's suite does that. The literal check
  needs `.git`, which the copy leaves out: run it on the desktop.
- Linux laptop (no GPU), one command from the Mac: `tools/graphics-rig/linux <command>`. It copies the
  working tree (submodules included) to `~/wcontainers-graphics/src/<branch>` and builds into
  `~/wcontainers-graphics/target/<branch>`, with 5 cargo jobs (2 when the laptop's load is 8 or more).
- Chrome's compile of the written files: copy `~/wcontainers-graphics/out` from the laptop to a
  directory here (`rsync -a tom@192.168.2.240:wcontainers-graphics/out/ <dir>/`), then
  `crates/wcontainers-shaders/web/check <dir>`: one GPU job on the desktop, `chrome.txt` comes back
  into each corpus directory, exit 1 if any file fails.
- When the desktop refuses new SSH connections (reset after the key exchange; run 6 lost an hour to
  it, run 7 half an hour, it did so all through run 8, and again in run 9), already open shared connections keep working. Runs 7 and 9 pointed this
  checkout's rig socket (`~/.ssh/wc-rig-<hash of the git dir>`, see `tools/graphics-rig/common.sh`)
  at another checkout's live one with a symbolic link, and removed the link afterwards. The rig also
  notes the desktop's clock once went back 8 hours, so cargo may keep stale objects: run 7's first
  draw build ran `cargo clean -p wcontainers-shaders -p naga -p wcontainers-gpu
  -p wcontainers-gpu-webgpu -p wcontainers-gpu-webgl -p wcontainers-graphics-test` first.
- The laptop's disk (122 GB) filled during run 10 and a build failed with "No space left on device":
  run 10 deleted this branch's `debug/incremental` and three stale `wcontainers-shaders-*` build
  directories under `~/wcontainers-graphics/target/phase-2-graphics-shaders` (16 GB free after). The
  independent checks' directories `~/wcontainers-graphics/check-g6a*` hold about 29 GB and were left
  alone.
- Building on the desktop: the rig's `remote` does not fetch submodules, so a build command that
  needs this crate starts with `git submodule update --init crates/wcontainers-shaders/dxbc-spirv
  crates/wcontainers-shaders/dxil-spirv && git -C crates/wcontainers-shaders/dxil-spirv submodule
  update --init third_party/spirv-headers && cargo build ...` (they stay checked out afterwards).
- Drawing (Windows, needs the GPU and ANGLE at C:\wc\tools\angle-mozangle-0.7.1, which the WebGL
  track's `angle\build.cmd` makes; `WCONTAINERS_ANGLE` names another):
  `S="git submodule update --init -q crates/wcontainers-shaders/dxbc-spirv crates/wcontainers-shaders/dxil-spirv && git -C crates/wcontainers-shaders/dxil-spirv submodule update --init -q third_party/spirv-headers"; tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test draw --no-run" -- "cargo test -j 3 -p wcontainers-shaders --test draw -- --nocapture --test-threads 1"`.
  With `WCONTAINERS_SHADERS_OUT` set in the job the pictures are kept as raw texels under `draw`.
- Windows lint (the draw test is Windows only): `tools/graphics-rig/remote "$S && cargo clippy -j 3
  -p wcontainers-shaders --all-targets -- -D warnings"`.
- The rig's scenes through the direct3d-11 track's library: `tools/graphics-rig/scenes --build "$S &&
  cargo build -q -j 3 -p wcontainers-directx" --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll' --api d3d11`.
- Unit and corpus tests: `tools/graphics-rig/linux sh -c 'WCONTAINERS_SHADER_CORPUS=$HOME/wcontainers-graphics/corpus/dxbc
  WCONTAINERS_SHADERS_OUT=$HOME/wcontainers-graphics/out GLSLANG=glslangValidator cargo test -p
  wcontainers-shaders -- --include-ignored --skip mutated'`. Each corpus's verdicts must equal
  `tests/expected/<corpus>.txt`; the written WGSL and GLSL, glslang's output beside each GLSL file
  it refuses (`.log`) and `verdicts.txt` (the records' form since run 47, whole verdicts: copy it
  over the record after a deliberate change) land in `~/wcontainers-graphics/out/<corpus>`.
  Without glslang a record's compile outcome reads as `written`; a WebAssembly build checks only
  the part before ` ; glsl ` (it has no WebGL 2 back end to write with). The 191-shader corpus test is ignored without
  `--include-ignored` and fails without `WCONTAINERS_SHADER_CORPUS`; glslang runs only with both
  `GLSLANG` and `WCONTAINERS_SHADERS_OUT`.
- Geometry shaders through the stage helper (Windows, one GPU job, about 2 s for 4 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test geometry_draw --no-run" -- "set WCONTAINERS_GEOMETRY_SEED=1&& set WCONTAINERS_GEOMETRY_CASES=4&& cargo test -j 3 -p wcontainers-shaders --test geometry_draw -- --nocapture --test-threads 1"`.
  Each comparison prints the texels Direct3D drew and how many differ; with
  `WCONTAINERS_SHADERS_OUT` the pictures are kept. The same job runs the replay route on ANGLE
  (`geometry_shaders_replayed_on_webgl_draw_as_direct3d_does`, since run 52).
- Stream output: the same job runs `stream_output_matches_direct3d` (about 3 s for 8 seeds of both
  tests together) and, since run 49, `streams_past_the_first_match_direct3d` (four streams, about
  2 s for 8 seeds; a new Direct3D 11 device per comparison).
- The laptop's whole suite with every corpus and glslang: scratch `graphics-shaders/run49/suite.sh`
  copied to `~/wcontainers-graphics/suite-run49.sh`, run as `tools/graphics-rig/linux sh -c 'sh
  $HOME/wcontainers-graphics/suite-run49.sh'` (about 9 minutes from a clean target).
- Typed buffers: the `compute_draw` job below runs `typed_buffers_match_direct3d` too (about 1 s for 8
  seeds); `variant_draw` runs `typed_buffers_draw_as_direct3d_does` (`WCONTAINERS_TEXEL_SEED`,
  `WCONTAINERS_TEXEL_CASES`, default 8 seeds, about 1.5 s).
- Atomics, counters and storage textures (Windows, one GPU job, about 2 s for 4 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test compute_draw --no-run" -- "set WCONTAINERS_COMPUTE_SEED=1&& set WCONTAINERS_COMPUTE_CASES=4&& cargo test -j 3 -p wcontainers-shaders --test compute_draw -- --nocapture --test-threads 1"`.
- Compare-exchange loops under contention in a pixel shader (Windows, one GPU job, under 1 s for 8
  seeds): `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test contention_draw --no-run" -- "set WCONTAINERS_CONTENTION_SEED=1&& set WCONTAINERS_CONTENTION_CASES=8&& cargo test -j 3 -p wcontainers-shaders --test contention_draw -- --nocapture --test-threads 1"`.
- Early tests in the program against real Direct3D 11 (Windows, one GPU job, under 1 s; seeded scenes,
  default 32): `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test early_draw --no-run" -- "set WCONTAINERS_EARLY_SEED=1&& set WCONTAINERS_EARLY_CASES=32&& cargo test -j 3 -p wcontainers-shaders --test early_draw -- --nocapture --test-threads 1"`.
  Each scene prints how many pixels Direct3D ran the shader for and the marks and texels apart, for
  the fxc and the dxc build.
- The built-in clear against WARP and the GPU's Direct3D 11 (Windows, one GPU job, about 3 s for
  256 cases): `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test clear_draw --no-run" -- "set WCONTAINERS_CLEAR_SEED=1&& set WCONTAINERS_CLEAR_CASES=256&& cargo test -j 3 -p wcontainers-shaders --test clear_draw -- --nocapture --test-threads 1"`.
- The tessellator against the GPU's (Windows, one GPU job, about 12 s for 16 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test tessellation_draw --no-run" -- "set WCONTAINERS_TESSELLATION_SEED=1&& set WCONTAINERS_TESSELLATION_CASES=16&& cargo test -j 3 -p wcontainers-shaders --test tessellation_draw -- --nocapture --test-threads 1"`.
  The same job runs the tessellated patches and the rig's tessellation scene against real
  Direct3D 11 (about 1 s; with `WCONTAINERS_SHADERS_OUT` set the pictures are kept as raw RGBA
  under `draw`, 256 by 128).
- Compiling own shaders: `tests/own/compile.cmd` on the desktop (through `tools/graphics-rig/remote`);
  run 17 fetched the new files back as hex (`certutil -encodehex -f <file> <file>.hex 12`, then
  `type`) and decoded them here.
- Filtering in the program (Windows, one GPU job, about 90 s for 8 seeds):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test filter_draw --no-run" -- "set WCONTAINERS_FILTER_SEED=1&& set WCONTAINERS_FILTER_CASES=8&& cargo test -j 3 -p wcontainers-shaders --test filter_draw -- --nocapture --test-threads 1"`.
  Each comparison prints texels apart from both references, from each, and before moving the level
  of detail; with `WCONTAINERS_SHADERS_OUT` the pictures are kept.
- The stage helper's routes of a vertex shader alone, antialiased lines and primitive numbers
  (Windows, one GPU job, about 2 s for 4 seeds): `tools/graphics-rig/gpu-job --build "$S && cargo
  test -j 3 -p wcontainers-shaders --test stage_draw --no-run" -- "set WCONTAINERS_LINE_SEED=1&&
  set WCONTAINERS_LINE_CASES=4&& cargo test -j 3 -p wcontainers-shaders --test stage_draw --
  --nocapture --test-threads 1"`. `WCONTAINERS_LINE_COVERAGE=1` also prints the alpha the RTX
  3080, WARP and ours give twelve chosen lines.
- The laptop takes at most 4 build jobs per track and long jobs under `choom` (owner's rule of
  2026-11-18): `tools/graphics-rig/linux sh -c 'CARGO_BUILD_JOBS=4 choom -n 600 -- cargo test ...'`.
- The variant tests (Windows, one GPU job, about 1.5 s):
  `tools/graphics-rig/gpu-job --build "$S && cargo test -j 3 -p wcontainers-shaders --test variant_draw --no-run" -- "cargo test -j 3 -p wcontainers-shaders --test variant_draw -- --nocapture --test-threads 1"`.
  Each comparison prints its counts and allowance. The WebGL 2 back end leaves its GL context current
  when it is made, so a test makes a second one (without EXT_clip_control) only after the first one's
  last draw.
- The real programs' shaders: `tools/graphics-rig/remote "$S && cargo run -q -j 3 -p wcontainers-shaders --example containers -- C:\wc\shaders-games C:\wc\programs"`,
  then copy C:\wc\shaders-games to scratch `graphics-shaders/games` and the laptop's
  `~/wcontainers-graphics/corpus/games`; the corpus test reads them with
  `WCONTAINERS_GAME_SHADERS=$HOME/wcontainers-graphics/corpus/games` (and `--include-ignored`).
- The cache's seeded cuts: `tools/graphics-rig/linux sh -c 'WCONTAINERS_CACHE_SEED=7
  WCONTAINERS_CACHE_CUTS=1000 cargo test -p wcontainers-shaders --test cache -- --nocapture'` (each
  cut is printed with its seed; defaults seed 1, 10 cuts). `WCONTAINERS_CACHE_DAMAGES=n` sets how
  many damaged copies of each record `damaged_records_decode_to_a_conversion_or_an_error` and
  `damaged_records_decode_without_a_panic` (gap A16) decode (default 20; they print their counts
  with `--nocapture`).
- Fuzz (long, one command; not re-run in run 9): `tools/graphics-rig/linux sh -c 'WCONTAINERS_SHADER_CORPUS=$HOME/wcontainers-graphics/corpus/dxbc
  WCONTAINERS_FUZZ_SEED=7 WCONTAINERS_FUZZ_CASES=3000 cargo test -p wcontainers-shaders --test fuzz
  -- --ignored --nocapture'`. Each case is printed before it runs; `WCONTAINERS_FUZZ_FIRST=n` starts
  at case n. It ends at the first case that crashes (Known gaps); scratch
  `graphics-shaders/fuzz/resume.sh <seed> <cases>`, run on the laptop after one build, restarts past
  each crash and lists them; the cases our own checks refused go to
  `~/wcontainers-graphics/fuzz-ours-<seed>.txt`. On the x86 machine (since G6a run 3): scratch
  `graphics-shaders/g6a3/x86-fuzz.sh`, its `seed=SEED cases=CASES` line set, passed as the command
  of `x86-run . --name graphics-shaders-fuzz -- "$(cat ...)"`. The fuzz binary refuses any single
  Rust allocation over 1 GiB, so a size taken unchecked from the bytecode ends it on every machine.
- WebAssembly in one command: scratch `graphics-shaders/g6a4/wasm.sh` (copied to
  `~/wcontainers-graphics/wasm-g6a4.sh`; it mounts every corpus, the runner's included), run
  through `tools/graphics-rig/linux bash -c 'bash $HOME/wcontainers-graphics/wasm-g6a4.sh'` after a
  native suite run has filled `~/wcontainers-graphics/out`: builds `wasm_imports` and the corpus
  test, runs the test under wasmtime and lists, per corpus, the written files that differ from the
  native ones (`converter-version.txt` among them, so a differing converter version shows). The
  native GLSL files show as "Only in" the native directory: the WebAssembly build has no WebGL 2
  back end to write them; compare the files both write (about 8 minutes under wasmtime).
- WebAssembly: add `WASI_SDK_PATH=$HOME/wcontainers-graphics/tools/wasi-sdk` and `--target
  wasm32-wasip1`; run a test with `~/wcontainers-graphics/tools/wasmtime/wasmtime run --dir $S::$S
  --dir corpus/dxbc::/corpus --env WCONTAINERS_SHADER_CORPUS=/corpus <test>.wasm --include-ignored`,
  where `$S` is the source copy (`~/wcontainers-graphics/src/phase-2-graphics-shaders`), which the
  tests read at their build path.
- After cloning: `git submodule update --init crates/wcontainers-shaders/dxbc-spirv
  crates/wcontainers-shaders/dxil-spirv` and `git -C crates/wcontainers-shaders/dxil-spirv submodule
  update --init third_party/spirv-headers`. dxbc-spirv's own SPIRV-Headers and dxil-spirv's other
  submodules (SPIRV-Tools, SPIRV-Cross, its copy of dxbc-spirv) are not needed. The rig's `remote`
  sends a git bundle, which carries no submodules: building this crate on the desktop needs the
  submodules fetched there (not done yet; a matter for the rig's remote script).
- The corpus is in scratch (`wcontainers-scratch/graphics/corpus/dxbc`, from Wine's tests, LGPL, test
  data only) and was copied to `~/wcontainers-graphics/corpus/dxbc` on the laptop.

Installed on the laptop (Ubuntu 20.04) for this track: rust target `wasm32-wasip1` for 1.99.0 (rustup),
`glslang-tools` 8.13 (apt), wasi-sdk 25.0 and wasmtime 38.0.4 (release archives from their GitHub
releases, under `~/wcontainers-graphics/tools`). rustup, the 1.99.0 toolchain, gcc 9.4 and clang 10
were already there.

## Dependencies and licences

- G6a run 5: dev-dependency `md-5` 0.11 (MIT OR Apache-2.0, its Cargo.toml; already in the
  workspace's lock through wcontainers-direct3d-12), whose `block_api::compress` signs the fuzz
  test's mutated containers.
- Run 21: `spirv` 0.4.0 (Apache-2.0; SPIR-V's enums, already in the tree under naga), used to read
  and edit dxil-spirv's SPIR-V before naga reads it.
- Run 20: `src/stage/tessellator.wgsl` is a port (to WGSL) of Microsoft's reference tessellator,
  MIT ("Copyright (c) Microsoft Corporation", the licence text at the head of Mesa's
  src/gallium/auxiliary/tessellator/tessellator.cpp, read 2026-11-17); the program's header names it.
  No new crate.
- dxbc-spirv, https://github.com/doitsujin/dxbc-spirv, submodule at 6196cc6c62b7b04acb4f3e713ff3bb1a7e7950fd
  (upstream main's head on 2026-11-17): MIT, "Copyright (c) 2025 Philip Rebohle" (its LICENSE).
- dxil-spirv, https://github.com/HansKristian-Work/dxil-spirv, submodule at
  510991e707cda3aa389305448edad7c7dd76e3e7 (upstream master's head on 2026-11-17): MIT (LICENSE.MIT,
  "Copyright (c) 2019-2022 Hans-Kristian Arntzen for Valve Corporation", SPDX MIT). Of its submodules
  only third_party/spirv-headers (Khronos, MIT-style) is fetched; its glslang SPIR-V builder and
  bitcode reader are in its own tree (licences in the plan's section 3).
- naga 30.0.2 (carried, see above): MIT OR Apache-2.0; its `spv-in` reader is now used.
- `cc` 1.11 (build dependency, compiles the C++): MIT OR Apache-2.0 (its Cargo.toml); with `jobserver`
  0.1 (MIT OR Apache-2.0) for its parallel feature.
- `blake3` 1.8: as in the plan's section 3; also a build dependency now (hashing the C++ sources).
- Run 15: naga's `wgsl-in` feature (naga as above; already built into every program using this
  crate through wcontainers-gpu).
- Run 9, for the cache: `postcard` 1.1 (MIT OR Apache-2.0), `serde` 1.0 with `derive` (MIT OR
  Apache-2.0, also now a dependency of wcontainers-gpu, with `bitflags`' `serde` feature), `crc32fast`
  1.5 (MIT OR Apache-2.0), naga's own `serialize`/`deserialize` features; all already in the
  workspace's lock. Dev-dependency `fastrand` 2.5 (Apache-2.0 OR MIT) for the seeded cuts. Licences
  read from each crate's Cargo.toml.
- Dev-dependencies of `tests/draw.rs`, Windows only, already in the workspace's lock: `windows` 0.62
  (MIT OR Apache-2.0), `wgpu` 30 (MIT OR Apache-2.0), `pollster` 1 (Apache-2.0 OR MIT), and the
  workspace's own wcontainers-gpu-webgpu, wcontainers-gpu-webgl and wcontainers-graphics-test.
- Both C++ projects confirmed in run 3: the submodules' URLs are the upstream repositories, the
  pinned commits are on upstream's main (dxbc-spirv, still its head) and master (dxil-spirv, whose
  head has since moved to a781dce), and neither checkout has local changes.
- Test tools only, never shipped: glslang (BSD-3-Clause and others, Khronos), wasi-sdk (Apache-2.0
  WITH LLVM-exception; its libc++ and wasi-libc are linked into the WebAssembly build: libc++ Apache-2.0
  WITH LLVM-exception, wasi-libc MIT-style and others), wasmtime (Apache-2.0 WITH LLVM-exception).

## Departures from the plan

- Run 67: dxbc-spirv's `boundCheckShaderIo` is off (cpp/dxbc.cpp). Its check bounded every
  vertex-indexed input or output load by the input vertex count, so a hull shader's patch
  constant function read 0 from an output control point past the input count (the third check).
  Out-of-range indices of a stage's inputs are undefined in Direct3D; ours index arrays of the
  declared count, which WGSL bounds.
- Run 67: the own geometry test programs' dxc builds (`vs_geometry`, `ps_geometry`, `vs_cull`,
  `gs_*`) are made without `-Gis`, as programs are built: Direct3D 12 keeps an IEEE-strict build's
  multiply and add apart, which the common form cannot ask of a back end (A21). Every other own
  dxc build keeps `-Gis` for its NaN tests.
- Run 67: a DXIL hull's `SV_Position` control point data stays the location dxil-spirv gives it
  (run 66 made it the position built-in, which only a DXBC domain shader reading a DXIL hull's
  records would need; no front pairs the routes).

- Run 66: dxil-spirv's patch location offset is set to 0 (cpp/dxil.cpp), so a patch constant's
  location is its own register and the `Patch` decoration, not the location, tells it from control
  point data; dxil-spirv's default places patch constants past the control points' registers, which
  would not match a DXBC route's hull or domain shader. Its SPIR-V then has two variables at one
  location, which only our rewrite (which makes them private) ever reads.
- Run 66: Direct3D's pow2 partitioning stays integer partitioning on the DXIL route as on the DXBC
  route (dxil-spirv and dxbc-spirv both write it so; the RTX 3080 draws the own pow2 cases equal).

- DXIL geometry shaders (run 65): G6b's task names the carried naga reader reading the Geometry
  model with emit and cut. The model is instead made a compute program in the SPIR-V words before
  the reader (`dxil/geometry.rs`), as this crate already prepares widths, registers and images
  there: the reader needs no new carried patch, and the stage values' places are read from the
  decorations directly. Same result (the shared pass builds the forms), one fewer change to carry.
- The status file's run-by-run record (runs 1 to 58, 2,900 lines, with its superseded "Next"
  lists) moved to `docs/status-graphics-shaders-history.md` in G6a run 1, so that this file stays
  short and every statement in it is current; "Run N" references point there.
- Stream output from a vertex shader alone on WebGL 2 (run 58) is written on the host from a read
  back of the vertex stage's capture, where the plan's section 7 says native transform feedback,
  repacked by a second capture where the declaration needs it (`StreamOutRepacked`). Direct3D's
  overflow (no primitive written unless every buffer holds it), filled sizes and counts need the
  primitive count before writing, which transform feedback does not give; the host writer the
  replay route already has is exact for all of it (drawn equal to Direct3D 11). One blocking read
  a draw, counted as `StreamOutAfterReplay`.

- Indexed strips through the replay route (run 54) are listed on the host from the indices read
  back (one blocking read a draw on WebGL 2), where the compute route lists them on the GPU: WebGL
  2 has no compute, and the listed count decides the capture's size. The host listing follows
  `strips.wgsl`'s order, so both routes give the geometry shader the same primitives.

- Normalised targets held as integers (run 54) are compared with real Direct3D 11 within one code
  per channel, not exactly: the program rounds the scaled value to nearest, and Direct3D allows
  its own conversion `D3D11_FLOAT32_TO_INTEGER_TOLERANCE_IN_ULP` (0.6), which the RTX 3080's render
  targets use (ties of 16-bit codes go the other way, and some others). The decoded reads are
  exact (correctly rounded, equal to the GPU's).

- Stream output after a replayed geometry shader on WebGL 2 (run 53) is written on the host from a
  read back of the replay's capture (`StreamOutAfterReplay`), not by transform feedback as section 7
  says for WebGL 2's stream output: the replay captures every output slot, cut ones included, and
  dropping them needs each invocation's count of whole primitives summed over the invocations
  before it, which neither a vertex program nor transform feedback can make. Exact (drawn equal to
  Direct3D 11), and slow: two read backs a draw, and one more per buffer written. Stream output from a vertex shader alone
  stays the front's native transform feedback.

- Indexed strips through the stage helper (run 50) run the vertex stage once per place of the
  listed indices, not once per index of the strip: a vertex shared by a strip's triangles is shaded
  up to three times. This keeps one form (an indexed list) for every route after it instead of
  teaching each route a list of record numbers; the cost is more vertex work on a route that is
  already a counted slow path (`GeometryAsCompute`, `StreamOutAsCompute`,
  `CullDistanceAsGeometry`, `AntialiasedLinesAsQuads`, `PrimitiveIdAsGeometry`). A vertex shader
  with side effects (writing storage, run 86) may see the difference, as Direct3D leaves how often
  a vertex is shaded to the device (its functional specification); a vertex of no whole primitive
  is not shaded at all. The
  restart index is always the format's largest, as Direct3D 11 and WebGPU restart; a Direct3D 12
  pipeline with restarts off (`D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED`) is the front's to
  route.

- Run 47 touched the WebGL 2 back end's crate, which another track owns, by six lines: a public
  module `wcontainers_gpu_webgl::glsl` re-exporting its own `write`, `needs`, `GlslSettings` and
  `ShaderText` (its own commit, nothing else changed), so the corpus test checks the GLSL that back
  end writes rather than a copy of its writing (the check of run 46 found the copy hid its fixes).
  The WebGL 2 track should keep it or give the same through its own path; it is listed for them
  in "Asked of other tracks".
- G6's pass bar (run 46): reported complete with a written gap list (top of this file) instead of
  every item met, as this round's task asks, in the Direct3D 12 track's form; the independent check
  then runs on the converter as it stands and the gaps are worked after it.
- The corpus test's GLSL ES (run 46) is written from the key the WebGL 2 front asks
  (`TEXTURE_INFO`), not the WebGPU key, since plan section 12 asks each back end's compile of what
  that back end can run.

- Alpha-antialiased lines (run 45): plan 4.12 says the rectangle is widened by "the reference's 3
  by 3 pixels"; the kernel is the implementation's own (functional specification 3.4.4), so ours
  is the pixel box filter (the rectangle reaches one pixel past the axis and half a pixel past each
  end), which the RTX 3080's nearly is; WARP's is wider. The test holds ours to the RTX 3080 with
  an allowance of 64 of 255 and prints WARP's differences.

- The literal check (run 38): adopted as the brief of 2026-11-18 (late) says, by merging
  origin/phase-2-graphics-direct3d-11 (which had merged origin/phase-1-literals and adopted it)
  and then origin/phase-1-literals itself (tip 7668c010), then `init` once in the commit after the
  merge. The init ran on the desktop's checkout (the laptop's git 2.25 lacks an `ls-files` option
  the check uses), where only the submodules this crate builds from are checked out: the
  baseline's lines for scripts inside dxil-spirv's other submodules (SPIRV-Tools, SPIRV-Cross, its
  copy of dxbc-spirv, dxbc-spirv's SPIRV-Headers) became four `unparsed` lines, as in this branch's
  "After cloning". A checkout with every submodule checked out would see those scripts as growth;
  a `skip` policy for third-party submodules is the literals track's to add.

- Storage offsets (run 28; not built yet, see Next): 4.5 said every converted program adds
  `StorageOffset`; it is a flag (`VariantFlags::STORAGE_OFFSET`) instead, so programs whose views are aligned (most, on Direct3D
  11) pay no load and add per access, and a front that cannot know ahead (Direct3D 12) sets it on
  every program, which is the plan's behaviour. The plan's text now says so.

- Run 26: the laptop stopped answering, so the non-GPU tests ran on the desktop (3 cargo jobs,
  inside the GPU job) instead; the corpus and WebAssembly runs, which need the laptop's corpus and
  tools, were not rerun.

- Vertex programs without a position (run 23): the plan is silent. They convert with the position
  (0, 0, 0, -1) added, which every clip test rejects, so a direct draw of one rasterises nothing
  instead of the conversion being refused; the stage helper's routes (stream output, hull and
  geometry stages) never rasterise it.
- Direct3D 12's numbering (run 22) is the program's own, not the root signature's: each kind's
  slots are the program's resources in (space, register) order, in the stage's set. The plan had
  sets by visibility and ranges in root signature order with the root signature's hash in the key.
  The back ends make each pipeline's layout from its modules, so a root-signature numbering would
  share nothing more, while it would make a program per root signature (more conversions and cache
  entries) and need the root signature at every conversion; the front already finds descriptors by
  space and register. The plan's 6.4 text is changed to match (interface commit). `VariantKey::layout`
  and `convert`'s `root_signature` stay for root constants read as immediates (not built; refused
  `RootSignatureLayout`).

- Stores of normalised formats through typed buffers are held to Direct3D's permitted tolerance
  (0.6 of a code from the exact value, functional specification 3.2.3.4 and 3.2.3.6), not to the
  RTX 3080's codes: the GPU rounds some values 0.51 to 0.55 above a code down, which no exact
  formula of the specification reproduces. Every other stored and loaded value is compared exactly.
- The formats of typed buffer views at texture slots are a variant field (`texel_formats`), so a
  program is made per bound format; decoding by a format code read at run time would cost a branch
  per load.

- The stage helper (6.4's sketch `Stages::draw(backend, call, outputs)`) is `Stages::run`, which
  records the compute work outside the pass and returns what the front draws in its own pass: the
  interface allows dispatches only outside a pass, and the front owns its passes. The helper's own
  bindings are in set 1 of its compute programs (no compute program of the API uses set 1), so the
  interface needed no new binding numbers. Adjacency topologies are the helper's own
  `InputTopology` (they reach only the geometry stage), not the interface's `Topology`.

- Filtering in the program is held to two references, real Direct3D 11 on the RTX 3080 and WARP,
  texel by texel equal to either (or within 1/256 of a level of detail of either, or a third answer
  where the two differ), not to one picture: the two differ from each other in most pictures with
  linear mip filtering (the GPU's trilinear optimisation), which no program reproduces. Anisotropic
  cases are a known failure (Known gaps).
- The filter library is WGSL text in the crate, parsed by naga's `wgsl-in` and imported into the
  converted module, rather than naga IR built by hand: the same input for the same output as a
  hand-built tree, far shorter, and reusable for the stage helper's programs.

- `VIEWPORT_DEPTH_FLIP`'s reference is computed (1 - z of real Direct3D 11's depth buffer, the
  swapped range's definition), within 4 f32 steps (3 measured): no API on the desktop draws a swapped
  range natively (Known gaps). The other variants are held to a native feature or real Direct3D's
  picture, exactly.
- The counting test counts into an rgba8unorm target (the grid's triangles do not overlap, so each
  texel counts at most one), not an r16float one: the WebGL 2 back end reads no float target back.
- The real programs' shaders stay outside the repository (scratch and the laptop), as the plan's
  191-shader corpus does, rather than in it beside the rig's.

- wasi-sdk is not in the flake (6.2 says G6 adds it): the only machine that builds for WebAssembly is
  the laptop, which has no Nix (the Mac must not build). It stays a release archive there (How to
  run) until a Nix machine builds WebAssembly.
- DXIL whose checksum is the bypass value dxc writes when it does not sign (sixteen bytes of 1) is
  taken as signed; real Direct3D 12's answer to it is not measured (every DXIL we have is signed).
- Malformed bytecode is contained natively by C++ exceptions at the boundary and `catch_unwind`
  around our lowering; 6.2 builds the C++ with exceptions off everywhere. WebAssembly keeps them off
  (wasi-sdk 25's libc++ has none), where the module instance is the boundary.
- naga is carried as a copy in the tree (`third_party/naga`, patched in by the workspace), not as a fork
  by git revision (6.2): a fork needs a new repository on the owner's account, and this track commits
  only to its own branch. Moving to a git revision later changes one line of the workspace manifest.
- `isnan` from DXBC is lowered by `convert` itself (a bit test on the float's bits), not by the WGSL
  back end (6.5). From DXIL, naga's reader gives the IR's `isnan`/`isinf`, which the carried naga's
  WGSL writer writes as the same bit tests (f32 only; f16 and f64 stay refused by the writer), and
  GLSL writes as GLSL's own `isnan`/`isinf`.
- The content key's converter version (8) is the crate version plus a hash of all the sources that
  make the module (the C++, our Rust, the carried naga), not the crate version and the C++
  revisions: a change to the lowering or to naga changes modules without changing either.
- `[earlydepthstencil]` and conservative depth are not put in the module: WGSL has no form for either
  and naga's validator refuses them under WebGPU's capabilities. Early tests are reported in
  `StageExtras::Pixel::early_tests`; conservative depth is only an optimisation hint and is dropped
  (so GLSL ES loses it too).
- Every user input and output is a four-component value at location = register (6.4 speaks of packing
  by component; WGSL has no component qualifier, and fixed four-component values let two stages
  converted apart always agree).
- The cache stops reading at the first entry that is cut short or fails its CRC, and drops the rest
  (the plan says only that a cut-off last entry is ignored): past a damaged length nothing can be
  trusted, and dropping keeps appends well-formed. The CRC covers the entry's kind, key and length
  as well as its payload.
- A converted shader's record keeps the naga module and the reflection, not naga's `ModuleInfo`
  (naga cannot read it back); `decode` validates the module again.
- The WebAssembly build imports ten WASI functions, not two (above).
- The corpus test writes GLSL with the WebGL 2 back end's own `wcontainers_gpu_webgl::glsl::write`
  (since run 47). Only `naga_refusal` (tests/corpus.rs) copies naga's GLSL options, to record naga's
  own reason beside a refusal of that writing; if the back end changes its options, that reason
  follows by hand.
- dxil-spirv's converter library is not free of dxbc-spirv as 6.2 says: its bitcode module
  (`bc/module_dxbc_ir.cpp`) reads dxbc-spirv's IR. It is built against our one copy of dxbc-spirv
  (newer than the commit dxil-spirv pins; it compiles), and all of dxil-spirv is built as C++17, as
  its CMake does for that file.

## Known gaps found so far

- naga's GLSL writer (carried here) assigns a vertex program's cull distances as a sized array to
  `gl_CullDistance`, which it does not redeclare at that size; Chrome refuses it ("cannot convert
  from 'highp array[3] of float' to 'CullDistance highp array[8] of float'", run 47 chrome.log,
  `vs_cull` and the runner's clip-cull-distance-1 files). No harm today: the WebGL 2 back end
  declares `CULL_DISTANCE` absent. To fix before it is declared: redeclare `gl_CullDistance` with
  the program's size, as the writer does for `gl_ClipDistance`.

- Alpha-antialiased lines (run 45): the coverage is the box filter's, within 52 of the RTX 3080's
  in the drawn test; the RTX 3080 draws a line's end at about 0.64 where ours gives 0.5 and draws
  nothing past it where ours fades over half a pixel. Lines are cut at the near plane (clip z = 0)
  whatever the draw's depth clipping, so with depth clipping off a line crossing that plane loses
  the part Direct3D would draw clamped. Two triangles of a strip's joints overlap as Direct3D's
  rectangles do; the blend there follows draw order on both.

- Clip distances applied in the pixel program on lines (run 39): Direct3D clips a line and
  rasterises the rest by its diamond rule, so a clipped end can light a pixel whose centre the
  distance rejects; the program keeps exactly the pixels whose centres it accepts. Seen once in each
  of two seeds of `vs_cull`'s line list (one texel each). Triangles are unaffected beyond rounding.
  A texel of a line strip away from the clipped ends and the plane is another matter (A10, seed
  6202).
- Clip distances in the program (run 38): where a pixel centre lies exactly on a clip plane,
  Direct3D's clipped edge passes through it and its fill rule decides; the program keeps a
  distance of exactly zero (and its interpolated value may round either side). Seen only with a
  plane chosen through pixel centres; such pixels may differ.
- The stage helper's vertex records carry no clip distances (run 31): a vertex shader's
  `SV_ClipDistance` feeding a geometry or hull stage is dropped, which is exact since nothing after
  reads it, but stream output straight from a vertex shader cannot stream it; a front must refuse a
  stream output declaration naming it (run 31; not rechecked against the cull and replay routes,
  whose records hold clip distances).

- fxc fails with internal errors on programs with several loops in one switch and on switches
  whose cases call functions inlined many times (run 27, the tessellator): the built-in programs
  avoid them; the tessellation test on wgpu's Direct3D 12 is what catches a new one.
- Tessellator output buffers are sized for every patch reaching factor 64 (about 180 KB a patch),
  since the counts are known only on the GPU; a draw of thousands of patches needs hundreds of MB.
  After a domain stage (run 30) the domain records and the geometry output are sized the same way
  (every point and primitive the patches can make), and the geometry program runs for each.
- A geometry shader's strip whose vertices name different layers or viewports (run 30): each vertex
  is drawn in its own run's layer, where Direct3D takes a primitive's layer from its first vertex.
  Shaders that write one layer per strip (the usual form) are exact.
- Run 30 to 79, closed in run 80: a subnormal tessellation factor (1e-40 in the seeded tests)
  moved through a DXBC hull shader was flushed to zero, so the patch was culled where Direct3D
  draws it, and the drawn test counted such cases as known. The stated cause (wgpu's Direct3D 12
  path flushing) was wrong, as G6b's check found: only DXBC cases were known, and the DXIL route on
  the same back end kept the factor. The cause was dxbc-spirv's clamp of each factor to the
  declared largest (`IoMap::applyMaxTessFactor`), which the back end's compiler flushes; the
  lowering now keeps a subnormal factor's bits past that clamp (src/lower.rs,
  `loads_tessellation_factor`), and the drawn tests no longer flush or excuse any factor.
  To size by the counts instead (read back later, grown before the next draw) once draws run.
- Inside a patch the GPU's tessellator gives the primitives in another order than the reference, and
  starts about two thirds of its triangles at another vertex (winding kept); ours follow the
  reference. A domain shader's flat outputs (taken from a triangle's first vertex) can then differ.
- Typed buffers: DXIL's atomics are refused by naga's SPIR-V reader; through a texture, a view reaches at most 2048 times the back end's
  largest texture height elements (32M on ANGLE's 16384), short of Direct3D 10's 2^27, which the
  front must report.

- WGSL has only the weak compare-exchange (`atomicCompareExchangeWeak`), which may fail without
  cause; Direct3D's does not. Where the browser's back end writes it as a strong one (Direct3D 12's
  `InterlockedCompareExchange`, Vulkan's `OpAtomicCompareExchange`) the result is exact; a spurious
  failure elsewhere would return the compared value while not exchanging. Not seen; not tested apart.

- The stage helper's refusals are gap A5 (WebGL 2), A6 (DXIL) and A7 (the rest) at the top. Its
  buffers grow and are never shrunk.

- Anisotropic filtering in the program (`filter_in_shader`, `border_in_shader`) is a known failure
  of `tests/filter_draw.rs`: the footprint is the implementation's own (the RTX 3080's and WARP's
  differ from each other, by up to 0.14 in seed 8), and the program's taps along the major axis are
  a third (up to 0.59 from the GPU with gradients scaled 3 and 0.5). Seed 14 (anisotropy 4) lost
  ANGLE's device in a WebGL 2 draw (cause not found; first suspect: the program's tap loop as
  ANGLE's HLSL compiles it).
- Filtering in the program refuses comparisons, so a shadow map sampled with a border colour on
  WebGPU (no border colours there) is refused until `SamplerState` carries the comparison (Next).

- The minimum level-of-detail clamp's zero is (0, 0, 0, 0): Direct3D gives the format's default
  for components the format lacks (alpha 1 for an R8 texture, for example), which the program does
  not know. Drawn only on RGBA8.
- `DEPTH_BIAS` leaves a depth the program writes as it is; that Direct3D does not bias a written
  depth is expected (as Vulkan defines it), not measured.
- wgpu's Direct3D 12 path has no clip distances (wgpu issue 6236), so natively on Windows the
  WebGPU back end refuses programs with them unless made on Vulkan; Chrome's WebGPU compiles them.
- `draw_base: Arguments` serves WebGPU only: WebGL 2 has no storage buffers to read the arguments
  from (nor indirect draws; its front reads them back and draws directly with `Helpers`).

- Native and WebAssembly builds give some DXBC shaders' modules in another expression order: 27
  over every corpus (gap A8 has the list, the cause and what it takes). Same values, but a cache
  made natively and one made in the browser disagree on those entries (counted, the first kept).
  Neither build's order is the one to prefer: no host shares a cache between the two yet.
- The GPU's Direct3D 11 clears integer values into storage views otherwise than WARP and the
  specification (Run 19); the built-in clear follows the specification, so a program relying on
  the RTX 3080's saturation would see other texels.

- `SAMPLE_MASK` (the `SampleMaskInShader` route) is drawn against the native mask on WebGPU only:
  mozangle, the ANGLE the drawn tests use, lacks OES_sample_variables, which Chrome has. On WebGL 2
  it is checked by Chrome compiling the corpus's shaders that write the mask (G6a run 1) and by the
  WebGL track's per-sample test in Chrome, not by a drawn test of this crate.
- Neither real Direct3D 11 nor ANGLE on it draws a depth range with near beyond far as Vulkan and
  OpenGL define it (both gave the unswapped depths), so `VIEWPORT_DEPTH_FLIP` is checked against the
  definition applied to Direct3D 11's depth, not against a native swapped range.

- Signed but malformed DXIL can end the process natively: dxil-spirv's bitcode reader faults or calls
  `std::terminate` (`bc/cast.hpp`) on it (the third check: 26 of 6,000 cases at seed 5555; the
  second: 26 of 4,000 at seed 8128; 7 of 3000 in run 8, 9 in run 7, 10 in run 5, 15 in run 4,
  Measurements). dxil-spirv is used unmodified, so that fix belongs upstream (not yet reported).
  Malformed DXBC ended the process by exhausting memory in 3 of 3000 in run 5 (1 in run 4, none in
  run 7), in 1 of 6,000 in the third check and in 1 of 8,000 in the fifth. The third check's was
  this crate's lowering, fixed in G6a run 3; the fifth check's is dxbc-spirv sizing its list of
  temporaries by a register index from the bytecode (gap A11); runs 4 and 5 named dxbc-spirv's
  C++ `bad_alloc` (caught at the boundary under `ulimit -v`) without a backtrace, likely the same
  kind (not shown). Only
  bytecode whose checksum matches reaches the converters, so this needs a program that signs
  malformed bytecode. `tests/fuzz.rs` stays failing at the DXIL cases.
- Mutated but signed bytecode that dxbc-spirv accepts can give IR our lowering refuses (`Malformed`)
  or a module naga's validation refuses (`Invalid`, for example a constant index past an array's
  end, which Direct3D would read as zero): refused, not converted (Measurements).
- GLSL ES 3.00 has no `noperspective` and no derivative control: the WebGL 2 back end writes the
  first with NV_shader_noperspective_interpolation where the context has it (Chrome on the RTX 3080
  does) and refuses fine derivatives, which the corpus records name as `FINE_DERIVATIVES`.
- DXIL constants that are NaN or infinite are refused (`NonFiniteConstant`): naga's IR takes no such
  literal, and making them at run time needs new expressions ahead of their users. The DXBC route
  reads them from variables.
- A DXIL constant buffer component picked at run time (`cb[i]` of a float4's components) is refused
  (`ConstantComponentIndexing`): dxil-spirv reads it through a second view of the buffer as scalars
  4 bytes apart, which WebGPU's uniform layout and WGSL's one variable per binding refuse.
- fxc writes ordinary arithmetic and the common full-screen vertex shader with `bfi`/`ubfe`, which
  GLSL ES 3.00 lacks; the WebGL 2 back end defines them in the program since its 2080826c.
- The MSL writer's `continuing` fix (run 9) is checked by reading its output only; no Metal compile.
- The WebGL 2 back end's fragment depth (z arrived as (z+1)/2) is fixed there: `ps_depth` draws equal
  through it since run 7.
- The WebGL 2 back end's `textureSize` from a hidden uniform (its 3ea9af6) left `ps_size` reading 1
  by 1; fixed by its 4ab7bc84, equal since run 9.
- Direct3D links stages by register here; programs whose vertex outputs and pixel inputs use different
  registers for one semantic need remapping by semantic (also a variant to add).
- Out-of-bounds texture loads give what WebGPU's robustness gives, not Direct3D's zero. So does a
  dynamically indexed constant read past the size the bytecode declares (WebGPU clamps the index).
- Found while drawing, for other tracks: the WebGL 2 back end keeps GL's previous viewport when a
  draw names none (the 1 by 1 pbuffer's at first), where the WebGPU back end uses the whole target;
  the interface does not say what an empty `viewports` means. And its read back later calls
  `glGetBufferSubData`, which ANGLE's GLES lacks (`read_back_now` works).

## Interface changes

- Run 70, `BLEND_HALF_CODES` and `BLEND_CONSTANT_COMPONENTS` (commit "interface: the target combine
  value's code and constant counts are named once"): the array lengths of `TargetCombineValue`
  named, which the literal check asked for. No layout change.

- The paused commit 1efc1d61 (2026-11-20, recorded in run 70), `TargetCombineValue` and
  `HelperValue::TargetCombine`'s doc (crates/wcontainers-gpu/src/reflect.rs; the commit is not an
  "interface:" one, as it was committed whole at the pause): the helper value's layout as a
  `#[repr(C)]` struct with `new` (from a target's blend, the logic operation and the blend
  constant) and `bytes`, where the doc gave it in words (eight u32 codes, then four f32). Same
  layout; test `target_combine_values_are_written_as_laid_out`.

- Run 58, `SlowPath::StreamOutAfterReplay`'s doc and reason (commit "interface: stream output of a
  vertex shader alone written on the host counts as the replay route's"; plan text in section 7's
  WebGL 2 paragraph): the slow path names stream output written on the host on the replay route,
  after a replayed geometry shader or from a vertex shader alone. Same variant and number.

- Run 56, `VariantKey::texel_formats`'s doc (commit "interface: texel_formats also names the views
  of textures read in another component kind than declared"; plan text in 6.4's list): the bound
  view's format at a texture slot, already given for typed buffers, now also for a texture view of
  another component kind than the program declares, whose loads the program then reads by their
  bits. Doc only: one field for one fact (the view's format at a texture slot).

- Run 53, `SlowPath::StreamOutAfterReplay` (commit "interface: a slow path for stream output after
  a replayed geometry shader ..."; plan text in section 7's WebGL 2 paragraph): stream output after
  `GeometryAsReplay` is written on the host from a read back, a slow path of its own with its
  reason; appended last, so every other reason keeps its number.

- Run 51, `VariantFlags::GEOMETRY_REPLAY` (commit "interface: a variant flag for a geometry
  shader's replay form ..."; plan text in 6.4's flag list): plan section 7's `GeometryAsReplay`
  needs a geometry shader made a vertex program, which the variant key must ask for so that the
  cache keys the two forms apart; a flag, as the other forms a back end's lack asks for.

- Run 49, `StageValue::stream: u8` (commit "interface: gave a stage value its geometry stream
  ..."; the plan names the type only): a geometry shader writing several streams declares each
  stream's outputs at registers of their own, so one semantic can sit at different registers in
  two streams; stream output names (stream, semantic), which the reflection must tell apart. 0 for
  every value but a geometry shader's outputs of a later stream.

- Run 48, `VariantKey::register_ranges: Vec<(SlotKind, u32, Range<u32>)>` (commit "interface: gave
  the variant key the binding layout's register ranges ..."; plan text in 6.4's Direct3D 12 sets):
  an array declared without a length (Direct3D 12's unbounded descriptor ranges inside a root
  signature range of finite length) has as many elements as the range has registers past its
  first, which the program alone does not say; the key carries them so that the conversion stays
  a function of the source and the key.

- Run 33, `TargetInShader::format`'s doc (commit "interface: a target written in the program takes
  the output's bits where its format is not normalised"): it said only that the program encodes
  into the guest's format (`NormAsInteger`); a target of another type than the program's output
  (Direct3D 12's rt-format-mismatch) needs the field too, and for a format that is not normalised
  the program writes the output's bits as the format's type. Doc only.

- Run 29, `CompareFn`'s variants numbered 1 to 8 (Direct3D's numbers) as the codes
  `HelperValue::EarlyTests` holds, zero there a test that is off (its doc said "u32 codes" without
  naming them), and `EARLY_STENCIL_BINDING`'s doc says a front binds any single-sampled unsigned
  texture there for a format without stencil (WebGPU wants every declared binding bound; the
  program reads it only where the stencil test is on). Commit "interface: numbered the compare
  functions ...", which also gives the shared line's new slot test the `stride` field.

- Run 28, `VariantFlags::STORAGE_OFFSET` and `ReflectedBinding::stride` (commit "interface: ...";
  plan text in 4.5): `HelperValue::StorageOffset` existed with nothing asking for it; storage views
  at any first element (Direct3D 12's buffer_srv, the fronts' refused offsets) need the program to add
  it, and the front needs the program's element size to choose where to bind from.

- Run 26, `VariantKey::texture_layers: Vec<(u32, u32)>` and `AddedBinding::TextureLayer(slot,
  layer)` (commit "interface: gave the variant key the layer counts of multisampled array
  textures ..."; plan text in 4.2 and 6.4): plan 4.2 has fronts hold a multisampled array as a
  texture per layer, which a program can only read through a binding per layer.
- Run 26, `VariantKey::swizzled: Vec<(u32, [ViewComponent; 4])>`, `ViewComponent` and
  `SlowPath::SwizzleInShader` (plan text in 6.4): the default for back ends without
  `VIEW_SWIZZLE` (wgpu 30 has none), as the Direct3D 11 and 12 tracks asked.
- Run 26, `Reflection::slow_paths: Vec<SlowPath>` (with serde derives on `SlowPath`) and
  `SlowPath::AtomicExchangeLoop`: slow paths a program takes by itself whatever its variant, so the
  front can count and warn of them.

- Run 24, `HelperValue::Layer` and `HelperValue::Viewport` (commit "interface: gave fragment
  programs the layer and viewport of the draw's run as helper values"; plan text in 4.12's layered
  drawing): a pixel shader reading its layer or viewport index needs the run's, which only the
  front knows (`LayerSplit`, `MultiViewportSplit`), and zero where no earlier stage writes one;
  WebGPU has no such fragment input. Both fronts refuse helper values they do not write, so they
  build unchanged and refuse such shaders by name until they write them.

- Run 22, `ReflectedBinding::slot()` and the per-slot fields of `VariantKey`, the sampled pairs and
  `AddedBinding::ConstantsAs*` named by slot (commit "interface: named variant slots by a binding's
  place in its kind's range ..."; plan text in 6.4's Direct3D 12 sets): under Direct3D 12's
  numbering registers can be any size and repeat across spaces, so a bit per register cannot name
  them; the slot is the register under Direct3D 11's.

- Run 20, `DOMAIN_PATCHES_BINDING` (13) and `DOMAIN_POINTS_BINDING` (14) in the helper set (commit
  "interface: named the bindings in the helper set where a domain program reads ..."; plan text in
  6.4's set 3 list): a domain program, the final draw's vertex program, reads the hull stage's
  patches and the tessellator's points, which the front binds there for the stage helper.
- Run 20, `StageExtras::Hull` gained `point_locations` and `constant_locations` (commit "interface:
  gave a hull shader's reflection the locations it writes per control point and per patch"): the
  stage helper sizes the patch records from them.
- Run 18, `HelperValue::TexelLayout { slot }` given the view's width, height and layers or depth
  (five u32 in two 16-byte rows; commit "interface: gave the texel layout of a storage view read
  through a buffer the view's size ..."; plan text in 4.12): bounds and size queries need them, and
  the row pitch is not the width.
- Run 18, `VariantKey::texel_formats: Vec<(u32, Format)>` and `HelperValue::TexelBufferView { set,
  binding }` (commit "interface: gave the variant key the formats of typed buffers at texture slots
  ..."), then `TEXEL_BUFFER_TEXTURE_WIDTH` (2048) and the texture route's documentation (commit
  "interface: laid out the texture a typed buffer is read through on webgl 2 ..."); plan text in
  4.5's variant paragraph and 6.4's helper values: a program decoding a typed buffer must know the
  bound view's format, place and length, which only the front knows.

- Run 15, `HelperValue::TextureComponents { texture }` (commit "interface: gave the helper constants
  the components of a texture's format ..."; plan text in 6.4's list of helper values): a border
  colour applied in the program is read as a texel of the view's format, which only the front
  knows; a per-texture value, since one sampler serves textures of several formats.

- Run 14, `CONSTANTS_TEXTURE_WIDTH` (256) and `AddedBinding::ConstantsAsTexture`'s texture described
  (commit "interface: laid out the texture a constant buffer is read through ..."; plan text in 4.12):
  the front's copy and the program's fetch must agree on where each row lies.

- Run 14, `HelperValue::PulledStream { input }` (was `{ stream }`, three u32) as four u32 (offset,
  stride, divisor, bias), and `AddedBinding::PulledStream(n)` documented as the binding of the input
  at location n (commit "interface: gave each pulled vertex input its own binding ..."; plan text in
  4.12): the program then does not depend on how inputs share streams, and a step rate's first
  instance folds into the bias, so no base is needed.

- Run 13, `VariantKey::linked: LinkedValues { location: u32, clip_distances: u8 }` (commit
  "interface: gave the variant key where the values clip distances and depth clamping add pass
  ..."): the two programs are converted apart and must agree where the added values travel; plan
  text in 4.12. And `HelperValue::SamplerState`'s codes numbered as the variants' order of `Filter`
  and `AddressMode` (doc only).

- Run 9, two commits touching only `crates/wcontainers-gpu` (and the lock): serde `Serialize` and
  `Deserialize` derives on the variant key, the reflection and every type they hold (`Format`,
  `VertexFormat`, `ShaderStage` included), and on the state blocks of `state.rs` and `CompareFn`, as
  section 8 plans for the cache's postcard records.

- `VariantKey::output_interpolation: Vec<(u32, Interpolation)>` (commit "interface: gave the variant
  key the interpolation ..."): how the pixel program interpolates each vertex output, by location,
  where not `Perspective`; the vertex program writes them so. A variant field, as 4.4 has all
  rewrites that depend on what else is bound; plan text in section 4 says so.

- `VariantFlags::TEXTURE_INFO` (run 42, commit "interface: texture_info also reads sizes as zero
  ..."): under the flag a size asked at a level at or past `HelperValue::TextureLevels` reads
  zero, so a front binds it where a slot asked its size is empty (zero levels written there), on
  any back end; doc of the flag and plan 4.12 only, no new field.

- `VariantFlags::LAYER_FROM_STAGE` and `HelperValue::LayerCount` (run 43, commit "interface: a
  layer named past the views drawn into the first ..."): the Direct3D 11 track's layered item. A
  flag in both programs' keys (as `CLIP_DISTANCE`), the named layer a flat u32 after `linked`'s
  values (doc of `LinkedValues`), and the views' layer count a value of the pass-through's block;
  plan 4.12's layered drawing says so. Fronts that do not set the flag see no change
  (`PASSTHROUGH_HELPERS` keeps its two values).

- Where `LINE_COVERAGE` and `PRIMITIVE_ID_FROM_STAGE` read their values (run 45, commits
  "interface: the line coverage values pass after the linked values" and "interface: the
  primitive's number passes after the linked values"): documentation of `LinkedValues` and of the
  two flags only, no new field. The line coverage (four f32 without perspective) and then the
  primitive's number (a flat u32) follow `linked`'s other values; `stage::line_coverage_location`
  and `stage::primitive_id_location` compute them from the pixel program's key.
