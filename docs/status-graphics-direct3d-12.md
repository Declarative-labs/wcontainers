# Direct3D 12 and DXGI infrastructure status (G5)

Branch: agents, in the agents' shared worktree (since 2026-11-21). Plan: docs/phase-2-graphics.md,
sections 4.9, 5.2, 9, 10 and G5 in 12. This track owns crates/wcontainers-directx-graphics-infrastructure
and crates/wcontainers-direct3d-12. This file holds the current state only; the full record of runs 1
to 47 is this file at commit cf55bbe8 (`git show cf55bbe8:docs/status-graphics-direct3d-12.md`), and
later runs are in its git history.

The Direct3D 11 track committed two pieces in the infrastructure crate on 2026-11-22 while this
track was idle (f9ecca97, extended-range swap chains in the blit models; c3879851, the
`B8G8R8X8_UNORM` vertex element row; 22b0e1a3, names). Reviewed in run 53 as this track's own: kept.

## Where things stand

Run 60 (2026-11-24, milestone G5r, its sixth run), RTX 3080, desktop shared (the shaders track's
fuzzing held the GPU queue for long stretches); scratch run60/:
- The use test reads varied texels of the packed and block-compressed formats too (finding 1;
  337361f6 and the commit after it): `R10G10B10A2_UNORM` and `_UINT`, `R11G11B10_FLOAT`,
  `R9G9B9E5_SHAREDEXP`, `B5G6R5_UNORM`, `B5G5R5A1_UNORM`, `B4G4R4A4_UNORM` from fields packed by
  the test, and `BC1` to `BC5` (both signs), `BC6H` (both) and `BC7`, each a block of sixteen
  equal texels decoded by `bcdec_rs` (MIT). Loads, samples, gathers, multisampled loads, typed
  buffer and typed storage loads wherever answered. Misreads are now gathered, not fatal, so every
  use of a format is still made. use4.log (the final test code, lint clean in lint4.log): 931 uses
  by resources and 1099 by programs (run 59: 1029), none failing.
- What the first run of it found, and what real Direct3D 12 does with the same texels (scratch
  case `r60-packed-loads`, own1.log, on the RTX 3080, WARP and ours): the RTX 3080 reads the 5-bit
  fields of `B5G6R5` and `B5G5R5A1` widened to 8 bits (7 reads 57/255, not 7/31), and ours reads
  the same words; WARP reads v/31. The signed `BC4` and `BC5` read -89 as -0.7007965 on the RTX 3080
  and through ours, -0.7007753 on WARP; exact is -0.7007874. Every word of ours equals the RTX
  3080's for all seven formats of the case. The test takes these spreads: 5- and 6-bit fields
  within 1/255, signed `BC4` and `BC5` within a 16-bit step, `BC1` to `BC3` colours within 1/255.
- Alpha-antialiased lines beside every value (gap list, values between the stages), measured
  for the first time (scratch cases `r60-aa-4`, `-30`, `-31`: a vertex shader writing the position
  and n `float4` values, a pixel shader reading them all, line list): real and WARP draw all three.
  Ours drew 4 and 30 close to real (lit pixels: RTX 3080 372, WARP 536 and 560, ours 384 and
  404; at 30 every channel's sum over the picture within 1% of the RTX 3080's, as WARP's are). At 31 the process aborted: the stage helper's `record_layout` (shaders crate,
  stage/helper.rs:2382) indexed a 33rd vertex attribute while the pipeline was made, so the gap
  list's "refused by name" was wrong. A guard in this front (refusing where the pass-through
  module reads more vertex attributes than the back end has) was tried and dropped: the
  pass-through's arguments do not match the attributes the helper lays out (it refused run 58's
  32-register geometry case, which draws; scratch run60/x86-1.log), so only the helper can bound
  it. Linked clip distances beside 30 values with these lines are refused by name ("a shader
  feature the converter lacks"). Asked of the shaders track: the bound in the helper, and a line
  route needing no location or attribute of its own.

Run 59 (2026-11-24, milestone G5r, its fifth run), RTX 3080, desktop shared:
- The use test reads varied texels by every load it makes (8d37aad9): multisampled loads of
  each answered sample count of a target cleared to the varied texel's components (integer
  components a float does not hold exactly unchecked), typed buffer loads of a varied element,
  typed storage loads of a varied texture, each against Direct3D's conversions as the plain loads
  are. run59/use2.log: 931 uses by resources and 1029 by programs (run 57: 833), none failing.
  Still zeros only: the packed and block formats (no decode of them in the tree).
- New test `every_answered_option_is_served` (tests/answers.rs, GPU): `D3D12_OPTIONS`' answers
  beyond formats and binding counts that name abilities, asked and used: resource heap tier 2's
  one heap holding a buffer, a texture and a target, each read or drawn and checked;
  `TypedUAVLoadAdditionalFormats` present exactly where each of its 15 formats is answered with
  typed storage loads (which the use test uses); logic operations refused by name where answered
  absent (drawn and checked where answered). Passes (run59/use2.log). Not used by a program of our
  own: the wave lane counts and shader model 6.0 (need dxc; the shader runner's wave files use
  them), float64 and 64-bit integers (answered absent, their test files fail as listed).
- The shader runner's clip-cull-distance-2 check count explained ("Shader runner"): the runner's
  shader model probe, not a fault.
- The alpha-antialiased line coverage beside 31 values is met on the RTX 3080's 31 locations
  (gap list, values between the stages): an 11_0 combination unmet, owner the shaders track's
  stage helper, asked ("Asked of other tracks").
- Lint with warnings denied of the infrastructure crate and this front (run59/lint2.log): clean.
  Literal check (run59/lit1.log, the desktop's copy): no finding of this run's code; only bind.rs's
  two wrapped messages again, the desktop checkout's `\r\n` (run 58).
- The whole chain again at 4c07e6c4 (the shaders crate's DXIL register check, e352352c, came in
  since run 58; scratch bin/rest59.sh, logs run59/; the OpenGL track's job reset the display
  driver at 09:45 and held the queue, so the chain ran in two parts, `*1` and `*2`/`*3`), every
  figure held: GPU tests (gpu1.log) pass; test files (full2.log) 189 of 198, the same 9; own
  program (check2.log) draw 0 of 4096 beyond 1, 35 of 35 cases equal, `antialiased-lines` largest
  difference 38, features 154 of 872, outputs 22 of 110, answers 2 of 40, none beyond real; shader
  runner (runner2.log) 219 of 302, the same 83 failing files; the two samples equal (0 beyond 8);
  Godot's 2D platformer equal, its 3D platformer the same one pixel; bgfx 01-cubes, 26-occlusion,
  35-dynamic, 38-bloom and 37 every frame of both our runs within the allowance (740 to 1270
  pixels beyond 8 of 20,480); scenes (scenes2.log) 110 pass, 5 fail, 1 absent; Wine's five dxgi
  tests 3 of 5.

Run 58 (2026-11-24, milestone G5r, its fourth run): the eighth independent check (scratch
icheck58) found 11_0's 32 registers to the pixel stage not built behind the stage helper and
beside the values the front links: a geometry shader writing the position and 31 values was
refused (E_NOTIMPL), and a vertex shader passing 29 values beside clip distances the pixel shader
reads lost the device (wgpu refused location 31). Built (bc98cbd4): the moves are taken from the
two modules drawn, the vertex program, the domain program or the stage helper's pass-through, and
the pixel program, every location each passes counted, the linked ones too
(`stage_registers::passed_locations`); the refusal "values between the stages past the back end's
locations behind a geometry, hull or domain shader or stream output, or beside values the front
links" is gone. On the RTX 3080 (scratch run58/own2.log) the check's cases and a domain shader
case of this run draw what real Direct3D 12 and WARP draw: 32 registers behind a geometry shader
and behind a domain shader, 20, 29 and 30 values beside linked clip distances. Tests:
`registers_behind_geometry_and_domain_shaders_and_beside_linked_values_draw_as_direct3d_12`
(tests/registers.rs, GPU, real's hashes) and
`values_behind_the_stage_helper_and_beside_linked_ones_move_or_are_refused_by_name` (no GPU). The
check's minor findings: the 11_1 format table's differences named (Departures), run 56's commit
stated exactly ("Results of runs 56 and 57"), the runner's extra checks recorded as open ("Shader
runner"), the half-way stores measured again by the check (gap list).

Runs 56 and 57 (2026-11-23, milestone G5r): the fresh run of every figure at 1d583db4 (run 56's
chain, "Results of run 56") and the shader runner's failing files each sorted with its owner
("Shader runner"). What that sort showed of this front's own was built: a pixel shader reading
both its clip and its cull distances was converted with only the first refused one linked
(clip-cull-distance-1; 38d99d9c). The use test now also samples, compares and gathers texels of
varied components of the 42 formats `varied_texel` decodes, not only zeros ("Gap list",
formats). Milestone G5r's figures hold or are bettered on that fresh run (shader runner 219 of
302, from 218); its three findings and the sixth check's are closed or recorded below; every
remaining gap names its owner ("Gap list", "Shader runner", "Scenes").

Run 55 (2026-11-23, milestone G5r, its third run): the sixth independent check (scratch
icheck55) found finding 2 open again and the gap list naming wrong owners; both are closed below,
and its seven minor findings are each closed or recorded. Runs 53 and 54 closed the fifth check's
three findings; the seventh check had passed G5 on 2026-11-20 before the owner's ruling of
2026-11-22 made the level a hard requirement.
- Finding 2 reopened, 11_0's 32 input slots and elements and 32 registers from the vertex to the
  pixel stage (plan section 9): neither was built nor counted, and a valid 32-slot layout aborted
  the process inside wgpu (16 vertex buffers, `MAX_VERTEX_BUFFERS`). Built in run 55: elements of
  slots past the back end's vertex buffers, or of input registers past its attribute locations,
  are fetched in the vertex program (`VertexPulledInShader`, pull.rs `pulled_streams`); values
  between the vertex and pixel programs past its inter-stage locations move onto free ones after
  conversion, both programs alike (Departures, `stage_registers` in the infrastructure crate). The
  level counts both (`serves_stage_registers`), and, also unread before, resource binding tier 1's
  counts and 8 targets (`binds_tier_1`, test `no_device_without_binding_tier_1_counts_or_eight_targets`).
  Test `no_device_without_32_input_slots_and_stage_registers`
  (no GPU); test `thirty_two_input_slots_and_stage_registers_draw_as_direct3d_12` (tests/registers.rs,
  GPU) draws the check's own case (programs from fxc, tests/registers/) and must give real
  Direct3D 12's values (the check's runs on the RTX 3080 and on WARP, equal).
