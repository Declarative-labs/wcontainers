# Graphics track status (interface, G1)

Plan: `docs/phase-2-graphics.md`. Brief: `pyro-parked/wcontainers-scratch/orchestrator/brief-graphics.md`.
This branch, phase-2-graphics, is the shared line: the interface track merges the other graphics
branches into it at the start and end of every run.

## Where things stand

- G0 is done (eight runs and eight independent checks on 2026-11-17; detail in the git log and the
  plan): the plan, and `crates/wcontainers-gpu` with the interface as real Rust.
- G1a (two runs, 2026-11-17) is done: the recording back end and the first 13 suite tests.
- G1, run 1 (2026-11-17), done and pushed, every item of section 12's pass bar met on the recording
  back end (item by item below), with two parts the pass bar does not reach left for later (Next):
  - the five findings of G1a's second check fixed (below);
  - in `wcontainers-gpu`: the pass tracker (`passes.rs`), the occlusion counter (`occlusion.rs`) and
    the combination tracker (`combinations.rs`);
  - in `wcontainers-gpu-test`: the call stream and its replay (`stream.rs`), the seeded random
    call-sequence generator (`sequence.rs`), storage buffer writes from fragments, the rule checks
    G1a left (usage flags, typeless groups, the no-overwrite promise, usage scope, attachment
    slices), and the tests: `tests/rules.rs`, `tests/fast_paths.rs`, `tests/helpers.rs`,
    `tests/sequences.rs`, and 8 new suite tests (21 now);
  - the six track branches merged at the start and again at the end.
- G1, run 2 (2026-11-17), done and pushed: the two parts run 1 left inside G1's scope.
  - The recording back end draws four samples (standard positions, per-sample depth, stencil,
    blending and occlusion counts, programs run per sample where they read the sample index or a
    value at the sample), resolves (at a pass's end and by `resolve`), copies multisampled textures
    whole, loads samples in programs, and checks the multisampled rules; it now declares
    `MULTISAMPLE_READ` and `SAMPLE_MASK`, and four samples on every format it draws into.
  - The pass tracker fits multisampled attachments of different sizes (plan 4.7): by a draw reading
    each sample where the back end has `MULTISAMPLE_READ` and `SAMPLE_SHADING`, otherwise through the
    resolved value (`MultisampleCopyResolved`); integer formats without the first are reported
    `Absent(MULTISAMPLE_READ)`. A new built-in program (`Builtin::SampleCopy`) does both.
  - The occlusion counter's count in halves past its format's bound is tested (on a recording back
    end without blendable `r32float`, so it counts in `r16float`), and so is its count of samples on
    a four-sample target.
  - New suite tests: `multisampled_targets_cover_their_samples_and_resolve`,
    `programs_run_per_sample_and_read_samples`, `occlusion_queries_answer_whether_samples_passed`
    (24 now). The random sequences now include multisampled passes resolved into the texture named.
  - `Recorder::with_caps` makes a recording back end with capabilities taken away, for the routes
    of helpers on back ends without them.
  - The direct3d-12 branch merged at the start (the others had nothing new); at the end webgpu,
    webgl and direct3d-12 had new work and were merged, with no conflict.

- G1, run 3 (2026-11-17), done and pushed: the seven findings of G1's independent check fixed
  (below). The start merges brought webgl, direct3d-11, direct3d-12 and shaders in without
  conflict; the shaders track added `VariantKey::output_interpolation` (kept: it serves every
  front). The end merges brought webgpu, webgl, direct3d-11 and direct3d-12 in without conflict; the
  webgpu track added `SlowPath::IntegerClearByDraw` (kept: a named slow path).

- G1, run 4 (2026-11-17): the five findings of G1's second independent check fixed (below). The
  start merges brought webgpu, webgl, direct3d-11, direct3d-12 and shaders in; one conflict, in the
  text of `TextureRegion::stencil` and plan 4.2, where the direct3d-11 track stated the same rule
  as run 3 (copies between textures move both aspects): kept one statement with both reasons. The
  mid-run merges (after the fixes were committed) brought all five in again; a conflict in the same
  plan paragraph, where the direct3d-11 branch held run 3's older text, kept ours. The end merges
  brought webgpu, direct3d-11 and direct3d-12 in without conflict (webgl, shaders and rig had
  nothing new).
- G1, run 5 (2026-11-17): the eight findings of G1's third independent check fixed (below). The
  start merges brought webgpu, webgl, direct3d-11, direct3d-12 and shaders in without conflict (rig
  had nothing new); none of them changed the interface. The end merges brought webgpu, webgl,
  direct3d-11, rig and then shaders in without a textual conflict and without touching the
  interface; the rig's and webgl's work together broke the build of a webgl example, fixed here
  (Departures). The whole workspace checks after them.
- G1, run 6 (2026-11-17): the five findings of G1's fourth independent check fixed (below). The
  start merges brought webgpu and webgl in (the others had nothing new). Two conflicts: `Cargo.lock`
  (the webgpu track's new test harness crates; settled by taking ours and letting cargo add theirs,
  at the versions their lock has), and `SlowPath`, where webgpu added `PresentConverted` and webgl
  `CaptureExpanded` at the same place: both kept (`SlowPath::ALL` now 99). The whole workspace
  checks after them. The end merges brought webgpu, webgl, direct3d-11 and shaders in (direct3d-12
  and rig had nothing new); the webgl branch held the same two slow paths in the other order, kept
  in ours. None of them changed the interface; the whole workspace checks after them.
- G1, run 7 (2026-11-17): the seven findings of G1's fifth independent check settled (below). The
  start merges brought direct3d-12 and rig in without conflict and without touching the interface
  (the others had nothing new); the end merge brought webgl in (the others had nothing new), again
  without conflict or interface change. The whole workspace checks after each.

## G1's fifth check, and how each finding was settled (run 7)

- A sampler's kind against the program (blocking): the recording back end now checks, at each
  draw, every sampler binding the program's entry points use against naga's type: a
  `sampler_comparison` binding takes a sampler with `compare` set, a `sampler` binding one without.
  A comparison through a sampler without `compare` is no longer made up as `Always`; the guard left
  in `sample_view` cannot be reached through the interface. Tests:
  `samplers_compare_exactly_where_the_program_compares` (Less gives 1, Greater 0, linear comparison
  of `depth32float` drawn, and both mismatches refused) and the suite's new
  `comparison_samplers_compare_with_the_stored_depth` (Less and GreaterEqual against 0.5 at four
  references, every back end).
- The filter rule by sampler, not by level (blocking): the check now runs on naga's `sampling_set`
  (every texture and sampler pair the entry point uses, through function calls) at each draw: a
  sampler that does not compare, with any of min, mag or mip linear, paired with a texture whose
  view's format lacks `FILTER`, is refused, whatever level the draw samples. The old per-sample check
  is gone. Test: `linear_filters_pair_only_with_textures_the_back_end_filters` (min, mag, mip each
  linear refused on `r32float` without `FILTER`; all nearest drawn).
- Several viewports (blocking): `DrawCall::viewports` and `scissors` now state one viewport and at
  most one scissor (`MultiViewportSplit`), as does plan 4.4; the recording back end refuses more
  (`a_draw_has_one_viewport_and_at_most_one_scissor`, two viewports, two scissors, one scissor with
  scissoring off drawn).
- A uniform binding smaller than the program's block (minor): `BindingResource::UniformBuffer` and
  `StorageBuffer` docs and plan 4.5 now state it (WebGPU's `minBindingSize`, WebGL 2's
  `UNIFORM_BLOCK_DATA_SIZE`; a runtime-sized array counts as one element), and the recording back
  end checks each buffer binding the program uses against naga's size of its type
  (`buffer_bindings_cover_the_programs_blocks`: 16 bytes for a 32-byte uniform block or a storage
  block of a `u32` and a `vec4` array refused, 32 drawn).
- An empty pass's size and sample count (minor): `EmptyPass` docs and plan 4.7: width and height 1
  to `max_texture_2d`, and a count `takes_samples` gives for `r8unorm` (the attachment WebGPU adds).
  Checked and tested (`an_empty_pass_has_a_size_and_a_sample_count_r8unorm_takes`: 0 each way,
  one past the limit each way, 0, 3 and 8 samples refused; 1 and 4 samples and the largest size
  taken).
- Offsets near 2^64 (minor): every buffer range a call names now ends through a checked add
  (`end` in `recorder.rs`), refused as past the buffer where it does not fit; a draw's index and
  vertex addresses saturate, so they fall past the buffer. Test:
  `ranges_ending_past_the_largest_offset_are_refused` (write, clear, both sides of `copy_buffer`,
  read back, both buffer-texture copies, a binding set, an index binding and a vertex stream).
- Stride zero in the suite (minor): `a_stream_of_stride_zero_reads_its_first_element` (a quad drawn
  twice with a colour stream of stride 0 holding three colours; every pixel the first). Suite 26.
- The form of 2de3a6b (minor): that run-5 commit changed a public helper item and touched a test
  and the plan, without the `interface:` prefix. It cannot be changed without rewriting history,
  which the project forbids; it is listed under Interface changes (run 5), and this run's interface
  change (1362848) touches only `crates/wcontainers-gpu` and the plan.

## G1's fourth check, and how each finding was settled (run 6)

- Parts of one-line conditions no test broke (blocking): the rule sweep now turns off each
  top-level `||` operand of every check's condition, on one line or over several (scratch
  `g1r6/sweep.py`, which takes the check's `g1-check4/disjuncts.py` way of splitting). New tests
  break each of the eight the check named: a depth attachment that leaves out the depth aspect of
  `depth32float` or names it on `stencil8`
  (`the_depth_attachment_names_depth_exactly_when_its_format_has_it`); a pass's multisampled resolve
  target, and `resolve` into `bgra8unorm` from `rgba8unorm` with the source's format named
  (`resolves_go_from_many_samples_to_one_of_an_averaging_format`); `SAMPLE` usage on a format
  without the `SAMPLE` use (`textures_take_only_the_usages_their_format_does`); a multisampled copy
  whole on one side only, both ways (`a_multisampled_textures_bytes_move_by_draws_resolves_and_whole_copies`);
  one row a layer in `copy_texture_to_buffer` (`buffer_texture_copies_take_pitches_covering_the_region`)
  and in a stencil aspect write (`texture_writes_take_pitches_covering_the_region`).
- A read back past its buffer's end (blocking): `reads_back_stay_inside_their_buffers`. The sweep
  now also gives every slice `.get(range)` refused by an `ok_or` an empty slice in place of `None`,
  so a refusal of that form no test reaches survives it (Measurements). Refusals written as
  `ok_or` on a call's state rather than a range (`end_pass without a pass`, the running query,
  `current_texture` of an unconfigured surface and the like) cannot be turned off this way; each is
  asserted by its message in a test except the guards listed under Next. Three more found by that
  audit now have tests: `a_query_ends_into_a_live_set` (the occlusion set destroyed while its query
  runs), `objects_are_used_live_and_as_their_kind` (a buffer named as a texture, a texture as a
  buffer, a view of a buffer, a buffer used after destroy) and
  `a_vertex_program_reads_only_locations_its_layout_has`. The same audit found two rules not
  checked at all, now checked and tested (`shaders_and_programs_hold_entry_points_of_their_stages`):
  a shader naming an entry point of its stage that its module lacks (`create_shader`), and a
  program whose vertex or fragment shader is of another stage (`create_program`).
