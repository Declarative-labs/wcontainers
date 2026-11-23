# Direct3D 11 front status

Branch: agents, in the agents' shared worktree /Users/thomaseaton/Documents/GitHub/wcontainers-agents
(since the evening of 2026-11-21). Plan: docs/phase-2-graphics.md, milestone G4 (section 12).
This file keeps the current state only; each earlier run's record (what it did, its measurements,
its literal counts) is in this file's git history (`git log -p docs/status-graphics-direct3d-11.md`).
Runs so far: six on G4a, 25 on G4 (2026-11-17 and 18), round 2 runs 1 to 12 (2026-11-19 and 20),
round 3 run 1 and round 4 runs 2 to 6 (2026-11-20), main runs 1 to 3 and agents runs 1 to 3
(2026-11-21), G4h runs 1 to 10 (2026-11-22 and 23), G4h round 6 run 1 (2026-11-23).

## Where things stand

G4 passed its check. G4h (feature level 11_0 reported truthfully, the owner's ruling of
2026-11-22) was reported complete at run 5; run 6 fixed what the sixth check found, run 7 ran
the whole bar at that code, and run 8 built what the seventh check found missing (below). The
device reports the highest level whose every format requirement of Microsoft's tables (their
video columns aside, Departures) it meets (4c9d43b2): the RTX 3080 device (WebGPU build, wgpu on
Direct3D 12) is 11_0 with every requirement met and tested, the WebGL 2 build (ANGLE on the
RTX 3080) is 10_0 since G4h run 5, and the WebGPU build without texture-formats-tier1 (a
browser's) is 10_0 since G4h run 9. Still differing from real Direct3D: the Gap list's absent
features. Both items the orchestrator ruled on 2026-11-23 are closed (G4h round 6 run 1, below).

G4h round 6 run 1 (2026-11-23, from about 08:50 Pacific), the orchestrator's two rulings:
- atg-simplecomputepc explained: the RTX 3080 gives either of the specification's two allowed
  results for the program's own calls. The real program on real Direct3D with frames 1 to 12
  captured shows ours' picture in all twelve, and its frame 10 fails its own reference by 154,515
  pixels, ours' count; with frames left uncaptured before it, the reference's. The program's calls
  traced with apitrace and replayed on the RTX 3080 give both too, and ours' frame 10 equals the
  replay's frame of that answer with 0 pixels beyond the tolerance. The constants, colour map,
  sampler and shader bytes in the trace are the isolated copies' own, so no Direct3D state
  decides it and there is no state to make a test of (Gap list, with every number). Counted
  passing under the graphics bar.
- The bar item for compute stores to `R11G11B10_FLOAT` and `R16G16_UNORM` typed views without
  texture-formats-tier1 is superseded by the feature level rule: on the gap list as absent on that
  back end, its two tests known failures named for that cause (4a2a8e8d).
- Literals: the context's slot counts named, ten baseline lines cut (Literals).
- Tests at 3fe49ed3 (main/q5-final.sh r6c-suite lint lib webgllib, logs/r6c-suite-*): lint of both
  builds clean; library com 15 pass (1 ignored), programs 68 pass (8 ignored, the known failures),
  as at run 10; the front's unit, front.rs (60) and cache tests and clippy pass on a rented machine
  at the same code (logs/r6c-t1-front.log). The WebGL 2 build's 10_0 cases 7 pass (1 ignored).

G4h run 10 (2026-11-23, from about 05:45 Pacific), the ninth check's findings:
- Packed targets blended within one draw: a draw of several primitives whose blend reads a
  `B5G6R5_UNORM` or `B5G5R5A1_UNORM` target is drawn one primitive at a time, instance by
  instance, rounded between each (Departures). Before, primitives of one draw read each other's
  8-bit values (the check's Draw(30) of 0.3-step increments gave 2 where both real
  implementations give 0, and a 1-bit alpha set where neither sets it). Through the check's own
  cases (check_g4h10 `overlap`, `packed_mrt`, `packed_msaa_mask`, `msaa_mask`, patched into a
  trial; logs/g4h10-r4-lib.log): additive ten draws, one draw and ten instances now equal both
  references exactly on both formats; the 1-bit alpha over one draw is 0 everywhere, as on both,
  and ours equals WARP's whole row. Left: single codes where both references agree and ours is
  one off, the 8-bit intermediate's rounding (Departures): `overlap` 2 of 280 channels (6-bit
  green 60 for 61), `packed_mrt` 1 of 56 (43 for 44), `packed_msaa_mask` 6 of 224 (resolves of
  masked samples), `msaa_mask` 0 of 384; the RTX 3080 and WARP differ from each other in 70, 6,
  50 and 96 of the same channels, by one code.
- The 1-bit alpha is compared exactly: own_cases `packed_codes` prints it as 00 or ff, where the
  one-code allowance of the "rounded" lines let it flip.
- Sample masks: a draw is dropped only when its mask keeps no sample of the targets' count (bit 0
  alone without multisampling); before, every mask without bit 0 dropped it, also into 4- and
  8-sample targets. The check's `msaa_mask` (masks 0x1 to 0xf0 at 4 and 8 samples, on
  `R8G8B8A8_UNORM` and `B5G6R5_UNORM`): every line of ours equals the RTX 3080's or WARP's.
- Tests: programs.rs `sample_masks_without_sample_0_as_direct3d_applies_them`, and
  `packed_blend_reads` within one draw (Tests); both pass on the RTX 3080 with
  `packed_targets_drawn_blended_and_resolved_as_direct3d_does` (logs/g4h10-r4-lib.log).
- At f002c33e (in HEAD ec243063), on the desktop under the queue (q5-final.sh g4h10-suite lint lib
  webgllib): lint of both builds clean; com 15 pass (1 ignored), programs 68 pass (8 ignored, the
  known failures); the WebGL 2 build's 10_0 cases 7 pass (1 ignored). The front's unit, front.rs
  (60) and cache tests pass on the rented x86 machine at the same code (logs/g4h10-x1-front.log);
  the literal check finds nothing new in this track's files or in the lines of the WebGPU back end
  this run wrote.
- atg-simplecomputepc counted failing and the bar item without texture-formats-tier1 left to the
  orchestrator's ruling (For the owner), as the check asked; the level check's circularity for
  multisample bits of formats without multisampled textures recorded there.