- The gap list's owners: the shader runner's failing files sorted again from a fresh run (each
  file's owner under "Shader runner"); the shaders track's delivered items moved to done ("Asked of
  other tracks"); what they unblocked of this front built in run 55: the level-of-detail query and
  a sample's clamp read the bound sampler's values, which the front wrote only where it applies
  the sampler in the program (calculate-lod, sampler: "a helper value the front does not write");
  a pixel shader reading its clip or cull distances gets them as values of the last vertex stage
  (clip-cull-distance-1 and -2); DXIL programs declaring arrays without a length get the root
  signature's ranges (unbounded-array-5.1).
- The minor findings: `R10G10B10_XR_BIAS_A2_UNORM` textures allowing target and unordered access
  views are made, as real makes them (no view of them can be made); the 11_0 table's link points
  at the page where it now is; the declared containers test holds every container to its recorded
  answers again (the corpora's 8 new containers recorded, every one declaring no typed view or
  multisampled array); the stale use count corrected; the scenes beyond the bar compared with
  WARP's pictures and their owner named ("Scenes"); the unorm half-way stores recorded with their
  numbers, within the bar; the use test loads a texel of varied components of 42 plain formats
  against Direct3D's conversions, and what it still reads only as zeros is named as this track's
  ("Gap list", formats); Godot's 3D pixel named as this track's, untraced.

- Finding 1 (runs 53 and 54), feature answers that do not match what is served: atomics were answered present in
  run 51. Test `every_answered_format_ability_is_served` (tests/answers.rs, GPU) asks and then uses
  every format answer, each DXGI format up to `B4G4R4A4_UNORM` on a device of its own. Resources
  (run 53): textures of every answered shape with and without mip chains, targets and depth
  targets cleared, every answered multisample count made and cleared, resolves, storage textures,
  copies to every other format of its group, and a format answered absent must be refused; every
  resource made must leave the back end whole (run 54). Programs (run 54, compiled by fxc,
  tests/answers/compile.cmd): loads, linear sampling, comparison sampling, gathers, comparison
  gathers and multisampled loads of each sample count, each of zeros and checked to read zero;
  typed buffer views loaded and stored; storage textures stored (checked written), loaded and
  every answered atomic (the values returned and left checked against Direct3D's operations,
  minimum and maximum of either sign on views of both); vertex input of the format and index
  input, drawn and checked; draws into targets of the format, blended where answered, of every
  answered sample count, and into its depth targets. On the RTX 3080 (run54/answers4.log): 931 uses by resources and 745 by programs, none failing.
  Not used by it: stream output buffers (`SO_BUFFER`), display and back buffer casts (swap
  chains), program uses of 1D, 3D and cube views (their resources are used).
  What it found and run 54 fixed, all three in the WebGPU back end (G2 idle since 2026-11-19; this
  track's commits f9fdaa02, 08fe9e5b, cc800174): a copy of `R32_SINT` into `D32_FLOAT` lost the
  device (the depth draw bound a signed texture as unsigned); every `R9G9B9E5_SHAREDEXP` texture
  was invalid (the adapter's flags gave it a resolve without drawing, so the front made it to draw
  into); storage writes to `B8G8R8A8_UNORM` lost the device (the adapter's flags gave storage the
  device was not asked for; now asked, and refused without it). Run 53's found storage views of
  `A8_UNORM` answered and not built, now served through a buffer.
- Finding 2 as runs 53 and 54 left it, the level: the device reports 11_0 only where every format requirement is built
  (Departures, first item; `D3D12CreateDevice` fails otherwise, naming each format short), from
  Microsoft's 11_0 format table, which now lives in the infrastructure crate (`required`). On the
  RTX 3080 every requirement is met (test `the_adapter_meets_every_format_requirement_of_11_0`); the
  run built the last two (`R10G10B10_XR_BIAS_A2_UNORM` textures, `R10G10B10A2_UNORM` back buffer
  casts) and took the rest from other tracks' work since 2026-11-20 (gap list, formats). What 11_1
  needs that is missing is named under Departures. No device is made in a browser until its WebGPU
  meets the table (Departures).
- Finding 3, bgfx 26-occlusion frame 10: closed in run 54, real Direct3D 12 on WARP and on the
  RTX 3080 itself drawing the same failure ("Real programs").
- What other tracks delivered since 2026-11-20 and this front took: the formats above (the
  Direct3D 11 track's back end and infrastructure work, runs 53 and 54), and the shaders track's
  converter items, keyed by this front in run 55 (second item of this list).

The pass bar, item by item (results under "Results of runs 56 and 57"):
- Wine's and vkd3d's Direct3D 12 tests giving real Direct3D 12's verdicts on the RTX 3080: 189 of
  198; every failing file is on the gap list.
- Real programs: Microsoft's D3D12HelloTriangle and D3D12HelloConstBuffers, bgfx 01-cubes,
  26-occlusion, 35-dynamic, 37-gpudrivenrendering and 38-bloom, Godot's 2D platformer.
- CheckFeatureSupport: the own program's `features` against real, every differing answer listed
  in features-differing.txt under its cause; the level and use tests above.
- The own program's 35 cases equal to real Direct3D 12.

## Results of run 58

2026-11-24, RTX 3080, desktop shared (other tracks' jobs held the queue for up to 20 minutes at
a time). The whole chain (scratch bin/rest58.sh, logs run58/) on bc98cbd4 with the worktree's
Cargo.lock (another track's lines uncommitted), every figure held:
- Test files (run58/full1.log): 189 of 198, the same 9 as runs 52 to 56, the same lines.
- Own program (run58/check1.log): draw 0 of 4096 pixels beyond 1; 35 of 35 cases equal;
  `antialiased-lines` 0 of 4096 beyond 63, largest difference 38; features 154 of 872 differ,
  outputs 22 of 110, answers 2 of 40, none beyond real.
- Shader runner (run58/runner1.log): 219 of 302, the same 83 failing files as run 56.
- Microsoft's two samples equal (0 pixels beyond 8); Godot's 2D platformer equal; Godot's 3D
  platformer the same one pixel (1 of 518,400 beyond 8, difference 48 at 921,317).
- bgfx 01-cubes, 26-occlusion, 35-dynamic, 38-bloom and 37 (b1-*.log, p1-bgfx37.log): every frame
  of both our runs within the allowance (726 to 1194 pixels beyond 8 of 20,480).
- Scenes (scenes1.log): 110 pass, 5 fail, 1 absent, the same as run 56; Wine's five dxgi tests the
  same 3 of 5.
- GPU tests (gpu1.log): the use test and both stage register tests pass.
- Every test of the infrastructure crate and this front on the desktop at bc98cbd4's code
  (run58/test1.log): all pass (unit 20, answers 2, cache 1, declared 1, device 20, front 15, gpu
  4, registers 2, swapchain 11, infrastructure 2). Lint with warnings denied of the interface,
  the infrastructure crate, this front and the Direct3D 11 front (run58/lint1.log): clean.
- tests/registers.rs again after naming its rounding step (run58/gpu4.log): both pass.
- Literal check (run58/lit3.log, the whole tree on the desktop's copy): no finding of this run's
  code. It lists bind.rs's two wrapped messages as changed only because the desktop's checkout
  ends their lines with `\r\n` (the baseline holds `\n`); on the rented ARM64 machine run 57
  found them equal.
- The check's cases and this run's (run58/own1.log, own2.log; own.sh lays them into the clone
  `tip2`, never committed): ic58-gs-registers, ic58-linked (20, 29, 30 values), ic58-packed and
  r58-registers (behind a geometry shader, behind a domain shader, beside linked clip distances)
  equal on real, WARP and ours.

## Results of runs 56 and 57

2026-11-23, RTX 3080, desktop shared (the OpenGL track's conformance runs held the GPU queue for
most of the evening; jobs waited up to 50 minutes). Run 56's chain (scratch bin/rest56.sh, logs
run56/) on da1fd6e7 with bind.rs and through.rs uncommitted, which 1d583db4 then committed
unchanged; the front's last commit of run 57, 38d99d9c (convert_stage), went only through the
crates' tests and two runner files, and the eighth check's fresh run at 6cbc878c, which holds it,
gave the same figures (scratch icheck58); run 57's checks (run57/) on its own clone `tip2`, branch
`phase-2-graphics-direct3d-12b` (its own desktop copy, C:\wc\src and C:\wc\target of that name).
- Test files (run56/full1.log): 189 of 198, the same 9 as runs 52 to 54.
- Own program (run56/check1.log): draw 0 of 4096 pixels beyond 1; 35 of 35 cases equal;
  `antialiased-lines` 0 of 4096 beyond 63, largest difference 38; features 154 of 872 differ,
  outputs 22 of 110, answers 2 of 40, every one listed and none beyond real.
- Shader runner (run56/runner1.log): 219 of 302 (run 54: 218); sorted under "Shader runner".
  clip-cull-distance-1 and -2 at 38d99d9c (run57/verify2.log): -2 passes, -1 fails no check
  (sections above shader model 6.0 only).
- Use test at 7ef9b914's test code (run57/verify2-answers.log): 931 uses by resources and 833 by
  programs, none failing; `binding_tier_1_counts_bound_at_once_are_read` passes.
- Microsoft's two samples equal (0 pixels beyond 8, 2.8 and 2.6 s); Godot's 2D platformer equal;
  Godot's 3D platformer the same one pixel (1 of 518,400 beyond 8, difference 48 at 921,317,
  frames 10, 60 and 120; 10.7 s) (run56/p1-*.log).
- bgfx 37 (p1-bgfx37.log), 01-cubes, 26-occlusion, 35-dynamic and 38-bloom (b1-*.log): every
  frame of both our runs within the allowance (735 to 1202 pixels beyond 8 of 20,480).
- Scenes (scenes1.log): 110 pass, 5 fail, 1 absent, the same outputs as run 54 ("Scenes").
- The use test and `thirty_two_input_slots_and_stage_registers_draw_as_direct3d_12` again at
  9d374918's tree, with both of run 57's commits (run56/gpu2.log): pass, 931 and 833 uses.
- Every test of the infrastructure crate and this front on the desktop at the same tree, GPU tests
  on Vulkan (run57/test1.log): all pass (unit 20, answers 2, cache 1, declared 1, device 19, front
  15, gpu 4, registers 1, swapchain 11, infrastructure 1).
- Lint with warnings denied (the rented ARM64 machine, the x86 one not answering; run57/arm-1.log
  to arm-4.log): the infrastructure crate, this front and the Direct3D 11 front clean; the tests
  without a GPU pass. Literal check (arm-5.log, the whole tree): no finding in this track's files;
  the Direct3D 11 front's two named under "Asked of other tracks".

## Results of run 55

2026-11-23, scratch run55/; the desktop's GPU queue was held by the OpenGL track's conformance
runs and the shaders track's seeded runs for most of the run (each of this run's jobs waited 20 to
60 minutes for them).
- Lint with warnings denied: the infrastructure crate, this front and the Direct3D 11 front clean
  on the rented x86 machine (run55/x86-3.log and later runs at 2453a26e's tree); this front with
  its Windows library on the desktop (lint2.log).
- Tests without a GPU, x86: infrastructure 1, front unit 20, cache 1, declared 1, device 19, front
  14, all pass. With the GPU (RTX 3080, Vulkan): test2.log at ae6cd91d's tree, every binary passes
  but registers.rs, whose read-back slice was wrong (fixed); gpu1.log at 83c563d9's tree:
  `every_answered_format_ability_is_served` (931 uses by resources, 787 by programs, none
  failing) and `thirty_two_input_slots_and_stage_registers_draw_as_direct3d_12` pass.
- Literal check (x86, 2453a26e's tree): in this track's files none; in the Direct3D 11 front's,
  the two redefinitions of `stage_registers`' names ("Asked of other tracks").
- The counted test files, the own program, the shader runner, the real programs, bgfx and the
  scenes: run55/rest1.out's chain (bin/rest55.sh 1), started 15:25 on code at 83c563d9.

## Results of run 54

2026-11-23, RTX 3080, desktop shared with other tracks (the OpenGL track's conformance runs held
the GPU queue for most of the run); scratch run54/, code at 29537184 (the final code; the status
file alone changed after it). Run 53's results are in this file's history.
- Lint with warnings denied: the infrastructure crate, this front and the Direct3D 11 front clean
  on the desktop (lint3.log); the WebGPU back end, the infrastructure crate and this front clean on
  the rented x86 machine (x86-lint5.log, x86-lint6.log at the final code).
- The crates' tests (test3.log): all pass, among them `every_answered_format_ability_is_served`
  and `the_adapter_meets_every_format_requirement_of_11_0`; the use test's counts (answers4.log):
  931 uses by resources and 745 by programs.
- Literal check (lit3.log): no finding in this track's files or in the WebGPU back end's lines
  this run wrote. Its new constant `SIGNED_BITS_TEXTURE` (fill.rs) makes the shaders track's test
  tests/variants.rs:1955, which writes the same WGSL type, a "restated" finding there.
- Test files 189 of 198 (full3.log, logs-full3/), the same 9 as runs 52 and 53.
- Own program (check3.log): draw 0 of 4096 pixels beyond 1; 35 of 35 cases equal;
  `antialiased-lines` 0 of 4096 beyond 63, largest difference 38; features 154 of 872 differ,
  outputs 22 of 110, answers 2 of 40, every one listed and none beyond real.
- Shader runner (runner3.log): 218 of 302, the same as run 53.
- Real programs (p3-*.log): Microsoft's two samples equal (0 pixels beyond 8); Godot's 2D
  platformer equal; Godot's 3D platformer the same one pixel as before (1 of 518,400 beyond 8,
  difference 48 at 921,317, frames 10, 60 and 120); bgfx 37 every frame of both runs within its
  allowance (829 to 890 pixels beyond 8).
- bgfx 01-cubes, 26-occlusion, 35-dynamic and 38-bloom (b3-*.log, references from 6 real runs):
  every frame of both our runs within the allowance (775 to 1182 pixels beyond 8 of 20,480).
- bgfx 26-occlusion pairs (pairs-occl3.log, pairs-occl4.log): "Real programs".
- Scenes (scenes3.log): 110 pass, 5 fail, 1 absent ("Scenes"). Wine's five dxgi tests (dxgi3.log):
  3 pass, the same two fail as in run 51 (no software adapter).

## Test files (Wine's and vkd3d's, through our libraries, RTX 3080)

How a file counts: the rig compares each test's verdict with real Direct3D 12's on this GPU; a
test that skips what real runs, or runs fewer checks, fails. Tests failing some checks on real
are held to every other check. The counted set is 198 files: d3d12 (Wine's) 10,
vkd3d-d3d12_invalid_usage 3, vkd3d-hlsl_d3d12 9, vkd3d-d3d12 176.

Failing at run 54 (run54/full3.log, logs-full3/, with the rig patch; the same as runs 52 and 53), each on the gap list: d3d12 10 of 10, vkd3d-d3d12_invalid_usage 3 of 3, vkd3d-hlsl_d3d12 9 of 9, vkd3d-d3d12
167 of 176. The 9:
- test_create_reserved_resource: ran 4 checks of real's 22, skipped 1. Tiled resources absent.
- test_get_resource_tiling: ran 11 of 1410, skipped 1. Tiled resources absent.
- test_output_merger_logic_op: ran 9 of 347, skipped 1. Logic operations absent.
- test_create_graphics_pipeline_state: d3d12.c:3487, a pipeline with a logic operation made
  without asking refused (E_NOTIMPL); the test then crashes on its null pipeline. Logic
  operations absent.
- test_shader_instructions: ran 4455 of 4759, skipped 28 against real's 2. Float64 absent.
- test_unbounded_resource_arrays: ran 7 of 2922, skipped 1. Resource binding tier 2 absent.
- test_unbounded_samplers: ran 7 of 97, skipped 1. Resource binding tier 2 absent.
- test_ps_layer: d3d12.c:28593, a pipeline whose vertex shader writes the layer refused
  (E_NOTIMPL). The layer from the vertex shader absent.
- test_copy_block_compressed_texture: 5 checks, the first d3d12_test_utils.h:669. Not to be
  matched (real Direct3D 12 crashes in it).
The refusals the counted test files printed (run 48's logs-full2): "logic operations without LOGIC_OP"
(create_graphics_pipeline_state) and "a shader feature the converter lacks"
(`RenderTargetArrayIndex`, ps_layer); none other.

Whole counts by run: 129 of 198 (run 17), 157 (20), 164 (25), 169 (28), 180 (31), 184 (34), 186
(35 to 45), 189 (46), 189 (48), 189 (49), 189 (50), 189 (51), 189 (52), 189 (53), 189 (54).

## Gap list

Every failing test file, shader runner file, scene output and real-program difference, and every
refusal by name in the front, is one of: an absent feature (reported absent as Direct3D 12 asks,
or refused by name where Direct3D 12 has no query), a listed loss naming what the back end lacks
(counted and warned), not to be matched (real Direct3D 12 crashes, or Direct3D leaves it
undefined), a named item of another track, or the front's own open part. Every refusal prints
its name ("wcontainers: ..."); a program meeting one gets E_NOTIMPL, E_INVALIDARG or a removed
device, never a fake success.

Failing test files:
- Absent, tiled resources (`TiledResourcesTier` 0): create_reserved_resource,
  get_resource_tiling.
- Absent, logic operations (`OutputMergerLogicOp` false): output_merger_logic_op;
  create_graphics_pipeline_state, which makes a pipeline with a logic operation without asking
  (d3d12.c:3487, refused, E_NOTIMPL) and then releases the pipeline it did not get, a null
  pointer, and crashes in its own code (0xc0000005; no panic in our library). Its 34 checks
  after that line (among them independent blending with a logic operation, d3d12.c:3491 on) are
  therefore never compared with real. What real Direct3D 12 answers on hardware without the
  ability (E_NOTIMPL here) is not known: every adapter at hand has it.
- Absent, float64 in shaders (`DoublePrecisionFloatShaderOps` false): shader_instructions.
- Absent, resource binding tier 2 (tier 1 answered where the back end lacks big tables):
  unbounded_resource_arrays, unbounded_samplers.
- Absent, the layer from the vertex shader
  (`VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation` false, plan
  4.12): ps_layer, whose context makes such a pipeline before asking (`RenderTargetArrayIndex`).
- Not to be matched: copy_block_compressed_texture (real Direct3D 12 crashes in it: its texture
  of part of a block is refused on both, and the test then uses the null texture). Before the
  crash the two part on when the device is removed: after `CopyTextureRegion` of the null texture
  `Close` answers E_INVALIDARG on both, but real has already removed the device (its
  `CreateFence` fails with 0x887a0005, d3d12_test_utils.h:669), where ours removes it only at
  `ExecuteCommandLists` of the list that did not close (its `CreateFence` succeeds, its `Signal`
  fails). Both remove the device; only when differs, on a call Direct3D leaves undefined.

Formats CheckFeatureSupport answers below real (run 53, features-differing.txt names each line
under its cause). Every ability Microsoft's 11_0 format table requires is answered on the RTX 3080
(test `the_adapter_meets_every_format_requirement_of_11_0`), and every answered texture ability is
used by test `every_answered_format_ability_is_served` (run 54: 931 uses by resources and 745 by
programs, below). What stays below real is
none of the table's requirements:
- The other video formats (`AYUV`, `Y410`, `Y416`, `P010`, `P016`, `Y210`, `Y216`, `NV11`,
  `AI44`, `IA44`, `P8`, `A8P8`): answered absent; the table leaves each format optional, the plan
  names no route (Departures).
- The video abilities of every format (decoder output, video processor input and output),
  section 9's named shortfall; `NV12`, `420_OPAQUE` and `YUY2` are served as ordinary textures
  (plan 4.12) with every other ability Direct3D defines for them.
- Tiled resources (`TILED` of the second word), absent.
- Logic operations on the integer formats, absent.
Served since run 53 (each was on this list): `B5G6R5_UNORM` and `B5G5R5A1_UNORM` targets, blending
and multisampled targets (the WebGPU back end's packed targets, built by the Direct3D 11 track);
`A8_UNORM` storage views (through a buffer of its texels, `StorageFormatThroughBuffer`, the
front's route now also for formats the back end holds converted, and storage textures of formats
the back end cannot store into are made without the storage use); `B8G8R8X8_UNORM` vertex input
and typed buffer views (the infrastructure crate's row, c3879851); `R10G10B10_XR_BIAS_A2_UNORM`
2D textures held in the 10-bit layout and copied with it, and `R10G10B10A2_UNORM` back buffers
viewed in the other formats of their group; the `R32G32B32_*` and `B4G4R4A4_UNORM` multisample load
and resolve bits (Departures: no resource they apply to can be made).
- Atomics on typed storage views of `R32_UINT`, `R32_SINT` (every operation) and `R32_FLOAT`
  (exchange) are answered present: the front serves them through a buffer
  (`AtomicsThroughBuffer`, counted and warned); the fifth check's own case (scratch
  icheck50/case-icheck-texture-atomics.log) gave real's atomic results through ours. The
  route's own refusals stand on the list below (in vertex and geometry shaders, multisampled, past
  slot 63, behind a geometry or domain shader); storage views through a buffer share them.
- Nothing answered is known not to be served: `every_answered_format_ability_is_served` uses
  every answer through resources and programs (above) with none failing (run 54). It leaves out
  stream output buffers, swap chain formats and program uses of 1D, 3D and cube views; those are
  compared with real Direct3D 12 by the counted test files, the own program and the shader runner.
  Since run 55 it also loads a texel of varied components (of either sign) of each of the 42
  formats of whole-byte components alike, checked against Direct3D's conversions (normalised
  ones within a bit): run55/gpu1.log, 931 uses by resources and 787 by programs, none failing.
  Since run 57 it also samples (linear), compares with 0.5 and gathers a texture of those texels
  in every texel, wherever answered, each checked against the same conversions (filtered floats
  within 1/1000): run57/verify2-answers.log, 931 uses by resources and 833 by programs, none
  failing. Since run 59 its multisampled, typed buffer and typed storage loads read varied texels
  too (run59/use2.log: 931 and 1029, none failing). Since run 60 the packed formats and the
  block-compressed ones but sRGB read varied texels as well, against Direct3D's conversions and
  `bcdec_rs`'s decode, within the spread measured between the RTX 3080 and WARP (Where things
  stand, run 60): run60/use4.log, 931 and 1099, none beyond it. Still zeros only: the sRGB formats
  (the curve's rounding not yet measured on real), the two-pixel formats (`R8G8_B8G8`,
  `G8R8_G8B8`) and the depth formats' views (this track's). Of `CheckFeatureSupport`'s
  answers other than formats, resource binding tier 1 is asked and used since run 55 (test
  `binding_tier_1_counts_bound_at_once_are_read`, tests/answers.rs: 128 shader resource views, 14
  constant buffers, 16 samplers and 8 unordered access views in one compute program, each read)
  and typed storage loads per format by the use test; `WaveOps` is asked by the shader runner
  (vkd3d's runner reads it before its wave sections) and used by its four wave files
  (wave-ops-float, -int, -uint, wave-reconvergence), each running every check real runs but those
  needing float64, int64 or shader models above 6.0, every one equal to real's; the lane counts
  themselves are compared with real by the own program's `features` only (a program of our own
  reading them needs dxc, which the desktop lacks). Since run 59 test
  `every_answered_option_is_served` asks and uses resource heap tier 2,
  `TypedUAVLoadAdditionalFormats` and logic operations (refused, answered absent). The lane counts
  and the packed and block formats' reads stay this track's own open part ("Next" 2).
- Typed storage stores of unorm formats rounding a value exactly half-way between two codes
  (the sixth check's case, texel = v * 0.5 + (0.25, -0.125, 0.0625, 0.5) stored and read back):
  ours gives one code off the RTX 3080's in a few channels (`R8G8B8A8_UNORM` d9 where it gives d8;
  the same in `B8G8R8A8_UNORM`, `A8_UNORM` through a buffer, and `R10G10B10A2_UNORM`'s 2-bit
  alpha 2 where it gives 3); WARP differs from the RTX 3080 in more channels (400010f3 400020d9
  ... where it gives 400010f2 400020d8). Within the 2026-11-22 bar (two real implementations
  already differ there by a code); every other byte of the 13 formats and of 2-layer arrays equal
  (the check's numbers; the eighth check measured them again, the same, scratch icheck58/own1.log).

Also absent, with no failing test file: conservative rasterisation, raster order views, ray
tracing, mesh shaders, variable rate shading, sampler feedback, 64-bit integers, depth bounds,
stencil export (answered absent whatever the back end has, section 9); timestamps where the
adapter lacks `timestamp-query`; drawing into and storing into `B5G6R5`, `B5G5R5A1`,
`B4G4R4A4`; minimum and maximum filters (no query in Direct3D 12; Microsoft's `D3D12_FILTER` page
sends the reader to Direct3D 11's `MinMaxFiltering`), refused at use.

A listed loss: `ExecuteIndirect` draws without indices whose arguments lie in GPU memory and that
fetch past a vertex buffer's end are skipped whole by WebGPU, where Direct3D 12 reads zeros
(`IndirectFetchPastEnd`, counted and warned). Direct draws, and indirect ones whose arguments
lie in an upload heap, are exact (`VertexFetchZeroed`).

Every refusal by name in the front (107 refusal sites by run 51's count, one more than then:
three depth clamping refusals and the pixel program for a variant's own replace three; from
`NotImplemented` in src/; `cargo test` of none of them fails; which ones the test files, the
runner, the scenes and the programs printed in run 48 is under "Results of run 48"). By cause:

Absent features or back ends a Direct3D 12 device is never made on:
- Answered absent by Direct3D 12's own query, refused at use: "logic operations without
  LOGIC_OP", "forced sample counts and conservative rasterization", "mesh shader pipeline
  statistics", "video decode statistics queries", "timestamps on this back end" (no
  `timestamp-query`), "minimum and maximum filters" (no query, above), "custom heaps" (custom heap
  properties this adapter does not have, as `ARCHITECTURE` answers).
- Back ends without what feature level 11_0 needs, on which `D3D12CreateDevice` makes no device
  (section 9, `LEVEL_11_0_FIXED_FUNCTION`, test `no_device_without_compute_11_0_sizes_or_d32_s8_targets`):
  "compute shaders on this back end", "geometry shaders, stream output and tessellation on this
  back end", "a depth bias clamp", "independent blending without INDEPENDENT_BLEND", "a partial
  sample mask without SAMPLE_MASK".
- Invalid in Direct3D 12, refused before this is reached: "a depth bias with no primitive
  topology type" (E_INVALIDARG first, case `undefined-topology`), "patch lists" and "patch
  topologies without a hull shader", "adjacency topologies" (direct adjacency draws drop the
  adjacent vertices, `AdjacencyDropped`; this names the others, below).

Named items of the shaders track (G6), printed with the converter's or stage helper's own name:
"a shader feature the converter lacks", "a shader the converter got wrong", "a geometry, hull or
domain shader the converter got wrong", "a shader feature the stage helper lacks", "a vertex
shader fetching its inputs did not convert" ("Shader runner" names each file's).

Not to be matched (Direct3D 12 leaves it undefined): "a shader resource view of another
dimension than the program declares" (since run 52 also a single-sampled view where the program
declares a multisampled texture, or the reverse, which the back end refused by losing the device)
and "an unordered access view of another dimension than
the program declares" (one-layer 2D arrays bound to 2D declarations are bound as 2D views; since
run 56 also a buffer view where the program declares a storage texture, which the atomics route
met as "not a texture", uav-atomics); "a raw or structured view where the program declares a typed
buffer" (run 56, before an invalid-use error, length-property);
"a shared buffer read after an overlapping one was written in the same submission".

The front's own open parts, each a route not built (no counted test file, case or real program
meets one on the RTX 3080; "Next" 2). Those only met where the back end lacks a feature name it;
browsers' WebGPU lacks `depth-clip-control`, `primitive-index`, polygon modes and texel buffers
on some adapters, and WebGL 2 makes no Direct3D 12 device:
- Depth clamping where the back end lacks `depth-clip-control` (`Features::DEPTH_CLAMP`):
  `DepthClampInShader` serves direct draws, pipelines without a pixel shader (the shaders crate's
  `builtin::pixel_program_for_variant` writes the clamped depth), and draws behind a geometry
  shader (its pass-through clamps), tessellation (the domain program clamps) and the cull route
  (the vertex program clamps); test
  `depth_clamped_in_the_programs_into_a_depth_target_draws_as_the_back_ends_clamp` draws each
  against wgpu's own clamp (run 52). Refused by name: "depth clamping the back end lacks, of
  alpha-antialiased lines" (the line route's coverage values are linked where the unclamped
  depth would pass), "... in the culled vertex or domain program stream output records" (the
  records would hold the clamped position), "... behind a geometry shader naming a layer, in a
  pipeline made with no depth-stencil format" (the pass-through passes the layer after a depth
  value the pixel program, which writes no depth there, does not take), and "... in a pipeline
  made with no depth-stencil format drawing while a depth-stencil view is bound" (its pixel
  variant would be made at the draw).