- Clear patterns in the plan (minor): plan 4.2 now gives the interface's list, 1, 2, 4, 8 or 16 bytes
  over a range of whole patterns.
- Capabilities the recording back end served but called absent, or checked as the wrong error
  (minor): it now declares `SEPARATE_SAMPLERS` (its programs pair any texture with any sampler) and
  `FORMAT_REINTERPRET` (draws read and write a view by the view's own format, so another colour
  format of the same block size is served as it is). Without `FORMAT_REINTERPRET` (taken away by
  `with_caps`) such a view is refused as `Absent` of it; a view in a format of another size, or a
  depth format, is `Invalid` (`views_in_other_formats_of_the_same_size_need_format_reinterpret`;
  `a_view_in_another_format_reads_the_bytes_as_that_format` draws 1.0 through an `r32float` view of
  `rgba8unorm` bytes). Without `TEXTURE_VIEWS` a draw binding one texture through two mip ranges is
  refused (`a_draw_binds_a_texture_through_one_mip_range_without_texture_views`, with the same draw
  accepted where it is declared). The interface now says reinterpretation is between colour formats
  only (Interface changes).
- The laptop's full disk (minor): this run deleted the build folders of runs 1 to 5 and of the G1
  checks (`g1/work/target-break`, `g1a-run2`, `work-g1r2`, `g1r4`'s four, `g1r5`'s two,
  `g1-check2`'s three, `check-g1a-2`), about 17 GB; their scripts and logs are kept. Free space went
  from 12 GB to 29 GB. Left alone: `target/phase-2-graphics-shaders` and `check-g6a*` (the shaders
  track's), and `target/phase-2-graphics-` (4.2 GB, 05:01 today, owner unknown).

## G1's third check, and how each finding was settled (run 5)

- Checks written as a helper call ending in `?` (blocking): the rule sweep now turns those off too
  (scratch `g1r5/sweep.py`: each statement calling `outside_pass`, `buffer_for`, `texture_for`,
  `copies_out`, `same`, `same_samples`, `draw_rules`, `usage_scope`, or the pass's `ok_or`, is
  removed in turn). New tests break each of the 16 the check found unbroken:
  `each_use_needs_its_usage_flag` (read backs, `copy_buffer` both sides, both buffer-texture copies
  on both sides, `clear_buffer`, `write_texture`, `copy_texture` both sides, `resolve` both sides,
  index bindings and indirect arguments, each with its exact message);
  `texture_copies_and_buffer_clears_happen_outside_a_pass` now also refuses
  `copy_buffer_to_texture`, `copy_texture_to_buffer` and `resolve` inside a pass; and
  `resolves_and_copies_out_need_the_formats_uses` a read back of a format without `COPY_OUT`.
  Indirect draws are absent on the recording back end, but its usage check comes first, so it is
  reachable and tested.
- The ring's wrap (blocking): a `Discard` write now gives the buffer fresh contents in the recording
  back end, as `WriteMode::Discard` states: the ranges earlier commands and ordered writes touched
  no longer bind a later `NoOverwrite`, and the bytes the discard did not write become undefined
  (filled with 0xcd, so a caller relying on them is caught). Tests: `the_ring_wraps_with_a_discard`
  (fills the 4 MiB ring, draws from it, wraps: modes Discard then NoOverwrite, no violation, the
  draw before the wrap kept what it read, the one after draws the new part) and
  `a_discard_write_gives_the_buffer_fresh_contents`. The random sequences' generator now treats a
  discard the same way (it drops the buffer's pending ranges instead of marking it all pending).
- Halves of instances without `BASE_VERTEX_INSTANCE`, and per-instance data (blocking and minor):
  the counter now raises the first instance only where the back end has `BASE_VERTEX_INSTANCE` and
  the front says every per-instance stream steps by one (`CountedDraw::instances_step_by_one`);
  otherwise it splits the draw by its primitives, each half drawn for every instance, which keeps
  every instance's reads. One primitive's instances past the bound where the first instance cannot
  be raised is refused (`GpuError::Invalid`, Known failures). Test:
  `the_occlusion_counter_splits_instances_only_where_each_reads_the_same` (with and without
  `BASE_VERTEX_INSTANCE`: a list of 1,000 triangles three times with a stream of step rate zero,
  12,000; the same with no per-instance stream, 12,000; the quad 3,000 times, 12,000 by instances
  with bases, refused without). Both faults put back fail it (`g1r5/faults.log`).
- A query index begun twice in a pass (minor): stated in `begin_query`'s docs and plan 4.7, refused
  by the recording back end, and tested (`a_query_index_is_begun_once_in_a_pass`).
- Step rates and typed texel buffers without a capability (minor): two new features,
  `INSTANCE_STEP_RATE` and `TEXEL_BUFFERS`; a back end without one refuses the layout or binding
  with `GpuError::Absent` of it, and fronts take `InstanceRateInShader` or `TexelBufferRoute::Storage`
  or `Texture`. The recording back end declares the first (and checks it when taken away) and
  refuses the second as absent (`step_rates_and_texel_buffers_need_their_features`).
- Stale departure (minor): the bullet now says what the generator does.
- The measure of `color_bytes_per_sample` (minor): stated. `Format::target_bytes` gives each format's
  cost and alignment as WebGPU counts them (from wgpu-types 30.0.2's
  `target_pixel_byte_cost`/`target_component_alignment`, which follow the WebGPU table; our formats
  WebGPU lacks count as their nearest one), and `color_target_bytes` sums a pass's targets in order.
  The recording back end counts by it (`passes_stay_within_the_colour_target_limits`: two
  `rgba8unorm` targets need 16, `r8unorm` then `r32float` need 8). The webgpu back end passes
  WebGPU's limit, which is in this measure already.

## G1's second check, and how each finding was settled (run 4)

- Rule checks no test broke (blocking): every rule check of the recording back end is now broken by
  a test, shown by a sweep that turns each check off in turn and runs the crate's tests
  (scratch `g1r4/sweep.py`; Measurements). Run 4's first sweep, on the code the check saw, left 47 of
  101 checks (and parts of checks) unbroken, the 13 the check named among them; 4 more are match
  guards the sweep cannot turn off, each now asserted by message. New tests in
  `tests/rules.rs` for each: attachment views of one level and of one layer, a 3D slice past its
  level's depth, colour formats as the depth attachment and depth formats as colour, `empty` with
  attachments and neither, a pass ended with its query running, `end_pass` and `end_query` without
  their begin, present inside a pass, surfaces in a format without `PRESENT` and `current_texture`
  before configuring, regions past the last level, layer and slice and outside their level, pitches
  of texture writes and buffer-to-texture copies (both aspects), data shorter than the region,
  buffer copies, buffer-texture copies and clears past their buffers, clear patterns of 3 bytes,
  formats not held, empty textures, cube layers not whole cubes, view ranges past the layers or of
  no level or layer, loads naming a sample from a single-sampled texture and none from a
  multisampled one, bindings of the wrong kind of resource, vertices and indices past their
  buffers, draws with no viewport, scissor on with no scissor, streams without buffers or
  descriptions, fans without `FAN_AND_LOOP`, indirect draws, query sets of absent kinds,
  `read_queries` outside its set or inside a pass, depth bounds, border and mirror-once samplers,
  the raster block's absent features (polygon mode, depth clamp, clip depth from -1), storage
  bindings of buffers without `STORAGE`, `resolve` from one sample or into many, past its levels
  or layers or from another level or layer.
  - The multisampled no-storage case could not fail as written: the multisampled-shape check now
    comes before the format-use check, and the test asserts each refusal's message.
  - New checks with their tests: a draw writing stencil into a read-only stencil aspect (it was
    skipped silently); one subresource attached or resolved into twice (refused at `begin_pass`;
    the draw's own check is now unreachable); and the limits the recording back end declared but
    did not check: texture sizes (2D, cube, 3D, layers), vertex streams, attributes and stride,
    sampler anisotropy, the vertex program's outputs against `inter_stage_slots`, colour targets
    and their bytes a sample (counted by block bytes, the recorder's own measure), and uniform
    buffers, textures, samplers and storage buffers per stage across a draw's sets.
- Four-sample bound (minor): the comparison stays on a pixel's sum over its samples (it keeps the
  resolved average exact); the text now says so (plan 4.12 step 4, the counter's docs, Known
  failures). The same reasoning found a real fault: the depth copy was saved for halves only where
  the draws' primitives reached the bound, not primitives times samples, so on four samples the
  halves could start from the depth the whole left. Fixed, and tested by
  `the_occlusion_counter_keeps_the_starting_depth_for_halves_on_four_samples` (9,616; 0 before).
- `clear_rects` depth and stencil (minor): `clear_rects_writes_depth_and_stencil_by_draws`
  (`depth32float` rectangle at 0.25, the rest 0; `depth24plus-stencil8` stencil 7 in column 0, its
  depth word unchanged).
- Prepared combinations drawn before (minor): a combination remembers its draws when first
  prepared and counts as used only for draws after; tested by the check's own case.
- 3D attachment slice (minor): `a_3d_attachment_slice_lies_inside_its_levels_depth` (level 1 of a
  4-deep texture takes slice 1, refuses slice 2).

## G1's check, and how each finding was settled (run 3)

- Rules stated but not checked (blocking): the recording back end now reads its own `Features`,
  `Limits` and format uses and refuses what they leave out. New checks, each broken by a test in
  `tests/rules.rs`: blocks with `same_for_all` false without `INDEPENDENT_BLEND`; the constant alpha
  factors without `BLEND_CONSTANT_ALPHA`; per-face stencil masks (at creation) and references (at the
  draw) without `STENCIL_PER_FACE`; raster blocks needing `PROVOKING_VERTEX_FIRST` or `_LAST`,
  `DEPTH_BIAS_CLAMP`, `FRACTIONAL_DEPTH_BIAS`, `SAMPLE_MASK`, `CLIP_DEPTH_ZERO_ONE`, `WIDE_LINES`;
  uniform and storage bindings off their alignment, past their buffer or over their limit; writable
  fragment storage without `FRAGMENT_STORAGE_WRITE`, vertex storage reads without
  `VERTEX_STORAGE_READ`, multisampled textures bound without `MULTISAMPLE_READ`; texture usages the
  format does not take; viewports with `min_depth` above `max_depth`, fractional viewports without
  `FRACTIONAL_VIEWPORT`; bases without `BASE_VERTEX_INSTANCE`; 8-bit indices without `INDEX_U8`;
  immediates past the limit; blending into a format without `BLEND`, filtering one without `FILTER`;
  programs run per sample without `SAMPLE_SHADING`; resolves of formats without `RESOLVE` (now
  declared where it resolves) and copies out of formats without `COPY_OUT`; views of other shapes,
  and bound views not of all layers in the texture's own dimension, without `TEXTURE_VIEWS`.
- Occlusion counter not exact across buffer writes and target changes (blocking): it now counts the
  draws kept so far before each change. `OcclusionCounter::flush` is called by the front before any
  write, copy, clear, resolve or target change inside a counted query; the counter refuses a draw or
  an end on targets changed without it (`PassTracker::generation`). The depth copy is stored by the
  count, so it follows the target. Tests: `the_occlusion_counter_counts_across_writes_and_target_changes`
  (the check's two cases, and the refusal).
- Halves from discarded depth (minor): the copy is stored, and where the draws have as many
  primitives as the bound it is saved first and restored before the halves, which run one after the
  other. Strips without restart are split too (triangle strips at an even triangle). Test:
  `the_occlusion_counter_counts_halves_from_the_depth_they_began_with` (12,004, the restore broken
  gives 0; a 3,000-triangle strip counted as two). A single fan, strip with restart or indirect draw
  past the bound is still refused (Known failures).
- Function calls in programs (minor): the interpreter runs `Statement::Call`. Test:
  `generate_mips_averages_the_four_texels_below_on_every_route` (rgba8unorm through the sampler,
  r32float without `FILTER` filtered in the program, rgba16uint holding rgba16unorm values), each
  against the exact averages. The line loop default is shown by its recorded stream
  (`loops_become_line_lists_in_the_ring`); the recorder still draws no lines and says so.
- Stencil aspect ignored (minor): buffer copies and read backs of a format with depth and stencil now
  move the aspect named (stencil one byte, depth four); copies between textures move both
  (`copies_of_depth_and_stencil_move_the_aspect_named`). Stated in the interface (below).
- Features never read (minor): see the first item; `with_caps` now takes features away for real.
- Narrow random sequences (minor): ten formats (unorm, sRGB, BGRA, 8 to 32-bit float, unsigned and
  signed integers), passes with depth and stencil (both aspects read back), uniform binding sets at
  either aligned offset, indexed draws (16 and 32-bit, instanced from a first instance), scissors and
  occlusion queries read back; no-overwrite writes wait only where they would touch a range in use,
  so they land beside ranges unfinished commands use (a generator broken to never wait is caught).
  The test prints how many of each it made. Passes destroy their views, buffers and query sets
  right after recording, before any submit: work recorded before a destroy still uses them.
- Hot paths (minor): `CombinationTracker::combination` looks a combination up when the front's
  state changes and `drawn(Combination)` is one indexed add per draw; the pass tracker skips its
  usage-scope check while neither the pass nor the draw writes storage and keeps its lists sorted.

## G1's pass bar, item by item

- Every rule of section 4 checked by the recording back end, with a test that breaks it (the list
  under "The rules and their tests"), shown by the sweep that turns each check off in turn: since
  run 4 the `if` checks, since run 5 also the checks written as a helper call ending in `?`, since
  run 6 also each top-level `||` operand of a condition and each slice range refused by `ok_or`
  (Measurements); refusals of other forms are asserted by message (run 6's audit).
- Every fast-path default shown built from the base by a recorded stream: `tests/fast_paths.rs`, one
  test each for `read_back_now`, `prepare`, `clear_texture`, `clear_rects`, `generate_mips` (and
  its three routes by their averages), `draw_fan_or_loop` (fans drawn; loops by their stream, the
  recorder draws no lines), `write_buffer_ring` (and its wrap, `the_ring_wraps_with_a_discard`)
  and `resolve_queries`; each asserts the calls
  recorded, by name, and the answer.
- The occlusion counter's exact counts on scripted draws with a counting program in WGSL:
  `the_occlusion_counter_counts_exactly` (draws that write depth counted against the depth copied at
  the query's begin; draws that do not, against the target read-only).
- The combination tracker's tuples (4.4) on a scripted sequence:
  `the_combination_tracker_picks_the_tuples_drawn` (a drawn vertex shader's tuples, the most drawn
  fitting tuple for a new one, ties to the smaller, the cache's record, prepared-then-drawn counts).
- The pass tracker fits multisampled colour targets of different sizes on both routes:
  `multisampled_targets_of_different_sizes_are_fitted_sample_by_sample` and
  `..._through_the_resolved_value` (each sample of the larger target read back by a program), and
  `integer_multisampled_targets_need_multisample_reads_to_be_fitted`.
- The occlusion counter past its format's bound: `the_occlusion_counter_counts_past_its_formats_bound_in_halves`
  (one draw's instances, one list's primitives, three draws, and below the bound counted once); on
  four samples: `the_occlusion_counter_counts_the_samples_of_multisampled_targets`.
- The pass tracker fits a larger depth buffer and colour targets of different sizes:
  `a_larger_depth_buffer_is_fitted` (temporary made once and pooled, copied in only for `Load` and
  out only for `Store`, scissors cut, depth outside the common area untouched) and
  `colour_targets_of_different_sizes_are_fitted`; and makes a clear the pass's load:
  `an_implicit_fronts_clear_becomes_the_passs_load`.
- The pass tracker splits a pass at a storage write followed by another use (`UsageScopeSplit`, the
  running query carried across as two interface queries), and the recording back end rejects the
  same draws in one pass: `a_pass_is_split_where_a_draw_reads_what_one_wrote_as_storage`.
- 100,000 random sequences, seeded, recorded and replayed identically (same stream with every
  answer, same bytes in every buffer and texture, same frames, no rule broken): in run 3, on the
  widened generator, seeds 0x5eed, 0xc0ffee and 0x7a11ed, 100,000 each; in run 4, on the recorder
  with the new checks, 0x5eed, 0xc0ffee and 0x9e4a11 (new), 100,000 each; in run 5, with discards
  giving fresh contents and the new checks, 0x5eed, 0xc0ffee and 0xd15ca4d (new), 100,000 each;
  in run 6, 0x5eed, 0xc0ffee and 0x51de6 (new); in run 7, on the recorder with the program binding
  checks, 0x5eed, 0xc0ffee and 0x5a3d7 (new), 100,000 each (Measurements).

## G1a's second check, and how each finding was settled

- Texture-to-buffer copies at an offset over layers: `copies_move_bytes_exactly` now copies two
  layers at offset 12, pitch 20, three rows per layer, and checks the bytes between rows untouched.
- Truncating float-to-unorm conversions: `clears_fill_exactly` clears 0.25, 0.85, 0.55 (63.75,
  216.75, 140.25 steps) exactly; `the_scissor_bounds_the_draw` and `stencil_operations_and_tests`
  draw those values exactly.
- Perspective: `values_are_interpolated_with_perspective` (w from 1 to 4, one step of tolerance;
  screen-linear is up to 64 steps off).
- Untested draw behaviours: `the_scissor_bounds_the_draw`, `stencil_operations_and_tests` (replace,
  invert, increment, equal and not-equal), `near_clipping_and_the_viewport_depth_range`,
  `linear_filtering_and_address_modes` (clamp and repeat, weights, two steps of tolerance),
  `instances_start_at_the_first_instance`, `indexed_strips_restart_and_add_the_base_vertex` (16 and
  32-bit, index offset, `first`, restart, base vertex; first instance and base vertex only where
  `BASE_VERTEX_INSTANCE` is declared).
- `create_blend` refuses second-source factors with `Absent(DUAL_SOURCE_BLEND)` at creation
  (`blocks_needing_absent_features_are_refused_at_creation`).
- All 13 faults of the check's script (`g1a-check2/breaks.py`; scratch copy `g1/breaks.py`, results
  `g1/breaks-run1.log`) now fail the suite on the recording back end; before, 9 passed it.

## The rules and their tests

Each rule a caller can break is refused (a violation that loses the device, or `GpuError` at
creation) and broken once in a test (`tests/rules.rs` unless named):
- draws inside a pass (`draws_happen_inside_a_pass`); passes do not nest; submits outside a pass;
  copies, clears and read backs outside a pass (also `copies_and_read_backs_inside_a_pass_are_refused`
  in `tests/recorder.rs`);
- attachments of one size and depth aspects of the format (`tests/recorder.rs`); one level and layer
  with render usage; a 3D slice inside its level;
- one usage scope per pass (`tests/helpers.rs`); a draw reads no subresource it writes;
- queries: inside a pass with a set, one at a time, an index inside the set;
- restart on strips only; every binding a program reads bound; each binding once in a set;
- usage flags for every use (the interface now states them, `BufferUsage` and `TextureUsage` docs);
- texture copies within one typeless group; views in the texture's format or its twin, ranges
  inside the texture; writes, copies and clears inside their buffers, no copy between overlapping
  ranges of one buffer, clear patterns of 1, 2, 4, 8 or 16 bytes; buffer-texture pitches covering
  the region;
- read backs finished when ready (`tests/recorder.rs`) and into bytes of their size;
- no-overwrite writes touch nothing an unfinished command reads or writes, nor an ordered write not
  waited for (the rule as the webgpu track widened it, below);
- objects not used after destroy, not destroyed twice (`tests/recorder.rs`); and the promise the
  other way, work recorded before a destroy still sees the object;
- blocks needing absent features refused at creation (`tests/recorder.rs`); storage writes only
  from fragments; present needs the current texture;
- every feature, limit and format use the back end declares, as listed under G1's check above
  (`tests/rules.rs`, one test per rule or per block kind);
- buffer copies and read backs of depth with stencil move the aspect named, texture copies both,
  and only such formats name stencil;
- since run 4 (each in `tests/rules.rs`): attachment views of one level and one layer, 3D slices
  inside their level's depth, depth formats only as the depth attachment, attachments or `empty`
  but not both, nothing attached or resolved into twice, no draw writing a read-only aspect, a pass
  ending with no query running, `end_pass` and `end_query` after their begins, present outside a
  pass, surfaces of presentable formats configured before `current_texture`, regions inside their
  level, layers and slices, pitches and data covering the region, buffer ranges of every copy and
  clear inside their buffers, bindings of the resource kind the program reads, vertices and indices
  inside their buffers, a viewport, a scissor where the raster block scissors, a buffer and a
  description for every stream an attribute names, query sets and `read_queries` ranges, and every
  limit the back end declares that a call can pass (buffer and texture sizes, vertex streams,
  attributes and stride, anisotropy, inter-stage slots, colour targets and their bytes, bindings
  per stage, immediates, binding sizes and alignments);
- since run 5: every usage flag and outside-a-pass check of every call
  (`each_use_needs_its_usage_flag`, `texture_copies_and_buffer_clears_happen_outside_a_pass`), a
  query index begun once in a pass, step rates other than one and texel buffers only with their
  features, colour bytes a sample by `color_target_bytes`, and a discard giving fresh contents
  (`a_discard_write_gives_the_buffer_fresh_contents`);
- since run 6: read backs inside their buffers, a query ended into a live set, objects used live
  and as their kind, views in other colour formats of the same size only on reinterpreting
  textures and with `FORMAT_REINTERPRET`, one mip range per texture per draw without
  `TEXTURE_VIEWS`, shaders naming entry points of their stage that their modules have, programs of
  shaders of their stages, vertex inputs with attributes, and each part of a check the check
  found unbroken (G1's fourth check, above);
- since run 7: samplers of the kind the program uses (comparison or not), no linear filter paired
  with a texture the back end does not filter (by the program's pairs, whatever level is sampled),
  buffer bindings at least the program's block, one viewport and at most one scissor a draw, empty
  passes of a size and sample count the back end takes, and buffer ranges past the largest offset
  refused, not wrapped (G1's fifth check, above);
- multisampled textures 2D, one level and layer, no storage
  (`a_multisampled_texture_is_2d_with_one_level_and_layer_and_no_storage`); their bytes moved only by
  draws, resolves and whole copies between equal sample counts
  (`a_multisampled_textures_bytes_move_by_draws_resolves_and_whole_copies`); attachments of one
  sample count (`attachments_share_one_sample_count`); resolves from many samples to one, same size,
  format (or twin for `resolve`), averaging formats only
  (`resolves_go_from_many_samples_to_one_of_an_averaging_format`); a load names a sample exactly
  when its texture is multisampled (an error of the draw).

## Interface changes

- G1 run 7:
  - 1362848 ("interface: stated the sampler a program's comparisons and filtered textures take,
    ..."): documentation only, of `BindingResource::Sampler` (a comparison sampler exactly where the
    program compares; no linear filter paired with a texture whose format lacks `FILTER`),
    `UniformBuffer` and `StorageBuffer` (at least the program's block), `DrawCall::viewports` and
    `scissors` (one viewport, at most one scissor) and `EmptyPass` (size 1 to `max_texture_2d`, a
    sample count `r8unorm` takes); plan 4.4, 4.5 and 4.7 say the same. For the other tracks: the
    webgpu back end already refuses several viewports; the Direct3D fronts should grow small
    constant buffers (`ConstantBufferGrown`) and take `FilterInShader` for any linear filter.

- G1 run 6:
  - abecce6 ("interface: stated that format reinterpretation is between colour formats only"):
    docs of `Features::FORMAT_REINTERPRET` and `TextureDesc::reinterpret`, plan 4.12 (the losses
    list): views of a texture made with `reinterpret` take another colour format of the same
    block size; depth and stencil formats are never reinterpreted (a front samples a depth texture
    through its own format). No back end declares the feature yet, so nothing changes for the
    other tracks.
  - From the webgpu track (8f7db1b, merged at the start): `SlowPath::PresentConverted`. From the
    webgl track (3dbc7d5, merged at the start): `SlowPath::CaptureExpanded`. Both added at the end
    of `SlowPath` at once; both kept, `SlowPath::ALL` made 99 long.

- G1 run 5:
  - 48876d8 ("interface: added capabilities for instance step rates and typed texel buffers, ..."):
    `Features::INSTANCE_STEP_RATE` and `Features::TEXEL_BUFFERS` (a back end without one refuses the
    layout or binding with `GpuError::Absent` of it; docs of `StepMode::Instance`,
    `BindingResource::TexelBuffer`, `TexelBufferRoute::Native`, plan 4.5 and 5); `Format::target_bytes`
    and `color_target_bytes`, the measure of `Limits::color_bytes_per_sample` (WebGPU's, plan 5);
    `begin_query`: each index at most once in a pass (plan 4.7). For the other tracks: the webgpu and
    webgl back ends refuse step rates and texel buffers as `Invalid` today and should answer
    `Absent` of the new bits; webgl may declare `INSTANCE_STEP_RATE` where its divisor serves; the
    Direct3D 11 front should take `InstanceRateInShader` where the bit is absent.
  - 2de3a6b (helpers, G1's own): `OcclusionCounter::draw` takes a `CountedDraw` (the counting program,
    alpha to coverage, whether it writes depth or stencil, and whether every per-instance stream
    steps by one) in place of three arguments; plan 4.12 step 4 says when instances are split. No
    other track calls the counter yet.

- G1 run 4:
  - c39aeac ("interface: kept the starting depth for halves ..."): helpers, G1's own, no public item
    changed. The occlusion counter saves the depth copy wherever the draws' primitives times the
    sample count reach the bound; `CombinationTracker::prepared_use` counts only draws after a
    combination was prepared. Plan 4.12 step 4 now says the bound is on a pixel's sum over its
    samples.
  - 9aa5da6 ("interface: stated that no draw writes a read-only aspect ..."): documentation of
    `AspectOps::read_only` and `PassDesc`, and plan 4.7: no draw writes an aspect marked read-only
    (WebGPU's `depthReadOnly`, `stencilReadOnly`), nothing is attached or resolved into twice,
    colour attachments within `color_targets` and `color_bytes_per_sample`.
  - From the direct3d-11 track (c8a7be9, merged at the start): `copy_texture` of a combined
    depth-stencil format copies both aspects in one call; the same rule as run 3's c743db5, so the
    two texts were made one.
  - From the webgl track (d37c5d9, merged mid-run): `capture_vertices` states what it writes (the
    vertex stage's location outputs, packed 32-bit, interleaved into one target or one output per
    target) and `Limits::capture_buffers` what it bounds. Kept: it states the base primitive for
    every back end.
  - From the webgpu track (3f5519a, merged at the end): `SlowPath::StencilWrittenByDraw`. Kept: a
    named slow path.

- G1 run 3:
  - c743db5 ("interface: stated the alignment and limits of buffer bindings and which aspect a copy
    of depth and stencil moves"): `BindingResource` and `TextureRegion::stencil` docs, plan 4.2. A
    uniform binding's offset is a multiple of `min_uniform_offset_alignment` (the limit existed, the
    rule was unstated). Depth of a combined format moves as four bytes (as plan 4.2 already said for
    buffers written into `depth24plus`). Direct3D 12's per-plane texture copies go through a buffer.
  - 9a2d9d3 ("interface: counted occlusion before every change ..."): helpers, G1's own.
    `OcclusionCounter::flush` (new; fronts call it before changes inside a counted query),
    `PassTracker::generation` (new), `Combination` and `CombinationTracker::combination` (new;
    `drawn` now takes a `Combination`, `record` is for the cache's record only). No other track uses
    the helpers yet. Plan 4.7 and 4.12 say the same.
  - From the shaders track (1ffcab5, merged at the start): `VariantKey::output_interpolation`. From
    the webgpu track (b999c16, merged at the end): `SlowPath::IntegerClearByDraw`. Both kept as
    they are; neither conflicts with anything here.

- G1 run 2:
  - 54ff74f ("interface: stated the rules for multisampled textures, attachments and resolve
    targets"): documentation of `TextureDesc::samples` and `PassDesc`, and the plan's 4.2; the rules
    are WebGPU's. A front holds a multisampled array (Direct3D's `Texture2DMSArray`) as one texture
    per layer; written into the plan for the Direct3D tracks.
  - 4bd2adb ("interface: stated that a resolve needs render usage on both sides"): `TextureUsage`
    documentation.
  - d7347d3 (helpers, G1's own): the pass tracker's multisampled fitting and `Builtin::SampleCopy`;
    a multisampled attachment the tracker fits needs `SAMPLE` usage too (`Target` docs). No public
    item added.
  - From the direct3d-12 track (4f1894c, merged at the end): `ShaderModule::from_wgsl`, one entry
    point of a WGSL module keyed as the built-ins are. Kept: it serves every front's WGSL programs.
    It validates with no extra naga capabilities, so the built-ins needing some (cube arrays, sample
    index) keep their own path.
- G1 run 1:
  - Commit 3fcb44f ("added the pass tracker, the occlusion counter and the combination tracker to
    the interface's helpers"): new items `PassTracker`, `PassTargets`, `ColorTarget`, `Target`,
    `PassOps`, `ScopeUse`, `cut_scissors`, `OcclusionCounter`, `CombinationTracker`; the crate now
    depends on `half` too. Additions only; G1 owns the helpers (plan 12).
  - Commit d3b5eb0 ("interface: stated which buffer and texture usage each call needs"):
    documentation of `BufferUsage` and `TextureUsage`. The recording back end enforces it.
  - From the webgpu track (db9e0d8, merged at the end): `WriteMode::NoOverwrite` now also promises
    that no ordered write made since the caller last waited touches the bytes, and that a no-overwrite
    write may land ahead of earlier commands. Kept as it is: it serves both back ends and the
    recording back end now checks exactly that.
  - From the webgl track (4b71023 at the start, ab4b7e9 at the end): the built-in programs drawn in
    the clip depth convention and with the provoking vertex the back end has. Kept.
  - Start-of-run merge conflict in the webgl crate (adapter info in `caps`): took the webgl track's
    form, which passes the renderer string into `caps`.
- Earlier runs, all kept: `Backend::resolve` takes a `Format` (c60a829); `Caps::adapter`
  (`AdapterInfo`, from the direct3d-12 track); clip distances and unclamped depth as
  perspective-correct values (275c370); `BLEND_CONSTANT_ALPHA` doc (webgl); `PatternClearCopied`
  (webgpu). Their detail is in this file's history.

## The recording back end and the suite

- `Recorder` keeps every object's bytes on the CPU, runs each call when it is recorded, checks the
  rules above, and logs every base call in `calls` as a `Call` (`stream.rs`) with its answer;
  `replay(&calls)` runs them again on a new one. Calls a base call makes inside itself are not logged
  (the outer one is). `contents()` gives every buffer's and texture's bytes and the frames shown.
- It draws through an interpreter of naga's IR (`program.rs`); since this run fragment programs
  write storage buffers (applied when the draw ends; an invocation sees its own earlier writes), and
  it declares `FRAGMENT_STORAGE_WRITE`. Since run 2 it draws four samples (above); since run 3 its
  programs call functions, and it refuses everything its own capabilities leave out; since run 4 it
  also checks every limit it declares that a call can pass; since run 6 it declares
  `FORMAT_REINTERPRET` and `SEPARATE_SAMPLERS`, which it serves. No format it holds has storage uses, so storage textures are refused. Still not drawn, each refused with an
  error naming it: points and lines, sample counts other than one and four, cube sampling, implicit
  level of detail over several levels, compute, indirect draws, alpha to coverage, storage writes
  from vertex programs, pointers to a function's locals passed to another function.
  `texture_bytes` gives a texture's bytes as held (samples as planes), for tests on back ends that
  cannot read samples in programs.
- Suite tests (26, `src/suite.rs` and `src/suite/draws.rs`; run 7 added
  `a_stream_of_stride_zero_reads_its_first_element` and
  `comparison_samplers_compare_with_the_stored_depth`): the 13 of G1a plus
  `values_are_interpolated_with_perspective`, `the_scissor_bounds_the_draw`,
  `stencil_operations_and_tests`, `near_clipping_and_the_viewport_depth_range`,
  `linear_filtering_and_address_modes`, `instances_start_at_the_first_instance`,
  `indexed_strips_restart_and_add_the_base_vertex`, `fragments_write_storage_buffers` (where
  `FRAGMENT_STORAGE_WRITE` is declared), and from run 2
  `multisampled_targets_cover_their_samples_and_resolve` (where `rgba8unorm` takes four samples),
  `programs_run_per_sample_and_read_samples` (and `SAMPLE_SHADING` and `MULTISAMPLE_READ`),
  `occlusion_queries_answer_whether_samples_passed`. Suite programs are validated with naga's
  `MULTISAMPLED_SHADING`. The webgpu and webgl hosts run them all (`suite!`).

## Next

- The independent check of G1 (run 7's fixes).
- The webgpu and webgl hosts run the whole suite through `suite!`, so they take the two new tests
  (`a_stream_of_stride_zero_reads_its_first_element`,
  `comparison_samplers_compare_with_the_stored_depth`) when they merge this branch; neither has been
  run on a GPU by this track.
- One more guard no call through the interface reaches: `sample_view`'s refusal of a comparison
  through a sampler without `compare` (the draw's binding check refuses it first).
- Recording back end limitations, each refused with an error naming it and so left without a
  breaking test: cube sampling, alpha to coverage, implicit level of detail over several levels.
  Two checks are unreachable through the interface and stay as guards: storage writes from vertex
  programs (refused when the binding set is made) and an attachment taken twice by a draw (refused
  at `begin_pass`). Likewise a colour attachment of a format with no `target_bytes` (depth formats
  are refused before it, block-compressed ones have no render use). The region check's block-grid parts are reached only once the recorder holds a
  block-compressed format.
- More guards no call through the interface reaches, each kept and left without a test: a bound
  range past its buffer at the draw (the binding set refuses it first and buffers do not shrink), a
  running query without an occlusion set (`begin_query` refuses it first). The interpreter's own
  guards against program shapes naga's validation already refuses (inputs and outputs without a
  binding, a vertex program without a position or that discards, a flat value missing) are not
  rules of section 4.
- Limits the recording back end does not check because nothing they bound is drawn: point size
  and line width (lines and points are not drawn; `WIDE_LINES` is checked), compute and transform
  feedback (absent).
- Not G1's: the occlusion counter's sum by a dispatch on WebGPU is G2's (plan 4.12 step 4); until
  then it reads back (`ReadBackWait`).
- More suite tests, beyond G1's pass bar: every format's clear and read back (needs reference bytes
  from real Direct3D through the rig, not the recorder's own encoder), mip levels, layered and 3D
  targets, fans and loops through `draw_fan_or_loop` on back ends that draw lines.

## How to run

- Linux loop (CPU only): `tools/graphics-rig/linux <command>` sends the working tree to
  `~/wcontainers-graphics/src/phase-2-graphics` on tom@192.168.2.240 and runs the command there with
  `CARGO_TARGET_DIR=~/wcontainers-graphics/target/phase-2-graphics`, 5 cargo jobs (2 when that
  machine's load average is 8 or more), at nice 10. Examples:
  - `tools/graphics-rig/linux cargo test -p wcontainers-gpu -p wcontainers-gpu-test`
  - `tools/graphics-rig/linux sh -c 'cargo clippy -p wcontainers-gpu -p wcontainers-gpu-test --all-targets -- -D warnings'`
  - `tools/graphics-rig/linux sh -c 'RUSTDOCFLAGS="-D warnings" cargo doc -p wcontainers-gpu -p wcontainers-gpu-test --no-deps'`
  - random sequences, seed and count as knobs (defaults 0x5eed and 1000; the test prints both and
    how many ran):
    `tools/graphics-rig/linux sh -c 'WCONTAINERS_GPU_SEED=0x5eed WCONTAINERS_GPU_SEQUENCES=100000 cargo test -p wcontainers-gpu-test --test sequences -- --nocapture'`.
    A failing sequence is shrunk by proptest and printed as its steps.
  - the check's faults against the suite: copy `g1r2/breaks.py` from scratch to the laptop and run
    `python3 -I breaks.py <tree> <work dir>` there (it copies the tree, changes one line, runs
    `tests/recorder.rs` and `tests/helpers.rs`); 18 faults, five of them added in run 2 for
    multisampling and the counter's halving.
  - the rule sweep: copy `g1r7/sweep.py` (run 6's, with `program_bindings` among the helpers) from
    scratch to the laptop, copy the tree beside it, and
    run `python3 -I sweep.py <tree> <target dir>` there (`list` in place of names prints the
    sites; names run only those); it turns each `if` that refuses something in `src/recorder.rs`
    and `src/draw.rs` off (whole, and each top-level `||` operand), removes each statement that
    calls a check helper with `?`, gives each slice range refused by `ok_or` an empty slice, and
    runs `cargo test -p wcontainers-gpu-test`, printing CAUGHT, SURVIVED or BUILD FAILED (the
    match guards). About 20 to 40 s a site at 4 jobs.
  - run 5's faults (the counter's instance split, the discard, the colour byte measure):
    `g1r5/faults.py`, run the same way (`python3 -I faults.py <tree> <target dir>`); run 7's (the
    sampler kind, the filter rule, unchecked ends, stride zero): `g1r7/faults.py`, `faults2.py`
    and `faults3.py` the same way.
  - Rust there: rustup with the 1.99.0 toolchain the repository names, installed by another track;
    this run installed nothing.
- A back end runs the suite with a test file holding a `Host` implementation and
  `wcontainers_gpu_test::suite!(MyHost);` (see `crates/wcontainers-gpu-test/tests/recorder.rs`).
- On the Mac only formatting: `nix develop -c cargo fmt -p wcontainers-gpu -p wcontainers-gpu-test`.

## Measurements

- G1 run 7, all on the Linux laptop (i7-8750H):
  - Random sequences on b329a34 (the recorder as it ends the run; scratch
    `g1r7/sequences-100k.log`, `--release`), each 100,000 asked and run, recorded with no rule broken
    and replayed identically: seed 0x5eed, 4,552,401 calls, 35.3 s; seed 0xc0ffee, 4,552,312 calls,
    36.9 s; seed 0x5a3d7 (new), 4,548,760 calls, 20,263 multisampled passes, 28,461 depth-stencil
    passes, 42,585 indexed draws, 28,603 queries, 11,954 no-overwrite writes without a wait, 35.8 s.
    Load 1.4 to 2.3. The first two give run 5's and 6's counts: the generator did not change.
  - The sweep of the new checks (scratch `g1r7/sweep.log`, load about 1.4 to 2.2): 12 sites (the
    empty pass's size check whole and by its four operands, its sample check, the viewport check
    whole and by both operands, the block size check, the filter check, and the
    `program_bindings` call): 12 caught. The comparison check is an `if let` chain the sweep does
    not turn off, so it is a fault instead.
  - Run 7's faults (scratch `g1r7/faults*.log`): the comparison kind unchecked, comparison samplers
    held to the filter rule, the filter rule on mag only, the write's, clear's and index range's
    ends unchecked, the draw's counted index and vertex ranges unchecked, stride zero read as packed:
    each caught by the test written for it. Two first attempts were wrong faults, not misses
    (`faults.log`: an index fault that never overflowed; `faults2.log`: a stride fault that changed
    every stride), each written again in `faults3.py` and caught.
  - End check after the end merges (scratch `g1r7/end-check.log`, load 1.9 to 4.2): the whole
    workspace checks; suite on the recording back end 26 of 26 (`tests/recorder.rs` 32 of 32);
    rules 97 of 97; helpers 15 of 15; fast paths 12 of 12; sequences 1 of 1 (1,000 at seed 0x5eed);
    `wcontainers-gpu` 32 of 32; clippy and docs with warnings denied clean.

- G1 run 6, all on the Linux laptop (i7-8750H):
  - Rule sweep (scratch `g1r6/sweep.log`, on c52b3fe, its own build directory, 4 jobs, load 1.4 to
    10 with the sequences beside it): 187 sites (`if` checks whole and by `||` operand, helper
    statements, slice ranges refused by `ok_or`): 175 caught, 4 match guards that do not build
    when turned off (each asserted by its message), 8 survived. Of those 8: one was a missing test
    (the stencil aspect write's data length, `.get(from..from + n)`), now written; the other 7 each
    have their reason (Next): the two block-grid parts, cube sampling, alpha to coverage, and three
    guards no call reaches (storage writes from vertex programs, an attachment taken twice by a
    draw, a bound range past its buffer at the draw). The checks added after c52b3fe and the
    missing test, swept on the final tree (`g1r6/sweep-rerun.log`, load 0.6 to 2.9): the
    `create_shader` check, the `create_program` check whole and by both operands, and the stencil
    data range: 5 of 5 caught.
  - Random sequences on a2f1b28 (the recorder as it ends the run; scratch
    `g1r6/sequences-100k-final.log`, `--release`), each 100,000 asked and run, recorded with no
    rule broken and replayed identically: seed 0x5eed, 4,552,401 calls, 36.5 s; seed 0xc0ffee,
    4,552,312 calls, 42.9 s; seed 0x51de6 (new), 4,561,652 calls, 20,037 multisampled passes,
    28,228 depth-stencil passes, 42,675 indexed draws, 28,384 queries, 11,825 no-overwrite writes
    without a wait, 43.0 s. Load 2.1 to 4.1. The same on c52b3fe with seed 0x6a7e5 (new) instead
    of the third: 4,548,874 calls, all passing (`g1r6/sequences-100k.log`, load 4.0 to 10.2).
  - After the end merges (scratch `g1r6/end-check.log`, load 4.1 to 4.9): the whole workspace
    checks; suite on the recording back end 24 of 24 (in `tests/recorder.rs`, 30 of 30); rules 91
    of 91; helpers 15 of 15; fast paths 12 of 12; sequences 1 of 1 (1,000 at seed 0x5eed);
    `wcontainers-gpu` 32 of 32; clippy and docs with warnings denied clean.

- G1 run 5, all on the Linux laptop (i7-8750H):
  - Random sequences on dda653b (the recorder and generator as they stand at the end of the run;
    scratch `g1r5/sequences-100k.log`, `--release`), each 100,000 asked and run, recorded with no
    rule broken and replayed identically: seed 0x5eed, 4,552,401 calls, 11,896 no-overwrite writes
    without a wait, 40.4 s; seed 0xc0ffee, 4,552,312 calls, 40.1 s; seed 0xd15ca4d (new), 4,551,294
    calls, 19,789 multisampled passes, 28,461 depth-stencil passes, 42,803 indexed draws, 28,355
    queries, 39.8 s. Load 2.2 to 3.3 (the rule sweep ran beside them). Fewer calls than run 4,
    as a discard now drops the generator's pending ranges and so needs fewer waits.
  - Rule sweep with helper checks (scratch `g1r5/sweep.log`, on dda653b, its own build directory):
    149 checks and parts (113 `if` checks and parts, 36 helper statements): 137 caught, 4 match
    guards that do not build when turned off (each asserted by its message), 8 survived. Two of
    those 8 were missing tests, now written and caught (`g1r5/sweep-rerun.log`: the depth
    attachment's sample count, and a draw outside a pass checked before its other rules). The 6
    left are run 4's, each with its reason (Next): the two block-grid parts, cube sampling, alpha to
    coverage, and the two unreachable guards. About 10 to 40 s a check, load 0.2 to 4.7.
  - Run 5's faults (scratch `g1r5/faults.log`, the final tree, load 4.6): the counter's instance
    split without regard to step rates, and without `BASE_VERTEX_INSTANCE`; a discard keeping the
    touched ranges, and keeping the old bytes; colour bytes counted by block size: each fails the
    test written for it. The first one at first passed, because the test drew the stepped draw's
    count with a counting program that did not read the shifts; fixed (c2bfd1e) and then caught.
  - Suite on the recording back end 24 of 24; rules 82 of 82; helpers 15 of 15; fast paths 12 of
    12; `tests/recorder.rs` 30 of 30; `wcontainers-gpu` 32 of 32; clippy and docs with warnings
    denied clean (scratch `g1r5/end-check.log`).

- G1 run 4, all on the Linux laptop (i7-8750H):
  - Random sequences on the final recorder (after the mid-run merges, 8f19e19; scratch
    `g1r4/sequences-100k.log`, `--release`), each 100,000 asked and run, recorded with no rule broken
    and replayed identically: seed 0x5eed, 4,556,023 calls, 41.1 s; seed 0xc0ffee, 4,555,910 calls,
    42.4 s; seed 0x9e4a11 (new), 4,561,411 calls, 20,137 multisampled passes, 28,288 depth-stencil
    passes, 42,432 indexed draws, 28,464 queries, 37.4 s. Load 2.4 to 5.7 (the rule sweep ran
    beside them). The generator did not change this run, so the first two seeds give run 3's counts.
  - Rule sweep before the fixes (scratch `g1r4/sweep-before.log`, on the start-of-run tree): of
    101 checks and parts, 50 caught, 47 survived, 4 match guards not turned off. After (scratch
    `g1r4/sweep-after.log`, the final recorder and tests, a clean build directory): of 111, 101
    caught, 4 match guards (each asserted by its message in `tests/rules.rs`), and 6 survived, each
    with its reason: the two block-grid parts of the region check (no format the recorder holds has
    blocks over one texel, so they cannot be reached), cube sampling and alpha to coverage (not
    drawn by the recorder, refused naming it), and the two guards unreachable through the interface
    (Next). About 8 to 40 s a check, load 2 to 7 (the other track's work beside it). An earlier
    after-sweep shared a build directory with the first and ran against a stale `wcontainers-gpu`;
    it was discarded and run again clean.
  - Suite on the recording back end 24 of 24; rules 78 of 78; helpers 14 of 14; fast paths 11 of
    11; `tests/recorder.rs` 30 of 30; `wcontainers-gpu` 32 of 32, with clippy clean, after the end
    merges (394dfb6). The whole workspace checks after the mid-run and the end merges.
  - The check's own 16 faults (its `g1-check2/mutate.py`) on the final tree: 15 fail a test, each
    the one written for it (scratch `g1r4/check-mutations2.log`); `attached-twice` passes, as that
    draw guard is unreachable now that `begin_pass` refuses the same subresource attached twice
    (`a_subresource_is_attached_once`). That script copies the tree with its file times kept, so after
    a fault in `wcontainers-gpu` cargo keeps the faulty build for the faults that follow; run here
    with plain copies (`g1r4/check-mutations.log` shows the contaminated first attempt).
  - The two new helper tests checked against the old code (scratch tree with the two fixes undone):
    the four-sample halves count 0 instead of 9,616, and `prepared_use` gives (4, 3) instead of
    (2, 5).

- G1 run 3, all on the Linux laptop (i7-8750H), on the final code after the end merges (b9a51ce):
  - Random sequences (scratch `g1r3/sequences-100k-final.log`, `--release`), each 100,000 asked and
    run, recorded with no rule broken and replayed identically, 35.0 to 35.1 s each, load 0.7 to
    0.9: seed 0x5eed, 4,556,023 calls (19,994 multisampled passes, 28,344 depth-stencil passes,
    42,632 indexed draws, 28,105 queries, 21,258 scissored draws, 7,507 tinted draws, 10,085
    no-overwrite writes without a wait); seed 0xc0ffee, 4,555,910 calls; seed 0x7a11ed (new),
    4,567,456 calls. The same three on the code before the view checks gave the same counts
    (`g1r3/sequences-100k.log`).
  - Suite on the recording back end 24 of 24; rules 45 of 45; helpers 13 of 13; fast paths 10 of
    10; `tests/recorder.rs` 30 of 30 (its 6 rule tests and the suite); `wcontainers-gpu` 32 of 32.
  - Faults checked by hand this run: the counter's restore of the saved depth removed (the halves
    test gives 0, not 12,004); the generator never waiting before a no-overwrite write (the
    recorder refuses it in the first sequences run).
  - The whole workspace checks (`cargo check --workspace --all-targets`) after the end merges.

- G1 run 2, all on the Linux laptop (i7-8750H):
  - Random sequences on the final recorder (scratch `g1r2/sequences-100k.log`, `--release`):
    seed 0x5eed, 100,000 asked and run, 3,327,668 calls, 19,128 multisampled passes, 19.8 s; seed
    0xc0ffee, 100,000, 3,332,160 calls, 18,957 multisampled passes, 21.4 s; seed 0x6b1d (new),
    100,000, 3,339,740 calls, 19,318 multisampled passes, 19.9 s. Each recorded with no rule broken
    and replayed identically. Load 0.4 to 2.3.
  - Suite on the recording back end 24 of 24; rules 26 of 26; helpers 11 of 11; fast paths 8 of 8;
    `tests/recorder.rs` rule tests 6 of 6; `wcontainers-gpu` 32 of 32.
  - The whole workspace checks (`cargo check --workspace --all-targets`) after the merges.
  - The 18 faults of `g1r2/breaks.py` (the 13 of G1a's check, and sample positions swapped, a
    per-sample run writing every sample, a resolve taking sample 0, the tracker skipping the sample
    load, the counter never halving): each fails at least one test (`g1r2/breaks-run2.log`; load
    1.6 to 3.3).
- G1 run 1, all on the Linux laptop (i7-8750H):
  - Random sequences on the final code (scratch `g1/end-checks.log`): seed 0x5eed, 100,000 asked,
    100,000 run, 3,249,453 calls, 22.5 s of test time; seed 0xc0ffee, 100,000 run, 3,263,974 calls,
    22.6 s; every one recorded without a broken rule and replayed identically. Load 0.75 to 0.98.
    Earlier the same day on the code before the widened no-overwrite rule: seeds 0x5eed and 0xc0ffee,
    100,000 each, all passing (`g1/sequences-100k-final.log`, load 0.2 to 0.9).
  - The whole workspace checks (`cargo check --workspace --all-targets`) after the end-of-run merges.
  - Suite on the recording back end 21 of 21; rules 22 of 22; fast paths 8 of 8; helpers 6 of 6;
    `tests/recorder.rs` rule tests 6 of 6; `wcontainers-gpu` 32 of 32.
  - The check's 13 faults: each fails the suite (`g1/breaks-run1.log`; load 0.3 to 1.6).
- Earlier runs' measurements are in this file's history.

## Dependencies added and their licences

- This run: `proptest` 1.12.0 in `wcontainers-gpu-test` (MIT OR Apache-2.0, its `Cargo.toml`;
  already in the workspace's lock through the kernel and translator test crates); `half` 2 in
  `wcontainers-gpu` (MIT OR Apache-2.0; already in the lock through `wcontainers-gpu-test`).
- Earlier: `naga` 30.0.2, `blake3` 1, `half` 2 (MIT OR Apache-2.0, CC0-1.0 OR Apache-2.0); plan 3.

## Departures from the plan

- G1 run 5: the end merges brought a break between two other tracks: the rig made the tolerance
  file's allowance optional (an unrepeatable frame has none) and the webgl track's scene example
  still indexed it as a plain value, so the workspace did not build. To keep the shared line
  building I changed that one statement in `crates/wcontainers-gpu-webgl/examples/webgl-scenes.rs`
  (7a0feba): a scene without an allowance is an error naming it. The webgl track should take it as
  it is or replace it.

- From G0 and G1a: written in the plan and in this file's history.
- G1 run 1 (each now written in the plan where it changes it):
  - A pass split by the pass tracker names its resolve target in every part, not only the last,
    since a part cannot be known to be the last when it begins (plan 4.7): one more resolve per split.
  - The occlusion counter sums its target by `read_back_now` on every back end (plan 4.12 step 4);
    the WebGPU dispatch is left (Next).
  - The recording back end checks `NoOverwrite` writes against every range a command since the
    caller last waited read or wrote, and against ordered writes, which needs fence waits in the
    call stream (`FenceDone`, `WaitFence`).
  - Random sequences: proptest, as the plan says, with a fixed ChaCha seed. Since run 3 the generator
    waits for all work only before a no-overwrite write that would touch a range an unfinished
    command or ordered write uses, and since run 5 a discard drops the buffer's ranges, so most
    no-overwrite writes land without a wait and the promise is still kept.

## Known failures

- The occlusion counter refuses (`GpuError::Invalid`) a single fan, strip with restart or indirect
  draw that brings a pixel's sum over its samples to its counting format's bound (2048 in
  `r16float`, only on back ends without blendable `r32float`): 2048 fragments at the one sample of a
  single-sampled target, 512 a sample on average on four samples; no test, as no route exists. A counting program
  reading the primitive index sees it start again in the second half of a split draw.
- Since run 5, the same refusal for one primitive drawn as many instances where the first instance
  cannot be raised (no `BASE_VERTEX_INSTANCE`, as on WebGL 2, or a per-instance stream stepping
  other than by one) and whose instances bring a pixel to the bound; tested
  (`the_occlusion_counter_splits_instances_only_where_each_reads_the_same`, the quad 3,000 times
  without bases). It needs `r16float` counting too.