G4h run 9 (2026-11-23, from about 00:10 Pacific):
- The WebGPU device without texture-formats-tier1 meets 10_0 (9_3 before): the WebGPU back end
  resolves by a draw averaging the samples (`ResolveByDraw`) every float format it multisamples
  but WebGPU does not resolve (`r32float` in core WebGPU), and declares `RESOLVE` for it (lib.rs
  `resolved_by_draw`, its multisampled textures also bound for sampling; draw.rs `resolve_impl`).
  The front reports the multisample loads and resolves the tables require of every format it draws
  no count of (the integer twins, `R32G32_FLOAT`, `R32G32B32A32_FLOAT` there), as before for
  formats never drawn into (Departures). Tests: the three format support comparisons without
  features now check that 10_0 is made and 10_1 and 11_0 refused, and compare at 10_0 (the 16-bit
  unsigned twins' blending named: optional at 10_0, not served for twins); programs.rs
  `copies_and_float_resolves_of_multisampled_textures_without_texture_formats_tier1_as_direct3d_does`
  (own_cases `msaa_copy` at 10_0, now also a 4-sample `R32_FLOAT` target resolved, bits compared
  exactly, and the route's warning checked). All pass on the RTX 3080 (logs/r9-t2-lib.log,
  r9-t4.out).
- Found by that test: real Direct3D drops copies of multisampled textures below 10_1 (the copies
  read back zero at 10_0); ours copied them. The front now drops them there too (context.rs
  `copy_subresource_region`). own_cases' vertex and pixel shaders are compiled for the device's
  level's model (4_0 at 10_0, 4_1 at 10_1), so cases can run at 10_x.
- Microsoft's requirement tables are one table: the Direct3D 12 track moved this track's
  generated rows into the infrastructure crate (8e473ba1, `required`, with `unmet`), so the front
  re-exports that module and its own copy (src/required.rs) is deleted.
- atg-simplecomputepc: the RTX 3080 gives ours' picture for the program's own dispatch with every
  part of its setup copied (Gap list); judged an open choice under the orchestrator's rule, so
  passing then; the ninth check counted it failing, and the bar item without texture-formats-tier1
  waits on a ruling (G4h run 10, For the owner).

G4h run 8 (2026-11-22, from about 20:50 Pacific), the seventh check's findings:
- Direct3D's 32 input slots and elements: elements of slots past the back end's vertex buffers
  (16 on wgpu's Direct3D 12) or of input registers past its attribute locations (30) are fetched
  in the vertex program (`VertexPulledInShader`), where the check's 32-slot layout crashed wgpu.
- 32 stage registers: where a pair passes a value past the back end's inter-stage locations (31),
  the front moves both programs' values onto free locations after conversion (Departures), where
  31 values beside the position removed the device; refused with `VaryingsBeyondLimit` when they
  do not fit.
- The level check counts both (16 registers up to 10_0, 32 from 10_1) and warns for a level passed
  over; the recorder declares wgpu's Direct3D 12 limits.
- A draw reading inputs the input layout (or none) does not give is dropped, as real Direct3D
  drops it, where ours removed the device.
- `A8_UNORM` typed storage loads are served through a buffer (the shaders crate's 5e73ebc2) and
  claimed, as the RTX 3080 claims them.
- `B5G6R5_UNORM`'s "Other Multisample Count RT" at 10_1 is a requirement row too.
- Corrected in this file and in the plan: a browser's WebGPU meets 10_0 at most, not 10_1 (8-sample
  `B5G6R5_UNORM` targets at 10_1), and no exact route to 8 samples exists; the bar's case without
  texture-formats-tier1 cannot hold beside truthful levels (Gap list, asked of the orchestrator);
  atg-simplecomputepc is counted failing until the orchestrator rules; the packed rounding's 0.62
  of a step for 6 bits is recorded (Departures); the multisample bits of formats with no
  multisampled texture kept, with the reason (Departures).
- Test: programs.rs `every_input_slot_and_stage_register_as_direct3d_draws_them` and own_cases
  `a8_storage`'s typed loads; shader.rs's unit test of the moves (Tests). The whole bar at the
  end: as at run 7, with the new test passing (Measurements).

G4h run 7 (2026-11-22, from about 17:10 Pacific):
- The whole bar at 54fc06fc, run 6's code (q5-final.sh r7-bar; Measurements): every result as at
  run 5, with run 6's two new library tests passing. Nothing failed that is not in the Gap list.
- Why run 6's library suite never started: its desktop job (pueue task 40) kept running after
  the Mac's side of it was stopped. It held the queue's hazard group from 15:47. A remote command
  of the agents' folder held the GPU slot shared, and its `cargo test -p wcontainers-shaders
  --no-run` waited on a lock with no CPU used for 90 minutes. Each waited for the other. This run
  killed task 40 (`pueue kill`) and then that hung cargo process, and the queue moved (How to run).
- atg-simplecomputepc: presenting after each dispatch does not change the RTX 3080's answer
  either: the program's own fractal.cso still divides to just below 4 (What comes next, item 1).
- Lint of both builds and the library suite at 317499e8, after this run's code change: clean, and
  com 15 (1 ignored) and programs 65 (8 ignored) pass (logs/r7-head-*).
- Literals: building ahead's tuple key and reported counts are a tuple and a named struct, and
  their three baseline lines are cut (d05bba20, 39658541). The check at 39658541 finds nothing in
  this track's crates.

G4h run 6 (2026-11-22, from about 14:00 Pacific), the sixth check's findings:
- Packed targets read by blends: the WebGPU back end ends the pass part and rounds the
  `B5G6R5_UNORM` and `B5G5R5A1_UNORM` targets before any blend that reads them, when a draw wrote
  them since (draw.rs `reads_target`, `PackedTargetReadByBlend`, a new slow path at the interface's
  end). Test: programs.rs `packed_targets_read_by_blends_as_direct3d_reads_them` (own_cases
  `packed_blend_reads`: the 1-bit alpha as a blend factor both ways, and ten additive draws of 0.3,
  1 and 2 steps). The increments are chosen where the RTX 3080 and WARP round alike, by the sixth
  check's own measurements (both drop 0.3 of a step and below; they differ near 0.6, where the
  RTX 3080 rounds down to 0.81 of a step), so the case's verdict needs no WARP run; WARP was not
  run on it.
- Typed storage loads: a program that loads from a storage texture of a format other than the
  three one-channel 32-bit ones reads and writes it through a buffer of its texels
  (`StorageReadWriteThroughBuffer`, a new slow path; `TypedView::loaded` from the program's
  `ld_uav_typed`). `UAV_TYPED_LOAD` is claimed only where served: no longer for `A8_UNORM`
  (optional; the RTX 3080 claims it). Test: `typed_storage_loads_as_direct3d_does` (each of the
  15 formats of Microsoft's set loaded, changed and stored twice).
- `B5G6R5_UNORM`'s "Other Multisample Count RT" at 11_0 is now a requirement row
  (`required::SAMPLES_OTHER`: a count other than 1, 4 and 8); the video columns are recorded as
  not part of a level (Departures, For the owner).
- atg-simplecomputepc traced (Gap list); the extended-range test's tolerance is one 8-bit step as
  its comment says; the WebGL 2 build's failing scenes re-judged (Gap list); this file's stale
  lines corrected.

G4h run 5 (2026-11-22, from about 11:50 Pacific):
- The WebGL 2 back end (crates/wcontainers-gpu-webgl, the task allows it for this) holds the five
  formats 10_0 required of it: `B5G5R5A1_UNORM` and `B4G4R4A4_UNORM` natively as GL's `RGB5_A1`
  and `RGBA4` (alpha moved from the top bits to the bottom on upload, a 16-bit rotation; read back
  as RGBA bytes packed again into Direct3D's fields, `format::packed_fields`, which B5G6R5 now
  shares); `A8_UNORM` in `RGBA8` as (0, 0, 0, a) (widened on upload, alpha alone read back, cleared
  with zero colour, drawn with colour unwritten through the per-target channel mask that kept
  BGRX's alpha, now four bits a target, `format::unwritten`); `R8G8_B8G8_UNORM` and
  `G8R8_G8B8_UNORM` kept on the host and stored decoded, a pixel a texel (`host::decoded`, as the
  block-compressed formats without their extension). `B4G4R4A4` is sampled only, as the RTX 3080
  has it; its same-format copies take `copyTexSubImage` (GL attaches it). Every conversion counts
  `FormatConverted` or `BlockDecoded`, as before.
- So the WebGL 2 build meets every 10_0 requirement and makes its 10_0 device again; G4's WebGL 2
  10_0 cases pass (Measurements).
- Test: programs.rs `small_formats_read_and_drawn_at_10_0_as_direct3d_does` (own_cases
  `small_formats 10_0`, shader model 4): 1D, 2D (two levels), 3D and cube textures of all five
  written, read back through staging, loaded and sampled; `A8_UNORM` and `B5G5R5A1_UNORM` cleared,
  drawn, blended and their mips generated. Against the RTX 3080, both builds (Measurements).
  The WebGL back end's own unit tests gained two (every 16-bit packed texel written and read back
  keeps its bits; 8-bit alpha widened and read back).
- A comparison sample of `A8_UNORM`, first in the case, was dropped: Microsoft's tables do not
  require it of that format (the Gap list's earlier wording was wrong), neither device claims it,
  and ours refuses it by name as before.

G4h run 4 (2026-11-22, from about 11:25 Pacific):
- The two infrastructure-crate pieces are committed (f9ecca97 extended-range swap chains,
  c3879851 the `B8G8R8X8_UNORM` element row), by this track: the Direct3D 12 track, which owns
  the crate, has not run since 2026-11-20, and the orchestrator's ruling says to build the missing
  abilities in whichever crate owns them (Departures; noted in its status file).
- The device's level from the tables (4c9d43b2, the run-3 patch): `Device::new` takes the highest
  level whose `unmet_format_requirements` is empty, warning for each format short at each level
  passed over. The B8G8R8X8, extended-range and requirement tests are no longer ignored and pass
  on the RTX 3080.
- A format of which no multisampled texture is made because the device reports no multisampling
  (the WebGL 2 build: no route for a partial sample mask) is now given the multisample loads and
  resolves Microsoft's tables require of it, as the run-2 rule already gave formats never drawn
  into (Departures). The 10_0 table requires those bits without requiring any sample count. This
  took the WebGL 2 build's 10_0 shortfall from 30 formats to 5.
- The feature-off parts of three library tests are split into their own tests and marked known
  failures. The tests are `float32_textures_filtered_in_the_program_as_direct3d_reads_them`,
  `normalised_targets_through_integer_twins_as_direct3d_draws_them` and
  `comparisons_through_integer_twins_of_colour_views_as_direct3d_does`; `clear_view_without_texture_formats_tier1`
  and both `typed_storage_stores_without_texture_formats_tier1` tests are marked whole. Each asks
  for an 11_0 device with WebGPU features off, which truthfully meets 9_3 only (Gap list). The
  parts that run with every feature on still run and pass.
- Results at 4c9d43b2's tree (trial b318d907, desktop i9-9900K with the RTX 3080, other tracks'
  jobs queued; logs/g4h4-t2-*): lint of the infrastructure crate, front and library, both builds,
  clean; front 60, unit 6, cache 1 and the infrastructure crate's 11 pass (rented x86 machine);
  library com 15 (1 ignored) and programs 61 (8 ignored) pass; Wine's test_vertex_formats 100
  checks as real Direct3D and test_buffer_srv pass; test_format_support (4223 checks, real
  4667) and test_swapchain_formats (126, real 146) fail no check, each skipping real Direct3D's
  11_1 device (absent, Gap list); d3d10core's two pass. The WebGL 2 build's 10_0 cases: 0 of 6
  pass, each refusing its 10_0 device.
- Literals: the landed code's five findings are fixed (entry point names and the recorder's
  sample counts named or derived, 22b0e1a3, ef84f023), and three baseline lines they left are cut
  (73e218c1). The check at 73e218c1 finds nothing in this track's crates or the infrastructure
  crate (logs/g4h4b-literals.log; its 98 lines are other tracks').

G4h run 3 (2026-11-22, from about 10:20 Pacific):
- The shaders track committed `VertexFormat::Bgrx8Unorm` with every crate's arm (02049aeb, this
  track's patches of run 2), so `B8G8R8X8_UNORM` elements read blue, green, red and 1 wherever the
  format is mapped.
- Extended-range swap chains, written for the infrastructure crate (scratch
  g4h/xr-infrastructure.patch): blit models only, buffers held in `Rgb10a2Unorm`'s bits, the present
  copy decoding each code as `(c - 384) / 510` clamped to the 0 to 1 an SDR display shows (the crate
  presents SDR only). The front reports `DISPLAY` and `BACK_BUFFER_CAST` of the format from 10_0
  where the infrastructure crate displays it (device.rs, committed; inactive until the patch lands).
  Test: programs.rs `extended_range_swap_chains_show_as_direct3d_shows_them` (example
  own_extended_range: chains made at 10_0 and 11_0, refused in the flip model, views of the group,
  then columns of codes presented and read back from the window with `PrintWindow`). With the
  patch, every answer equals real Direct3D's and the colours shown equal the specification's
  decoding within one 8-bit step (ours: codes 0, 200, 384 show 0; 500 shows 58; 639 shows 127; 800
  shows 208; 894 and 1023 show 255). Real Direct3D shows nothing on the rig's hidden desktop (its
  presents answer `DXGI_STATUS_OCCLUDED`), so its colours cannot be read there. Ignored (known
  failure) until the patch lands.
- `B8G8R8X8_UNORM` vertex and typed buffers: the infrastructure row (g4h/bgrx8-infrastructure-row.patch)
  and the shaders crate's texel arm (g4h/bgrx8-shaders-texel.patch; committed by the shaders
  track as 1958e97b after this run asked, 380c3637). own_cases `bgrx` now also
  reads the bytes through a typed buffer; with both patches it equals the RTX 3080, Wine's
  test_vertex_formats runs real Direct3D's 100 checks and passes, test_buffer_srv passes.
- The device's level from the tables (g4h/front-truthful-level.patch): `Device::new` takes the
  highest level the back end serves at which `unmet_format_requirements` is empty, warning for
  each format short at each level passed over, then the first asked level at or below it. The
  front's recording back end now declares 8 samples and multisampled reads, as every format there
  does everything. With all three patches at a trial commit: the RTX 3080 device is 11_0 and the
  requirement test passes (un-ignored); front 60, unit 6, cache 1 and the infrastructure crate's
  11 pass; lint of every crate touched, both builds, clean; Wine's test_format_support and
  test_swapchain_formats fail no check (both still differ by real Direct3D's 11_1 device, the Gap
  list's absent 11_1); the format support comparisons pass.
- What truthfulness costs elsewhere, measured at that trial commit: the WebGL 2 build meets no
  level above 9_3 (10_0 needs multisample resolves of 23 formats, which that build reports none
  of since main run 3, `A8_UNORM` targets, `B5G5R5A1_UNORM` and `B4G4R4A4_UNORM` textures, and
  `R8G8_B8G8_UNORM` and `G8R8_G8B8_UNORM` (logs/g4h3-t6-webgl.log)); the WebGPU
  build without texture-formats-tier1, 16-bit norm or float32-filterable meets no level above 9_3
  (10_0 needs resolves of multisampled 32-bit float targets and of the normalised formats held as
  integer twins; 11_0 also needs 8 samples, which WebGPU's sample counts of 1 and 4 never give).
  This run concluded a browser's WebGPU could reach 10_1; that was wrong: 10_1 too requires
  8-sample `B5G6R5_UNORM` targets, so a browser's WebGPU meets 10_0 at most (G4h run 8). Gap list
  and For the owner.
- Programs re-judged against WARP under the orchestrator's rule (g4h/warpshim, a test-only
  d3d11.dll making every device on WARP, through the rig's programs and its RTX 3080 references):
  atg-simplemsaa_pc's frame 10 on WARP has 25 pixels beyond the channel tolerance (largest 21),
  21 beyond their tiles' allowance; ours has 8 (largest 11), 4 beyond. Ours is closer to the RTX
  3080 than WARP is, so it passes under the rule (frames 60 and 120 pass for both).
  atg-simplecomputepc on WARP: 1,889 pixels beyond (largest 255, on the set's edge); ours 154,515
  (every pixel inside the Mandelbrot set another colour): a real difference, the shaders track's
  division (Asked), still failing.

G4h run 2 (2026-11-22, from about 09:00 Pacific):
- `B5G6R5_UNORM` and `B5G5R5A1_UNORM` targets are built in the WebGPU back end: held in
  `rgba8unorm` as before, now declared `RENDER`, `BLEND`, `RESOLVE` and `rgba8unorm`'s sample
  counts; drawn, blended and resolved at 8 bits (alpha kept one for `B5G6R5`), and each channel
  rounded to its own bits, `round(v * (2^n - 1))` widened by repeating its bits, when a pass part
  or a resolve ends: the subresource is copied and drawn back through a rounding program, per
  sample where multisampled (packed.rs `quantise_program`, fill.rs `quantise`, draw.rs
  `close_pass`). Named slow path `SlowPath::PackedTargetQuantised` (interface commit 45a4e0b7),
  counted and warned. So `CheckFormatSupport` claims their targets, blending, mip generation,
  4 and 8 samples and resolves as the RTX 3080 does, at every level, 9_x included; their rows left
  `support_differences`. `B4G4R4A4_UNORM` stays sampled only, as the RTX 3080 has it.
- A format held but neither a target nor a depth format has no multisampled textures (no quality
  level at any count), so the multisample loads and resolves Microsoft's tables require of it are
  reported as the RTX 3080 reports them, for no texture (device.rs `served_format_support`, from
  `required::at`): `R32G32B32_*` and `B4G4R4A4_UNORM`, the requirement test's other shortfalls.
  Departures.
- Test: programs.rs `packed_targets_drawn_blended_and_resolved_as_direct3d_does` (own_cases
  `packed_targets`: both formats cleared, drawn, blended twice, drawn with 4 and 8 samples and
  resolved, and their mips generated), channel codes compared within one code (the
  specification's 0.6 of a step); passed against the RTX 3080. Under the orchestrator's rule
  (Measurements): the RTX 3080 and WARP differ in 94 of the case's 1,232 channel codes, by 1 at
  most; ours differs from the RTX 3080 in 116 and from WARP in 114, by 1 at most. Most of ours'
  extra differences are in the twice-blended rows (72 against the RTX 3080, where WARP differs by
  12): ours blended at 8 bits and rounded once, at the pass's end (since run 6 also before each
  blend reading the target, Departures).
- The seven format support comparisons pass, and the requirement test (still ignored) lacks only
  formats 88 and 89 at each level (logs/g4h2-b.log).
- The B8G8R8X8 vertex format's arms in every crate but the shaders crate are written and checked
  to build with the shaders crate's three-line arm, as a patch for the shaders track to commit with
  that arm (scratch g4h/bgrx8-arms.patch and g4h/bgrx8-shaders-arm.patch; Asked).
- The WebGPU back end's converted-formats test expected no storage use of `A8Unorm` since run 1;
  it now names the stored and drawn formats (examples/scenes/converted.rs `STORED`, `DRAWN`).

G4h run 1 (2026-11-22, from about 08:00 Pacific):
- `A8_UNORM` typed storage views are served: the WebGPU back end declares storage use of
  `A8Unorm` through the `rgba8unorm` it holds it in (packed.rs `stored`), and the front binds such
  views with `A8Unorm` in the shaders crate's `storage_formats` (its run 81 makes every store keep
  alpha alone). `CheckFormatSupport` now claims `TYPED_UNORDERED_ACCESS_VIEW`, `UAV_TYPED_STORE`
  and `UAV_TYPED_LOAD` for it as the RTX 3080 does (typed loads withdrawn in run 6). Test:
  programs.rs `a8_storage_views_store_as_direct3d_does` (stores, then sampled), against the RTX
  3080: passed.
- `required::LEVEL_REQUIRED`: Microsoft's 10_0, 10_1 and 11_0 format tables, generated (scratch
  g4h/required_rows.py from the pages' markdown in downloads/msdocs), every required bit of both
  words, the 4- and 8-sample counts, and whether the format itself is optional. programs.rs
  `every_format_requirement_of_levels_10_0_to_11_0_met_where_direct3d_meets_it` compares ours with
  it wherever the RTX 3080 meets it (the format_support case now also prints 8-sample quality
  levels). The tables agree with the RTX 3080 everywhere but P016 and NV11, optional formats it
  lacks. Ours lacks, at 11_0: the B5G6R5 target (with blending, mip generation, 4 and 8 samples,
  load and resolve), the resolves of B5G5R5A1 and B4G4R4A4, `B8G8R8X8_UNORM` vertex and typed
  buffers, the extended-range display and back buffer cast, and the `R32G32B32` formats'
  multisample loads and resolve (new: the comparison test had masked multisample bits); 10_0 and
  10_1 lack the same less what those levels do not require. Known failure (ignored) until built.
- The device's level is not yet taken from the table (Gap list); the owner's ruling is that it
  must be, once the abilities exist.
- Programs under the orchestrator's rule of 2026-11-22 (a result passes when no further from a
  real implementation than two real implementations are from each other): atg-simplecomputepc is a
  real difference (every pixel inside the Mandelbrot set has another colour; the shaders track's
  division); atg-simplemsaa_pc's frame 10 (8 pixels beyond the channel tolerance, largest 11, on
  edges; the RTX 3080's own two runs differ by 0 and 1 such pixels) is not re-judged, as WARP was
  not run on it; bgfx-26-occlusion's missing memory line is an absent feature reported absent.
  Not re-measured this run.

Agents run 3 (2026-11-21, from about 21:10 Pacific):
- The task carried the fifth check's findings once more; each is closed as this file says (the
  WebGL 2 build at 10_0 with its listed cases passing; the family library the joint step after G4
  and G5, its two modules measured apart; the 11_0 shortfalls listed as failures of the reported
  level, each waiting on a named track, with 11_0 left to the owner; `CheckFormatSupport` and
  `FORMAT_SUPPORT2` compared bit by bit with the RTX 3080 for every format at every level; 16-bit
  normalised targets held as integer twins with their test running; the minor findings as listed
  under the Gap list and Differences). The bar was run again at this run's last code commit
  (Measurements).
- The owner's interface changes merged at a00ab267 (readiness, binding settings, the negative
  route crate, the DNS answer) touch nothing this track builds or calls; both crates build and
  their tests pass after the merge.
- Literals: 66 lines of this track cut from the baseline (Literals).

Agents run 2 (2026-11-21, from about 20:00 Pacific), the eighth check's findings:
- Compute programs reading textures held as integer twins (blocking): a compute program's variant
  now takes the decoded slots (`StageVariant::decoded`) as a draw's does, so a compute shader that
  loads or samples, 2D or 3D, a normalised texture held as its integer twin (`NormAsInteger`)
  reads it decoded instead of removing the device (`COMPUTE_VARIANT_OF_DRAWS`). The same applied
  to stencil views and to a compute program asking an empty slot's size, which now take their
  draws' routes (`SwizzleInShader`, `TextureInfoInHelpers`). So `SHADER_LOAD` and `SHADER_SAMPLE`
  of these formats are served in every stage, as claimed. Tests: programs.rs
  `normalised_targets_drawn_read_and_their_mips_generated_as_direct3d_does` (each of the nine
  formats drawn into, then read by a compute program; a 2x2x2 texture of each read by one; native,
  without texture-formats-tier1 and without 16-bit norm) and `stencil` (the stencil, depth and
  empty-slot reads by a compute program), against the RTX 3080.
- Minor: the 32-bit float 3D textures filtered in the program are compared with the RTX 3080 within
  the error the specification's 8-bit filter weights allow (`float32_reads`' 3D part, clamp, wrap
  and mirror). Linear samples of the normalised formats are compared the same way, as the RTX 3080
  is not exact even at texel centres (0.44880 against 0.44882 for one `R8G8B8A8_SNORM` value).
- Minor: the WebGL 2 build refuses a swap chain on another process's window with
  `DXGI_ERROR_INVALID_CALL`, warned, as the WebGPU build does. Before, ANGLE failed to make one on the
  desktop window and the device was removed (`DXGI_ERROR_DEVICE_REMOVED`). Wine's d3d10core tests
  that crash after a refusal removed the device are named under the WebGL 2 item of the Gap list.
- The others: the 11_0 shortfalls stay with the owner; the constant buffers' counts wait on the
  WebGPU track; building ahead and B8G8R8X8's X byte are as listed; the literal check's findings
  are none of this track's.

## Wine's Direct3D 11 tests through ours

Files passing (the bar: real Direct3D's verdict for every test, except the gap list), on the RTX
3080 through the queue, not alone, at beeec184 (G4h run 5, logs/g4h5-bar*), and the same again at
54fc06fc (G4h run 7, logs/r7-bar*):
- d3d11: 152 pass of 166; the 14 failing are the Gap list's absent features. test_format_support
  and test_swapchain_formats fail no check (each runs fewer than real Direct3D, which also makes an
  11_1 device), and test_vertex_formats passes, since G4h run 4.
- d3d10core: 110 of 111: test_desktop_window (Gap list).
- d3d10core through the WebGL 2 build (10_0): 96 of 111 (logs/a3b-wine10gl.log): the 13 failing
  since round 2 run 12 and, since main run 3 reports no multisampling there,
  test_alpha_to_coverage and test_check_multisample_quality_levels; all are under the WebGL 2
  item of the Gap list.
- dxgi (not on G4's bar): 4 of the 17 tests without a Direct3D 12 part at round 2 run 8.

## Gap list (the pass bar's written list)

Everything on G4's pass bar that does not give real Direct3D's verdict on the RTX 3080, each item
once, with its cause and whose work it is. A gap stands behind a named absent feature; an ability
Microsoft's 11_0 table requires is not an absence, so those shortfalls are listed as failures of
the reported level, with the track whose part they wait on. Nothing is excluded for convenience.
The rig's exclusion of d3d11 test_create_device applies to real Direct3D's run too: its child
process makes a swap chain with `Windowed = FALSE`, which would take the owner's screen
(tools/graphics-rig/wine-excluded.txt).

Waiting on the joint step after G4 and G5 (orchestrator's ruling, 2026-11-20):
- Section 10's one family library (one DXGI shell, `D3D12CreateDevice` exported, a Direct3D 11
  program loading Direct3D 12's code). Today there are two modules, measured apart (Measurements):
  ours, `wcontainers_directx.dll` (Direct3D 10 and 11 and a DXGI), and the Direct3D 12 track's
  `wcontainers_d3d12.dll` (its own DXGI). Nothing else of section 10 is missing: the forwarders
  (d3d11.dll, dxgi.dll, d3d10core.dll), the exports of Direct3D 10 and 11, and the WebGL 2 build of
  the same source are built and tested.

The reported level is the highest whose every requirement of Microsoft's format tables is met
(G4h run 4, 4c9d43b2); the RTX 3080 device meets 11_0. What that costs, measured (logs/g4h3-t6-*
and logs/g4h4-t2-*), each the highest level met, on that back end only:
- WebGPU in a browser: 10_0 at most (reasoned from the tables and WebGPU's specification, not run
  in a browser). WebGPU's `GPUTextureDescriptor.sampleCount` takes 1 or 4 only, and 10_1 already
  requires 8-sample `B5G6R5_UNORM` targets (its "8x Multisample RenderTarget" row); 11_0 requires 8
  samples of 47 formats and 4-sample `R32G32B32A32_FLOAT` and `R32G32_FLOAT` targets, which core
  WebGPU never multisamples (texture-formats-tier1 adds no multisampling of 32-bit floats). No
  exact route to 8 samples exists (plan 4.12): two 4-sample passes run the pixel shader twice,
  which storage writes and per-sample inputs would see. Also a browser's default limits (8 vertex
  buffers, 8 storage buffers a stage, 16 inter-stage values) give 16 input slots and stage
  registers, 10_0's count (G4h run 8).
- WebGPU without texture-formats-tier1, 16-bit norm or float32-filterable (natively by
  `WEBGPU_OFF`): 10_0 since G4h run 9 (9_3 before). 10_0 requires multisample resolves of the
  32-bit float formats and of the normalised formats held as integer twins. Core WebGPU draws
  `r32float` with 4 samples but resolves no 32-bit float format, so the WebGPU back end now
  resolves every float format it multisamples and does not resolve by a draw averaging the
  samples (`ResolveByDraw`, until then used only for sRGB twins in the browser; lib.rs
  `resolved_by_draw`). The twins, `R32G32_FLOAT` and `R32G32B32A32_FLOAT` have no multisampled
  textures there (no count reported), so their resolves apply to no texture and are reported as
  for the formats never drawn into (Departures). 10_1 and 11_0 stay out of reach there: 10_1
  requires 4-sample loads and targets of the 16-bit and 8-bit signed normalised formats, held as
  integer twins and not multisampled (plan 4.12), and 8 samples (above); 11_0 also 4-sample
  `R32G32B32A32_FLOAT` and `R32G32_FLOAT` targets, which core WebGPU never multisamples.
- The WebGL 2 build: 10_0, every requirement met since G4h run 5 (the five formats it lacked,
  `A8_UNORM`, `R8G8_B8G8_UNORM`, `G8R8_G8B8_UNORM`, `B5G5R5A1_UNORM` and `B4G4R4A4_UNORM`, built in
  the WebGL 2 back end). 10_1 and 11_0 are out of WebGL 2's reach: 10_1 requires 4-sample
  targets, which need a partial sample mask WebGL 2 has no route for (OES_sample_variables and
  `SampleMaskInShader`, not built); 11_0 requires compute shaders and storage views, which WebGL 2
  has not.
- The library tests that run cases on an 11_0 device with WebGPU features off fail by refusing
  that device, and are marked known failures (G4h run 4): clear_view_without_texture_formats_tier1,
  comparisons_through_integer_twins_of_colour_views,
  float32_textures_filtered_in_the_program, normalised_targets_through_integer_twins and both
  typed_storage_stores_without_texture_formats_tier1 tests. Their routes serve no level a browser
  without those features meets. The three format support comparisons without features check that
  no device above 10_0 is made and compare at 10_0 (G4h run 9; 9_3 before).
- Absent on that back end by the feature level rule (the orchestrator's ruling of 2026-11-23,
  superseding the G4 bar item "a compute shader storing to `R11G11B10_FLOAT` and `R16G16_UNORM`
  typed views on a device made without texture-formats-tier1"): typed storage views are shader
  model 5's, 11_0 only, and a device without texture-formats-tier1 is truthfully 10_0 (above), so
  it has none. The two tests, both `typed_storage_stores_without_texture_formats_tier1` ones, stay
  known failures named for that cause; the item no longer counts against the bar. The route itself
  (`StorageFormatThroughBuffer`, `NormAsInteger`) is built and was proven against the RTX 3080
  before G4h.
- Blending into the 8-bit signed and 16-bit normalised formats where the back end does not draw
  into them is part of the second item (`BlendInShader`, the shaders crate's `combine`, Asked).

Found in G4h run 3: on the rig's hidden desktop real Direct3D's presents to a shown window answer
`DXGI_STATUS_OCCLUDED`; ours never reports occlusion (the host says nothing of it). Not on Wine's
verdicts; own_extended_range leaves that answer out.

Absent features (each reported absent through its own query, so Wine's verdicts there differ):
every failing test of Wine's d3d11 file sits behind one of these:
- The 11_1 device and Wine's WARP fallback: `D3D11CreateDevice` with 11_1 alone fails, logic
  operations have no route (plan section 9), and this library offers no WARP adapter:
  test_state_refcounting, test_logic_op, test_create_shader, test_shared_resource (its 11_1 part).
- Debug devices (`D3D11_CREATE_DEVICE_DEBUG` fails) and the 11_1 device: test_device_interfaces.
- Min/max filtering (`D3D11_OPTIONS1.MinMaxFiltering` FALSE): test_filter_minmax.
- Video (`QueryInterface` for `ID3D11VideoDevice` and `ID3D11VideoContext` answers
  `E_NOINTERFACE`, and `D3D11_CREATE_DEVICE_VIDEO_SUPPORT` is refused with
  `DXGI_ERROR_UNSUPPORTED`): test_immediate_context (1 check, the video context),
  test_video_support (1 check of 49: the hardware device with video support, which real Direct3D
  makes; real Direct3D runs 51 there; no crash since main run 2), test_h264_decoder.
- Mapping default buffers (`D3D11_OPTIONS1.MapOnDefaultBuffers` FALSE, plan section 9):
  test_resource_access (6 checks at 11_0) and the 11_1 device.
- A vertex shader's layer and viewport (`D3D11_OPTIONS3.VPAndRTArrayIndexFromAnyShaderFeedingRasterizer`
  FALSE): test_layered_rendering skips that part (2 checks); its other 72 pass.
- Presenting to another process's window (the swap chain refusal, Departures): test_desktop_window
  runs 23 checks of real Direct3D's 28 and fails none; Wine's test accepts the refusal as `broken`.
Wine's d3d10core file: test_desktop_window alone, as above.

Not on Wine's verdicts, against the RTX 3080:
- `ClearView` of float depth (`D32_FLOAT`, `D32_FLOAT_S8X24_UINT`) to a value outside 0 to 1
  (NaN among them): the RTX 3080 stores the value as given; WebGPU's depth clears refuse such
  values and its draws clamp depth to 0 to 1, and float depth textures cannot be copied or stored
  into, so no route can write it. Refused by name, which removes the device (a `ClearView` has no
  result). Needs a route the interface does not have; no program measured reaches it.

The WebGL 2 build (10_0; the bar's WebGL 2 cases match real Direct3D through it): what it does
not give real Direct3D's verdict on (G4h run 5: Wine's d3d10core 97 of 111; the rig's scenes 95
pass, 17 fail, 30 absent), each with its owner. Re-judged under the orchestrator's rule (G4h run
6, from logs/g4h5-bar-webgl.log): no scene fails by its pixels. The 17 are the pictures of three
scenes (raster, views, geometry), each of which stops at a refusal (`E_NOTIMPL`, named below:
`FillModeConverted`, `TEXTURE_VIEWS`, `SV_PrimitiveID` and layers without compute) before writing
them, so there are no pixels to set against WARP's. The 95 that are drawn pass against the RTX
3080's references.
- Multisampling: plan section 9 makes it optional at 10_0 (Microsoft's 10_0 table requires no
  sample count, so the device meets 10_0 without it), and without a route for a partial
  sample mask (WebGL 2's `gl_SampleMask` needs OES_sample_variables and `SampleMaskInShader`,
  which is not built) none is reported, so multisampled textures are refused as Direct3D refuses
  a count it does not report (main run 3; before, 4 samples were reported and a partial mask
  removed the device). Wine test_sample_mask and test_multisample_resolve, and the skipped
  multisampled parts of test_alpha_to_coverage (51 checks of 67) and
  test_check_multisample_quality_levels (21 of 43); the rig's msaa scene.
- Multisampled textures read by shaders (absent on WebGL 2, plan section 6):
  test_unbound_multisample_texture.
- A depth bias clamp on float depth, refused by name (Direct3D's float step follows each
  primitive's largest depth exponent, which a pixel program does not see): test_depth_bias.
- Wine's test crashes (exit 0xc0000005) after a refusal removed the device, in test_depth_bias
  (the clamp above), test_texture and test_render_a8 (`A8_UNORM`, below): a later creation answers
  `DXGI_ERROR_DEVICE_REMOVED` and Wine's test uses the object it was not given. The crash is the
  test's; the gap is the refusal.
- Cull distances (`CullDistanceAsGeometry` needs compute): test_combined_clip_and_cull_distances.
- Views in another format of the group (`Features::TEXTURE_VIEWS`, the WebGL 2 back end's):
  test_create_shader_resource_view, the rig's views scene.
- `A8_UNORM` and the other four formats 10_0 requires are built since G4h run 5. Of the d3d10core
  tests once listed under `A8_UNORM` (G4h run 5, logs/g4h5-bar-wine10gl.log and the copied Wine
  logs): test_create_texture2d and test_clear_render_target_view_2d now fail no check and differ
  only in fewer checks run (116 of 117, 111 of 159), their multisampled parts refused as the
  first item says; test_texture still crashes after a view in another format of the group is
  refused (`TEXTURE_VIEWS`, the item above).
- Render targets of the six 8- and 16-bit signed normalised formats: WebGL 2 draws into none
  natively without `EXT_render_snorm`, which the back end does not use; the front draws them
  through their integer twins (`NormAsInteger`), never blended. A probe lost the WebGL 2 device at
  `R16G16B16A16_FLOAT`'s staging read back ("texture copy made through a buffer", then GL error
  0x502); not traced.
- test_format_compatibility (16 checks): a 16-bit depth texture copied into an `R16_TYPELESS`
  colour texture held as `R16Float`, copied by a draw that makes half-float NaNs one NaN (0x7fff).
  The WebGL 2 track's copy route (Asked).
- test_generate_mips (56 checks, all on 3D textures' second level): the WebGL 2 back end's. The
  front makes the same `generate_mips` call on both builds; the WebGPU back end takes the
  interface's default (`generate_mips_by_draws`) and passes all 410 checks, as real Direct3D's
  run counts them, while the WebGL 2 back end takes its own `glGenerateMipmap` route for a whole
  3D view (wcontainers-gpu-webgl lib.rs, `generate_mips`). Read from the code (Asked of the WebGL
  2 track).
- Wireframe without WEBGL_polygon_mode (`FillModeConverted`, not built for any back end); pixel
  shaders reading `SV_PrimitiveID` without a geometry shader (needs compute): the rig's raster and
  geometry scenes.
- Comparisons through colour views (new this run) were not run through the WebGL 2 build.
- Wine's d3d11 file through the WebGL 2 build (not on the bar): 78 of 166 at round 2 run 12.

Real programs: the three of the pass bar (godot-platformer-3d-d3d11, chocolate-doom,
atg-simplelightingpc) match real Direct3D on every compared frame (logs/a3b-programs.log, agents run 3), and so do
atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04
to 07; of bgfx's six (logs/q2f-bgfx.log, round 4 run 2) five pass or run to their end. Outside
the bar's three, three differ from the rig's recorded frames:
- bgfx-26-occlusion: 6,458 pixels beyond the tolerance in each saved frame, all in one line of
  the overlay: real Direct3D's frames print "GPU mem: 341 MiB / 10 GiB", ours leave that line out
  (scratch m2bgfx, crops of both frames; every other pixel within the tolerance). bgfx prints it
  only when DXGI gives a memory budget (`IDXGIAdapter3::QueryVideoMemoryInfo`); our adapter is
  `IDXGIAdapter1` alone and reports memory sizes of zero, as no browser API reports memory (the
  infrastructure crate's rule). An absent feature, reported absent through `QueryInterface`; the
  occlusion queries themselves match. Found in main run 1; earlier runs thought it the counts.
- atg-simplecomputepc: the RTX 3080 itself draws ours' picture for the program's own calls, so it
  passes under the graphics bar (G4h round 6 run 1; the orchestrator's ruling counted it failing
  until explained). Inside the Mandelbrot set the colour is `frac(1200 / 300)` sampled clamped from
  its 8-texel map; the functional specification lets `div` be 2.5 ULP off, so 3.9999998 (the map's
  last texel, 255, 255, 0: ours) and 4 (its first, 64, 0, 0: the reference) are both correct. What
  the RTX 3080 gives depends on no Direct3D state: the program's calls traced with apitrace 14.0
  (scratch r6c, logs/r6c-real.log) carry the constants (1280, 720, 300, 0; 4, 2.25, -0.65, 0),
  colour map, sampler and shader bytes the isolated copies used, bit for bit. Measured on the RTX
  3080, through the queue:
  - The real program on real Direct3D with every frame 1 to 12 captured (each capture waits for
    the GPU): frames 1 to 12 show the last texel, ours' answer, and frame 10 fails its own
    reference by 154,515 pixels, ours' count; frames 30, 60, 61, 62 and 120, after frames not
    waited on, show the first (r6c/dense.sh, logs/r6c-realdense.log).
  - The trace replayed by d3dretrace: snapshots at every tenth frame, five replays, 50 of 50 the
    first texel; snapshots at every frame 1 to 12, 12 of 12 the last; at frames 8 to 11, last,
    first, first, last (r6c/replays.sh, logs/r6c-rep1-summary.txt, r6c-replay2.log).
  - Ours' frame 10 against the replay's frame 10 with the last texel: 0 pixels beyond the tolerance,
    largest difference 0; the two replays' frames 10 differ from each other by 154,515 pixels
    (r6c/cmp.py).
  - The isolated copy (r6c/scp3, the program's device, map, sampler and constants) gives the last
    texel at every read from 0 to 9 s and after 115 dispatches: time and dispatch count alone do
    not flip it.
  So which of the two allowed answers the RTX 3080 gives changes from frame to frame and run to
  run for the same calls, inside its driver: always ours' while the early frames are each waited
  on, mostly the reference's once frames go by without a wait, but not by a fixed rule (the
  replay's frame 8 gave ours' after seven frames not waited on); the program's reference frames are recorded with no capture before
  frame 10 and show the first. WARP gives the first (1,889 pixels from the RTX 3080's reference, on
  the set's edge). No Direct3D state of the program decides it, so there is no state for an
  isolated test; own_cases `colour_map_edge` keeps the isolated dispatch, which equals the RTX
  3080. Making ours give the first everywhere needs a correctly rounded division in every program
  (the shaders crate's converter), a cost on every division that no real implementation requires;
  not done. The rig still reports the program failing against its single recorded answer
  (Asked of other tracks).
- atg-simplemsaa_pc: frame 10 has 8 pixels beyond the channel tolerance (largest 11), 4 of them
  beyond their tiles' allowance; frames 60 and 120 pass. Real Direct3D's own two runs had 0 and 1
  such pixels in frame 10 (logs/q3j-sysruns.log). WARP, drawing the same program, has 25 such
  pixels (largest 21), 21 beyond their tiles' allowance (G4h run 3): ours is closer to the RTX
  3080 than another real implementation, so it passes under the orchestrator's rule of 2026-11-22;
  the rig's per-program verdict still says FAIL, as its references allow only the RTX 3080's own
  run-to-run variation.

Listed cases of section 12, each with its test against real Direct3D:
- Every listed case passes against real Direct3D on WebGPU and the WebGL 2 cases through the
  WebGL 2 build, but the two cases without texture-formats-tier1 (or 16-bit norm), which need an
  11_0 device that configuration truthfully is not (the truthful-level item above).
- Failing: constant buffers updated per draw, counted on the WebGPU back end itself (com.rs
  `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu`, ignored as a known
  failure): after each buffer's first draw, 2 more bind groups for 2 more draws, for
  `Map(WRITE_DISCARD)` and for `UpdateSubresource` alike. Cause: the WebGPU back end puts a buffer
  in its uniform arena only when `desc.usage == BufferUsage::UNIFORM` and the intent is `Often`
  (wcontainers-gpu-webgpu src/lib.rs, `create_buffer`), while the interface makes `COPY_DST` the
  condition of `write_buffer`; no buffer a front may write is uniform alone. This front declares
  constant buffers `UNIFORM | STORAGE | COPY_SRC | COPY_DST`. The WebGPU track's (Asked). The
  drawing itself matches real Direct3D (`own_handled`).
- Failing: aliased lines (`MultisampleEnable` FALSE) into a multisampled target
  (`aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does`, ignored): real
  Direct3D covers every sample of the pixels the diamond rule picks; WebGPU draws lines into a
  multisampled target by its own rule. Needs an aliased-line route (the shaders track and the
  interface).

Differences from the RTX 3080 that Wine's verdicts do not show:
- `GetDimensions` of a view of a YUY2 texture: the RTX 3080 reports the texture's width in pixels,
  ours the view's texels (pixel pairs). The interface has no helper value giving a program another
  width than its texture's (interface and shaders tracks; Asked). Known failure
  `yuy2_view_sizes_read_as_direct3d_reads_them` (ignored).
- An NT handle of a shared resource is an event object made by this library, where Windows makes
  a `DxgkSharedResource`: duplicating, closing and opening it work; its object type differs.
- Shared resources are shared between the devices of one process; a handle from another process
  is not found (`E_INVALIDARG`), where Windows opens it.
- A swap chain's back buffer used after the program released the swap chain: real DXGI keeps it
  working; ours destroyed its texture and the next view lost the device (not fixed).
- `ClearView` of an integer storage view with negative values: the RTX 3080 writes -5 into an
  8-bit unsigned channel as 0xfb and any negative value into a 16-bit signed channel as 0x7fff,
  and its whole and rectangle clears differ from each other; ours clamps to the channel as the
  functional specification (5.2.3.3) says. The listed case leaves negative storage values out.
- Target clears of integer formats convert as the RTX 3080 does (`target_integer`), which is not
  the specification's clamp for values past 32 bits (Departures).
- `ClearView` of a buffer view's rectangles, and of rectangles of a storage view held as an
  integer twin or stored through a buffer, of a 3D texture or one no pass draws into, are refused
  by name. No program measured reaches them.
- `GenerateMips` of an integer view (no `MIP_AUTOGEN` claimed by either side): ours leaves the
  levels and warns once; the RTX 3080's driver fills them by its own rule (Where things stand).
- Creation refusals answered `E_NOTIMPL` where the RTX 3080 answers `E_INVALIDARG` (both fail;
  only the code and the warning differ): render target views and mip generation of 1D, 3D and
  array textures of the R32G32B32 formats; textures of `B4G4R4A4_UNORM` bound as render targets
  of every dimension, plain 2D included (fifth check's probe12 b5); 1D, 3D, array and cube-array
  textures of `R1_UNORM` and of the video formats ours holds no texture of; shared textures of
  P016 and NV11; typed buffer views of `R1_UNORM`, the extended-range format and the video
  formats. 22 formats differ so in the eighth check's probe (probe8/out). Needs Microsoft's
  per-level table at 10_0 and up in the front; not built.
- d3d11 test_clear_buffer_unordered_access_view passes through ours, where real Direct3D fails 223
  of its 934 checks: ours clears by the documented rule; NVIDIA's driver does not.
- `IDXGISwapChain::GetContainingOutput` answers `E_NOTIMPL` (the infrastructure crate's).
- `B8G8R8X8_UNORM` and its sRGB twin: the X byte reads 0xff after a clear, a draw, a blend, a
  multisample resolve and initial data, where the RTX 3080 writes 0x00 for clears, draws, blends
  and resolves and keeps the bytes it was given (the
  WebGPU back end's `bgra8unorm`, src/packed.rs; Asked).
- Values just past a half between two codes drawn into a 16-bit normalised texture held as its
  integer twin: 6553.5006 is written 6553 by the RTX 3080 and 6554 by ours; both within the
  specification's tolerance. Filtered samples and mips of twins differ by 1 ULP at most.
- `GenerateMips` of `R8G8B8A8_SNORM`: the RTX 3080 averages the bytes as unsigned (levels of
  -0.4 and 0 give 0x66, +0.8; measured in main run 3, logs/m3e-lib.log), where `R8G8_SNORM` and
  `R8_SNORM` average the signed values; ours averages the signed values for every format, as the
  functional specification filters. The normalised targets case varies positive channels only.
- `ID3D11Device3::ReadFromSubresource` and `WriteToSubresource` are not served: they warn and
  remove the device. No valid call reaches them (`D3D11_OPTIONS2.MapOnDefaultTextures` FALSE).
- A swap chain on another process's window is refused with `DXGI_ERROR_INVALID_CALL` (Departures).
- Drawing a geometry stream past 0 is refused by name (`GEOMETRY_STREAM_DRAWN_PAST_ZERO`).
- Building ahead at a shader's arrival (plan 4.4) has never been hit. Traced in Godot 3D on main
  (a build with a print per shader made, pair tried and combination first drawn, not committed;
  scratch programs/m1dbg, main/ahead-analyse.py): it makes 98 vertex and 99 pixel shaders, mostly
  a pixel shader then its vertex shader, and draws 36 programs in 62 first-drawn combinations. The
  pairing finds 34 of the 36 drawn programs; the misses are the tuples. Of the 32 drawn pairs that
  were prepared, every one was drawn with another input layout and other strides, targets or
  binding layouts than the tuple picked, and 28 also with other blend, depth or raster objects: the
  layout Godot (through ANGLE) makes for a program is made at or after its first draw, and its
  draws do not use the most drawn tuple. Pairing each new shader with the newest unpaired one of
  the other stage would halve the pairs tried (98 for 186) with the same 34 drawn, but it never
  prepares a second pixel shader for a vertex shader already paired (front test
  `a_new_pair_is_built_ahead_with_the_states_its_vertex_shader_was_drawn_with`), so it was not
  made. The cost is about 110 to 124 background pipeline builds per Godot 3D run that are never
  used; the recorded combinations built at the next start are what serves these programs (all 61
  of Godot 3D's drawn, Measurements).

## Format support against the RTX 3080

Every bit of both words of every format, and the second query's result, compared with real
Direct3D on devices of 11_0 (`format_support_as_direct3d_reports_it_but_the_named_shortfalls`),
10_1, 10_0, 9_1, 9_2 and 9_3, and at 11_0 without the 16-bit normalised formats; the creations of
9_x line by line (`level9_make`). The differences, by cause:
- Absent features, never claimed: video (the first word's bits from `DECODER_OUTPUT` up), logic
  operations, tiled resources and multiplane overlays. Where these were a format's only
  second-word bits, ours fails the second query as Direct3D fails a word without bits.
- Multisampling per format (plan section 9's listed shortfall): claimed only where the back end
  draws a count of the format, a subset of real Direct3D's. Formats held but never drawn into
  (`R32G32B32_*`, `B4G4R4A4_UNORM`) report the multisample loads and resolves the tables require,
  with no count, as the RTX 3080 does (G4h run 2).
- Optional and served: `R32G32B32_FLOAT` is sampled (and at 11_0 gathered) by ours from 10_0.
- The 8-bit signed and 16-bit normalised formats held as their integer twins (without
  texture-formats-tier1 or 16-bit norm) claim `SHADER_LOAD` and `SHADER_SAMPLE` as the RTX 3080
  does and are read decoded in every stage, compute included (agents run 2).
- Optional video formats absent altogether: AYUV, Y410, Y416, P010, P016, Y210, Y216, NV11, AI44,
  IA44, P8, A8P8.
- Shortfalls of the reported level: none. Every requirement of Microsoft's 10_0, 10_1 and 11_0
  tables but their video columns (Departures) is met at 11_0 on the WebGPU build (G4h runs 1 to 4:
  `A8_UNORM` storage views, the B5 targets, `B8G8R8X8_UNORM` elements and typed buffers, the
  extended-range display; run 6: the B5 targets blended in one pass as stored, and `B5G6R5_UNORM`'s
  "Other Multisample Count RT", now a row; run 8: its 10_1 cell too), and of 10_0 on the WebGL 2
  build (G4h run 5); the device reports the highest level met. Beside the tables, the level check
  counts input slots and elements and stage registers (16 up to 10_0, 32 from 10_1; G4h run 8).
- Typed storage loads (`UAV_TYPED_LOAD`, optional): claimed only where served (G4h run 6). WebGPU
  loads and stores one storage texture in one program only in `r32float`, `r32uint` and
  `r32sint`, so a program that loads from a storage view of another format reads and writes it
  through a buffer of its texels (`StorageReadWriteThroughBuffer`). `A8_UNORM` goes the same way
  since G4h run 8 (the shaders crate's 5e73ebc2 reads and writes its alpha through a buffer) and
  claims typed loads, as the RTX 3080 does.
- Below 10_0, as the RTX 3080 answers: only the 10Level9 table's bits; no buffers, comparisons,
  gathers or storage; `SHAREABLE` for the 17 formats the RTX 3080 reports it for, and shared
  textures made of them but `R8G8B8A8_UNORM` and its sRGB twin (both refused there, measured in
  check-r15's probe11 share9). This copies the RTX 3080's own contradiction at 9_x: those two
  are reported shareable and then refused, and `R32_FLOAT` is made though not reported. Ours
  follows the measured answers rather than its own rule; for the owner (Departures).
- Below 11_0: sharing is claimed from 10_0 and the second query answers it alone; gathers at 10_1
  only for formats of a red channel alone, uncompressed, one aspect of depth, NV12 and YUY2;
  multisampled depth is not read at 10_0.
- `D3D11_OPTIONS2.TypedUAVLoadAdditionalFormats` TRUE at 11_0 where the set claims typed loads;
  at 10_x as the RTX 3080 answers (compared at each level).
- Every creation ours claims succeeds and every one it serves is claimed, but those where the RTX
  3080 itself claims and refuses alike (cubes of the depth formats, NV12 and YUY2 views without a
  description; the seventh check's probe). `CheckMultisampleQualityLevels` matches the RTX 3080
  for every value 0 to 200 at counts 1 to 33 at 11_0 but the absent video
  formats' single sample; at every level its 4-sample answer is compared per format
  (the same code, levels only where the RTX 3080 has them).

## Known failures

- `aliased_lines_into_a_multisampled_target_hit_every_sample_as_direct3d_does` and
  `yuy2_view_sizes_read_as_direct3d_reads_them` (programs.rs) and
  `constants_rewritten_between_draws_make_no_bind_group_per_draw_on_webgpu` (com.rs) fail on every
  build and are marked `#[ignore]` with the reason (Gap list); run them with `--include-ignored`.
- The six tests with WebGPU features off named in the Gap list's truthful-level item, marked
  `#[ignore]` with that reason.
- On the WebGL 2 build only: the library tests asking for a device above 10_0 fail by refusing
  it; the bar's WebGL 2 run takes the 10_0 cases. Of those,
  `direct3d_10s_device_draws_and_keeps_its_state_as_direct3d_does` (an 11_0 device with a 10_1
  state) is marked ignored on that build with that reason.

## What comes next

In order:
0. Values moved between a geometry stage's or domain program's pass-through and the pixel
   stage, as between vertex and pixel programs now (Departures). The tests that ask an 11_0 device
   with WebGPU features off could run their 10_0 parts on the 10_0 device without
   texture-formats-tier1 (shader model 4 forms of their cases).
1. atg-simplecomputepc is explained (Gap list); nothing left of it for this track unless the rig
   records both answers.
2. Re-judge the other programs against WARP the same way when they differ (g4h/warpshim).
3. The YUY2 view's size, once the interface and shaders tracks give the helper value (Asked).
4. Constant buffers rewritten per draw: once the WebGPU back end's arena admits uniform buffers
   with other usages (Asked), un-ignore the counters test and run it.
5. Building ahead at a shader's arrival: the measured miss is the tuple (Differences from the RTX
   3080). What could hit: build again when an input layout arrives (it names its vertex shader's
   signature), and pick tuples by the targets and binding layouts last drawn rather than the most
   drawn tuple; measure with the same trace.
6. A back buffer kept by the program past its swap chain's release: keep the chain's textures
   alive while a buffer is held.
7. Blending into integer twins (`BlendInShader`) once the shaders crate builds `combine`.
8. Creation refusals of optional abilities ours does not claim as `E_INVALIDARG` at 10_0 and up:
   needs the per-level requirement table (9_x has it since round 4 run 6).
9. Run the comparison case and the 9_x tests through the WebGL 2 build (its 9_x fallback of plan
   section 9); prove whose the WebGL 2 build's 3D second-level failure of test_generate_mips is.
10. Replace the cube array probe (`makes_cube_arrays`) with a `Caps` feature once one exists.
11. Wine's dxgi file: the null-out-pointer crashes, `CreateSurface`, `CreateSubresourceSurface`.
12. Section 10's joint step with the Direct3D 12 track, after both milestones pass.
13. The literal fix list, a part per run.

## How to run

Everything builds and runs on the desktop through the rig (docs/status-graphics-rig.md, "How to
run"); the Mac runs only formatting, git, `cargo check -j 2` of the front, and the front's tests
on the rented x86 machine. Scripts named below are in scratch (pyro-parked/wcontainers-scratch/
graphics-direct3d-11); logs go to its logs/. Since every track is on one branch (agents), the rig's own commands
would give all tracks one desktop folder (they name it by the branch), so this track runs the copies
in scratch main/ (the scripts below, run from the checkout after `source main/env.sh`), which
use scratch rigmain/: the rig with the desktop folder named `phase-2-graphics-direct3d-11`
(`WC_RIG_BRANCH`) and only committed trees sent, so other agents' unfinished edits are never built.
Commit before running anything. Do not send this checkout's tree (remote, gpu-job,
scenes) while another of its jobs waits in the queue: the waiting job then stops ("holds tree
..., not the sent ..."). Never run `tools/graphics-rig/gpu-job --help` (it queues `--help` as a
command). The desktop's checkout needs the shaders crate's submodules, so commands start with
`git submodule update --init --recursive -q`. Stopping a gpu-job on the Mac does not stop its
job on the desktop. Before a run, look at the queue with `ssh tom@192.168.2.183
"C:\wc\bin\pueue.exe -c C:\wc\pueue\pueue.yml status"`. Kill a leftover job of this track (label
`phase-2-graphics-direct3d-11: ...`) with `... pueue.yml kill <id>`.

- Work in progress through the rig without touching the shared index (G4h): `c=$(g4h/trial.sh
  [--patch <file>]... <path>...)` makes a commit of HEAD with this track's working files of the
  paths and the patches applied; then `TRIAL=$c g4h/t-run.sh <label>` (lint of both builds, the
  format support, requirement, B8G8R8X8, extended-range and swap chain tests, Wine's four format
  tests), `TRIAL=$c g4h/t-suite.sh <label>` (the library suite and the WebGL 2 build's 10_0
  cases), `g4h/x86-front.sh $c <label>` (front, unit, cache and the infrastructure crate's tests and
  clippy on the rented x86 machine) and `CMD=<command> g4h/x86-cmd.sh $c <label>` (any command
  there).
- Real programs drawn by WARP: g4h/warpshim is a test-only d3d11.dll (built on the desktop in
  C:\wc\out\phase-2-graphics-direct3d-11\warpshim) that makes every device of the system's
  Direct3D 11 on WARP; pass it to `programs run` as `--dll 'd3d11.dll=<its path>'`.
- The whole bar in one command: `q5-final.sh <label> [steps]` (the literal check, lint of both
  builds, the library suite, the WebGL 2 build's 10_0 cases, Wine's two files, scenes, programs,
  sizes and load times, building ahead, Wine's d3d10core through the WebGL 2 build). Steps:
  literals lint lib webgllib bar sizes warm wine10gl.
- Library tests matching a filter: `run-q1.sh <label> "lib=--test programs -- <filters>
  --test-threads 1"`; `r6/r6b.sh <label> "<filters>"` runs filters and then real Direct3D's own
  output of some cases.
- The literal check at HEAD (commit first): `lit-check.sh <label>`; a `-` line it prints is a
  baseline line to delete in the same commit.
- Lint: `tools/graphics-rig/remote "git submodule update --init --recursive -q && cargo clippy -q
  -j 3 -p wcontainers-direct3d-11 -p wcontainers-directx --all-targets -- -D warnings"`, and the
  same with `--features wcontainers-directx/webgl --target-dir %CARGO_TARGET_DIR%\webgl`.
- Front tests (recording back end, no GPU): `orchestrator/cloud/x86-run <clone> --name g-d3d11
  --jobs 32 -- "cargo test -q -p wcontainers-direct3d-11"`, where `<clone>` is a scratch
  `git clone --shared` of the commit to test with the submodules' files copied in (x86-run wants a
  git checkout; the worktree itself holds other agents' unfinished edits).
- Library tests (real devices and a window, so a GPU job on the hidden desktop):
  `tools/graphics-rig/gpu-job --desktop --build "<sub> && cargo test -q -j 3 -p wcontainers-directx
  --no-run" -- "cargo test -q -j 3 -p wcontainers-directx -- --test-threads 1"`; the WebGL 2 build
  with `--features webgl --target-dir %CARGO_TARGET_DIR%\webgl`. The random lifetimes comparison
  takes `WC_LIFETIMES=<first seed>,<seeds>,<steps>` (default 1,8,300); a failure names the seed.
- Wine's files through the library (build it first with `cargo build -q -j 3 -p
  wcontainers-directx`): `rigfix/wine.sh --dll 'd3d11.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'
  d3d11[:test_a/test_b]`, and `--dll 'd3d10.dll=...'` with `d3d10core`. rigfix/wine.sh is the rig's
  wine-tests with its runner fixed in one line. Wine's dxgi file: `run-dxgi.sh <label>
  [test_a/test_b]` (needs the test-only d3d10_1.dll forwarder in
  C:\wc\out\phase-2-graphics-direct3d-11\d3d10_1fwd; remake it with link.exe from its .def there).
- Scenes and programs through the whole library (our DXGI too): `--dll
  'd3d11.dll=%CARGO_TARGET_DIR%\debug\d3d11.dll' --dll 'dxgi.dll=%CARGO_TARGET_DIR%\debug\dxgi.dll'
  --dll 'wcontainers_directx.dll=%CARGO_TARGET_DIR%\debug\wcontainers_directx.dll'` on
  `tools/graphics-rig/scenes` or `tools/graphics-rig/programs run`; always the three together.
  bgfx's examples also take `--dll 'nvapi64.dll=C:\wc\out\phase-2-graphics-direct3d-11\nonvapi\nvapi64.dll'`.
- The WebGL 2 build loads ANGLE from `WEBGL_ANGLE_DIR` (default C:\wc\tools\angle-mozangle-0.7.1)
  and takes the WebGL extensions `WEBGL_OFF` lists as missing; `WEBGPU_OFF` does the same for
  WebGPU features on the WebGPU build.
- Load time and release sizes of both builds and of the Direct3D 12 track's module:
  `run-r8-sizes.sh <label>` (example `load_time`).
- src/com/generated.rs: `python3 crates/wcontainers-directx/generate.py <windows-0.62.2 crate dir>`,
  then `cargo fmt -p wcontainers-directx`. A method named in its `hand` table calls the hand-written
  one.
- If a run on the desktop does not reflect a change (its clock once moved back), run
  `tools/graphics-rig/remote "cargo clean -q -p wcontainers-directx -p wcontainers-direct3d-11"`.

## How it is built

- `Device<B: Backend>` holds the back end, the immediate context's state and caches of programs (per
  vertex and pixel shader pair), vertex layout blocks (per input layout and strides) and vertex
  shaders' input signatures (by blake3 of the bytecode). Front objects hold back-end handles in
  `Kept`, which on drop hands the handle to a graveyard emptied at the next call.
- Shaders (src/shader.rs): the stage is read from the DXBC container's version token, so bytecode of
  the wrong stage is `E_INVALIDARG`; then `convert` with the default `VariantKey`. Bytecode the
  converter rejects is `E_INVALIDARG`; what it cannot convert yet is `E_NOTIMPL`, warned with the
  converter's reason.
- Binding sets: set 0 vertex, set 1 pixel, made from the reflection when a slot or shader changed.
- Empty slots read what Direct3D defines (`Zeros` in device.rs, each made at its first use).
- Views a resource cannot be seen as natively are made on a twin of the view's format
  (`View::twin`), copied in before each use and out after each write; colour views a program
  compares through read a depth copy (`View::compared`); both are `ReinterpretCopy`.
- Format support (`Device::format_support`): the infrastructure crate's answer from the back end's
  uses, with what the front serves by its own routes, what the feature level allows, and below
  10_0 the 10Level9 table (`format::LEVEL9_SUPPORT`), which creation checks too.
- Creation checks run in the front before any back-end call and answer `E_INVALIDARG` as Direct3D
  does; what is valid but not served answers `E_NOTIMPL` with a warning. A draw that cannot be
  served removes the device; COM methods not served answer `E_NOTIMPL`, and those without a result
  remove the device, each warned once.
- The Windows library: `wcontainers_directx.dll` exports `D3D11CreateDevice`,
  `D3D11CreateDeviceAndSwapChain`, `D3D11On12CreateDevice` and Direct3D 10's entry points; the
  device, context and adapter are made with the `windows` crate's `implement`, the device children
  with our `counted!` (two counts); src/com/generated.rs is generated, each method taking the
  library lock and calling the hand-written method.
- The back end is wgpu on Direct3D 12 (`com::backend`): what browsers' WebGPU uses on Windows.
- build.rs makes d3d11.dll, dxgi.dll and d3d10core.dll with the MSVC linker from generated `.def`
  files: no code, each export forwarding to `wcontainers_directx`.

## Tests

At G4h run 10: programs.rs gains `sample_masks_without_sample_0_as_direct3d_applies_them`
(own_cases `sample_masks`), and own_cases `packed_blend_reads` also blends within one draw (a list
of ten triangles, ten instances, strips with culled odd triangles, an indexed strip with a
restart, and four half-transparent triangles over one `B5G5R5A1_UNORM` target); `packed_codes`
prints a 1-bit alpha as 00 or ff, so it is compared exactly. The WebGPU back end's unit test
`draws_split_into_their_primitives_in_order` (packed.rs).
At G4h run 8: unit 7 (shader.rs's new
`stage_values_past_the_locations_move_into_free_ones_both_stages_agree_on`); programs.rs gains
`every_input_slot_and_stage_register_as_direct3d_draws_them` (own_cases `stage_registers`: 32
input slots, 15, 30 and 31 values between stages, a draw with inputs and no input layout), and
own_cases `a8_storage` also loads its storage view.
At G4h run 5: programs.rs 70 (8 ignored); new: `small_formats_read_and_drawn_at_10_0_as_direct3d_does`
(own_cases `small_formats`). The WebGL back end's unit tests 17 (two new, format.rs).
At G4h run 4: programs.rs 69 (8 ignored, the known failures listed); the infrastructure crate's
swap chain tests gained two (extended-range buffers held and decoded; flip models refused).
New in G4h run 4: the three split feature-off tests (Known failures).
At G4h run 3: programs.rs 66 (5 ignored). New in
G4h run 3: `extended_range_swap_chains_show_as_direct3d_shows_them` (example own_extended_range);
own_cases `bgrx` also reads a typed buffer.
At G4h run 1: front.rs 60, tests/cache.rs 1, unit 6 (shader.rs 5, stream.rs 1;
wcontainers-direct3d-11, recording back end); com.rs 16 (1 ignored) and programs.rs 64 (4 ignored)
(wcontainers-directx, real devices). New in G4h run 1: programs.rs
`a8_storage_views_store_as_direct3d_does` (own_cases `a8_storage`) and the requirement test
(ignored, known failure). New in round 4 run 6: programs.rs `comparisons_through_colour_views_as_direct3d_does`
(own_cases `compare_colour`, 16-bit norm on and off), `depth_clears_outside_the_range_as_direct3d_does`
(`depth_clears`) and `format_support_and_creations_below_10_0_as_direct3d_reports_them` (9_1, 9_2
and 9_3, `format_support` and `level9_make`); front.rs
`devices_below_10_0_refuse_what_the_10level9_tables_do_not_give`. programs.rs runs each example
once beside the system's d3d11.dll and once beside ours and compares what each printed; the random
lifetimes comparison is seeded (`WC_LIFETIMES`, default seeds 1 to 8 at 300 steps each). Every
listed case of section 12 has its test against real Direct3D.

## Departures from the plan

- The infrastructure crate's two G4h pieces (extended-range swap chains, f9ecca97; the
  `B8G8R8X8_UNORM` element row, c3879851) were committed by this track (G4h run 4), though the
  crate is the Direct3D 12 track's and the task named only the interface and the two WebGPU and
  WebGL back ends. The owning track has not run since 2026-11-20 and did not answer the ask, and
  the orchestrator's ruling of 2026-11-22 says to build the missing abilities in whichever crate
  owns them. Each is a commit of its own touching only that crate, tested there (its 11 tests,
  lint) and through both fronts' builds; its status file says so.
- Multisample loads and resolves without multisampling (G4h run 4): where the device reports no
  sample count for a format, it reports the `MULTISAMPLE_LOAD` and `MULTISAMPLE_RESOLVE` that
  Microsoft's tables require of the format. No multisampled texture of it can exist there, so no
  call can rely on them. This is how Direct3D's drivers report formats they do not multisample
  (the RTX 3080 for `R32G32B32_*`), and the 10_0 table requires these bits without requiring any
  sample count. It applies on the WebGL 2 build, which has no route for a partial sample mask.
  The seventh check called this claim copied from the requirement it is checked against. Kept
  (G4h run 8), with the reason: the bits describe a multisampled texture's loads and resolves, of
  which none can be made there, so no call they allow exists to serve or fail; real drivers report
  them the same way for formats they do not multisample, and the alternative, deriving them from
  a route for textures that cannot exist, would be the same claim under another name. The owner
  is told (For the owner).
- Stage values past the back end's inter-stage locations (G4h run 8): the shaders crate places
  every value at location = register (its design, so two stages converted apart agree), and
  Direct3D numbers 32 registers, one of them the position, where wgpu on Direct3D 12 has 31
  locations (0 to 30). Where a pixel shader reads a location past the back end's, the front moves
  its inputs into order from 0, and moves the vertex shader's outputs to meet them, after
  conversion (shader.rs `input_moves`, `output_moves`, `relocate`: the naga module's bindings,
  the reflection and the content key). Only such pairs are converted again; nothing changes at
  run time. Pairs that do not fit after moving, or that also pass linked values (clip distances,
  depth clamp in the program), are refused with `VaryingsBeyondLimit`, counted and warned. Not
  moved: values passed by a geometry stage's pass-through or a domain program (their programs are
  made apart; no measured program passes 31 values there).
- Input slots and elements past the back end's (G4h run 8): wgpu on Direct3D 12 takes 16 vertex
  buffers and 30 attribute locations; an element of a slot from 16 or of an input register from
  30 is fetched in the vertex program (`VertexPulledInShader`), each from a storage buffer, so
  Direct3D's 32 of each are served. The level check counts both (device.rs
  `stage_values_served`: 16 registers up to 10_0, 32 from 10_1).
- A draw whose vertex shader reads an input the bound input layout (or none) does not give is
  dropped with one warning, as both the RTX 3080 and WARP drop it (the seventh check's slots32
  with refused layouts); before, the back end refused the pipeline and the device was removed.
- 16-bit packed targets (G4h runs 2, 6 and 10): drawn and blended in an 8-bit format and rounded
  to their bits at each end of a pass part and before every blend that reads the target
  (`PackedTargetReadByBlend` ends the pass part there), so every blend reads values the format
  holds, as on real Direct3D (the sixth check found blends reading the 8-bit values of earlier
  draws in the same pass, the ninth those of earlier primitives of the same draw). WebGPU has no
  5- or 6-bit format and no blending that reads a target in the program, so a draw of several
  primitives with such a blend is drawn one primitive at a time, instance by instance, a pass
  split and a rounding between each (`packed::pieces`; a strip's odd triangles with front faces
  reversed, as the strip reverses them); indirect arguments, and a strip's indices where it
  restarts, are read back first, which waits on the GPU. Draws without such a blend are not split:
  a texel they write is rounded later, which gives the same value. Each primitive of such a blend
  costs a pass split. Rounding from 8 bits adds up to `(2^n - 1) / 510` of
  a step to exact rounding: 0.56 of a step for 5 bits, 0.62 for `B5G6R5_UNORM`'s 6-bit green,
  0.02 past the specification's 0.6. Judged under the orchestrator's rule (seventh check's case
  rounding): the RTX 3080 itself rounds to 8 bits and drops the low bits (up to a whole step off:
  0.85295 gives 27 where WARP gives 26), WARP rounds exactly, and ours is within one code of both
  on every channel; in 6 of the check's 24 channels ours gave a code neither gave, by 1. A held
  format wider than 8 bits would remove it, but blending into a float twin would not clamp as a
  normalised target does; not built.
- Multisample loads and resolves of formats with no multisampled textures (G4h run 2): reported
  where Microsoft's tables require them, though no sample count of the format exists, exactly as
  the RTX 3080 reports them (`R32G32B32_*`: load from 10_1, the float one's resolve from 10_0;
  `B4G4R4A4_UNORM`'s resolve). Nothing can use them, so nothing is claimed that is not served; the
  other reading, building multisampled `R32G32B32` textures, would claim counts the RTX 3080
  itself refuses (and core WebGPU multisamples no `rgba32` format). For the owner too. Since G4h
  run 4 the same holds where the device reports no multisampling at all (the WebGL 2 build), and
  since run 9 for every format of which the device draws no count (device.rs `no_samples`: no
  `MULTISAMPLE_RENDERTARGET`), which on a back end without texture-formats-tier1 are the integer
  twins and the 32-bit float pairs and quadruples; `CheckMultisampleQualityLevels` answers no
  count of each.

- Integer target clears follow the RTX 3080 where it departs from the functional specification's
  clamp for values past 32 bits (`target_integer`); storage views keep the clamp
  (`storage_integer`), which the RTX 3080's storage rectangles follow.
- Below 10_0, format support and creation follow the runtime's 10Level9 answers measured on the
  RTX 3080 (`LEVEL9_SUPPORT`): Microsoft's 9_1 and 9_3 tables with the driver's optional rows, and
  for 9_2, for which Microsoft has no table, the measured answer alone.
- Shared resources are shared between the devices of one process only, each device holding its own
  copy, copied through the host when a device flushes or releases the keyed mutex
  (`SharedThroughHost`); a copy is made only where the bytes differ from the last hand-over. NT
  handles are event objects (`OpenSharedResource1` finds a share by `CompareObjectHandles`).
  Multisampled shared textures are refused by name.
- 420_OPAQUE is held as NV12's two planes and refused with bind flags, CPU access or another usage
  than default, as the RTX 3080 refuses them. NV12 is two textures (luma `r8unorm`, chroma
  `rg8unorm` at half size); YUY2 is held at half width; one level each (more answer `E_NOTIMPL`).
- `R10G10B10_XR_BIAS_A2_UNORM` textures are held in `Rgb10a2Unorm`'s bits; no view of the format is
  made, as on the RTX 3080.
- Normalised clear values of integer twins round halves toward zero, as the RTX 3080 clears.
- 16-bit normalised textures held as integer twins are blended nowhere (refused by name) where plan
  4.12 blends them in the program (`BlendInShader`, refused by the shaders crate).
- The level reported is the highest whose every requirement of Microsoft's format tables is met
  (section 9's rule), but for their video columns: video is reported through its own interface
  (`ID3D11VideoDevice`), which a device of any level may lack (`D3D11_CREATE_DEVICE_VIDEO_SUPPORT`
  fails creation where the driver has none), and ours serves none (Gap list, Absent features), so
  `VIDEO_PROCESSOR_OUTPUT` and the other video bits are never claimed. The RTX 3080 claims
  `VIDEO_PROCESSOR_OUTPUT` of the formats the 10_0 and 11_0 tables mark (10, 24, 28, 29, 87, 88,
  and 89 at 11_0). required.rs says so; its generator (scratch g4h/required_rows.py) names why it
  skips them.
- A swap chain on another process's window is refused with `DXGI_ERROR_INVALID_CALL` (warned): the
  WebGPU back end presents through a flip-model swap chain, which presents only to the process's
  own windows.
- The names of Windows' runtime libraries and entry points live in the front crate
  (`wcontainers_direct3d_11::runtime`), as the example own_handled must name them and cannot link
  the library crate.
- Wine's dxgi file runs with a test-only d3d10_1.dll forwarder made by hand on the desktop;
  d3d10.dll and d3d10_1.dll are not built as forwarders (they also export the effects framework,
  shader compiling and reflection, which this library does not serve).
- The library reads `WEBGPU_OFF` beside `WEBGL_OFF`, so routes browsers without a feature take are
  tested natively against real Direct3D.
- The feature level check probes the back end for cube arrays at device creation (no `Caps` feature
  names them); a refusal stops the level at 10_0, which is how the WebGL 2 build reports 10_0.
- Typed buffers read through texture copies copy the whole buffer into a texture of each view
  format the first time a draw reads it after a write.
- Aliased lines drawn into multisampled targets by the back end's rule are not compared with
  Microsoft's conformance tolerance (no such comparison exists; Gap list).
- Depth bias applies by the primitives rasterised: lines and points are not biased, as on the RTX
  3080 (plan 5.1 says `DepthBiasInShader`); a geometry stage's biased lines and points are refused.
- A stage reading more constant buffers than the back end's uniform buffers keeps one uniform buffer
  for the helper constants whenever its variant may have them.
- A geometry shader's primitive naming a layer past the bound view's is drawn nowhere, where real
  Direct3D draws it into the first layer (Asked of the shaders track).
- A Direct3D 10 device made without an adapter answers its adapter's `GetParent` with DXGI 1.0's
  factory; `CreateDXGIFactory` gives DXGI 1.0's factory only when asked for the 1.0 interfaces.
- `EarlyTestsPerPrimitive` is built for list topologies, one instance, direct draws only; the rest
  is refused by name.
- Direct3D 10's device converts each object it is handed by `QueryInterface`; the system's d3d10.dll
  and d3d10_1.dll calling `D3D10CoreCreateDevice` get ours; in Direct3D 10 mode the final release of
  a bound resource does not unbind it; `CreateGeometryShaderWithStreamOutput` gives the one buffer
  its `OutputStreamStride` (not compared with real Direct3D).
- `TargetsSplit` runs each group's draw in a pass of its own; queries, pixel storage views and stream
  output during such a draw are refused by name.
- An over-release answers 0xffffffff and leaves the counts as they were.
- A multisampled texture of more than one layer is refused (`E_NOTIMPL`) instead of held as a
  texture per layer (plan 4.2).
- Stencil views always move the stencil in the program (`SwizzleInShader`): `TextureViewDesc` has no
  swizzle.
- Programs taking `AtomicExchangeLoop` are refused at creation after one reset the display driver.
- Passes whose depth buffer is larger than their colour targets are begun through the pass tracker;
  the front's other passes are still its own (section 4.7 has every pass on the tracker).
- The examples compile their shaders with the system's d3dcompiler_47.dll at run time.
- An 8- or 16-bit integer vertex element read with the other signedness is fetched in the program
  (`pulled`, `VertexFormatConverted`).
- `IDXGISurface1::GetDC` is a memory DC made as DXGI makes it (`D3DKMTCreateDCFromMemory`), filled
  through a staging copy and written back whole at `ReleaseDC`; no slow path names it.
- A render target view of a buffer is a view of a one-row texture of its elements, copied in at a
  pass's start and out at its end (`ReinterpretCopy`).
- A uint clear of a view of floats is written as the texels' bytes from the host.
- A storage view of some slices of a 3D level is made on a texture of those slices (`View::window`).
- Direct3D 11.3's interfaces are served; tiled resources and conservative rasterization are not
  (not reported); `Flush1` with an event waits on the calling thread.
- bgfx's examples are run with an empty nvapi64.dll beside them, so NVIDIA's NVAPI does not load and
  take our device for its driver's (the Direct3D 12 track does the same).
- The native cache file lives in the directory `WC_GPU_CACHE` names, one per program; without it
  every run is a first run. Building the record happens at the process's first device only.
- Views on twins and storage textures read through buffers are copied at every use, without
  tracking which copy is newest (correct, slow, counted).
- A stream output target's filled size lives in a 4-byte GPU buffer; `DrawAuto` reads it back.
- The stage helper's buffers and programs are kept per (vertex shader, input layout, geometry
  shader, pixel shader) for the device's life.
- A texture view whose kind differs from its slot's declaration reads as an empty slot, zeros, as
  real Direct3D answers in Wine's test_generate_mips.
- A cube texture whose layers are not whole cubes is a 2D array in the back end.
- A constant buffer window's copy is counted as `ConstantBufferGrown`.
- `EnumAdapterByLuid` with another identifier answers `DXGI_ERROR_NOT_FOUND`;
  `IDXGIFactory3::GetCreationFlags` answers 0.
- With only d3d11.dll replaced, the device made on a system adapter reports our own adapter.
- Depth bias in the program is refused on float depth formats where the back end lacks the clamp.
- Building ahead's counts are printed at each present where they changed.
- `D3D11CreateDevice` takes only `D3D11_SDK_VERSION`.
- Vertex buffers get storage usage on back ends without `INSTANCE_STEP_RATE`; every default-usage
  2D texture of a format that multisamples and resolves gets render usage (a resolve's destination).
- `LodBiasInShader` differs by one unit at 4 of 256 texels (measured); biased rows are compared
  within one unit.
- Pipeline statistics' vertex shader invocations equal the vertices fetched; stream output
  statistics answer zero.
- A sampler whose `MinLOD` is above its `MaxLOD` is made with the maximum raised to the minimum.
- Timestamps are answered with `Disjoint` TRUE and CPU times (no back end writes GPU timestamps).
- `CheckMultisampleQualityLevels` and `CheckFormatSupport` are served through a copy of the
  device's function table with those two entries replaced.
- An indirect draw whose vertex program reads `SV_VertexID` or `SV_InstanceID` has its arguments
  read back (`IndirectReadBack`); the shaders crate refuses `DrawBaseFromArguments`.
- The draw bases' helper block is kept between draws with equal bases, the front being the only
  writer of the back end's data ring.
- Building ahead pairs shaders and fits tuples by location and scalar type.
- DXGI's factory entry points and `D3D11CreateDevice` decide by the caller's module whose library a
  call wants (no device thread exists to mark).
- Comparing through a sampler that does not compare (and the reverse) binds a twin sampler.
- The WebGL 2 build is a cargo feature (`webgl`) choosing the back end type.
- No call buffer or device thread: COM calls run on the calling thread under one re-entrant lock.
  Plan 15.1 moves the channel, its encoder and this library's device thread to the second batch
  (G7), where the three fronts' shells move onto it together. So section 13's cost of the creation
  checks per shader is not measured apart: here a check and its shader's conversion are one call
  on the program's thread, with nothing to separate them from.
- Device children are made by our `counted!` macro, not `implement`, for Direct3D's two counts;
  windows-core is pinned to exactly 0.62.2 for its object layout.
- The recording back end is in the front crate (`record.rs`), not G1's wcontainers-gpu-test.
- State objects are kept by the device for its life once made.
- `CheckFeatureSupport` answers `D3D11_FEATURE_THREADING` with both FALSE.
- The feature level is 11_0 at most.
- The adapter reports zero for its memory sizes (the infrastructure crate's rule).
- `D3D11CreateDevice` with an adapter uses ours whatever is passed; the debug flag answers
  `DXGI_ERROR_SDK_COMPONENT_MISSING`; software, WARP, reference and null drivers, and
  `D3D11_CREATE_DEVICE_VIDEO_SUPPORT` (no video device is served), `DXGI_ERROR_UNSUPPORTED`.
- `D3D11_OPTIONS.ClearView` is TRUE: target, depth and storage views, whole or by rectangles;
  storage rectangles through the shaders crate's storage clear program; 3D target rectangles copied
  into each slice from a cleared slice.
- The library draws through wgpu on Direct3D 12 only.

## For the owner

- Truthful levels cost the browsers: WebGPU allows 1 or 4 samples only, so a browser's Direct3D 11
  is 10_0 at most: 10_1 already requires 8-sample `B5G6R5_UNORM` targets, and 11_0 8 samples of 47
  formats and 4-sample 32-bit float targets core WebGPU never multisamples. No exact route to 8
  samples exists (plan 4.12: two 4-sample passes run the pixel shader twice, which storage writes
  see). Without texture-formats-tier1 it is 10_0 too since G4h run 9 (resolves of 4-sample
  `R32_FLOAT` targets by a draw); the WebGL 2 build is 10_0 (10_1 needs 4-sample targets,
  which need a partial sample mask WebGL 2 has no route for). Programs requiring a higher level
  then refuse to start there, as on hardware without the level. Natively on the RTX 3080 the
  device is 11_0.
- atg-simplecomputepc is explained (Gap list): the real program on real Direct3D on the RTX 3080
  draws ours' picture, pixel for pixel, in frames 1 to 12 when each is captured (waited on), and
  the reference's in later frames after frames not waited on; the two allowed results of the
  specification's 2.5-ULP division, and no Direct3D state of the program decides between them.
  Counted passing under the graphics bar from G4h round 6 run 1; the rig's recorded frames hold only
  one of the two, so its comparison still says FAIL.
- Video is read as no part of a feature level (Departures): the tables' video columns are not
  required of ours, which serves no video device. If you read them as required, 10_0 and above
  need a video processor (`ID3D11VideoDevice`, `VideoProcessorBlt`) built first.

- `R32G32B32_*` and `B4G4R4A4_UNORM` report the multisample load and resolve bits Microsoft's
  11_0 table requires while reporting no sample count, as the RTX 3080 does (Departures). The
  WebGL 2 build, which makes no multisampled texture at all, does the same for every format at
  10_0 (G4h run 4; without it that build would be 9_3): the bits promise nothing a program can
  use, as no multisampled texture of the format can be made. For these formats the level check
  cannot find the two bits missing (device.rs `no_samples` takes them from the same table), so the
  level there rests on this reading, not on a check. If you read the tables as requiring
  real multisampled textures of them, say so: then 11_0 needs those textures built (natively on the
  RTX 3080 through `rgba32` twins; no route in core WebGPU), and the WebGL 2 build is 9_3.

- Sharing at 9_x copies the RTX 3080's own contradiction: `R8G8B8A8_UNORM` and its sRGB twin are
  reported shareable and refused when made, and `R32_FLOAT` is made though not reported
  (`format::level9_shared`, measured). The other choice is to report only what ours makes, which
  would then differ from the reference. Yours to decide.

## Interface changes

- 8a714142 "interface: named the slow paths of a packed target read by a blend and of a storage
  texture loaded and stored through a buffer": `SlowPath::PackedTargetReadByBlend` and
  `SlowPath::StorageReadWriteThroughBuffer`, at the list's end (G4h run 6).
- 45a4e0b7 "interface: named the slow path of packed targets drawn wider and rounded to their
  bits": `SlowPath::PackedTargetQuantised` (G4h run 2).

- 116c369d and ff482b63 "interface: named the slow path of a pass split where its query set has no
  index left" (and its wording): `SlowPath::QuerySetSplitPass`, and one clause in plan 4.7.
- 2f3b3263 "interface: named the faces of a cube once as CUBE_FACES": `wcontainers_gpu::CUBE_FACES`,
  which this front and the NIR track's crate had each defined (the literal check's redefinition
  once both branches were on main); the front now imports it.

- 978d1661 "interface: named the slow path of a resource shared between devices copied through the
  host": `SlowPath::SharedThroughHost`, and one sentence in plan section 9.
- 8b556a05 "interface: named each normalised format's integer twin as Format::integer_twin and read
  generate_mips' twins from it".
- c8a7be9 "interface: copied combined depth-stencil textures whole in copy_texture".
- 7f73347 "interface: named the slow path of a constant buffer grown to its shader's declared size,
  and said a depth attachment holds both aspects".

## Measurements

G4h round 6 run 1 (2026-11-23, 09:50 to 10:10 Pacific; desktop i9-9900K with the RTX 3080, driver
32.0.16.2101, through the queue in the hazard group, one job at a time, after the OpenGL track's
conformance run):
- atg-simplecomputepc's middle pixel (inside the set) per frame, F the colour map's first texel
  (the reference), L its last (ours): real Direct3D with frames 1 to 12, 30, 60 to 62 and 120
  captured: LLLLLLLLLLLL FFFFF (logs/r6c-realdense.log, r6c-dense-summary.txt). Its trace replayed
  by d3dretrace five times with snapshots at frames 10 to 100 by tens: 50 F; once with snapshots at
  frames 1 to 12: 12 L; once at frames 8 to 11: L F F L (logs/r6c-rep1-summary.txt,
  r6c-replay2.log). Through ours (frames 10, 60, 120): 154,515 pixels beyond in each, as before
  (logs/r6c-o1.log); ours' frame 10 against the replay's L frame 10: 0 beyond, largest 0.
- The isolated copy r6c/scp3: L at 0.008 s, after 101 dispatches, at 3 s and 9 s
  (logs/r6c-replay1.log).

G4h run 9 (2026-11-23, Pacific; desktop i9-9900K with the RTX 3080, through the queue, not alone,
the OpenGL track's conformance runs holding the queue's hazard group for most of the run):
- The whole bar at 968aef83 (main/q5-final.sh r9-bar, logs/r9-bar-*): lint of both builds clean;
  library com 15 (1 ignored) and programs 67 (8 ignored) pass, the run-9 test among them. The
  later steps (the WebGL 2 build's cases, Wine's files, scenes, programs, sizes, building ahead)
  were still queued behind the OpenGL track's job at 03:45 when this run ended; the detached run
  goes on writing logs/r9-bar-*. Run 8's whole bar (below) is the last complete one.
- Front 60, unit 7, cache 1 and the infrastructure crate's 11 tests pass at 6572e2e9's tree (the
  desktop, as the rented x86 machine was being replaced); lint of the front, the infrastructure
  crate and the WebGPU back end clean at e295cb5a's tree (rented x86); the WebGPU back end's own
  tests pass but `wireframe_fill_matches_direct3d`, the WebGPU track's known failure.
- The literal check at e295cb5a (rented x86, logs/r9-lit-front.log): nothing in this track's
  files but an "unparsed" line for the deleted src/required.rs, which no tree tracks (the remote
  copy's stale file list); the `SUPPORT2_SHAREABLE`, `CPU_LOCKABLE`, `MIP_AUTOGEN` and level
  redefinitions the infrastructure crate's new constants made are fixed (505618c1) and the old
  resolve refusal's baseline line cut (e295cb5a).
- atg-simplecomputepc's dispatch copied whole (g4h9/scp2): the RTX 3080 gives (255, 255, 0) inside
  the set in all 8 combinations of colour map, sampler and device, in both sessions and through
  the forwarder (logs/r9-scp2b.log, r9-scp2c.log, r9-scp2fwd.log).

G4h run 8 (2026-11-22 and 23, Pacific). The whole bar at c1abc9c4 (this track's code at
d0422d34; main/q5-final.sh r8-bar, logs/r8-bar-*). Machine: the desktop (i9-9900K, RTX 3080),
through the queue, not alone, other tracks' jobs queued (an OpenGL conformance run and the shaders
track's tessellation tests).
- Lint of both builds: clean. Front 60, unit 7, cache 1 and the infrastructure crate's 11 tests,
  and clippy, pass on the rented x86 machine (trials 7a999bc2 and the code of d0422d34).
- Library: com 15 pass (1 ignored), programs 66 pass (8 ignored): run 7's 65 and
  `every_input_slot_and_stage_register_as_direct3d_draws_them`. Before the last fix that test
  failed (logs/r8-t1-lib.log): slots past 16 were also bound as vertex buffers, which wgpu refused
  ("Vertex buffer index 16 is greater than the device's requested `max_vertex_buffers` limit
  16"); d0422d34 binds only the slots the back end fetches. Its 32-slot draw, 31 values and the
  dropped draw without a layout now print what the RTX 3080 prints, byte for byte.
- The WebGL 2 build's 10_0 cases: 7 pass, 1 ignored.
- Wine d3d11: 152 of 166; d3d10core: 110 of 111; d3d10core through the WebGL 2 build: 97 of 111.
  The failing tests are the same as at run 7 (compared by name), all in the Gap list.
- Scenes: 137 pass, 0 fail, 2 absent. Through the WebGL 2 build: 95 pass, 17 fail, 30 absent, as
  before.
- Programs: as at run 7. The bar's three and eight more pass; atg-simplemsaa_pc frame 10 has 8
  pixels beyond (largest 11; WARP 25, largest 21), passing under the orchestrator's rule;
  atg-simplecomputepc 154,515 pixels beyond in each frame (WARP 1,889), failing (For the owner);
  bgfx as before (bgfx-26-occlusion's memory line, an absent feature).
- Literal check (rented x86 machine, at 1531751a's tree): nothing in this track's crates; three
  recorder baseline lines cut.

G4h run 7 (2026-11-22, Pacific). The whole bar at 54fc06fc (q5-final.sh r7-bar, logs/r7-bar-*).
Machine: the desktop (i9-9900K, RTX 3080), through the queue, not alone unless noted, with other
tracks' jobs queued.
- Literal check: nothing in this track's crates. The webgl back end's `clear_buffer::BLOCK` line
  is new because the kernel added `ECHO_BLOCK`; that line belongs to the WebGL track. At 39658541
  (rented x86 machine, logs/r7-lit2-front.log): nothing in this track's crates.
- Lint of both builds: clean. Front 60, unit 6, cache 1 and the infrastructure crate's 11 tests,
  and clippy, at 39658541's code (trial d0f0585e on the rented x86 machine): pass.
- Library: com 15 pass (1 ignored), programs 65 pass (8 ignored). The WebGL 2 build's 10_0 cases:
  7 pass, 1 ignored.
- Wine d3d11: 152 of 166 pass. The 14 failures are the same tests as at run 5, all in the Gap
  list. d3d10core: 110 of 111 (test_desktop_window).
- Scenes through the library: 137 pass, 0 fail, 2 absent. Through the WebGL 2 build: 95 pass, 17
  fail, 30 absent, as before. Wine's d3d10core through the WebGL 2 build: 97 of 111 pass. The 14
  that fail are the same tests as at run 5, all listed under the Gap list's WebGL 2 item.
- Programs: the eleven pass as before. atg-simplemsaa_pc frame 10: 8 pixels beyond (largest 11), 4
  beyond their tiles' allowance, as before. atg-simplecomputepc: 154,515 beyond in each frame, as
  before. bgfx: five of six pass or run to their end, and bgfx-26-occlusion fails on its memory
  line, as before.
- Release modules (built from 54fc06fc's tree): `wcontainers_directx.dll` is 14,581,760 bytes on
  WebGPU and 10,154,496 on WebGL 2. The Direct3D 12 track's `wcontainers_d3d12.dll` is 13,983,744.
  Load times, 20 runs each, `gpu-job --alone`, 18:06, given as min / median / max: the system's
  d3d11.dll 1,122 / 1,185 / 1,425 us; ours on WebGPU 1,575 / 1,688 / 2,585 us; ours on WebGL 2
  1,059 / 1,137 / 1,910 us; the Direct3D 12 module 1,586 / 1,691 / 2,679 us.
- Building ahead (r7-bar-warm, 18:34 to 18:44; two warm runs of five programs): Godot 3D built its
  61 recorded combinations at start and drew all 61 by the end. It built up to 124 more ahead and
  drew none of them, and built 26 at their first draw. The other four programs built their
  recorded combinations at start and drew them. All ten runs pass against real Direct3D. This is
  the same as agents run 3; the miss is still the tuple (item 5 of What comes next).

G4h run 6 (2026-11-22, Pacific). Logs in scratch logs/r6*; the desktop (i9-9900K, RTX 3080),
library tests through the queue, not alone, other tracks' jobs queued.
- Trial 3 (g4h/r6-trial3.commit, r6c): lint of the interface crate, both back ends, the
  infrastructure crate, the front and the library, both builds, clean; the interface crate's and
  the front's tests pass (front 60, unit 6, cache 1 after the recording back end took two samples
  too); the library's `packed_targets`, `typed_storage`, `colour_map`, `format_support`,
  `every_format_requirement`, `a8_storage` and `extended_range` tests, 15 pass, 2 ignored (as
  before), against the RTX 3080. The WebGPU back end's own `raster` test
  `wireframe_fill_matches_direct3d` fails (214 pixels, on Vulkan) at HEAD 2338bf61 as at the
  trial (r6d-raster-head.log): not this change's; the WebGPU track's test.
- Trial 4 (the init conversion's dead read-write retry dropped): lint of the front and the
  library and the front's tests pass. The whole bar at aa99df0f (q5-final.sh r6-bar): the
  literal check found two new findings of this run (the fractal case's 300.0, fixed in 196fca6a)
  and one baseline line to cut (cut, 457ff8a1); lint of both builds clean; the library suite then
  waited in the queue's hazard group from 15:24 to past 17:20 behind another track's job
  (a `cargo test -p wcontainers-shaders --no-run` held since 15:47), so the whole library suite,
  Wine's files, scenes and programs were not run again this run.
- atg-simplecomputepc through ours at trial 1: 154,515 pixels beyond, as before; real Direct3D
  through the rig at 14:40: passes, 0 beyond. The replicas (own_cases `colour_map_edge`): real and
  ours both give 0x3f7ffffc and the colour map's last texel, bit for bit (r6a-*; Gap list).
G4h run 5 (2026-11-22, Pacific). Logs in scratch logs/g4h5-*; the desktop (i9-9900K, RTX 3080)
through the queue, not alone, other tracks' jobs queued; the rented x86 machine (96 cores, load 0
to 5).
- The WebGL back end's lint (warnings denied) and unit tests, 17 with the two new: pass (x86,
  g4h5-b). The literal check at the trial: no finding in this track's crates or the WebGL back
  end; eleven baseline lines its change removed were cut (c59e2d87).
- Trial fd1cce9b (g4h5-t3): lint of the front, the library and the WebGL back end, both builds,
  clean; `small_formats_read_and_drawn_at_10_0_as_direct3d_does` passes on the WebGPU build; the
  WebGL 2 build's 10_0 cases, the new case among them, 7 pass, 1 ignored (the 10_1-state case, as
  before); the WebGL back end's own suite on ANGLE natively, 105 pass. An earlier trial (g4h5-t1)
  had the WebGL 2 build's six earlier 10_0 cases passing (0 of 6 at run 4); the new case then
  failed at `B4G4R4A4_UNORM`'s first texture copy ("copies of these formats" not built: GL copies
  only formats reported as targets), fixed in fd1cce9b.
- At beeec184 (the bar, q5-final.sh g4h5-bar): front 60, unit 6, cache 1, infrastructure 11 and
  clippy pass on x86 (g4h5-front); lint of both builds clean; library com 15 (1 ignored) and
  programs 62 (8 ignored) pass; the literal check finds nothing in this track's crates, the
  infrastructure crate or the WebGL back end (its 100 lines are other tracks'). The WebGL 2
  build's 10_0 cases 7 pass, 1 ignored. Wine d3d11 152 of 166 (the 14 failing are the Gap list's
  absent features; test_vertex_formats passes since run 4), d3d10core 110 of 111
  (test_desktop_window). Scenes through the library 137 pass, 0 fail, 2 absent; the WebGL 2
  build's 95 pass, 17 fail, 30 absent (unchanged). Programs: the same eleven pass; bgfx five of six
  as before (bgfx-26-occlusion's memory line); atg-simplemsaa_pc frame 10 has 8 pixels beyond
  (largest 11), 4 beyond their tiles' allowance, as before (WARP: 25, largest 21: passes under the
  orchestrator's rule); atg-simplecomputepc 154,515 pixels beyond in each frame (largest 255; WARP
  1,889), the real difference waiting on the shaders track's division.
- Release modules (the sizes step built at b1302c24, the HEAD then): wcontainers_directx.dll
  14,573,056 bytes on WebGPU and 10,149,888 on WebGL 2; the Direct3D 12 track's
  wcontainers_d3d12.dll 13,979,648. Load times (gpu-job, not alone, 13:10), each of the last
  four runs: the system's d3d11.dll 1,118 to 1,126 us; ours on WebGPU 1,579 to 1,615 us; on
  WebGL 2 1,036 to 1,112 us; the Direct3D 12 module 1,563 to 1,686 us.

G4h run 3 (2026-11-22, Pacific). Logs in scratch logs/g4h3-*; the desktop through the queue, not
alone; the rented x86 machine (96 cores, load 0 to 5).
- Before any change, the requirement test at 14519166 on the RTX 3080: formats 88 and 89 short at
  10_0, 10_1 and 11_0 (g4h3-a); with texture-formats-tier1, 16-bit norm and float32-filterable off:
  12 formats short of 10_0 (multisample resolves), more at 10_1 and 11_0 (g4h3-c).
- Trial commits (g4h/trial*.commit; HEAD plus this track's files and the patches):
  t1, the extended-range patch alone: lint clean, the infrastructure crate's tests pass, the seven
  format support comparisons and the swap chain test pass, the requirement test finds format 88
  alone short. t4: front 60, unit 6, cache 1, infrastructure 11 and clippy pass on the x86
  machine. t5 (all patches): lint of both builds clean; format support comparisons, requirement,
  B8G8R8X8 and swap chain tests pass; Wine test_vertex_formats 100 checks and test_buffer_srv 234
  as real Direct3D; test_format_support and test_swapchain_formats fail no check (each skips the
  11_1 level as before); d3d10core's two pass. t6 (all patches, the extended-range test's final
  form): library suite 55 pass, 9 fail (the six feature-off tests refusing their 11_0 device, and
  the three feature-off comparisons in their earlier form, fixed after), 2 ignored; the WebGL 2
  build's 10_0 cases 0 of 6 (each refused a 10_0 device: 186 shortfall lines, logs/g4h3-t6-webgl).
  The extended-range test passed at t6 (g4h3-t6-suite). The Direct3D 12 crate passes clippy and
  its tests build with the infrastructure patches (x86, g4h3-d12).
- Programs on WARP (g4h3-warp-programs, 11:30): atg-simplemsaa_pc frame 10: 25 of 921,600 pixels
  beyond 8 (largest 21), 21 beyond their tiles' allowance, 10 tiles; frame 60: 16 (largest 37),
  7; frame 120: 16 (largest 79), 7. atg-simplecomputepc, each frame: 1,889 beyond (largest 255),
  256 tiles.
- The committed state (t7 = fdbdb281's tree): front 60, unit 6, cache 1, infrastructure 10 and
  clippy on the x86 machine; lint of both builds; the format support, swap chain and extended-range
  material 9 pass, 3 ignored; Wine as before the run (test_format_support and
  test_swapchain_formats 1 check each, test_vertex_formats 98 of 100).

G4h run 2 (2026-11-22, Pacific). Logs in scratch logs/g4h2-*; trial trees sent as commits made by
g4h/tree.sh (HEAD plus this track's working files, in a private index), then committed.
- Lint, warnings denied, of the interface, the WebGPU back end, the front and the library, both
  builds, on the desktop: clean (g4h2-c). The interface's 37 and 8 tests pass; the WebGPU back
  end's tests through the queue on the RTX 3080 (Vulkan): every binary passes but raster's
  `wireframe_fill_matches_direct3d`, the WebGPU track's known Vulkan wireframe difference, which
  this change does not touch (g4h2-d). Front 60, unit 6 and cache 1 on the rented x86 machine (96
  cores, load 0 to 1): pass (g4h2-front).
- Desktop, through the queue, not alone: `packed_targets_drawn_blended_and_resolved_as_direct3d_does`,
  the seven format support comparisons and the requirement test (ignored, run with
  `--include-ignored`): 8 passed, the requirement test failing on formats 88 and 89 alone
  (g4h2-b).
- The bar at 27ffc315 (q5-final.sh literals lib bar; logs/g4h2-bar-*), desktop
  through the queue, not alone: the library suite passes; Wine d3d11 151 of 166 (the same 15
  failing; test_format_support down from 7 failing checks to 1), d3d10core 110 of 111; scenes
  through the library 137 pass, 0 fail, 2 absent (135 and 4 absent before); the WebGL 2 build's
  scenes 95 pass, 17 fail, 30 absent (unchanged); programs: the same eleven pass and
  atg-simplecomputepc and atg-simplemsaa_pc differ as before. The literal check at f67240a3: no
  finding in this track's crates or the WebGPU back end (one stale baseline line of the WebGPU
  copy program dropped, 9c7ade41).
- After the rounding program's literal fix, at 97a4ac55: lint of the WebGPU back end, the front
  and the library, warnings denied, clean; the packed targets case and the seven format support
  comparisons (8 passed) and the back end's converted-formats tests (3 passed) on the RTX 3080
  (logs/g4h2-e.log).
- Wine through ours at 27ffc315: d3d11 test_format_support 1 check failing (XR_BIAS display),
  test_generate_mips 410, test_render_target_views 240 and test_multisample_resolve 172 checks
  passing; d3d10core test_format_support 382 and test_multisample_resolve 172 passing.
- own_cases `packed_targets`, 1,232 channel codes (two formats; cleared, drawn, blended twice,
  4 and 8 samples resolved, mip level 1; g4h/packed-*.txt, g4h/codes-diff.py): RTX 3080 against
  WARP 94 differ, largest 1 code (drawn 12, blended 12, resolves 60, mips 10); ours against the
  RTX 3080 116, largest 1 (drawn 4, blended 72, resolves 36, mips 4); ours against WARP 114,
  largest 1. Clears equal everywhere.

G4h run 1 (2026-11-22, Pacific). Logs in scratch logs/h1-* to h3-*; the rig copy as in How to run.
- Lint of the front, the library and the WebGPU back end, warnings denied, on the desktop at
  3825e2ab: clean (h2-check). Front 60, unit 6, cache 1 and clippy of the front and the WebGPU back
  end on the rented x86 machine (96 cores, load 0 to 8): pass (h1-front).
- The literal check at 3825e2ab on the rented x86 machine: no finding in this track's crates or
  the WebGPU back end (h3-lit; its lines are other crates' and the scratch clone's submodules).
- Desktop, through the queue, not alone (a display-reset hazard job of another track held it
  before): `a8_storage_views_store_as_direct3d_does` and the seven format support comparisons
  pass (h1-lib); the requirement test fails as listed under Where things stand (h2-lib, 3 s).
- The whole library suite at 3825e2ab (h3-lib, 08:52 to 08:54): com 15 pass, 1 ignored; programs
  60 pass, 4 ignored. Lint of the WebGL 2 build, warnings denied: clean (h3-webgl-check). Wine's
  files, scenes, programs, sizes and load times were not run again (the change touches only
  `A8_UNORM` storage and what the examples print).
Earlier runs' measurements follow (agents run 3).

Agents run 3 (2026-11-21, Pacific). Machine: the desktop (Windows 10, RTX 3080, i9-9900K), through
the queue, not alone except the load times; the shaders track's display-reset hazard jobs held the
queue about 21:21 to 21:34 and 21:59 to 22:24. Code: agents at 3b1cd4f3, which holds this run's
literal commits (scratch main/q5-final.sh a3b, logs/a3b-*; the library suite again as a3c, as its
first try met a run left by the stopped agents run 2 sending another tree to the same desktop
folder, which this run stopped).
- The literal check at 3b1cd4f3: 107 lines to act on, none this track's. Lint of both builds clean
  (21:35 to 21:37).
- Library (a3c-lib, 23:03 to 23:20): com 15 pass, 1 ignored; programs 59 pass, 3 ignored. The
  WebGL 2 build's 10_0 cases: 6 pass, 1 ignored (a3b-webgllib). Front 60, unit 6, cache 1 and
  clippy clean on the rented x86 machine (96 cores, load 0 to 9, a3-front, a3-lit4).
- Wine (21:44 to 21:48): d3d11 151 of 166, the same 15 failing tests as since round 4 run 3;
  d3d10core 110 of 111 (test_desktop_window). Through the WebGL 2 build d3d10core 96 of 111
  (a3b-wine10gl, 22:53 to 23:03), as at agents run 1.
- Scenes through the library 135 pass, 0 fail, 4 absent; the WebGL 2 build's 95 pass, 17 fail,
  30 absent, as before.
- Programs (a3b-programs, 22:24 to 22:26): the bar's three (godot-platformer-3d-d3d11,
  chocolate-doom, atg-simplelightingpc) match real Direct3D on every compared frame, and so do
  atg-simplebezierpc, atg-simpleinstancingpc, godot-platformer-2d-d3d11 and sdk-tutorial02 and 04
  to 07; atg-simplecomputepc (154,515 pixels) and atg-simplemsaa_pc (frame 10: 8 pixels, 4 beyond
  their tiles' allowance) differ as before (Gap list).
- Module sizes (section 10), release builds at 22:30 to 22:32 (a3b-sizes): `wcontainers_directx.dll`
  14,359,552 bytes on WebGPU and 9,930,240 bytes on WebGL 2; the Direct3D 12 track's
  `wcontainers_d3d12.dll` 13,767,680 bytes. The family library itself is the joint step.
- Load time (section 10), alone, `LoadLibraryW` in 20 fresh processes each, medians (a3b-load,
  22:32 to 22:35): the system's d3d11.dll 1,200 us; ours on WebGPU 1,674 us; ours on WebGL 2
  1,150 us; `wcontainers_d3d12.dll` 1,684 us.
- Building ahead (a3b-warm, 22:35 to 22:49): Godot 3D's first run built 54 to 115 ahead and drew
  none; its warm starts built their 61 recorded combinations at start and drew 60 and 61, built
  115 and 124 ahead and drew none, 25 and 26 at their first draw. sdk-tutorial07 and the others
  warm: their recorded combinations built at start and all drawn.
Earlier runs' measurements are in this file's git history.

## Known issues

- Threads: one lock serialises all calls; no parallelism between a program's threads.
- No front (recording back end) tests for 1D and 3D textures or queries; they are checked against
  real Direct3D instead (`own_volumes`, `own_queries`, the rig's textures scene).
- The rig's replacement check cannot see wcontainers_directx.dll behind the forwarder.

## Asked of other tracks

- Interface track: a `Caps` feature for cube arrays, declared by the WebGPU back end at the same
  time.
- Infrastructure crate (the Direct3D 12 track, G5): `D3D11_FORMAT_SUPPORT2_SHAREABLE` in its
  `support2` list. Its two G4h pieces were committed by this track in G4h run 4 (f9ecca97,
  c3879851; Departures). Done by the shaders track at this
  track's ask: `VertexFormat::Bgrx8Unorm` with every arm (02049aeb) and its typed buffer arm
  (1958e97b).
- WebGPU track: `FormatCaps` from the device's format features rather than the adapter's
  (`FILTER` of the 32-bit floats is claimed without float32-filterable while the adapter's own
  format features are on; wgpu-core takes it away on the device);
  the uniform arena admitting uniform buffers whose other usages lie within the arena's
  own; `B8G8R8X8_UNORM`'s X byte kept as given and written 0 by clears and draws.
- WebGL 2 track: copies from a depth texture into a colour texture stored as a half float keeping every bit pattern; mips of whole
  3D views as Direct3D generates them.
- Interface and shaders tracks: a helper value giving a program a view's width other than its
  texture's (YUY2 views).
- Shaders track: blending into integer twins (`TargetInShader::combine`, built in its run 80 for
  single-sample targets; not yet used here); the geometry replay's streams past
  the first; an aliased-line route for multisampled targets (with the interface track).
- NIR track: import `wcontainers_gpu::CUBE_FACES` in crates/wcontainers-nir/src/lib.rs in place of
  its own copy; until then the literal check reports that copy as a redefinition (its one finding
  outside the shaders and OpenGL crates' new code that touches this track).
- Rig track: atg-simplecomputepc's references hold one of the two answers the RTX 3080 gives for
  that program (Gap list): with frames 1 to 12 captured, real Direct3D's own frame 10 fails its
  reference by 154,515 pixels. A recording that also captures every early frame would hold the
  other.
- Rig track: the rig names the desktop's folder by the branch, which is `main` for every track now
  (this track runs its scratch copy, How to run).
- Rig track: a way to run Wine's dxgi file with its Direct3D 12 tests kept off the GPU; the scene
  programs reporting every `--dll` library they loaded.

## Literals

- G4h round 6 run 1 (2026-11-23): the context's slot counts are named (`CONSTANT_BUFFER_SLOTS`,
  `RESOURCE_SLOTS`, `SAMPLER_SLOTS`, `TARGET_SLOTS`, `STREAM_OUTPUT_SLOTS`; bf0291d7) and their ten
  baseline lines cut (3fe49ed3): this track's crates hold 581 baseline lines (591 before). The
  check at that tree on a rented machine (logs/r6c-t1lit-front.log) finds nothing new in this
  track's files but the remote copy's stale "unparsed" line for the deleted src/required.rs.

- G4h run 3 (2026-11-22): the check at f23c2563 finds nothing in this track's crates (one
  restatement in the new example, a wait equal to another crate's constant, changed in that
  commit). The infrastructure patch names its decoding constants in its WGSL text; checked once
  landed.

- Agents run 3 (2026-11-21): 66 lines of this track cut: the stage helper's refusals behind
  geometry, hull and domain shaders, its draw range (`Direct`) and word sizes
  (`wcontainers_gpu::STORAGE_WORD`), the patch list count, stream output declarations' buffer and
  stream counts (the shaders crate's `STREAM_BUFFERS` and `STREAMS`), the counting programs'
  names, sizes and refusals (the shaders crate's `ENTRY_POINT`), split draws' write mask and
  refusals, and the combination cache's key type (`blake3::OUT_LEN`). Left:
  wcontainers-direct3d-11 338 lines (404), wcontainers-directx 200. Checked on the rented x86
  machine (logs/a3-lit1 to a3-lit4): no new finding of this track's.
- Agents run 1 (2026-11-21): 36 lines of this track cut: the helper block takes each value's
  length from the words it writes (`size_of`), and colours, blend factors, border colours and
  scissor rectangles use the front's `Color`, `Rect` and `COLOR_COMPONENTS` in both crates. Left:
  wcontainers-direct3d-11 404 lines (430), wcontainers-directx 200 (210). The check at
  05a2cff4 exits 1 with 20 lines to act on, none of this track's (the OpenGL crate, the kernel
  interface's poll events, the NIR crate's cube faces, two socket suites).
- Main run 2 (2026-11-21): 19 lines of this track cut from the baseline, the library's messages of
  what it does not serve named once (`CLASS_LINKAGE`, `NO_COMPUTE`, `NO_TESSELLATION`,
  `LOGIC_OPERATIONS`, `FULLSCREEN` and others in device.rs and swapchain.rs, and the device
  makers' `NOT_HARDWARE`, `NOT_HARDWARE_10`, `NO_VIDEO`); the new video refusal adds no line.
  Left in the baseline: wcontainers-direct3d-11 432 lines (499 at main run 1; the rest went with
  the shared container reader, 7090247d), wcontainers-directx 210 (229).
- Main run 1 (2026-11-21): the check at main 10d3fd72 exits 1 with 50 lines to act on, none this
  track's but the cube faces' redefinition, which came from two branches meeting on main: the
  front now imports `wcontainers_gpu::CUBE_FACES` (Interface changes); the NIR crate's copy is
  that track's (Asked). The rest are new code of the shaders, OpenGL and interface crates.
- Round 4 run 6: the new refusals' messages are named once (`COMPARED_FORMAT_UNCLAIMED`,
  `COMPARED_VIEW_INVALID`, `LEVEL9_BUFFER_VIEWED`, `LEVEL9_USE_ABSENT`); new code adds no line and
  makes no mark. wcontainers-direct3d-11 499 lines, wcontainers-directx 229 (728), as at round 4
  run 5 (no line cut this run: the run went to the findings). The check exits 0 at 318f6a9e
  (logs/r6x-literals.log).
Left: the export names in build.rs (a build script cannot import the crate it builds), and the rest
of the fix list (orchestrator magic-literals/fix-graphics.md), a part per run.

## Licences

G4h run 1 added no dependency. Downloaded to scratch downloads/msdocs: Microsoft's 10_0, 10_1 and
11_0 format support pages (markdown from github.com/MicrosoftDocs/win32, CC-BY-4.0 text; the
table in required.rs is facts generated from them, not their text).

Dependencies of what this track owns, each read in its Cargo.toml when added: `windows` and
`windows-core` 0.62.2, `wgpu` and `naga` 30.0.2 (MIT OR Apache-2.0), `pollster` 1.0.1
(Apache-2.0/MIT), `blake3` 1.8.7 (CC0-1.0 OR Apache-2.0), `cc` 1.11.2 (build), `parking_lot`
0.12.5, `serde` 1, `rand` 0.9.5 and `rand_chacha` 0.9.0 (dev) (MIT OR Apache-2.0); through the
`webgl` feature our `wcontainers-gpu-webgl` with `glow` 0.18.0 (MIT OR Apache-2.0 OR Zlib),
`khronos-egl` 6.0.0 (MIT/Apache-2.0), `libloading` (ISC) and `bcdec_rs` 0.2.1 (MIT); our
`wcontainers-shaders` (which builds dxbc-spirv, MIT, in) and
`wcontainers-directx-graphics-infrastructure`. Test tools never shipped: Wine's tests (LGPL),
ANGLE (BSD-3-Clause), the system's d3dcompiler_47.dll. Installed on the desktop: build output in
C:\wc\target\phase-2-graphics-direct3d-11, pictures and logs in C:\wc\out, the 15 KB runner copy
in C:\wc\out\phase-2-graphics-direct3d-11\rigfix, an empty nvapi64.dll there (bgfx's examples),
the programs' cache files under C:\wc\out\phase-2-graphics-direct3d-11\gpu-cache, the
test-only WARP d3d11.dll (G4h run 3, built with no dependency; 2.6 MB with its build folder)
in C:\wc\out\phase-2-graphics-direct3d-11\warpshim, the test-only
d3d10_1.dll forwarder in C:\wc\out\phase-2-graphics-direct3d-11\d3d10_1fwd, and the LF worktree
C:\wc\src\phase-2-graphics-direct3d-11-lf for the literal check, and apitrace 14.0 (MIT; a
released build, 48 MB without its debug files, in C:\wc\d3d11-tools\apitrace-14.0, used to trace
and replay programs for testing only, never a dependency; its trace and dumps in
C:\wc\out\phase-2-graphics-direct3d-11\r6c, 7 MB). On the Mac (not the desktop),
rustup's x86_64-pc-windows-msvc standard library for 1.99.0.