- Clip distances where the back end lacks them (`Features::CLIP_DISTANCE`):
  `ClipDistanceInShader` serves direct draws, pipelines without a pixel shader (the pixel program
  for the variant), geometry and domain shaders, and the cull route (the vertex program passes
  them, the pass-through passes them on); the same test draws the two new routes with wgpu's clip
  distances hidden (run 52). Refused by name: "clip distances from a geometry, domain, culling or
  antialiased line stage without the back end's (ClipDistanceInShader)", now met only by the
  antialiased line route and a culled vertex shader with stream output.
- Wireframe where the back end lacks polygon modes: "wireframe without the back end's polygon
  mode (FillModeConverted, which needs the stage helper's route)". Direct3D culls triangles
  before drawing their edges, so the route needs each triangle's positions after the vertex
  shader, through the stage helper's vertex capture (Departures).
- The primitive index where the back end lacks it: "the primitive index without the back end's
  (PrimitiveIdAsGeometry)" (the stage helper's `primitive_numbered` route is not used yet).
- Unordered access views: "storage views of part of a 3D texture" (`FirstWSlice` not 0: WebGPU's
  3D views cover every slice; needs the slice offset added in the program), "clearing a
  structured buffer view whose stride is no multiple of 4", "float clears of raw and structured
  buffer views", "clearing a typed buffer view of this format", "clearing an unordered access
  view of this format", "clearing rectangles of a 3D texture's view in another format", "null
  views of this storage texture", "null views of one-dimensional textures", "unordered access
  buffers in a custom heap the CPU sees".
- Views and binding: "a buffer view starting inside one of the program's elements", "a constant
  buffer view shorter than the program's block, or past its buffer's end, of a buffer not in an
  upload heap", "a texture read as another type than its view's format, of a stencil view or in
  a geometry, hull or domain shader or a vertex shader before one" (elsewhere the program is
  converted again reading the view's values by their bits, `VariantKey::texel_formats`, run 52;
  a sample of such a view the converter refuses, `SampledViewKind`), "a view of several layers of a
  multisampled texture array", "a view reinterpreting a texture's bits without its twin",
  "arrays of resources in a shader" (beyond those the root signature bounds), "buffers read
  through views", "this kind of root binding", "a multisampled texture array viewed in a format
  it cannot be viewed as", "a multisampled texture viewed in a format reinterpreting its bits",
  "comparison sampling of a multisampled texture not stored in a depth format", "comparison
  sampling of a texture of no depth format's bits", "a resolve in a format reinterpreting the
  stored bits".
- Sampling and formats in the program (`FilterInShader` and the like) in stages before the pixel
  shader: "a component mapping or stencil view read in a geometry, hull or domain shader or a
  vertex shader before one", "level-of-detail bias, border or mirror-once sampling in a
  geometry, hull or domain shader or a vertex shader before one", "a minimum level-of-detail
  clamp in a vertex or geometry shader or past texture slot 127", "a multisampled array of
  several layers read in a vertex, geometry, hull or domain shader", "sampling in the program
  through a sampler slot past 15", "typed views of other formats in a vertex shader", "a pixel
  program converted for the bound views and samplers behind a geometry or domain shader", "a
  program for the bound views and samplers", "shaders reading helper constants the front does
  not write", "a helper value the front does not write".
- Storage textures read through buffers (where the converter refuses atomics on them):
  "storage textures read through a buffer in a vertex or geometry shader", "multisampled storage
  textures read through a buffer", "a storage texture read through a buffer at a slot past 63",
  "storage textures read through buffers behind a geometry or domain shader".
- Values between the stages past the back end's inter-stage locations (runs 55 and 58,
  Departures): "more values between the stages than the back end passes" (`VaryingsBeyondLimit`:
  more than its locations after moving). Where 11_0 is reported (31 locations or more) a valid
  Direct3D pipeline meets it only where the front adds a value Direct3D passes in no register
  beside all 31 of the program's: the coverage of alpha-antialiased lines, a depth clamped in the
  program (back ends without depth clamping) or the primitive's number from the stage helper
  (back ends without the primitive index); refused by name and counted (no test, case or program
  meets it). On the RTX 3080 the first is met: wgpu-hal 30 gives 31 locations on both paths
  (Vulkan: the smaller of 128 output and input components over 4, less the position; Direct3D 12:
  32 registers less the position). Measured in run 60 (scratch case `r60-aa-31`): real Direct3D
  12 on the RTX 3080 and WARP draw alpha-antialiased lines beside 31 values; ours aborted the
  process inside the stage helper (33 vertex attributes for the pass-through's records): not refused
  by name, an abort, until the helper bounds it (asked). 11_0 is reported with this one combination of its requirements unmet (reported to the owner in
  run 59); beside 30 values ours draws close to real (`r60-aa-30`). The coverage takes a location of its own
  (`line_coverage_location`, the shaders crate's stage helper); building it needs a route there
  that takes no location, such as the pixel program working the coverage out from its position
  and the line's ends read from the helper's records by the primitive's number (owner: the
  shaders track's stage helper, asked; this front keys it). The helper's records take a vertex
  attribute a slot (33 beside 31 values). Clip and cull distances take no more locations than their
  registers: fxc gives them registers of their own (run 58, case r58-packed-clip: a `float3`
  beside `SV_ClipDistance0` in 31 registers is refused by fxc itself, "output limit (32)
  exceeded").
- Vertex input fetched in the program: "a vertex input fetched in the program from no vertex
  buffer", "a per-instance element beyond 4 GiB", "an indirect draw of a per-instance stream
  fetched in the program that steps other than once an instance", "a per-instance stream that
  never steps in an indirect draw", "a vertex buffer view read past its end at an address not a
  multiple of 4".
- Draws: "indexed or indirect adjacency draws without a geometry shader", "a predicated draw
  with a first instance on this back end", "a predicated strip draw whose indices restart at
  another value", "fractional viewports" (where the back end lacks `FRACTIONAL_VIEWPORT`; the
  `ViewportInShader` variant is not used yet), "alpha-antialiased lines without a pixel shader", "alpha-antialiased
  lines after a geometry or tessellation stage or stream output, or of wireframe triangles".
- Geometry, tessellation and stream output through the stage helper: "a draw choosing a layer of
  target views of different layer counts", "a draw choosing the slice of a 3D texture's target
  view", "a draw split per layer or viewport while a query runs", "a draw through a geometry,
  hull or domain shader or stream output while a counted occlusion or statistics query runs", "a
  predicated draw through a geometry, hull or domain shader or stream output", "an indirect draw
  through a geometry, hull or domain shader or stream output", "a stream output buffer at an
  offset the back end cannot bind", "drawing a stream other than 0", "patches of another control
  point count than the hull shader takes".
- Queries: "an occlusion query across passes", "counting pixel shader invocations inside a
  binary occlusion query", "counting pixel shader invocations of a draw fetching its streams in
  the program", "a counted draw of a vertex program converted for the bound views", "a counted
  draw of vertex inputs fetched in the program", "a counted draw reading storage textures
  through buffers".
- Early depth-stencil tests in the program (`EarlyTestsInShader`): "early depth-stencil tests in
  the program behind a geometry or tessellation stage", "... on a depth buffer denying shader
  resources", and per primitive "of a pixel shader reading its primitive index", "of a predicated
  draw or while a query runs", "of an indexed strip", "of an indirect draw", "of an instanced
  draw", "of an adjacency draw", "of a vertex program converted for the draw".
- "a feature the back end lacks": a back end's own `GpuError::Absent` passed on with the bit's
  name.

The Windows library's COM methods answering E_NOTIMPL, each printing "wcontainers: X::Y is not
implemented" (87 names, `unsupported` in examples/directx; list them with `grep -oh
'unsupported("[A-Za-z0-9]*")' examples/directx/*.rs | sort -u`). By cause:
- Abilities Direct3D 12's own queries answer absent (section 9): ray tracing
  (`CreateStateObject`, `AddToStateObject`, `SetPipelineState1`, `DispatchRays`, the four
  acceleration structure methods), mesh shaders (`DispatchMesh`), tiled resources
  (`CreateReservedResource`, `CreateReservedResource1`, `GetResourceTiling`,
  `UpdateTileMappings`, `CopyTileMappings`, `CopyTiles`), sampler feedback
  (`CreateSamplerFeedbackUnorderedAccessView`), variable rate shading (`RSSetShadingRate`,
  `RSSetShadingRateImage`), depth bounds (`OMSetDepthBounds`), view instancing
  (`SetViewInstanceMask`), programmable sample positions (`SetSamplePositions`), shared
  resources (`CreateSharedHandle`, `OpenSharedHandle`, `OpenSharedHandleByName`,
  `GetSharedResourceAdapterLuid`, `OpenExistingHeapFromAddress`,
  `OpenExistingHeapFromFileMapping`), protected sessions (`CreateProtectedResourceSession`,
  `CreateProtectedResourceSession1`, `SetProtectedResourceSession`, `CreateLifetimeTracker`),
  meta commands (`InitializeMetaCommand`, `ExecuteMetaCommand`: none enumerated), cached
  pipeline blobs (`PipelineState::GetCachedBlob`: `SHADER_CACHE` answers no single-pipeline
  cache; every bgfx run calls it and goes on without one).
- No software adapter is offered: `EnumWarpAdapter`, `CreateSoftwareAdapter` (Wine's two dxgi
  tests below).
- Parts of a Windows display a browser's canvas has no counterpart of: full screen and outputs
  (`GetFullscreenDesc`, `ResizeTarget`, `TakeOwnership`, `ReleaseOwnership`, `WaitForVBlank`,
  `GetFrameStatistics`, `GetDisplaySurfaceData`, `GetDisplaySurfaceData1`,
  `SetDisplaySurface`, the three gamma control methods, `DuplicateOutput`, `DuplicateOutput1`,
  `CheckOverlaySupport`, `CheckOverlayColorSpaceSupport`, `GetRestrictToOutput`), composition
  and CoreWindow swap chains (`CreateSwapChainForComposition`, `CreateSwapChainForCoreWindow`,
  `GetCoreWindow`, `GetSourceSize`, `SetSourceSize`, `GetMatrixTransform`,
  `SetMatrixTransform`, `GetRotation`, `SetRotation`, `GetBackgroundColor`,
  `SetBackgroundColor`), status notifications (`Register` and `Unregister` of occlusion,
  stereo, content protection teardown and video memory budget), memory budgets
  (`QueryVideoMemoryInfo`, `SetVideoMemoryReservation`: no browser reports memory, Departures).
- The front's own open part (no counted test file, case or real program calls one; of all 87,
  bgfx calls `GetCachedBlob`, `QueryVideoMemoryInfo` and `CreateReservedResource`, the last
  once a run, absent above, and goes on without it): `IDXGIObject`'s
  `GetPrivateData`, `SetPrivateData` and `SetPrivateDataInterface` on the factory, adapter,
  output and swap chain and its `GetParent` on the factory; `CheckInterfaceSupport`;
  `ResizeBuffers1`; `ClearState`; `AtomicCopyBufferUINT`, `AtomicCopyBufferUINT64`;
  `ResolveSubresourceRegion`; `SetStablePowerState`.

Other differences beyond the bar, each named with its owner: the shader runner's failing files
("Shader runner", each file's owner there), the scenes' failing outputs ("Scenes": the WebGPU back
end's Vulkan line rule, and the NVIDIA driver's Vulkan path for the anisotropy and alpha to
coverage cells), Wine's two dxgi tests that need a software adapter (none is offered;
`EnumWarpAdapter` answers E_NOTIMPL; this track's, no route in a browser), and Godot's 3D
platformer's one pixel (1 of 518,400 beyond 8, difference 48 at 921,317 in frames 10, 60 and 120;
not traced; this track's own open part: Godot offers the rig no WARP adapter to compare with).

## CheckFeatureSupport

The answers are section 9's: feature level 11_0 at most (`LEVEL_11_0_FIXED_FUNCTION` is only what
core WebGPU has: a depth bias clamp, independent blending, a partial sample mask; depth clamping
the back end lacks is done in the programs, wireframe it lacks is refused by name), resource
binding tier 1 without big resource tables (tier 3 with them, never 11_1), shader model 6.0,
everything section 9 reports absent answered absent. Format answers: what the back end holds
natively or by the front's exact routes (`FilterInShader`, twin textures for casts,
`StreamOutAsCompute` for `SO_BUFFER`), every texture shape the format's blocks allow; `NV12`,
`420_OPAQUE` and `YUY2` what their planes' or pixel pairs' formats have of the abilities
Direct3D defines for them (`video_format_support`). Tests:
`webgpu_reports_11_0_and_tier_1`, `resource_tables_give_tier_3_but_not_11_1`,
`no_device_without_compute_11_0_sizes_or_d32_s8_targets`,
`browsers_without_polygon_mode_or_depth_clamping_get_11_0`,
`format_support_follows_the_back_ends_format_uses`. The own program's `features` compares every
answer with real Direct3D 12's: an answer above real fails unless served
(`SERVED_BEYOND_REAL`: `R32G32B32_FLOAT` sampled and gathered), an answer below real fails unless
listed with both values in examples/check/features-differing.txt under section 9's reason, and a
listed line that no longer holds fails. `outputs` and `answers` are compared the same way, with
their own lists (examples/check/outputs-differing.txt: the 21 `CheckColorSpaceSupport` answers for
studio-range, BT.2020, PQ and 2.4-gamma spaces on `R10G10B10A2`, `R8G8B8A8` and `B8G8R8A8`, which
real presents on this desktop and a canvas cannot show, and the output's primaries, the monitor's
own on real and BT.709's on ours; answers-differing.txt: the two refusals, each with its cause).

## Cases (own program against real Direct3D 12)

`check_d3d12 cases` runs each case in its own process on real Direct3D 12 and through ours and
compares the lines exactly (examples/check/cases.rs); `antialiased-lines` also compares its
picture within 63 of 255 (Direct3D leaves the line filter to the implementation). The 35 cases:
- Placed resources: alias-copy, alias-storage, alias-cpu (overlapping placed buffers written by a
  copy, a root UAV and the CPU, read through the other), alias-texture, custom-heap.
- Queues and fences: queues (copy and compute queues waiting on each other and on a late CPU
  signal), timestamps (both queues, calibrations, resolved to default and readback heaps).
- Queries and predication: predication, predicated-resolve, occlusion-count, statistics.
- Indirect: indirect-draw, indirect-count, indirect-dispatch, indirect-short,
  indirect-vertex-buffer, indirect-vertex-id.
- Others: depth-planes, vertex-past-end, vertex-pulled, restart, render-pass, ms-array, geometry,
  layers, antialiased-lines, filter-depth, vertex-views, line-bias, markers, undefined-topology,
  no-scissors, early-per-primitive.
- Formats: video-formats (`NV12`, `420_OPAQUE` and `YUY2` stored as ordinary textures),
  texture-kinds (textures of `R8G8B8A8_UINT`, `R32G32B32A32_FLOAT`, `R8G8B8A8_SINT` and
  `R8G8B8A8_UNORM` loaded by a compute shader declaring another component kind; new in run 52).

## Shader runner (vkd3d-shader_runner, 302 files)

Run with our d3d12.dll alone (our dxgi.dll serves Direct3D 12 only and every file starts with a
Direct3D 11 part). Run 56 (run56/runner1.log, code at 1d583db4): 219 of 302, 83 failing (run 54:
218; run 52: 214). Each failing file sorted from that run's logs (scratch `bin/classify56.py`,
run56/classify-runner1.txt), with its owner:
- 57 run fewer checks than real, none failing, only for shader models above 6.0 (real runs each
  section at 6.0 to 6.5, ours at 6.0): absent by plan, section 9 answers shader model 6.0.
- 12 run fewer checks, none failing, for absent features (section 9): float64
  (arithmetic-float-uniform, cast-64-bit, wave-ops-float), 64-bit integers (arithmetic-int-uniform,
  arithmetic-uint, bitwise, cast-64-bit, wave-ops-int, wave-ops-uint), depth bounds (depth-bounds),
  raster order views (rasteriser-ordered-views), the layer and viewport index from any stage
  (rt-array-index, vp-array-index).
- 14 fail some checks:
  - Absent: switch (64-bit integers, `Int64`), sample-minmax (minimum and maximum filters),
    tiled-status (sparse feedback belongs to tiled resources, tier 0 answered; the shaders track's
    `SparseFeedback` stays a refusal by plan), frexp (one check: float subnormals in shader models
    4 and 5, plan 4.12's listed loss, frexp line 57).
  - The shaders track (G6), asked ("Asked of other tracks"): gather-cmp (`DepthGatherComponent`),
    eval-attrib (`CentroidPositionPerSample`), uav-rwbuffer, uav-rwbyteaddressbuffer,
    uav-rwstructuredbuffer and uav-rwtexture (`GloballyCoherent`, and memory operands wider than
    the workgroup).
  - Not to be matched (views of another kind than the program declares, which Direct3D 12 leaves
    undefined; real draws them, ours refuses each by name and removes the device, so the lines
    after are not reached): texture-load (line 51, a `Texture2DMS` declaration bound to a
    single-sampled view), uav-atomics (line 152, a buffer view where the program declares a
    storage texture), length-property (line 145, a structured view where the program declares
    `RWBuffer<float>`; vkd3d marks this section todo for itself).
  - This front's own: clip-cull-distance-1, a pixel shader reading both its clip and cull
    distances was converted with only the first one refused linked. Fixed in run 57 (38d99d9c;
    scratch run57/verify2.log): it now fails no check and runs 3,874 of real's 3,919, the rest
    sections above shader model 6.0, so it joins the first group (58 there, 13 here).
- clip-cull-distance-2 passes with 685 checks where real runs 676 (the eighth check's minor
  finding): both fail none and skip one. Explained in run 59 (scratch run59/trace1-real.txt and
  trace1-ours.txt, every check printed, `VKD3D_TEST_DEBUG=2` on the desktop's session through
  `gpu-job --desktop`; run 58's trace stopped after the Direct3D 11 part because real DXGI
  refuses swap chains in the queue's session 0): every check of the file's lines is the same on
  both, model by model; the only difference is shader_runner_d3d12.c:1452, the check of each
  `CheckFeatureSupport(D3D12_FEATURE_SHADER_MODEL)` the runner makes asking from 6.6 down until
  one is answered: real (6.5) asks twice, ours (6.0, plan section 9) five times, in each of three
  Direct3D 12 parts: 6 checks against 15. Not a fault; WARP (6.2) runs 681 for the same reason.

## Scenes (the rig's Direct3D 12 scenes, beyond the bar)

Run 56 (run56/scenes1.log) the same as run 54, each failing output by the same counts. Run 54
(run54/scenes3.log): 110 outputs pass, 5 fail, 1 absent (run 52: 95, 22 and 3; the
geometry, stream output, tessellation and raster scenes now run, the shaders track's converter
work, and `B5G6R5` targets are served). The failing outputs: topology (43 pixels beyond 1, all in
line cells: the aliased line listed loss), samplers-filters (anisotropy cells) and
msaa-alpha_to_coverage, as before (each draws with largest difference 0 on wgpu's Direct3D 12 path,
run 36: the NVIDIA driver's Vulkan path this library draws on, "Next" 4); new among the scenes that
now run, raster-fill (64 of 16,384 pixels beyond 1 in 20 tiles, largest difference 229) and
tessellation (310 of 32,768 beyond 4 in 87 tiles, largest difference 228). WARP's pictures of the
same scenes (the rig's references/d3d12/tolerances.txt): raster-fill 1,921 pixels differing, by 1
at most; tessellation 768, by 4 at most. So both are beyond the bar. Their differing pixels are
line pixels (the shaders track counted them per cell, its status A6: both differ only in cells
drawn as lines, and 308 of tessellation's 310 are pixels real left at the clear colour and ours
drew; raster-fill's cells drawn as lines are its three wireframe ones): wgpu's Vulkan path draws polygon-mode lines by Vulkan's line
rule, as the topology scene's line cells and the WebGPU back end's own test
`wireframe_fill_matches_direct3d` (214 pixels on Vulkan, none on wgpu's Direct3D 12 path) show.
Owner: the WebGPU back end's Vulkan path, a listed loss (plan 15.1); removed for this front by
drawing on wgpu's Direct3D 12 path ("Next" 4). Absent: minimum and maximum filtering.

## Real programs

RTX 3080, desktop shared with other tracks, not `--alone`, debug builds, 120 frames. Frames 10, 60
and 120 compared with real Direct3D 12's at 8 of 255.
- Run 54: as listed under "Results of run 54". Run 53 the same. Run 52 (this file's history) and run 51 the same. Run 50: the same, bgfx against two real runs. Run 49: the same but times (Godot's 3D platformer
  10 s), and bgfx 741 to 1225 pixels beyond 8. Run 48 (run48/p1-*.log at 3dfef503, p2-*.log at 708bd126; b1-*.log): Microsoft's
  D3D12HelloTriangle and D3D12HelloConstBuffers equal (0 pixels beyond 8; 2.6 and 2.5 s);
  Godot's 2D platformer equal (0 of 518,400 beyond 8; 3.8 s); bgfx 37-gpudrivenrendering,
  01-cubes, 26-occlusion, 35-dynamic and 38-bloom every frame of both our runs within the
  allowance (751 to 1218 pixels beyond 8 of 20,480 allowed, the overlay's text). Godot's 3D
  platformer runs 120 frames (10.1 s) and draws frames 10, 60 and 120 each with 1 of 518,400
  pixels beyond 8, largest difference 48 at 921,317 (gap list).
  At 708bd126 (p2-*.log) the same: the samples equal (2.7 and 2.5 s), Godot's 2D platformer
  equal (3.8 s), bgfx 37 every frame of both runs within its allowance, Godot's 3D platformer
  the same one pixel (10 s).
- bgfx is compared with the overlay's GPU time and memory lines ignored (rectangle 0,208,320,64);
  24-nbody draws differently on every real run, so it has no frames to compare.
- bgfx 26-occlusion, the fifth check's finding (frame 10 failing in both of our runs, six tiles,
  largest difference 254, against references from two real runs; also seen in the third check,
  icheck31; both at code before run 51). Closed in run 54 under the 2026-11-22 bar, by runs of
  real Direct3D 12 on the RTX 3080, of real Direct3D 12 on WARP (bgfx's `--microsoft`, its overlay
  naming the adapter "Microsoft") and of ours, each compared with each real run as a reference
  (scratch bin/pairs54.ps1, recompare54.ps1, pairs54.py):
  - pairs-occl3 (20 real runs, 10 of ours; its runs marked "warp" were real runs on the RTX 3080,
    bgfx's `--sw` choosing no software adapter): frame 10 fails in 18 of 90 comparisons of a real
    run against another and 18 of 100 of the second ten against the first, always 5 tiles, the
    worst at 224,608, largest difference 254, the fifth check's own failure; ours fails 10 of 100
    there, the same tiles and difference. Frame 120: real 18 of 90, ours 10 of 100, one tile at
    224,496.
  - pairs-occl4 with WARP (10 runs each; the renderer and timing lines of the overlay, which name
    the adapter, ignored, panel-occl4.log): WARP's frame 10 fails against every one of the 10
    real references (100 of 100), 6 tiles, the worst at 224,608, largest difference 254; ours 0
    of 100. Frames 60 and 120: WARP fails 100 and 90 of 100 at 224,496; ours 0 and 10.
  The tile at 224,608 is the corner view of the cubes the occlusion queries let through, at
  224,496 the line counting passing pixels: both follow occlusion results that arrive a frame or
  more late, so which cubes show depends on timing (the overlay's latency read "L: 0" in one real
  run on the RTX 3080 and "L: 1" in one on WARP, frames 60 and 10). Ours differs from real less often than real and WARP differ from real, in the
  same tiles by the same amount.
- Setup learnt on the way: the rig's forwarding library takes the name d3d12.dll and loads ours
  behind it (`WC_D3D12`), hence `wcontainers_d3d12.dll` and two forwarders; it aborts a program
  calling an export the library behind it lacks; bgfx calls `QueryVideoMemoryInfo` on the adapter
  `EnumAdapters` gives; NVIDIA's NVAPI calls into our device as if it were the driver's
  (harmless, absent in a container); a Rust panic in the library shows as 0xc0000409.

## Asked of other tracks

- WebGPU back end (G2), new in run 48: plan 4.11 and the interface's `FormatCaps` say BC formats
  are served in 3D textures, held decoded where the API lacks it. The back end sets
  `BLOCK_COMPRESSION` from `texture-compression-bc` alone and makes 3D BC textures natively; on an
  adapter with `texture-compression-bc` but without `texture-compression-bc-sliced-3d` such a
  texture fails. Asked: hold 3D BC textures decoded there (`BlockDecoded`). The RTX 3080 has both.
- WebGL 2 back end (G3): the same for S3TC and BPTC, which WebGL takes in 2D arrays only.
- WebGPU back end (G2), run 52: `d3d::TEXTURE_DATA_PLACEMENT_ALIGNMENT` is in the infrastructure
  crate, as asked; its `DX12_PLACEMENT_ALIGNMENT` can import it.
- WebGPU back end (G2), run 54, done by this track in G2's crate (G2 idle since 2026-11-19), for
  G2 to review: the depth copy by draw from `R32_SINT` binds the source as signed (f9fdaa02,
  cc800174); `BGRA8UNORM_STORAGE` is asked of the device and `bgra8unorm` takes no storage use
  without it; a native resolve needs the format to be drawn into (08fe9e5b). The last two change
  what the back end answers on adapters whose own format flags claimed more than WebGPU allows
  (the RTX 3080 through Vulkan: `rgb9e5ufloat` loses its resolve use, which the front never
  answered); the Direct3D 11 and OpenGL fronts' answers may move with them.
- Shaders (G6), still open, each with the runner file it fails (run 55's sort, "Shader runner"):
  `DepthGatherComponent` (gather-cmp; their A4), `globallycoherent` views and DXIL memory
  operands wider than the workgroup (uav-rwbuffer, uav-rwbyteaddressbuffer,
  uav-rwstructuredbuffer, uav-rwtexture), a centroid `SV_Position` in a program run per sample,
  refused by name (`CentroidPositionPerSample`, eval-attrib).
- Shaders (G6), run 59: the stage helper's antialiased line route without a location of its own
  for the coverage values (`line_coverage_location`; `ExtraSlot::LineCoverage`). Example: a
  vertex shader writing the position and 31 `float4` values, a pixel shader reading all 31,
  `AntialiasedLineEnable` with multisampling off, a line list: Direct3D 12 draws it (run 60, the
  RTX 3080 and WARP). A route: the pixel program works the coverage out from its position and the
  line's two ends in pixels, read from the helper's records by the primitive's number. Run 60
  adds: the same pipeline aborts the process in `record_layout`
  (stage/helper.rs:2382, "the len is 32 but the index is 32"): `Stages::antialiased_lines` takes
  records of the locations plus the clip distances' slots, and its pass-through reads them one
  vertex attribute a slot plus the position, 33 beside 31 values, past `MAX_VERTEX_STREAMS`
  (clip distances not read by the pixel shader take slots too; not run); `line_drawing`'s `AntialiasedLinesSlots` refusal bounds
  them at 32 slots, not at the attributes. Asked: refuse there (or read the records from a
  storage buffer by vertex index, which needs no attribute a slot).
- Shaders (G6), delivered and taken (their status lines 186 to 192 asked this list to move them):
  `TypedBufferFormatKind` (run 55 there; srv-buffers, tgsm and wave-ops-float now run every check
  but those of absent features), DXIL `InterpolateAt` (run 94), `DxilGeometryShader` and
  `DxilHullShader` (G6b; geometry, tessellation and tessellation-patch-vars pass),
  `CalculateLevelOfDetail` (runs 91 and 92) and `SampleClamp` (run 93), which then met this
  front's own unwritten sampler values, built in run 55 (calculate-lod, sampler), `PrimitiveId`
  (runs 97 and 107; primitive-id passes), `min16float` constants and DXIL `frexp` of a NaN (run
  111; arithmetic-float-uniform and minimum-precision fail no check), a clip or cull distance read
  by a pixel shader (`CLIP_DISTANCE` with `linked`, run 102; `CULL_DISTANCE_LINKED`, run 110),
  which this front keys since run 55 (clip-cull-distance-1 and -2), unbounded arrays on the DXIL
  route (`register_ranges`), keyed since run 55 (unbounded-array-5.1). Earlier: the pixel program
  for a variant alone, `DEPTH_CLAMP` on the pass-throughs, a texture view read in another
  component kind and clip distances through the cull route (used since run 52). Not theirs: float
  subnormals in shader models 4 and 5 are plan 4.12's listed loss (frexp line 57), the shader model
  4.0 multisampled load in a loop is a view the program does not declare (not to be matched).
- Direct3D 11 (G4), run 55: the infrastructure crate's `stage_registers` holds `input_moves`,
  `output_moves` and `relocate` as the Direct3D 11 front's src/shader.rs has them (taken from
  there, returning the refusal's name); that front could use it and drop its copy. Until then the
  literal check finds its `MOVED_ONTO_A_TYPE` and `MOVED_INVALID` (src/shader.rs:317 and 319)
  redefinitions of the infrastructure crate's (run 55, x86: the only findings in either front's
  or this crate's files).
- WebGPU back end (G2), on wgpu's Direct3D 12 path: clears through a float that keeps the bits
  (an `R32_FLOAT` denormal), and buffer-to-texture copies of several slices whose
  `rows_per_image` exceeds the copy's height (wgpu-hal 30's placed footprint).

## Next, in this order

1. Keep the crates building after each merge the orchestrator makes into `agents`; keep the
   literal check free of this track's findings. Apply the rig patch (sync56.sh does, in the
   clone only) for test file runs. Run 58's scripts: bin/`rest58.sh` (the whole chain on
   `tip`), run58/`test58.sh <n>` (lint and every test on `tip2`), run58/`own.sh <n> <cases>`
   (own cases of the checks and of run 58 on real, WARP and ours, on `tip2`); run 57's
   `verify57.sh <n> <runner files>` (with `ANSWERS=1` also the use test) on `tip2`,
   `armcheck57c.sh` (lint and the tests without a GPU, the ARM64 machine) on `tip3`; `tip2`'s
   desktop copy needs the submodule step the script carries.
2. The front's own open parts, beyond the bar: the four remaining depth clamping refusals,
   wireframe without polygon modes (`FillModeConverted` through the stage helper's vertex
   capture), a value the front adds beside all 31 of a program's (gap list; the line coverage
   asked of the shaders track: key it when built, and run scratch run60's `r60-aa-31` against
   real again), Godot's 3D
   platformer's one pixel (trace which draw writes it, on real and ours; Godot offers no WARP
   adapter choice the rig uses), the use test's reads of the sRGB, two-pixel and depth view
   formats (measure real's sRGB rounding first, as run 60 did for the packed and block formats),
   the wave lane counts used by a program of our own (needs dxc on the desktop), the other
   refusals of the gap list. Run 60's scripts (scratch run60/): `use60.sh <n>` (sync `tip2`, lint,
   the use test alone), `test60.sh <n>` (lint, every test of both crates, then the line cases),
   `own.sh <n> <cases>` (the checks' cases, run 58's and run 60's, on real, WARP and ours;
   apply.py lays them into `tip2`, never committed). Run 59's run59/`trace59.sh <n> <runner
   file>` (one shader runner file on real and ours, every check printed, through `gpu-job
   --desktop`).
3. Feature level 11_1 (Departures names what it lacks) and the 11_1 format table in `required`.
4. Draw on wgpu's Direct3D 12 path as browsers do on Windows (run 36's experiment, scratch
   run36/dx12-path.patch; it broke three things, each another track's). It also removes the
   scenes' line differences (wgpu's Vulkan line rule).
5. Plan section 10's one library (`crates/wcontainers-directx`): by the orchestrator's ruling of
   2026-11-20, one joint step with the Direct3D 11 track after both G4 and G5 have passed their
   checks, not part of G5.
6. The literal sweep, a part per run.

## How to run

All on the Windows desktop through the rig, or on the rented x86 machine for what needs no GPU;
on the Mac only rustfmt of single files. The rig names a checkout's desktop copy by its branch, and
every agent's worktree is on `agents`, so this track drives the rig from a clone of its own:
scratch `tip/` (`git clone --shared`, submodules initialised), on a local branch named
`phase-2-graphics-direct3d-12` that is never pushed, so that the desktop's copy, build folder,
Wine log folder and this track's desktop scripts (C:\wc\out\phase-2-graphics-direct3d-12) keep
their paths. Scratch `bin/sync54.sh` resets the clone to the worktree's HEAD, lays this track's two
crates over it as they are in the worktree (committed or not), and the files of other crates it
names (run 54: two of the WebGPU back end), and applies the rig patch below; do not sync while a
job of the clone waits, and run one job of the clone at a time (a second sends its tree over the
first's: "holds tree ..., not the sent ..."). Every build command starts with
`set CARGO_INCREMENTAL=0&& ` (stale incremental objects once failed to link). Run 54's scripts
(scratch bin/): `quick54.sh <n>` (sync, lint with the Direct3D 11 front, the crates' tests, the
literal check), `answers54.sh <n>` (sync, the format use test alone, its counts shown),
`files54.sh <n>` (build, every test file, the own program, the shader runner), `real54.sh`,
`bgfx54.sh`, `pairs54.sh <tag> <count> [example]` (bgfx run on real, on WARP through bgfx's `--sw`
and ours in turn, every run compared with every real run, tallied by `pairs54.py`), `scenes54.sh
<n>` (the scenes, then Wine's five dxgi tests), `run54all.sh <n>` (all of them); logs land in
scratch run54/. The use test's programs: `tests\answers\compile.cmd` on the desktop (the Windows
SDK's fxc), the .dxbc files committed beside it. On the x86 machine:
`orchestrator/cloud/x86-run <scratch>/tip --name d3d12-tip -- "cargo clippy ..."`.
- Lint: `tools/graphics-rig/remote "cargo clippy -j 3 --all-targets -p wcontainers-gpu -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 -- -D warnings"`.
- Tests (three draw on the GPU): `tools/graphics-rig/gpu-job --build "cargo test -j 3 -p
  wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12 --no-run" -- "cargo test
  -j 3 --no-fail-fast -p wcontainers-directx-graphics-infrastructure -p wcontainers-direct3d-12"`.
  `WEBGPU_API=dx12|vulkan` picks wgpu's path for the GPU tests.
- Literal check: `tools/graphics-rig/remote "cargo run -j 3 -q -p wcontainers-literals -- check"`.
- The libraries: `wcontainers_d3d12.dll` holds everything; `d3d12.dll` and `dxgi.dll` forward to
  it by that name; all three go beside a program. Build: `tools/graphics-rig/remote "cargo build
  -j 3 -p wcontainers-direct3d-12 --examples"`.
- Test files: `E='%CARGO_TARGET_DIR%\debug\examples'; tools/graphics-rig/wine-tests run --dll
  "wcontainers_d3d12.dll=$E\wcontainers_d3d12.dll" --dll "d3d12.dll=$E\d3d12.dll" --dll
  "dxgi.dll=$E\dxgi.dll" d3d12 vkd3d-d3d12 vkd3d-d3d12_invalid_usage vkd3d-hlsl_d3d12` (or
  `vkd3d-d3d12:test_a/test_b`). Logs land in pyro-parked/wcontainers-scratch/graphics-rig/wine-logs.
  The shader runner: the same with `vkd3d-shader_runner` and the first two `--dll`s.
- The rig patch: the rig's comparison stops at the first test failing a check real passes
  (`$allowed` is `$null` for a test with no known failures). Before test file runs, `git apply`
  scratch run20/rig-wine-run-fix.patch (it replaces the `$allowed` and `$worse` lines of
  tools/graphics-rig/desktop/wine-run.ps1 with `$worse = @($run.Failed.Keys | Where-Object { -not
  ($known.ContainsKey($name) -and $known[$name].Contains($_)) })`); `git checkout` the file
  afterwards and before any merge; never commit it.
- Own program: `tools/graphics-rig/gpu-job --desktop --build "cargo build -j 3 -p
  wcontainers-direct3d-12 --examples" -- "crates\wcontainers-direct3d-12\examples\check_d3d12.cmd"`
  (`draw`, `features`, `outputs`, `answers`, `cases`, `fence-wait 2000`, real and ours, then the
  comparisons; `--desktop` because real DXGI refuses outputs in session 0). One case:
  `crates\wcontainers-direct3d-12\examples\check_case.cmd <name>`. Cases compile HLSL with the
  system's `D3DCompile`.
- Scenes: `tools/graphics-rig/scenes --build "cargo build -j 3 -p wcontainers-direct3d-12
  --examples" --dll ... (all three) --api d3d12 [scenes]`.
- Real programs: `tools/graphics-rig/programs run` with the three `--dll`s (scratch
  `bin/prog49.sh <log> <programs>`); bgfx through scratch bgfx-compare.ps1 in
  C:\wc\out\phase-2-graphics-direct3d-12 (`bgfx49.sh`).
- com.rs: `python3 examples/directx/com.py > examples/directx/com.rs`, then `cargo fmt` (it reads
  the windows crate under `$CARGO_HOME`).
- The desktop needs the shaders crate's submodules once (checked out since run 4).

## No magic numbers

The literal check passes (run52/lit4.log at the last code commit, marks 0). Baseline lines of this track's crates: run 43
wcontainers-direct3d-12 471 and the infrastructure crate 63; run 48 458 and 63; run 49 457 and
63 (main.rs's feature levels now the windows crate's `D3D_FEATURE_LEVEL_*`); run 50 456 and 63; run 51 456 and 63; run 52 455 and 63 (the placement alignment's line gone)
(the planes' block sizes from the interface's formats). Named in run 48:
the cull modes, comparison functions and primitive topologies (`state`), the root signature
versions and the input layout flag (`root`), the buffer layout (`resource`); the tests' DXGI
formats, entry point and write mask from their one definitions. Left: Direct3D's own enum values
no crate this branch uses names (`features.rs`'s wave lane counts, `resource_heap_tier`, the
feature level numbers in tests/front.rs), the depth-stencil view's read-only flags (both fronts define
them; they wait for the Direct3D 11 track to put them in the `d3d` module), and the fix list's
order: Direct3D 12 placement alignments, WebGPU alignments and limits, byte widths, then the
Direct3D enum values. Facts another crate owns stay until its constant is on a branch this one
merges.

## Interface changes

- Run 49: `SlowPath::FrameStretched`, a frame shown stretched to a window of another size against
  the swap chain's scaling (a listed loss, counted at each such present), with a sentence in
  plan 4.9's surface formats paragraph (commit "interface: named a frame shown stretched ...").

- Run 48: `FormatCaps` says every format with a use is served in every texture dimension it can
  have (1D where its blocks are one texel high, 3D but for depth and stencil), held decoded or
  converted where the back end's API lacks it; plan 4.11 says the same (commit "interface:
  ..."). Doc only; the asks of G2 and G3 above follow from it.
- Earlier, each its own "interface:" commit: `Caps::adapter: AdapterInfo` (run 1);
  `ShaderModule::from_wgsl` (run 5); `COUNTER_BINDING` and `SlowPath::PixelInvocationsCounted`
  (run 22); `OcclusionCounter::end_into` and the counter's GPU sum (run 23);
  `SlowPath::SequentialBuffersCopied` (run 26); `SlowPath::IndirectFetchPastEnd` (run 30);
  `SlowPath::StorageOffsetInShader` (run 34). Details in the record at cf55bbe8.

## Measurements

- Run 54: no timing taken (the OpenGL track's conformance runs held the GPU queue most of the run).
- Run 53: no timing taken (the desktop's GPU queue was shared with the OpenGL track's
  conformance runs all run).

- Run 52, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at ce605d0d
  (run52/alone1.log, 06:15 to 06:17 on the desktop's clock; desktop CPU load 1 % before and 2 %
  after, Win32_Processor LoadPercentage): real first 6 µs, mean 1 µs, 99th percentile 1 µs,
  largest 6 µs; ours first 42 µs, mean 17 µs, 99th percentile 20 µs, largest 42 µs.
- Run 51, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at d9497d6e
  (run51/alone1.log; desktop CPU load 4 % before and 5 % after, Win32_Processor LoadPercentage):
  real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 58 µs; ours first 54 µs, mean 26 µs,
  99th percentile 66 µs, largest 154 µs.
- Run 50, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at a6412ea6
  (run50/alone1.log, 00:13; desktop CPU load 1 % before and 2 % after, Win32_Processor
  LoadPercentage): real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 8 µs; ours first
  72 µs, mean 20 µs, 99th percentile 24 µs, largest 72 µs.
- Run 49, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at d3ec3f99
  (run49/alone1.log, ended 23:06; desktop CPU load 1 % before and 1 % after, Win32_Processor
  LoadPercentage): real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 8 µs; ours first
  157 µs, mean 26 µs, 99th percentile 30 µs, largest 157 µs (run 48: mean 18 µs; the change
  is not explained: nothing on the fence path changed in this run).

- Run 48, fence-wait (own program, 2000 rounds) as `gpu-job --alone` at 3dfef503
  (run48/alone1.log, 21:10; desktop CPU load 1 % before, 32 % after, Win32_Processor
  LoadPercentage): real first 8 µs, mean 1 µs, 99th percentile 1 µs, largest 8 µs; ours first 94
  µs, mean 18 µs, 99th percentile 26 µs, largest 105 µs.
- Run 46, fence-wait (own program, 2000 rounds) as `gpu-job --alone` (run46/alone9.log, desktop
  CPU load 2 % before and after): real mean 1 µs, largest 8 µs; ours mean 17 µs, largest 97 µs.
- Real programs' times (not alone, shared desktop, load not taken), run 47: bgfx 01-cubes,
  26-occlusion, 35-dynamic and 38-bloom ours 2.5, 2.6 to 2.8, 3.0 to 3.1 and 2.5 s against real's
  2.4 to 2.5 s; Godot's 3D platformer 9.9 s for 120 frames.

## Departures from the plan

- Run 54 changed the WebGPU back end, G2's crate, which this track does not own: three answered
  format uses lost the device there and G2 has been idle since 2026-11-19; by the ruling of
  2026-11-22 the missing ability is built in the crate that owns it ("Asked of other tracks").
- The feature level (run 53, the owner's ruling of 2026-11-22 that no level is reported whose
  requirements are not met): `D3D12CreateDevice` makes its 11_0 device only where the back end has
  11_0's abilities (`has_abilities_of_11_0`; since run 55 with its 32 input slots and elements and
  32 stage registers, `serves_stage_registers`: the back end's vertex buffers and attribute
  locations plus a storage buffer per stream fetched in the program reach 32, its inter-stage
  locations plus the position reach 32; the test recorder's Chrome-like limits, 8 vertex
  buffers, 10 storage buffers and 28 locations, do not, expected of browsers, not measured; and
  since run 55 resource binding tier 1's counts in a stage, 128 shader resource views, 14 constant
  buffers beside the helper block's uniform buffer, 16 samplers, 8 unordered access views of
  either kind, and 8 targets, `binds_tier_1`; a back end of 14 uniform buffers a stage, as wgpu's
  Direct3D 12 path is expected to have, makes no device: the helper block takes one) and
  the front answers every ability Microsoft's 11_0
  format table requires (`unmet_format_requirements`, from the infrastructure crate's
  `required` table, which Direct3D 11 shares the source of); otherwise it fails with
  `DXGI_ERROR_UNSUPPORTED` and prints each format short and the bits it lacks. Direct3D 12 has no
  level below 11_0, so where the table is not met there is no device at all: by the Direct3D 11
  track's measurement a browser's WebGPU meets 10_0 at most (8-sample `B5G6R5_UNORM` targets), so
  in a browser this front makes no device until that is built. The RTX 3080 through wgpu meets it
  (test `the_adapter_meets_every_format_requirement_of_11_0`). The table's video columns are no
  rows (video is reported by the video devices, which no level requires), as Direct3D 11's.
  Refusals by name of 11_0 abilities where the back end lacks a feature (depth clamping of
  alpha-antialiased lines and three other routes, wireframe without polygon modes) stay on the gap
  list as the front's own open parts: no counted test, case or program meets them on the RTX 3080.
- Direct3D's 32 registers between the vertex and pixel stages (run 55) where the back end has
  fewer inter-stage locations (wgpu: 31 on the RTX 3080 through Vulkan): the values both programs
  pass move onto free locations after conversion (the infrastructure crate's `stage_registers`,
  the design the Direct3D 11 front built in its own crate; its copy could use this one), a pixel
  program's inputs in order from location 0 and the vertex program's outputs to meet them, and
  every program the pipeline makes later follows the same moves (`StageMoves`). Plan section 9
  names the requirement, not the route. Since run 58 the moves are taken from the two modules
  drawn (`passed_locations`), not from the reflections: the vertex program, the domain program or
  the stage helper's pass-through (geometry shaders, stream output, the cull and line routes) and
  the pixel program, with every value the front links beside theirs (clip and cull distances, a
  depth clamped in the program, a named layer). One limit stays (gap list): a value the front
  adds that Direct3D passes in no register (line coverage, a depth clamped in the program, the
  primitive's number) beside all 31 of the program's is refused by name.
- Wireframe without the back end's polygon mode is refused by name, not converted
  (`FillModeConverted`): Direct3D culls triangles before drawing their edges, so the route needs
  positions after the vertex shader through the stage helper's capture (the Direct3D 11 front
  refuses it the same way).
- Depth bias on lines and points is not applied: real Direct3D 12 draws them unbiased (case
  `line-bias`), where plan 5.7 routed them through `DepthBiasInShader`.
- The own program's cases are compared line for line, exactly; `antialiased-lines` compares its
  picture within 63 (the line filter is the implementation's).
- Clearing a float format's bits that a colour clear does not keep (an `R32_FLOAT` denormal on
  wgpu's Direct3D 12 path) is the back end's part, not the front's.
- A texture the converted program declares as depth is filtered in the program wherever a linear
  non-comparison sampler samples it: WebGPU's depth textures take no filtering sampler.
- A 2D array view of one layer bound where the program declares a 2D texture is bound as a 2D view
  of that layer.
- The literal check was adopted by merging origin/phase-1-literals itself, not through main.
- Every COM call crosses to the device thread as a boxed closure and waits for its answer; plan
  4.8's call buffer is not used (Next 4).
- Indirect draws without indices fetching past a vertex buffer's end: exact for direct draws and
  for arguments in an upload heap (`VertexFetchZeroed`), the listed loss otherwise.
- The Windows library is built as cdylib examples of `wcontainers-direct3d-12`, not in
  `wcontainers-directx` (Next 4); it is `wcontainers_d3d12.dll` with `d3d12.dll` and `dxgi.dll`
  forwarding to it.
- The library draws on wgpu's Vulkan path: with our d3d12.dll loaded, wgpu's Direct3D 12 path
  would load it as the system's (Next 3).
- Presenting: `CheckColorSpaceSupport` answers as Windows does on this SDR desktop for the spaces
  we present and absent for the 7 others real presents (outputs-differing.txt); the output's
  primaries are BT.709's (no browser reports the monitor's); a 16-bit float back buffer is presented encoded into `rgb10a2unorm` (or
  `rgba8unorm`); sync intervals 2 to 4 show a frame for at least one refresh; `DXGI_SCALING_NONE`
  frames are shown stretched (`FrameStretched`, counted at each such frame); `SetHDRMetaData` is checked and changes nothing.
- DXGI's memory sizes are zero and `QueryVideoMemoryInfo` and its relatives answer E_NOTIMPL (no
  browser reports memory). No software adapter is offered.
- `420_OPAQUE` is stored as `NV12` (plan 4.12) but is one plane to the program, its footprint
  the luma plane's, as real Direct3D 12 answers (case `video-formats`); its chroma texture is
  never reached. `NV12`'s target and storage views write the plane they name only.
- `GetResourceAllocationInfo` answers this front's layout (copyable footprints, 64 KiB pages, 4
  MiB for multisampled textures). Planar footprints (depth-stencil, `NV12`, `420_OPAQUE`) align
  rows to 512 bytes as real Direct3D 12 does here.
- Feature level 11_1 is never reported. What it needs beyond 11_0 that the device lacks, by name
  (Microsoft's hardware feature levels page): logic operations in the output merger
  (`OutputMergerLogicOp`, no WebGPU route; Wine's two logic operation files fail on it),
  target-independent rasterisation and unordered access view only rendering with a forced sample
  count (refused by name: "forced sample counts and conservative rasterization"), unordered access
  views in every stage (WebGPU stores from fragment and compute programs only) and 64 of them
  bound at once (beyond WebGPU's storage limits), and of the 11_1 format table (run 58: every
  row of Microsoft's 11_1 page against its 11_0 page, scratch run58/tablediff.py) the output
  merger's logic operations (`D3D12_FORMAT_SUPPORT2_OUTPUT_MERGER_LOGIC_OP`) on the 11 unsigned
  integer formats `R32G32B32A32_UINT`, `R32G32B32_UINT`, `R16G16B16A16_UINT`, `R32G32_UINT`,
  `R10G10B10A2_UINT`, `R8G8B8A8_UINT`, `R16G16_UINT`, `R32_UINT`, `R8G8_UINT`, `R16_UINT` and
  `R8_UINT`; it requires nothing else of any format beyond 11_0 (the two pages differ otherwise
  only in a note on `B8G8R8A8_UNORM`'s optional typed views). The infrastructure crate's
  `required` table holds no 11_1 row until a device can meet one.
  `ARCHITECTURE` answers a discrete adapter. Wave lane counts are WebGPU's bounds (4 to 128).
  `CheckFeatureSupport` answers E_INVALIDARG past feature 34 and for values naming no shader
  model, as this desktop's runtime does; check_d3d12 asks no shader model 6.7 or root signature
  1.2.
- The infrastructure crate's tests keep their own recorder: G1's finishes every fence at a submit.
- `SetEventOnCompletion` with no event waits on an event of the calling thread's own.
- A draw with no pipeline state set draws nothing; a draw reading a root argument not set since
  the root signature was set is skipped with a warning (real keeps the device).
- After the device is removed every fence reports `u64::MAX`.
- Descriptors are 64 bytes (real NVIDIA's are 32); a view counter is kept in two of their words.
- `MULTISAMPLE_RESOLVE` only for float and normalised formats rendering multisampled,
  `MULTISAMPLE_LOAD` only for target formats, whatever the back end's flags; but of a format no
  multisampled texture is made of (`R32G32B32_*`, `B4G4R4A4_UNORM`), the multisample load and
  resolve bits the 11_0 table requires are answered (run 53), as the RTX 3080 answers them and as
  the Direct3D 11 front does (its Departures, put to the owner): no resource they would apply to
  can be made, so no call relies on them.
- `R10G10B10_XR_BIAS_A2_UNORM` (run 53): 2D textures held in `R10G10B10A2_UNORM`'s bits, copied
  with that group, with no view, target or storage use (the 11_0 table gives it none; views and
  other shapes are refused). Its display and back buffer cast bits are answered as the RTX 3080
  answers them, though no swap chain of Direct3D 12's flip models takes the format (DXGI's rule,
  which the infrastructure crate's swap chains follow), so no call relies on them; the same
  reasoning as the multisample bits above.
- `ExecuteIndirect` leaves the state the last read-back command set (Direct3D leaves it
  undefined). `ResolveQueryData` resolves into a buffer of the front's and copies. A predicate
  covers the work up to the next `SetPredication` in the same list. An occlusion query across
  passes is refused, not summed.
- Null descriptors bind a front-owned resource of zeros of the shape the program reads.
- Custom heaps are checked as a discrete adapter's. `ALLOW_SHADER_ATOMICS` is taken on committed
  resources allowing unordered access and refused elsewhere, as this desktop's runtime does.
  `MakeResident`, `Evict` and the like succeed and do nothing.
- A texture made with more mip levels than its chain has is stored with the full chain only.
- Bundles record into the direct list and replay in its state. Back buffers share one reference
  on their swap chain.
- Creation checks follow this desktop's runtime (write states combined, buffer clear values,
  textures in upload heaps, block-compressed textures of part of a block, and more;
  `resource::check_creation`).
- Typeless `R32` and `R16` textures allowing depth views are stored as depth formats; views
  reinterpreting a typeless texture's bits get a twin texture.
- Clears cover every slice of the view; 3D UAV clears with rectangles copy from a scratch 2D
  texture.
- The frame latency waitable object is a semaphore of the swap chain's.
- `GetClockCalibration` answers E_FAIL without timestamps and pairs a lone timestamp with
  `QueryPerformanceCounter`.
- Root signatures are checked at `CreateRootSignature` only and are shared objects (`Arc`).
- Storage formats both read and written follow WGSL's rule (`R32_FLOAT`, `R32_UINT`,
  `R32_SINT`). Storage textures are first converted as 32-bit single-channel formats and again
  for other bound formats at the draw (`PipelineMiss`); where the converter refuses atomics on
  them, every typed texture view of that shader takes the buffer route, which ends the pass
  around the draw.
- Filtering in the program takes the sampler from its descriptor at the draw (a program per
  pipeline and bound state, `PipelineMiss`). Mirror-once addressing the back end lacks goes
  through `border_in_shader`.
- `CheckFeatureSupport` answers `BUFFER` and `SHADER_LOAD` for typed buffer formats the storage
  route decodes; the texel buffer route is `Storage` wherever the back end lacks `TEXEL_BUFFERS`.
- Stream output filled sizes are written back as their low 4 bytes. Every draw through a geometry
  shader ends the open pass. A draw whose topology does not give the geometry shader's input
  primitive draws nothing.
- bgfx's frames are compared with the overlay's GPU time and memory lines ignored, against a
  reference recorded from several real runs (6 by default, bin/bgfx-compare.ps1): bgfx
  26-occlusion's frame 10 is drawn three ways by real Direct3D 12 itself (run 51).

## Licences

- `slab` 0.4 (MIT): the device's object tables.
- `half` 2.7.1 (MIT OR Apache-2.0), half floats in UAV clears.
- `wcontainers-shaders` (ours), through it dxil-spirv and dxbc-spirv (MIT).
- `naga` (MIT OR Apache-2.0): the sizes a program's uniform blocks declare.
- `md-5` 0.11 with `digest` 0.11, `block-buffer` 0.12, `crypto-common` 0.2, `hybrid-array` 0.4
  (each MIT OR Apache-2.0): the DXBC checksum.
- `llvm-bitcode` 0.4 (MIT AND Apache-2.0 WITH LLVM-exception) with `num_enum` 0.7.6
  (BSD-3-Clause OR MIT OR Apache-2.0): DXIL's resource list.
- `bytemuck` 1.25.2 with `derive` (MIT OR Apache-2.0 OR Zlib): query data structures' bytes.
- Windows library only (dev-dependencies): `windows` and `windows-core` 0.62.2 (MIT OR
  Apache-2.0); `blake3` 1 (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception): the cache
  file's name.
- Tests only: `wcontainers-gpu-webgpu`, `wgpu` 30.0.2, `pollster`, `png` (MIT OR Apache-2.0);
  `bcdec_rs` 0.2.1 (MIT; already a dependency of both WebGPU and WebGL 2 back ends), the use
  test's reference decode of block-compressed texels (run 60).
- Wine's and vkd3d's tests (LGPL-2.1) are run from the rig for testing only; nothing of them is in
  this repository. No dependency added in runs 49 to 51.

## Installed on the desktop

Nothing of this track beyond the rig's setup:
- The shaders crate's submodules in this branch's source directory
  (C:\wc\src\phase-2-graphics-direct3d-12\crates\wcontainers-shaders, 68,417,586 bytes with git's
  files).
- C:\wc\out\phase-2-graphics-direct3d-12: bgfx-run.ps1, bgfx-capture.ps1, bgfx-compare.ps1 (1 to 2
  KB each), bgfx's captured frames and logs (about 12 MB, evidence), the rig's outputs, which each
  run overwrites.
- The rig's build folder C:\wc\target\phase-2-graphics-direct3d-12 is deleted at the end of each
  run (run 49: 8,318,929,943 bytes deleted; run 50: 8,314,346,230 bytes; run 51: 8,390,595,197
  bytes; run 52: 8,393,597,398 bytes). Run 51's repeated bgfx runs keep their logs and the evidence frames in
  C:\wc\out\phase-2-graphics-direct3d-12\loop (5,119,280 bytes; the copied programs and libraries
  deleted). On the rented x86 machine run 51's one copy (/work/src and /work/target
  graphics-direct3d-12, 359,310,563 bytes) is deleted; run 52 used no rented machine (every heavy
  run went to the desktop through the rig). Nothing installed in runs 49 to 54; no build output
  in scratch or on the Mac.
- Run 54: the build folder deleted (8,689,786,121 bytes); the bgfx pairs' frames and logs kept as
  evidence in C:\wc\out\phase-2-graphics-direct3d-12\run53 (20,240,385 bytes) and run54
  (30,220,189 bytes), their copied programs and libraries deleted; the use test's shader sources
  and fxc output in ...\answers (28,342 bytes). On the rented x86 machine the clone's copy and
  build folder (/work/src and /work/target d3d12-tip, 2,913,387,433 bytes) deleted.
